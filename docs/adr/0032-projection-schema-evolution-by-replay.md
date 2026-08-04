# 32. Projection schema evolution by replay

- **Status:** Accepted
- **Date:** 2026-08-04

## Context

`next_human_id` and `find_view_by_human_id` (`genealogy-db`) read every stored `human_id` by
JSON-extracting it out of `payload` on every call — the "query surface" ADR 0009 §3 fixed
deliberately, on the understanding that "when a real query is too slow over the JSON path, the fix
is local — add a column or index for that query" (ADR 0009 §4). That need arrived: issue #233 and
`docs/research/gis-norway.md` §9 ("Ingest shape and cost") worked the arithmetic for a ~1,100-place
import — two full projection scans per create, an O(n²) cost before any domain rule runs — and
`docs/research/performance-profiling.md` recorded `find_person` as "flat / O(1) — indexed" from a
benchmark that, on inspection, measured a first-row hit (`fetch_optional` stopping at the
first-inserted row), not an index at all.

Exercising ADR 0009 §4's escape hatch means picking *how* the column arrives on an existing
workspace. `docs/migration-considerations.md` §1 listed this as an open question: view tables today
carry no schema-version marker and SQLite has no schema-migration framework in this stack (ADR
0002 chose no ORM/migration tool; the DDL is owned directly by `genealogy-db`). A `GENERATED ALWAYS
... STORED` column sharpens the question further: SQLite cannot `ALTER TABLE ADD COLUMN ... STORED`
onto an existing table at all, so "migrate in place" is not available regardless of framework.

This is engine-neutral in the sense ADR 0009 and ADR 0010 are: the decision governs both backends,
even though SQLite is the one with no other option and Postgres could in principle `ALTER TABLE`
a generated column in (PG 12+ supports it, at the cost of a table rewrite).

## Decision

1. **A view table's shape may change over time; a shape change is applied by dropping the stale
   table and replaying the event log, not by an in-place `ALTER TABLE`.** This generalizes what ADR
   0009 §5 already established — "projections are derived and disposable" — from *content*
   (rebuild after a correction or an upcaster change, ADR 0010) to *shape* (a column added after the
   table already existed). Both backends use the same recipe, even though Postgres could migrate a
   generated column in place: one migration story for both engines is simpler than two, and the
   drop-and-replay path already exists and is already tested (ADR 0010).

2. **`genealogy-db` detects staleness structurally, not with a version marker.** Each backend probes
   the table for the column the current code expects (SQLite `PRAGMA table_xinfo` — not
   `table_info`, which hides a `GENERATED ... STORED` column as "hidden" and would see every table,
   migrated or not, as missing it; Postgres `information_schema.columns`). A table that does not
   exist yet is not stale (the caller creates it fresh); a table that exists without the expected
   column is. This needs no schema-version table and no coordination beyond the DDL itself, at the
   cost of only detecting "this specific column is missing" rather than an arbitrary shape delta —
   sufficient for the one kind of change this ADR covers.

3. **The migration runs unconditionally at `Store::open`, before any command executes.** Per
   view table: probe → if stale, drop and remember it was dropped → create the table in the current
   shape (idempotent either way) → if it was dropped, replay that aggregate's full event log back
   through the live `GenericQuery` (the exact mechanism ADR 0010 built) to repopulate it. A workspace
   opened by old code, upgraded, then reopened needs no separate `migrate` command — opening is the
   migration.

4. **The concrete change this ADR ships: a `human_id` column, `GENERATED ALWAYS AS (...) STORED`
   from `payload`, plus two indexes, on every human-id-bearing view table.** An equality index
   (`{table}_human_id_idx`) serves `find_view_by_human_id` and `list_views`' `ORDER BY`; a composite
   index (`{table}_human_id_len_idx` on `(length(human_id), human_id)`, `COLLATE "C"` on the second
   key on Postgres) serves `next_human_id`'s new per-length-group descending scan — grouping by
   length first because `IdFormat::extract_number` does not check digit count, so a lexical scan
   across mixed widths (`I00000003` sorts after `I10001`) would hand back a number that is not the
   true maximum. The length groups themselves are enumerated by walking that index one probe at a
   time, because `SELECT DISTINCT length(human_id)` visits every index entry on SQLite (no loose
   index scan) and would keep the allocator O(rows) — measured in
   `docs/research/performance-profiling.md`. Neither index is `UNIQUE`: duplicate `human_id`s are not prevented anywhere today,
   and a workspace already holding one must still open. Tag has no `human_id`; its column is always
   `NULL`, which is also what `list_tags`' existing `ORDER BY` over an always-`NULL` expression
   already assumed.

## Rationale

- **Drop-and-replay over an in-place migration (1).** SQLite gives no other choice for a `STORED`
  generated column; extending the same recipe to Postgres means the two backends share one tested
  code path instead of a SQLite-only migration plus a separate Postgres `ALTER TABLE`, for a
  workspace population (genealogy logs) where a full replay costs seconds, not minutes (measured in
  `docs/research/performance-profiling.md`).
- **Structural detection over a version marker (2).** A version table is the standard answer, but it
  is a second piece of state that itself needs a migration story the first time it is introduced,
  and this system already has an authoritative structural fact to probe — the column either is or
  is not there. Introducing version tracking is deferred until a shape change this probe cannot
  express actually arrives.
- **Migrate at `open`, not via a command (3).** ADR 0010's rebuild is already a maintenance
  operation the caller must not run concurrently with commands; running the shape check at open,
  before any framework is wired, keeps that invariant — no window where a command could execute
  against a table mid-migration.
- **STORED column + two indexes, not a bare expression index (4).** Measured: rewriting
  `json_extract(payload, ...)` as `payload->>'...'` does not make SQLite reuse an expression index
  built on the *other* expression — an expression index only fires when the query's expression
  matches the index's structurally. A stored, named column sidesteps that fragility entirely: any
  query that names the column can use the index, regardless of how the column's *definition*
  expression is spelled. SQLite also cannot use an expression index as a covering index for
  `count(expr)` (measured: 22.6 ms via the index vs. 2.4 ms via a `STORED` column at 50k rows) — a
  second, independent reason to materialize the value rather than index the expression.

## Consequences

### Positive

- `find_view_by_human_id` and `next_human_id` are index-backed instead of full-scanning `payload`
  on every call, closing the O(n²) bulk-import cost issue #233 named; `list_views`'
  `ORDER BY human_id` also benefits (measured in `docs/research/performance-profiling.md`).
- The migration path is exercised by the same tests that prove rebuild-from-log correctness (ADR
  0010): a workspace's data is never lost across a shape change, only recomputed.
- The recipe generalizes to the next column a slow query needs (ADR 0009 §4's escape hatch), without
  a new mechanism.

### Negative / costs

- **Write cost on every projection insert/update, on both engines, for every human-id-bearing
  table** — two more indexes to maintain per write. Measured and recorded in
  `docs/research/performance-profiling.md`, including the `rebuild_projections` benchmark's
  regression at growing log sizes.
- **A shape change is an all-or-nothing table replacement, not a targeted `ALTER TABLE`.** Acceptable
  because a view table holds only what the log already has, but it means every shape change pays a
  full replay of that aggregate's history, however small the actual column being added.
- **Structural detection cannot express every possible future shape change** — only "this column is
  present or absent." A change that needs finer-grained versioning (e.g. two possible shapes with the
  same columns but different semantics) would need a real version marker; not needed yet.

## Out of scope

- **A general schema-version table or migration framework** — deferred until a shape change this
  ADR's structural probe cannot express actually arrives.
- **Postgres `ALTER TABLE`-in-place as an alternative path** — technically available on Postgres
  (PG 12+), deliberately not taken, for the one-recipe-for-both-engines reason in Rationale.
- **A user-facing "migrating workspace..." progress indicator** — the migration is fast enough at
  today's population (seconds, per `docs/research/performance-profiling.md`) that Phase 3's rebuild
  command already covers the case where a user wants to see it happen explicitly.

## References

- ADR 0002 — engine selection (SQLite default, Postgres feature-gated); no ORM/migration tool, so
  `genealogy-db` owns DDL directly, which is what makes a migration story `genealogy-db`'s to write.
- ADR 0004 §3 — the aggregate-tax cross-aggregate reads this ADR's indexes also serve.
- ADR 0009 §3 — the JSON-path query surface this ADR's column supersedes for `human_id` specifically;
  §4 — the "add a column or index when a query measurably needs one" escape hatch this ADR exercises;
  §5 — "projections are derived and disposable," generalized here from content to shape.
- ADR 0010 — event-version upcasting and projection rebuild; this ADR's migration reuses that
  rebuild mechanism verbatim, applying it to a shape change rather than a payload-version bump.
- `docs/research/gis-norway.md` §9 — the ingest-cost arithmetic that named this as a prerequisite for
  the geography import.
- `docs/research/performance-profiling.md` — before/after measurements, and the corrected
  `find_person` finding.
- `docs/migration-considerations.md` §1 — the open question this ADR answers.
