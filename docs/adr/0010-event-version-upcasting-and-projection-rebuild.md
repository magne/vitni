# 10. Event-version upcasting and projection rebuild

- **Status:** Accepted
- **Date:** 2026-06-19

## Context

ADR 0004 §4 fixed the event encoding — internally-tagged JSON, every variant carrying an
`event_type` (its variant name) and an `event_version`, changes **additive / append-only** so every
historical event stays decodable. It deliberately left two pieces of *tooling* undecided, and ADR
0002 / ADR 0009 both deferred them explicitly:

1. **How a stored event written under an old payload shape is read after the shape changes** — the
   upcasting mechanism: where upcasters live, how a version bump is expressed, and when upcasting
   runs.
2. **How a read model is rebuilt from the log** — ADR 0009 fixed the projection schema and
   guaranteed it does not obstruct rebuild, but built no rebuild path.

Spike B (`docs/roadmap.md`) is the slice that proves both before the log grows large, while
upcasting and rebuild are cheap. The spike informs this decision: the worked change is the Event
aggregate's `EventCreated` gaining a `private` flag (Gramps' universal privacy flag), bumping that
variant's payload from `1.0` to `2.0`, with an upcaster that backfills `false` for historical events.

This decision is **engine-agnostic**, like ADR 0009. Upcasting and replay both act at the
`cqrs-es` `PersistedEventRepository` boundary, which every backend ADR 0002 selects between (SQLite
default, Postgres feature-gated) implements. SQLite is the first implementation; the contract does
not change per engine.

## Decision

1. **Upcasting uses the `cqrs-es` `EventUpcaster` mechanism.** An upcaster rewrites a *serialized*
   payload (a `serde_json::Value`) from an older shape to the current one. We use the built-in
   `SemanticVersionEventUpcaster`, keyed on `(event_type, event_version)`: it fires only for a
   matching `event_type` whose stored version is *less than* the upcaster's version. This is the
   standard cqrs-es facility, not a bespoke layer.

2. **Versions are per-variant.** `DomainEvent::event_version()` returns the version of the specific
   variant (`EventEventBody::version()`), not one version for the whole aggregate's event type. A
   variant is bumped **only when its own payload changes**, additively (ADR 0004 §4): today
   `EventCreated` is `2.0` while every unevolved Event variant stays `1.0`. This keeps versions
   honest and scopes each upcaster to exactly the variant it migrates.

3. **The upcaster set is owned by `genealogy-core`, next to the events.** Each aggregate that has
   evolved a variant exposes an ordered `upcasters()` function (e.g. `genealogy_core::event::upcasters`)
   returning `Vec<Box<dyn EventUpcaster>>`. The event schema is a core concern, so its migrations
   live with it, independent of any backend. Aggregates whose schema has not changed contribute no
   upcasters. Upcasters are ordered oldest-first so a payload is migrated through each step in turn
   (`1.0 → 2.0 → …`).

4. **Upcasting runs at read time, never by rewriting stored events.** Stored events are immutable
   (ADR 0001). Upcasters are applied on the two read paths only: aggregate **load** on the command
   side (the event store is configured `with_upcasters(...)`), and projection **rebuild** (below).
   The event log on disk is never mutated; an old payload is migrated each time it is read.

5. **Projection rebuild clears each read model and replays the log through the live query.** A
   rebuild, per aggregate, deletes the view table's rows and replays that aggregate's entire history
   (`cqrs-es` `QueryReplay::replay_all`, `with_upcasters` applied) back through the **same**
   `GenericQuery` the live store uses, so the rebuilt projection is folded by the identical `evolve`.
   Because each replay binds its aggregate type, an aggregate sees only its own events. Rebuild is a
   **maintenance operation**: the caller ensures no commands run concurrently. It is engine-neutral —
   every backend behind `PersistedEventRepository` rebuilds the same way; SQLite is the first
   implementation (`genealogy-db`). No CLI surface is added here; promoting rebuild to a maintenance
   command is roadmap Phase 3.

## Rationale

- **Built-in upcaster over a custom migration layer (1).** cqrs-es already applies upcasters at both
  the points we need (load and replay); reusing it keeps the migration story inside the framework's
  guarantees rather than a parallel mechanism we maintain.
- **Per-variant version (2).** A single aggregate-wide version would force every variant to "move"
  whenever any one payload changes, and would make an upcaster's version comparison ambiguous. Tying
  the version to the variant keeps each bump and each upcaster local to the payload that actually
  changed, which is exactly the additive-only rule ADR 0004 §4 set.
- **Upcasters in core (3).** The payload schema is owned by `genealogy-core`; its migrations are part
  of that contract and must not depend on a storage backend. `genealogy-db` only *wires* the
  registry into the store and the rebuild.
- **Read-time, not rewrite (4).** Rewriting stored events to the new shape would violate the
  append-only, immutable log (ADR 0001) and destroy the guarantee that the log is the authoritative
  record. Read-time upcasting keeps the log untouched and the migration reversible by code change.
- **Replay through the live query (5).** Rebuilding through the same `GenericQuery`/`evolve` the
  store uses guarantees a rebuilt projection is identical to one built incrementally — there is no
  second code path to keep in sync, and corrections (`AssertionRetracted` / `AssertionSuperseded`)
  are reflected for free.

## Consequences

### Positive

- A variant's payload can evolve additively at any time; historical events keep decoding, proven by
  test on both the load and rebuild paths.
- A corrupted or schema-changed projection is recoverable by rebuild without touching the event log.
- The migration registry is one obvious place per aggregate, in the crate that owns the schema.

### Negative / costs

- Upcasting cost is paid on every read of an old event (until, if ever, a snapshot story changes
  that); acceptable at genealogy log sizes and deferred deliberately (ADR 0002).
- Rebuild is offline by contract — it assumes no concurrent writes; a future online-rebuild story is
  out of scope.
- Upcasters accumulate over time and must stay ordered; this is a known maintenance cost, localized
  to each aggregate's `upcasters()`.

## Out of scope

- **Snapshotting** — still deferred (ADR 0002, 0004).
- **A user-facing rebuild/maintenance command** — roadmap Phase 3 promotes the `genealogy-db` routine
  into an application use-case and CLI command.
- **The Postgres rebuild implementation** — this ADR fixes the engine-neutral contract; the concrete
  Postgres wiring lands with the Postgres backend (ADR 0002, roadmap Phase 3). The contract does not
  change.
- **Schema-version migration of the storage tables themselves** — distinct from event-payload
  upcasting; not needed by this spike.

## References

- ADR 0001 — event sourcing; the append-only, immutable log that upcasting must not rewrite.
- ADR 0002 — `cqrs-es`, per-workspace SQLite/Postgres; deferred upcasting and snapshotting, and fixes
  the `PersistedEventRepository` boundary this ADR's mechanisms act at.
- ADR 0004 — event-sourcing implementation contract; §4 fixed the versioned, additive encoding this
  ADR's upcasters rely on.
- ADR 0009 — read-model and projection schema; guaranteed projections are rebuildable and named this
  rebuild path as its deferred follow-up.
- `docs/roadmap.md` — Spike B (the slice this ADR gates) and the "New ADRs required" table.
