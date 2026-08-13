# Research — performance profiling & the snapshotting verdict (Phase 11 workstream B)

- **Status:** Findings. Provides the measurement ADR 0004 defers snapshotting on.
- **Date:** 2026-07-25
- **Gating ADR:** [0004](../adr/0004-event-sourcing-implementation-contract.md) — snapshotting is
  "deferred until replay cost warrants it"; this workstream is that measurement.
- **Harness:** `crates/vitni-db/benches/store.rs` (criterion, `harness = false`).

## Question

ADR 0004 §"Out of scope" defers snapshotting as "a performance concern, deferred until replay cost
is *measured* to warrant it." Two cost centres it named need real numbers: **event-log replay /
projection rebuild** at growing log sizes, and the **hot query paths** (person list/detail, the
`places_in_bbox` spatial query, the `ResearchNote` reverse-by-subject `json_each` index). This doc
measures them and states the snapshotting verdict.

## Methodology

- **Harness.** [criterion](https://crates.io/crates/criterion) 0.8.2 (dev-dependency of
  `vitni-db`, `default-features = false` + `cargo_bench_support` only — the plotters/rayon
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
| `find_person` (by `HumanId`)   |   24.8 µs |   24.5 µs |   26.3 µs | **not actually indexed — see the ADR 0032 update below** |
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
   recovery, the `vitni rebuild` command) — never interactive. Steady-state reads never trigger
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
3. **`find_person` was flat, but not because it was indexed — see the ADR 0032 update below.** It
   looks up `I0000001`, the first-inserted row, and `fetch_optional` stops the scan at the first
   hit; the flat ~25 µs across sizes is a first-row hit, not evidence of an index.
4. **`places_in_bbox` scales with match count, not table size** — the R\*Tree does its job; the
   benched worst case (bbox covering every place) is the ceiling, and a realistic viewport returns a
   subset, so the real-world number is lower.

## ADR 0032 update — the `human_id` index (2026-08-04)

[ADR 0032](../adr/0032-projection-schema-evolution-by-replay.md) added a `GENERATED ALWAYS ... STORED`
`human_id` column plus two indexes to every human-id-bearing view table, replacing the
`json_extract`/`->>` scans `next_human_id` and `find_view_by_human_id` did before (issue #233). This
re-measurement corrects finding 3 above and gives the before/after numbers `docs/issues.md`'s bullet
asked for. **Quick mode** (fewer samples than a full run — the invocation is in *Reproducing* below),
same machine and sizes as above:

Both lookups are benched against the **last** allocated id, not the first. The old `find_person` row
in the table above looks up `I0000001`, and `fetch_optional` stops a scan at its first hit, so it
timed a first-row hit and reported it as an index. The `before` column here is the pre-#233 query
shapes (`json_extract(payload,'$.state.human_id')` scan for both) measured against that same last-id
lookup, so the two columns differ only in how the id is found:

| query (mean)                      |   before |   after | change |
| --------------------------------- | -------: | ------: | -----: |
| `find_person`, 1 052 ev           |   325 µs | 25.5 µs |   13 × |
| `find_person`, 10 530 ev          |  3.16 ms | 22.4 µs |  141 × |
| `find_person`, 52 650 ev          |  14.3 ms | 21.8 µs |  654 × |
| `next_person_human_id`, 1 052 ev  |   514 µs | 60.4 µs |  8.5 × |
| `next_person_human_id`, 10 530 ev |  4.63 ms | 63.4 µs |   73 × |
| `next_person_human_id`, 52 650 ev |  24.3 ms | 57.7 µs |  421 × |
| `list_persons`, 1 052 ev          |   890 µs |  616 µs |  1.4 × |
| `list_persons`, 10 530 ev         |  13.1 ms | 8.56 ms |  1.5 × |
| `list_persons`, 52 650 ev         |  64.6 ms | 46.4 ms |  1.4 × |

**Both hot paths are now flat in table size** — 50 × the rows (180 → 9 000 persons) leaves both
within noise of each other, where before each grew linearly. That is what issue #233 asked for.
`EXPLAIN QUERY PLAN` shows `SEARCH person_view USING INDEX person_view_human_id_idx (human_id=?)` for
the lookup, and a `SEARCH … USING INDEX person_view_human_id_len_idx (<expr><?)` probe for the
allocator's length walk; a unit test pins both
(`sqlite::tests::the_human_id_lookup_and_allocator_queries_use_their_indexes`).

`list_persons` got moderately faster as a side effect — ordering by the indexed generated column
avoids a `json_extract` per row during the sort and the temp b-tree the expression-based `ORDER BY`
needed — but it stays O(n): it still loads and decodes every payload, which finding 1 above is about.

**One planner trap worth recording.** The allocator enumerates its length groups by walking them one
index probe at a time ("the longest id shorter than the last one"). The obvious
`SELECT DISTINCT length(human_id) …` is not equivalent: SQLite has no loose index scan, so it visits
every index entry. Measured directly on a 50 000-row table, 1.745 ms against 0.010 ms for one probe —
which is the difference between an allocator that reads every row and one that does not, and it does
not show up as a plan difference (both report `… USING INDEX …_len_idx`). The first version of this
change shipped the `DISTINCT` form and benched at 58/123/485 µs across the three sizes — visibly O(n)
under a 100 × smaller constant than the JSON scan, but still O(n).

**Rebuild write cost.** ADR 0032's decision to add both indexes to all 12 human-id tables trades read
speed for write cost on every projection insert/update, paid during `rebuild_projections` and every
live command. Quick-mode `rebuild` numbers, same sizes:

| events | before (mean) | after (mean) | change |
| -----: | ------------: | -----------: | -----: |
|  1 052 |       76.0 ms |      77.4 ms |   +1.8 % |
| 10 530 |      925.2 ms |     962.7 ms |   +4.1 % |
| 52 650 |      5.585 s |      5.947 s |   +6.5 % |

Smaller than the 20–40 % expected going in (from an isolated bulk-insert measurement of ~+71 % showing
the two index writes' raw cost) — in `rebuild_projections` that cost is a modest fraction of the total
per-event cost, which also covers the R\*Tree geometry index and the succession cross-reference index
on `place`, and quick mode's smaller sample count makes each number noisier than the full-run figures
elsewhere in this doc. Criterion's own significance test calls all three "No change in performance
detected" at `p > 0.05`; read the row as an upper bound on the write cost, not a precise regression
figure.

## Reproducing

```bash
cargo bench -p vitni-db                 # full run (~3-4 min): fixture build + rebuild + query
cargo bench -p vitni-db -- rebuild      # rebuild group only
cargo bench -p vitni-db -- query        # query group only
```

**`--quick` needs the compiled binary, not `cargo bench -- --quick`.** `cargo bench` also runs the
lib's unit tests in bench mode, and `--quick` is forwarded to every binary it runs; the unit-test
binary doesn't understand it and the whole invocation aborts before the actual `store` bench ever
runs (`error: Unrecognized option: 'quick'`). Build once, then invoke the `store` binary directly,
passing the `--bench` flag criterion normally receives from `cargo bench` itself so it doesn't fall
back to its own "smoke test" mode (`cargo test --benches`'s mode, which just prints `Testing … /
Success` with no timings):

```bash
cargo bench -p vitni-db --bench store --features sqlite --no-run
./target/release/deps/store-<hash> --bench --quick   # <hash> from the build output above
```

The fixture is built once per size and shared by both groups; criterion writes estimates under
`target/criterion/`.
