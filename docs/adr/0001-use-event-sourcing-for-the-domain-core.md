# 1. Use event sourcing for the domain core

- **Status:** Accepted
- **Date:** 2026-06-17

## Context

Genealogy data is inherently historical and collaborative. A conclusion ("these two
people are the same individual", "this birth date is 1847") is a *claim* made by some
researcher at some time, on some evidence — and it gets revised as new sources surface
or earlier reasoning is shown wrong. Two needs follow directly:

- **Provenance.** Users must be able to see *who* asserted *what*, *when*, and *why*.
- **Non-destructive revision.** Correcting a conclusion must not erase how the previous
  conclusion came to be.

State that is mutated in place cannot answer "how did this conclusion come to be?" — the
prior value is simply gone. The Gramps v6 data model is our reference for *what entities
exist* (people, families, events, sources, citations), but Gramps itself stores mutable
records and does not retain this history; adopting its schema does not give us provenance.

## Decision

Domain state in `vitni-core` is **derived by replaying an append-only event log**, not
mutated in place.

- Every change is an **event** appended to the log; events are the source of truth.
- Each event belongs to an **aggregate** and carries an **event context** recording the
  **operator** (user) who caused it, when, and why.
- Read models / **projections** are derived from the log and are **disposable** — they can
  be deleted and rebuilt by replaying events.
- Derived/projected state is never edited directly. A correction is a *new event*, so the
  audit trail stays complete.

## Consequences

### Positive

- **Audit trail by construction** — who/what/when/why is recorded for every change, not
  bolted on after the fact.
- **Time travel / replay** — past state at any point is reconstructable; projections are
  rebuildable from `event_id = 0`.
- **Non-destructive corrections** — revisions are new events; nothing is overwritten.
- **Testable decision logic** — business rules can be expressed as pure functions
  (`state + command -> events | error`) and unit-tested without a database.

### Negative / costs

- **More moving parts** — events, projections, and projection cursors instead of a single
  mutable table.
- **Eventual consistency on the read side** — projections lag the log; reads may not be
  immediately consistent with the latest write.
- **Versioning discipline** — event shapes must be versioned explicitly; old events must
  stay decodable forever.
- **Append-only growth** — storage only grows; pruning/archival is a deliberate exercise,
  not a side effect of editing.

### Mitigating habits

Carried over from the analysis in `docs/research/event-sourcing-rust.md`:

- Keep decision logic **framework-agnostic** (`state + command -> events | error`) so
  storage/framework choices stay swappable.
- Write **self-contained events**: put every identifier you might ever query by into the
  event payload, not implicitly in a stream key. This is the one thing that cannot be
  retrofitted later.
- Use **plain JSON payloads** plus **explicit event versioning** for portability.

## Out of scope

This ADR decides *only* that the domain core is event-sourced. The following are deferred
to follow-up ADRs, with `docs/research/event-sourcing-rust.md` as the input:

- **Event-store framework** — from-scratch vs. `cqrs-es` vs. `disintegrate` (DCB).
- **Database engine** — e.g. SQLite vs. Postgres, owned by the `vitni-db` crate.

## References

- `docs/research/event-sourcing-rust.md` — from-scratch event stores, the DCB model, and a
  `cqrs-es` vs. `disintegrate` comparison.
- Gramps v6 data model — <https://github.com/gramps-project/gramps> (entity reference, not
  the persistence approach).
