# 4. Event-sourcing implementation contract

- **Status:** Accepted
- **Date:** 2026-06-17

## Context

ADR 0001 chose an event-sourced core; ADR 0002 chose `cqrs-es` with self-contained events,
a framework-agnostic decision core, and plain JSON plus explicit event versioning.
`docs/data-model.md` then specified the domain: entities, value objects, aggregates, the
event catalog, and the `EventContext` provenance envelope.

A readiness review for the first `vitni-core` code found that the *domain vocabulary* is
complete but the *implementation contract* wiring it onto `cqrs-es` is not. Five questions
must be answered before the first aggregate is written, because each shapes every event:

1. Where does the `EventContext` (operator, when, why, surety, citations) physically live?
2. How is a single assertion identified, so a correction can reference the claim it revises?
3. Where do non-deterministic inputs (clock, generated ids, operator identity) enter, given
   the decision core must stay pure and unit-testable?
4. How are events typed and versioned on the wire?
5. What concrete identifier type do aggregates and assertions use?

This ADR answers those five. It is grounded in the current stack — **cqrs-es 0.5.0** (verified
latest, 2025-12-30), whose `EventEnvelope { aggregate_id, sequence, payload, metadata:
HashMap<String, String> }`, `execute_with_metadata`, and `Aggregate { Command, Event, Error,
Services }` with async `handle(&mut self, cmd, svc, sink)` + `apply` are documented in
`docs/research/event-sourcing-rust.md`. The command catalog and per-aggregate error taxonomy
are domain vocabulary and live in `docs/data-model.md` §10, not here.

## Decision

1. **Provenance lives in the event payload, not in cqrs-es metadata.** Every assertion event
   embeds its `EventContext` (operator `Agent`, `occurred_at`, `rationale`, `confidence`,
   `citations`, `evidence_analysis` — data-model §8). The cqrs-es `metadata`
   `HashMap<String, String>` is reserved for **ops/tracing only**: correlation id, trace/span
   id, request id, host. That metadata is non-domain and discardable; provenance is neither.

2. **Each assertion event carries an explicit `AssertionId` (UUID v7) in its payload.**
   Corrections — `AssertionRetracted` and `AssertionSuperseded` (data-model §10) — reference
   the prior claim by its `AssertionId`. They never reference the cqrs-es
   `(aggregate_id, sequence)` pair, which is implicit in the stream key, not a queryable
   payload field, and not reconstructable after a move to `disintegrate`.

3. **The decision core is pure; non-deterministic inputs are supplied, never sampled.** The
   core is `decide(state, command) -> Result<Vec<Event>, Error>`, and the cqrs-es `Aggregate`
   impl is a thin adapter over it (ADR 0002 portability habit). The clock (`occurred_at`),
   generated ids (`AssertionId`, new aggregate ids), and the operator `Agent` are produced by
   the **application layer** and passed in on the command/context — `decide` never reads a
   clock or generates a uuid. So decisions are unit-testable given/when/then with no I/O.
   cqrs-es `Services` is reserved for **cross-aggregate projection reads** (the "aggregate
   tax" of data-model §9), not for clock or id generation.

4. **Event encoding and versioning convention.** `DomainEvent::event_type` returns the variant
   name; `event_version` returns a string bumped **only** on an incompatible payload change.
   Payloads are serde **internally-tagged** JSON (the `#[serde(tag = "type")]` shape of research
   §1). Event-shape changes are **additive and append-only** — fields are added optional,
   variants are added, nothing stored is mutated, and every historical event stays decodable
   forever. Version **upcasting tooling** stays deferred (ADR 0002, out of scope); this ADR
   fixes only the encoding so that versioning is possible from the first event.

5. **Aggregate ids and `AssertionId` are UUID v7.** Time-sortable, so insertion order tracks id
   order — good index locality, and consistent with the `sortval` ordering the date model
   already prefers.

## Rationale

- **Provenance in payload (1).** Provenance is the whole point of choosing event sourcing
  (ADR 0001: who/what/when/why), so it is domain data, not the "auditing/logging/debugging"
  metadata the cqrs-es book puts in the `HashMap`. It is also structured — `Agent`,
  `Vec<CitationRef>`, `EvidenceAnalysis` — and cqrs-es metadata is `HashMap<String, String>`,
  so metadata would force JSON-stuffing into string values and lose queryability. Decisively,
  ADR 0002's self-contained-events rule and the planned escape hatch to `disintegrate` (which
  has no metadata concept) require every queryable field to be in the payload; this is the one
  thing that cannot be retrofitted. The cost — every event embeds its context — is accepted.
- **Explicit `AssertionId` (2).** It is the same self-contained-events rule applied to identity:
  a correction must name the claim it revises with an id that is in the payload and survives a
  storage/framework change, which `(aggregate_id, sequence)` does not.
- **Pure core, supplied inputs (3).** Keeping `decide` free of clock/uuid/Services is what makes
  the rules testable without a database and keeps the cqrs-es-to-disintegrate adapter swap
  mechanical (ADR 0002).
- **Additive encoding (4).** Variant-name types plus a version string are the minimum that lets
  old events stay decodable while the model grows — required by ADR 0001's versioning-discipline
  consequence.

## Consequences

### Positive

- Provenance is queryable and portable; an assertion's full who/why/surety travels with it.
- Corrections and merges have a stable, portable target (`AssertionId`).
- Decision logic is pure and unit-testable; the storage framework stays swappable (ADR 0002).
- Events can evolve from day one without breaking replay.

### Negative / costs

- Every event embeds an `EventContext` — larger payloads and some repetition across a command's
  events.
- The application layer must construct ids, timestamps, and the operator `Agent` before calling
  `decide`; the purity is a discipline, not enforced by the framework.
- Version upcasting is still unbuilt, so a genuinely incompatible change is blocked until that
  deferred tooling exists.

## Out of scope

- The command catalog and per-aggregate error taxonomy — domain vocabulary in data-model §10.
- Concrete projection/read-model schema and event-version **upcasting** tooling — deferred by
  ADR 0002.
- Snapshotting strategy — a performance concern, deferred until replay cost warrants it.

## References

- ADR 0001 — event-sourced core (provenance and versioning consequences this ADR makes concrete).
- ADR 0002 — `cqrs-es`, self-contained events, framework-agnostic decision core, JSON +
  versioning (the habits this ADR operationalizes).
- `docs/data-model.md` — §8 `EventContext`, §9 aggregates, §10 commands/events/errors, §15
  sketches.
- `docs/research/event-sourcing-rust.md` — §1 (store + JSON tagging), §3 (cqrs-es 0.5 API),
  §6–§7 (portability habits, the shared `decide` core). Read against cqrs-es 0.5.0.
