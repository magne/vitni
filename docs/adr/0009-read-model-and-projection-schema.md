# 9. Read-model and projection schema

- **Status:** Accepted
- **Date:** 2026-06-19

## Context

ADR 0002 chose `cqrs-es` with SQLite (default) or Postgres per workspace, and explicitly left the
**concrete projection / read-model schema** out of scope. ADR 0004 then fixed the event encoding
and, in §3, reserved the cqrs-es `Services` slot for **cross-aggregate projection reads** (the
"aggregate tax" of `docs/data-model.md` §9) — but the *shape* those reads query was still undecided.

Spike A (`docs/roadmap.md`) is the first slice to add aggregates that reference each other
(Event→Place, Citation→Source) and so the first to need a read model that those cross-aggregate
checks can interrogate. The roadmap flags this ADR as gating Spike A, and the sequencing rule is
that the spike informs the decision: the Person and Family projections already in `vitni-db`
show the working shape, so this ADR fixes it rather than speculating.

The current read model (Person, Family) is the baseline this ADR generalizes: each `cqrs-es` `View`
is folded by the same pure `evolve` the aggregate uses, persisted by `GenericQuery` into a
`SqliteViewRepository`, and queried with SQLite `json_extract` (`HumanId` allocation and human-id
lookups both already work this way).

This ADR decides the **read-model schema only**. The mechanism of cross-aggregate invariant checks
is already decided by ADR 0004 §3 (the `Services` projection-reader); this ADR fixes the query
surface those checks read, nothing more.

The decision is **engine-agnostic** — the same schema applies to both backends ADR 0002 selects
between (SQLite default, Postgres feature-gated). Only the JSON-path *dialect* differs between
engines, and that difference is isolated inside `vitni-db`'s query layer, never leaking upward.

## Decision

1. **A projection is an opaque-JSON view row.** Each aggregate has one read model — a `cqrs-es`
   `View` rebuilt by folding events through the aggregate's own pure `evolve` — stored one row per
   aggregate in a table of exactly three columns: an aggregate-id primary key, an integer version,
   and a JSON payload (the serialized view). This is the `cqrs-es` `GenericQuery` + `ViewRepository`
   shape already in use; it is identical across backends (`SqliteViewRepository` /
   `PostgresViewRepository`). Every new aggregate (Event, Place, Source, Citation, …) gets its own
   such table and nothing more.

2. **Primary lookup is by the view id, which is the aggregate id.** Existence and by-id reads — the
   cross-aggregate checks of ADR 0004 §3 (`UnknownPlace`, `UnknownSource`) — resolve against the
   primary key (`… WHERE view_id = ?`). No scan, no secondary structure. This is identical on both
   engines.

3. **Secondary lookups query a JSON path over the payload.** Queries keyed by a field inside the
   view (e.g. `human_id`) extract it with the backend's JSON-path operator — SQLite
   `json_extract(payload, '$.state.<field>')`, Postgres the equivalent `jsonb` path expression. The
   *query surface* (which fields are looked up) is engine-neutral and defined here; the concrete
   dialect is a `vitni-db` implementation detail. This covers what Spike A and the breadth
   phases need (human-id resolution, `HumanId` allocation, listing).

4. **No denormalized columns until a query measurably needs one.** We do **not** add typed columns,
   secondary indexes, or per-field tables ahead of a demonstrated need (YAGNI). When a real query is
   too slow over the JSON path, the fix is local — add a column or index for that query, per engine —
   and is itself an additive, rebuildable change because the view is always re-derivable from the log.

5. **Projections are derived and disposable.** A view row carries no information not in the event
   log; it can be dropped and rebuilt by replay. This ADR does not build that rebuild path — see
   below — but the schema commits to nothing that would obstruct it.

## Rationale

- **Opaque JSON over a relational schema (1).** A single JSON payload folded by the shared `evolve`
  keeps the read model trivially correct — corrections (`AssertionRetracted` / `AssertionSuperseded`)
  are reflected for free because the projection runs the same fold as the aggregate. A bespoke
  relational schema per aggregate would duplicate the model in SQL and have to re-implement the
  correction semantics. The cost — opaque payloads aren't directly queryable by arbitrary fields —
  is paid only where a query actually needs it (3, 4).
- **By-id over the primary key (2).** The cross-aggregate existence check is the whole reason this
  ADR gates Spike A. Routing it through the `view_id` primary key makes the aggregate-tax read an
  indexed point lookup, not a scan, so accepting the tax (ADR 0002) stays cheap.
- **JSON-path query over indexing (3, 4).** Each engine's JSON-path operator is sufficient for the
  row counts a genealogy workspace holds for a long time; committing to it now avoids a denormalized
  schema we would have to migrate and keep in sync with the model as the remaining ten aggregates
  land. The escape hatch (a column/index per slow query) stays open precisely because the view is
  rebuildable.

## Consequences

### Positive

- Every aggregate's read model is the same three-column shape — adding one is mechanical, matching
  the Person/Family template the codebase already follows.
- Cross-aggregate invariant checks (ADR 0004 §3) read an indexed primary key; the aggregate tax is a
  point lookup.
- Projections stay re-derivable from the log; no read-model state is authoritative.

### Negative / costs

- Arbitrary-field queries pay JSON-path parsing rather than hitting a typed column/index; this
  is acceptable until measured otherwise, at which point the fix is local.
- The payload is opaque to SQL tooling — inspecting a view means reading JSON, not columns.
- A query whose performance later matters must be revisited deliberately; this ADR makes that a
  known, bounded follow-up rather than a schema redesign.

## Out of scope

- **Projection rebuild-from-log tooling** — deferred to Spike B / the upcasting ADR (proposed
  0010). This ADR only guarantees the schema does not obstruct it.
- **The cross-aggregate invariant-check mechanism** — already decided in ADR 0004 §3 (the `Services`
  projection-reader). This ADR fixes only the query surface that reader interrogates.
- **Event-version upcasting** and **snapshotting** — deferred by ADR 0002 / 0004.
- **Per-engine query dialect and the Postgres read model** — this ADR fixes the engine-neutral
  schema and query surface; the concrete `jsonb` path expressions are wired when the Postgres backend
  lands (ADR 0002, roadmap Phase 3). The shape does not change.

## References

- ADR 0002 — `cqrs-es`, per-workspace SQLite/Postgres; deferred the concrete projection schema this
  ADR settles, and fixed the aggregate-tax trade-off these projections serve.
- ADR 0004 — event-sourcing implementation contract; §3 reserves `Services` for the cross-aggregate
  projection reads whose query surface this ADR defines.
- `docs/data-model.md` — §9 aggregates and the aggregate tax, §10.1 the `UnknownPlace` /
  `UnknownSource` checks that read this surface.
- `docs/roadmap.md` — Spike A (the slice this ADR gates) and the "New ADRs required" table.
