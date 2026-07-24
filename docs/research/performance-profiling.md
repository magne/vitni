# Research — performance profiling & the snapshotting verdict (Phase 11 workstream B)

- **Status:** Findings. Provides the measurement ADR 0004 defers snapshotting on.
- **Date:** 2026-07-25
- **Gating ADR:** [0004](../adr/0004-event-sourcing-implementation-contract.md) — snapshotting is
  "deferred until replay cost warrants it"; this workstream is that measurement.
- **Harness:** `crates/genealogy-db/benches/store.rs` (criterion, `harness = false`).

## Question

ADR 0004 §"Out of scope" defers snapshotting as "a performance concern, deferred until replay cost
is *measured* to warrant it." Two cost centres it named need real numbers: **event-log replay /
projection rebuild** at growing log sizes, and the **hot query paths** (person list/detail, the
`places_in_bbox` spatial query, the `ResearchNote` reverse-by-subject `json_each` index). This doc
measures them and states the snapshotting verdict.

## Methodology

- **Harness.** [criterion](https://crates.io/crates/criterion) 0.8.2 (dev-dependency of
  `genealogy-db`, `default-features = false` + `cargo_bench_support` only — the plotters/rayon
  HTML-report tree is dropped so `cargo deny` stays clean). One `[[bench]]` target, `store`,
  measuring the public [`Store`] surface. `rebuild_projections` is async and is driven with a single
  `tokio::runtime::Runtime` built once outside the measured closure (`rt.block_on(...)` inside
  `iter`), per the ADR-0004 pure-core contract (no clock/id inside the store).
- **Engine.** SQLite (the default engine, ADR 0002), an **on-disk** temp-dir database per size —
  the log must be persisted for a rebuild to replay it. Postgres is not benched (the spatial /
  succession queries are SQLite-only today).
- **Fixture.** A synthetic workspace built through the *public command surface* — the same pure
  `decide` → event-store path the integration tests use — so every datum is real replayed log, never
  hand-written projection rows. It spans six aggregates (Person: create + name + sex + fact; Place:
  create + coordinates; Source; Citation into an existing source; Family: create + 2 partners + 2
  children over existing persons; ResearchNote naming two person subjects). Counts are proportioned
  from a `persons` knob; the emitted event total is the benchmark parameter.
- **Sizes.** Person knobs `{180, 1800, 9000}` emit **1 052 / 10 530 / 52 650** events. The top size
  is **capped at ~50k events** so a full `cargo bench` finishes in a few minutes (fixture build +
  both groups ≈ 3-4 min); the rebuild group uses `sample_size(10)` because a single 50k rebuild is
  seconds. These are the committed sizes.
- **Build profile.** criterion compiles under the optimized `bench` profile (opt-level 3), not the
  dev profile. Numbers are a **single run on one developer machine** — an **Intel Core Ultra 7 165H**
  (22 logical cores), warm page cache — not a controlled CI measurement. Read them as **order of
  magnitude and scaling shape**, not absolute guarantees.

## Measured numbers

### Projection rebuild (`Store::rebuild_projections`)

| events | rebuild time (mean) | per-event | throughput |
| -----: | ------------------: | --------: | ---------: |
|  1 052 |             76.6 ms |   72.8 µs | 13.6 Kelem/s |
| 10 530 |            933.7 ms |   88.7 µs | 11.3 Kelem/s |
| 52 650 |            5 571 ms |  105.8 µs |  9.5 Kelem/s |

**Near-linear, with a mild super-linear drift.** 10× the log → 12.2× the time; the next 5× → 6.0×.
Per-event cost rises ~45 % across the range (73 → 106 µs/event) — consistent with index-insert /
B-tree costs that grow slowly with table size (the rebuild also re-derives the R\*Tree geometry
index and the succession cross-reference index from the freshly rebuilt Place projection). Straight-
line extrapolation on the observed per-event cost: **~100k events ≈ 11-13 s**, and a **100k-person**
workspace (~585k events at ~5.85 events/person) **≈ 60-90 s**.

### Hot query paths (mean latency)

| query                          |  1 052 ev | 10 530 ev | 52 650 ev | scaling |
| ------------------------------ | --------: | --------: | --------: | ------- |
| `find_person` (by `HumanId`)   |   24.8 µs |   24.5 µs |   26.3 µs | **flat / O(1)** — indexed |
| `list_persons` (full list)     |    845 µs |   9.51 ms |   61.7 ms | **O(n)** — full scan + per-row JSON decode (~6.8 µs/person) |
| `places_in_bbox` (R\*Tree)     |   44.1 µs |    194 µs |    861 µs | O(matches) — ~0.76 µs/place; here the bbox covers every place (worst case) |
| `research_notes_for_subject`   |   69.6 µs |    416 µs |   1.98 ms | O(notes) — full-table `json_each` scan (~0.88 µs/note; returns 1 row) |

Person counts backing the query sizes: 180 / 1 800 / 9 000. Place counts: ~22 / 225 / 1 125.
ResearchNote counts: 45 / 450 / 2 250.

## The snapshotting verdict

**Measured — snapshotting is NOT warranted at the 1.0 target scale. ADR 0004's deferral stands; no
follow-up ADR is needed.** The reasoning is two-fold:

1. **Snapshotting optimizes the wrong thing for this model.** cqrs-es snapshotting shortens the
   reload of a *single aggregate's* event stream before a command executes, so a hot aggregate with
   thousands of events need not replay them all per command. In this domain each aggregate instance
   accumulates only a **handful** of assertion events (a person gathers a few names/sex/facts/
   participations — tens even when heavily edited); a stream never grows into the thousands. So
   per-command aggregate load already replays a short stream **regardless of workspace size**, and a
   snapshot would save essentially nothing on the command path.
2. **The one measured full-replay cost is a bounded maintenance operation.** `rebuild_projections`
   replays the whole log, but it is a **maintenance / recovery** path (schema change, corruption
   recovery, the `genealogy rebuild` command) — never interactive. Steady-state reads never trigger
   a replay: projections update **incrementally** via the live cqrs-es `Query` as each command
   commits. At 52 650 events it is 5.6 s; even the pessimistic ~1-2 min for a very large
   (100k-person) workspace is acceptable for an infrequent operation — **and snapshotting would not
   help it**, since a full rebuild must replay the entire log to re-derive projections either way.

If the model ever grew long single-aggregate streams (it does not today), revisit — that is the only
condition under which snapshotting would pay off.

## Other findings (independent of snapshotting)

1. **`list_*` projections load every row with no `LIMIT`/`OFFSET`.** `list_persons` is a clean O(n)
   full scan (~6.8 µs/person): 61 ms at 9 000 persons, extrapolating to ~0.68 s at 100k. This is the
   real scaling lever for the interactive path — **pagination in the query / use-case layer** (a
   `limit`/`offset` on the list projections) is the fix, well before 100k-scale. This matches the
   already-noted absence of pagination infrastructure in the use-case layer. Not a blocker at the
   benched sizes; flag for the list/detail screens at large workspaces.
2. **`research_notes_for_subject` is a full-table `json_each` scan, not a true index.** The
   reverse-by-subject lookup walks `$.state.subjects` with a correlated `EXISTS (SELECT … json_each)`
   over every `research_note_view` row (`sqlite_query::list_views_by_subject`), so it is O(note
   count) even though it returns one row: 2 ms at 2 250 notes. Fine now; if research-note volume ever
   grows large, materialize a `(research_note_id, subject_kind, subject_value)` side-index at
   projection time and query that — the same shape as the external-id lookup. Recorded, not urgent.
3. **`find_person` is flat and indexed** (~25 µs across all sizes) — no action.
4. **`places_in_bbox` scales with match count, not table size** — the R\*Tree does its job; the
   benched worst case (bbox covering every place) is the ceiling, and a realistic viewport returns a
   subset, so the real-world number is lower.

## Reproducing

```bash
cargo bench -p genealogy-db                 # full run (~3-4 min): fixture build + rebuild + query
cargo bench -p genealogy-db -- rebuild      # rebuild group only
cargo bench -p genealogy-db -- query        # query group only
```

The fixture is built once per size and shared by both groups; criterion writes estimates under
`target/criterion/`.
