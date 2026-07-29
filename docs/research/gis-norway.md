# Research — Norwegian administrative geography: model fitness and a GIS import plugin

- **Status:** Research. No code, no ADR, no issue edits — recommendations only.
- **Date:** 2026-07-29
- **Question:** Is the `Place` aggregate up to the task of Norwegian historical administrative
  geography, and can a plugin create Norway, import counties and municipalities (ideally back to
  1838) with dated hierarchy and successions, and do the same for the ecclesiastical hierarchy?

## Summary

**The `Place` aggregate is close, but not currently up to the task.** Ten of thirteen fitness questions
came back Insufficient. Three are hard blockers: there is no `MultiPolygon`, so 200+ of 357
municipalities cannot be represented without discarding their islands; `PlaceRef` has no relation
kind, so a civil and an ecclesiastical parent collapse into one list and produce silently false
hierarchy titles; and `assert_place_enclosed_by` cannot write a date at all, so dated hierarchy is
unreachable from the app layer. All fixes are additive per ADR 0004 §4 — the model's shape is right,
its coverage is not.

**The premise "1838 is the earliest available GIS resource" is essentially right for municipalities,
and the data is better than expected.** Two separate things both reach 1838. The *change register* is
machine-readable — SSB KLASS classification 131 has 141 versions from `Kommuneinndeling 1838` onward,
with 2,093 coded change records. And *boundary geometry* is served too: **`kart.ssb.no` runs an
unauthenticated OGC API – Features service with 129 per-year municipality collections spanning
1838–1996**, plus a 22-collection improved-accuracy series for 1986–2019. Verified directly:
`Kommuner 1838` returns **395 features with real `MULTIPOLYGON` geometry**, the whole year weighs
4,613,865 bytes (inside the plugin's 8 MiB cap), and the service will **reproject server-side to
EPSG:4326** on request — so no SOSI parsing and no client-side reprojection are needed for the civil
import at all. County geometry is the exception: only current *fylker* are served, so county history
is entity-only.

**The ecclesiastical side inverts the expectation.** The seven prior chats never mentioned
*bispedømme* or *prosti* and invented a dataset ("HAG") that does not exist. In fact KLASS codes
diocese history back to **1070** and deanery history to **1865** — deeper than the civil *fylke*
series — and Kartverket's Soknegrenser does publish **1,154 current parish and 11 diocese polygons**,
each parish carrying a Brønnøysund *organisasjonsnummer* that is the best idempotency key found for
either hierarchy. The catch is that it is SOSI/EPSG:25833 only, needing a small bespoke reader, and
that **no historical parish geometry exists anywhere** — one CC BY 4.0 1801 layer aside. So: import
the ecclesiastical hierarchy as dated entities plus a current geometry snapshot; do not expect
historical polygons.

Recommended path: fix the model (one new ADR), index `human_id` before any bulk run, add seven additive
WIT verbs and a `reference-data` world, then ship a plugin that fetches everything over `net` — current
boundaries first, 188 years of entity *and* boundary history second, ecclesiastical entities third. The
decision that most needs making early is whether reference data is separable from a user's own places;
retrofitting that after importing ~1,100 municipalities is much harder than deciding it now.

## Scope and method

Three evidence tiers, labelled throughout:

- **Repo** — cited `path:line`. Authoritative.
- **Web** — cited by URL, all retrieved **2026-07-29**. Authoritative once fetched.
- **Chat** — the seven transcripts in `docs/research/gis-norway-chats/`
  (`chatgpt|claude|copilot|deepseek|gemini|grok|qwen.md`). **Unverified hypotheses.** None of the
  seven fetched a page; two contain invented endpoints. Every substantive claim is audited in
  §Chat-claim audit with a status of VERIFIED / UNVERIFIED / REFUTED.

Live verification performed for this document, not delegated: SSB KLASS classifications 131, 104, 510,
651, 655 and 644 plus the `changes` and `codesAt` endpoints; the `kart.ssb.no` OGC API collection list,
the `Kommuner 1838` collection metadata, its feature count, a sampled `MULTIPOLYGON`, the full-year
payload size and the server-side reprojection to EPSG:4326; the Kartkatalogen records for Soknegrenser
and the Kartverket historical dataserie (including its CSW ISO metadata); the data.norge.no page for
SSB's boundary series; the Brønnøysund church-entity register; the `atlefren` repository contents; and
eleven crates.io records plus four confirmed-absent crate names. Repo claims that carry weight — the
Postgres read break, the geography label bug, the `PlaceType::Custom` label path — were re-read directly
rather than taken from a summary.

Where a delegated finding disagreed with a primary source, the primary source won and the disagreement
is recorded rather than smoothed over. Three such corrections are noted in place: the KLASS full-range
`changes` query does not hard-fail, `Standard for prestegjeld` does exist, and the RHD *matrikkel*
licence is unstated rather than restrictive.

**A documentation caveat found in passing:** `docs/data-model.md` and `docs/data-model-diagram.md`
were never updated after Phase 9. Neither mentions `PlaceGeometry`, `AssertGeometry`,
`AssertSuccession`, or the effective-from rule. ADR 0024/0025/0026 and the code are the truth;
§10 of the data model still lists the pre-Phase-9 Place command set. This should be filed.

## Model fitness

Thirteen questions, each ending in a verdict. Every proposed change is checked for additivity against
ADR 0004 §4 (internally-tagged, append-only, `#[serde(default)]` for new fields).

### 1. Parallel, non-nesting hierarchies

**Web:** the 1837 *formannskapslov* made each *prestegjeld* one *formannskapsdistrikt* from
1838-01-01, so civil and ecclesiastical units began coterminous and then diverged — *kommuner*
split, *sokn* did not. SSB's 2026-01-01 change list alters *kommuner*, *sokn* and *prosti*
independently. So a farm in 1900 legitimately has a *kommune* parent, a *sokn* parent, a
*prestegjeld* parent and a *tinglag* parent, and these do not nest.

`PlaceRef` carries `{ place_id, date }` and no relation kind (`place_ref.rs:14`), so all four land in
one undiscriminated `Vec` (`place/state.rs:38`). `enclosed_by_as_of` returns exactly **one**
(`place/view.rs:143`); with two undated parents that is `.first()` (`view.rs:153`) and with both
dated 1838 it is `resolve_as_of`'s "later-encountered wins" tie-break (`temporal.rs:34`) — in both
cases **import order decides**. `resolve_hop` (`genealogy-app/src/place.rs:1147`) then continues up
whichever hierarchy that hop landed in, so `hierarchy_chain` can emit a mixed, factually false chain
and `generated_title` (`place_hierarchy.rs:60`) renders it as the Places-list label, the breadcrumb
and the record-picker label.

Worse, `PlaceSummary` has no field for the live enclosure set — only `enclosing`, built from the
resolved chain (`place.rs:159,1198`). A second parent is therefore invisible to the frontend: not
shown, not editable, and its `AssertionId` never reaches the UI, so it cannot be retracted.

Alternatives weighed. A separate aggregate per hierarchy duplicates all 17 commands, breaks
`Event.place_id` (a baptism happens in a *sokn*), and contradicts the twelve-aggregate closure
(data-model §9). Convention-only — filter the chain by the parent's `PlaceType` — needs no event
change but is fragile exactly where it matters (*prestegjeld* and *herred* were often coterminous and
would be identically typed), cannot express a judicial parent, and does not fix the invisible-parent
DTO gap.

**Verdict: Insufficient.** Smallest fix: `#[serde(default)] relation: Option<PlaceRelation>` on
`PlaceRef`, with `PlaceRelation = Civil | Ecclesiastical | Judicial | Statistical | Custom(String)`
in `enums.rs` — additive per ADR 0004 §4, the same shape `PlaceState.geometries` already uses
(`state.rs:43`). Then thread a relation argument through `view.rs:143,153`,
`place_hierarchy.rs:41`, `place.rs:1147`, and surface the full enclosure set on `PlaceSummary`.

### 2. Effective-from versus validity intervals — the dissolved municipality

**Web:** 0435 Os existed 1951–1965, merged into 0435 Tolga-Os on 1966-01-01, which split again on
1976-01-01 into 0436 Tolga and **0441** Os.

Take the Os(1951–65) aggregate with a 1951-effective name, enclosure and geometry, plus
`SuccessionAsserted { from: [Os, Tolga], to: [Tolga-Os], kind: Merged, date: 1966 }`. Then
`show_place_as_of(Os, 2026)` resolves the name to "Os" (`view.rs:159`), the enclosure to Hedmark
(`view.rs:143` — and `resolve_hop` walks happily through a county that itself ceased in 2020), and
the geometry to the 1951 polygon (`view.rs:168`). `show_geography(2026)` then plots that polygon as
a live marker (`geography.rs:134-159` — the loop has no existence test), overlapping its own
successors. **A dissolved unit resolves forever.**

`SuccessionAsserted` does not rescue this. Its date is optional (`place_succession.rs:26`), it is
read through a separate index consulted only by `show_place` (`place.rs:784-785`), and nothing in
the resolution path reads it — `resolve_as_of` (`temporal.rs:24`) has exactly three callers, the
three `*_as_of` wrappers, none succession-aware.

The boundary-transfer case comes out the opposite way. **Web:** SSB's 2026-01-01 *grensejustering*
moves area between Indre Østfold, Nordre Follo and Vestby with no unit created or ended.
Effective-from handles that correctly and cheaply — one new dated `AssertGeometry` per affected
place. Modelling it as a succession would be wrong, since no identity changes. So *grensejustering*
is not the gap; **cessation** is.

This touches the recorded decision **"Explicit `[from, until)` validity intervals"**, and the finding
**supports leaving it closed**: intervals are the wrong-sized fix, because only a place's *lifetime*
needs an end, not every assertion.

**Verdict: Insufficient — fixable without intervals and without an event change.** Add
`PlaceView::existed_as_of(target) -> bool` deriving cessation from data already in the payload (a
live succession dated `<= target` whose `from` contains this `place_id` and whose `to` does not),
consumed by `show_geography` (skip the marker) and `summarize_as_of` (flag "ceased 1966").

One residual this cannot reach: cessation with **no** successor. `decide.rs:224` rejects an empty
`to`, and **web:** the *prestegjeld* level was abolished wholesale — out of church administration in
2004, out of law in 2012 — with nothing succeeding it. That needs a real additive event,
`AssertDissolution` → `DissolutionAsserted { place_id, date }` across
`place/{command.rs,event.rs,decide.rs,state.rs}` (additive per ADR 0004 §4).

### 3. `PlaceType` coverage

**Web:** *amt* = *fylke*, 1662–1918, subdivided into *fogderier*; *stiftamt* = the *amt* holding the
bishop's seat (4–5 ever existed); *fogderi* ran ≈54 (1595) → 38 (1700) → 56 (1894), abolished
1898–1919; *tinglag* = court district, roughly a *skipreide* or a *prestegjeld*; *formannskapsdistrikt*
(1838) was renamed *kommune*/*herred* by the 1853 *matrikkellov*; a *prestegjeld* is one *hovedsokn*
plus *annekssokn*, under a *prosti*, under a *bispedømme*. FamilySearch glosses *sokn* as "Parish"
and *prestegjeld* as "District (usually two or more parishes)".

Already covered by `enums.rs:193`: *amt*/*fylke* → `County`; *formannskapsdistrikt*/*herred*/*kommune*
→ `Municipality` (a rename, ADR 0026 §2, not a new type); *by*/*kjøpstad*/*ladested* → `City`/`Town`;
*sokn* → `Parish`; *matrikkelgård* → `Farm`. **Genuinely missing: *prestegjeld*, *prosti*,
*bispedømme*** — the whole ecclesiastical hierarchy above the parish, which for Norwegian genealogy
is the part the sources are actually organised by. Also missing *grunnkrets*, the statistical unit SSB
expresses changes in.

The `Custom(String)` consequence is concrete, not theoretical: `crates/genealogy-ui/src/i18n.rs:519`
is `PlaceType::Custom(value) => value.clone()`, so `Custom("prestegjeld")` renders a raw Norwegian
string in every list, breadcrumb, picker and map-marker label — directly contradicting data-model §14
("stored as language-neutral codes … translated in the UI, never stored") and ADR 0003. Additivity is
not the obstacle: `PlaceType` is `#[serde(tag = "type", content = "value")]` (`enums.rs:192`), so new
variants are append-only; the cost is three lines each in `enums.rs` and `i18n.rs` plus one Fluent
key per locale, and `cargo xtask i18n-check` enforces the label.

**Verdict: Sufficient for the civil hierarchy, Insufficient for the ecclesiastical one.** Add three
variants named to generalise beyond Norway: `Diocese` (*bispedømme*/*stift*), `Deanery` (*prosti*),
`District` (*prestegjeld*). Leave *fogderi*/*tinglag*/*skipreide*/*stiftamt*/*grunnkrets* as `Custom`
(~150 units, Norwegian-only names) but record that as an explicit §14 exception rather than letting
it happen silently.

### 4. `SuccessionKind` coverage

**Web:** SSB's own taxonomy is *sammenslåing*, *deling*, *grenseregulering*/*grensejustering*
(populated and unpopulated areas counted separately), *endring av kommunenummer*, *overføring mellom
fylker*, plus status changes.

Mapping onto `Merged | Split | Absorbed | Elevated | Renamed` (`enums.rs:221`): *sammenslåing* →
`Merged`; *deling* → `Split` (Troms og Finnmark → Troms + Finnmark, and Ålesund → Ålesund + Haram,
both 2024-01-01); *herred* → *by* status change → `Elevated`. The gaps and their disposition:

- ***grensejustering*** — correctly **not** a succession. Both units keep identity; the right model
  is a new dated `AssertGeometry` (ADR 0024 §2). This is the numerically dominant change type across
  1838–1998, and the model handles it *provided* polygons exist (§9) and can be dated (they can —
  `AssertGeometry` takes a date, unlike enclosure).
- **transfer between counties** (Bærum 0219 → 3024 → 3201 as Akershus → Viken → Akershus; Leka from
  Nord-Trøndelag to Trøndelag in 2018) — not a succession, just a new dated `AssertEnclosedBy`. The
  vocabulary is fine; the **write path is broken** — `assert_place_enclosed_by` hard-codes
  `date: None` (`place.rs:387`). This is the filed issue "Dated name/enclosure use-cases".
- **code-only change** — a new `SetCode`, but `code` is last-writer-wins (`state.rs:50`), so the
  number history is lost. See §10.
- **dissolution with no successor** — genuinely unrepresentable. See §2.
- **re-establishment** — `Split` on the intervening unit suffices, and the repo rule *agrees with
  Norwegian practice*: **web:** re-separated municipalities get a **new** number (0435 Os → 0435
  Tolga-Os → 0441 Os). New number, new identity, new aggregate.

Note `SuccessionKind::Renamed` (`enums.rs:230`) is a trap: it invites recording a plain rename as a
succession, which ADR 0026 §2 forbids (Aa → Lyngdal, 1919, is a dated `AssertName`). Document it;
removing it would be non-additive.

**Verdict: Sufficient-but-awkward.** Add **no** new variants. The two real gaps are elsewhere:
no-successor dissolution (§2) and the undated-enclosure write path (already filed).

### 5. Identity rule and the natural key

(a) **Troms og Finnmark** is **five** aggregates under the repo rule, not three: Troms(1919–2019, 19),
Finnmark(1919–2019, 20), Troms og Finnmark(2020–2023, 54), Troms(2024–, **55**), Finnmark(2024–,
**56**) — **web:** the 2024 units received new numbers. Two successions:
`Merged { [19,20] → [54], 2020 }` anchored on Troms19 and `Split { [54] → [55,56], 2024 }` anchored
on TF54, both satisfying the anchor rule (`decide.rs:217`). The model is right; the *read* is thin —
`place_predecessors`/`place_successors` are single-hop (`sqlite.rs:233,242`), so "what became of
Troms(1919)?" takes two manual clicks.

(b) **Code reuse is real and documented.** **Web:** SSB publishes a list of duplicate municipality
codes — 0516 Heidal(1908–1964)/Nord-Fron(1977–2019); 0701 Svelvik/Borre; 0704
Åsgårdstrand/Tønsberg(1988–); 0712 Skoger/Larvik(2018–2019); 1523 Sunnylven/Ørskog; 1806
Svolvær/**Narvik(2020–)**; 1923 Øverbygd/Salangen. A *kommunenummer* identifies a unit only together
with its validity period. Nothing in the model forbids two aggregates carrying `code = "1806"` —
good, `code` has no uniqueness constraint — but **any importer keying on the code alone merges
Svolvær into Narvik.**

(c) **Dissolved then re-established** → new aggregate plus a `Split` from the intervening unit, which
matches (b)'s convention exactly (Haram merged into Ålesund 2020, re-established 2024 on its
pre-2020 border).

**Natural key:** `(authority = "ssb-klass", value = kommunenummer, valid_from = year)` — SSB's own
key. Place has no `ExternalId`: `for_each_db_external_id_aggregate!` lists only person and family
(`crates/genealogy-db/src/registry.rs:70-79`), and `genealogy-app/src/import.rs` has no place path
at all.

**Verdict: Sufficient on the identity rule — it matches Norwegian administrative practice precisely
— Insufficient on idempotency.** Widen `ExternalId` to Place: `#[serde(default)] external_ids` on
`PlaceState`, `AddExternalId` → `ExternalIdAdded` (additive per ADR 0004 §4), and one
`(place, find_place_by_external_id, PLACE_VIEW_TABLE, PlaceView)` line in `registry.rs:70`, which
yields the lookup free through the existing `json_each(payload,'$.state.external_ids')` query
(`sqlite_query.rs:79`). data-model §7/§17 already name Place as the deferred widening target, so this
is the sanctioned path rather than a new decision. The alternatives are disqualified: `human_id` is
allocator-owned (`sqlite_query.rs:23`), and `code` fails by (b).

### 6. Geometry generalization level

**Web:** Kartverket ships the same *kommune* boundary at N50/N250/N500/…/N5000. Two geometries on one
place with the same `date` and different resolution are indistinguishable —
`PlaceGeometryAssertion` carries only `{ geometry, date }` (`place_geometry.rs`), and
`geometry_as_of` (`view.rs:168`) delegates to `resolve_as_of`, whose tie-break on equal `sort_value`
is "later-encountered wins" (`temporal.rs:34`), i.e. import order. Meanwhile the spatial index
inserts *both* into the R\*Tree (`geo_index.rs:96`), so `places_in_bbox` is correct-but-duplicated
(masked by `SELECT DISTINCT`) while the *rendered* boundary is silently non-deterministic.
Re-importing at a finer scale is a coin flip, not an upgrade.

**Verdict: Insufficient.** Fix shared with §7: `#[serde(default)] accuracy:
Option<PositionalAccuracy>` on `PlaceGeometryAssertion` (additive per ADR 0004 §4), plus a
deterministic tie-break in `geometry_as_of` preferring the finest accuracy among same-date
candidates. Keep `resolve_as_of` generic — the tie-break belongs to the geometry wrapper, not the
shared rule.

### 7. Geometry provenance and metric accuracy

Two different things. The **derivation method** already has a home: `EventContext.evidence_analysis`
(data-model §7) maps cleanly — a surveyed Kartverket boundary is Original/Primary/Direct, a boundary
reconstructed from an 1838 *matrikkel* text is Derivative/Secondary/Indirect. Awkward (three axes to
read one fact) but honest and stored per assertion.

The **metric accuracy** (±10 m versus ±2 km) has no home, and `Confidence` is the wrong one — a
category error. `Confidence` is a surety scheme: the operator's surety in a *claim* (data-model §8),
whose ordinals ADR 0027 §1 freezes as a wire shape and §3 resolves through per-workspace
*relabeling*. A workspace could rename ordinal 2 to "Archive-confirmed" and your metres would inherit
that label. Positional accuracy is a property of the measurement, not a degree of belief; mapping
"±500 m" onto VeryLow..VeryHigh discards the metric and conflates two subjects. ADR 0027 §4 also
declines to re-purpose the type.

"No polygon exists, only a point for an area" is half-expressible: `PlaceGeometry::Point` is legal on
a `Municipality` (`geo.rs:71`), but nothing distinguishes "this place *is* a point" from "we only have
a centroid", so `representative_point` (`geo.rs:133`) returns a real centre for one and a guess for
the other with no way to tell.

**Verdict: Sufficient-but-awkward for method, Insufficient for metric accuracy.** One field serves
§6 and §7 both: `PositionalAccuracy { metres: u32, method: GeometryMethod }` with
`GeometryMethod = Surveyed | Digitised | Derived | Approximated`, attached as `#[serde(default)]
accuracy` on `PlaceGeometryAssertion` (additive per ADR 0004 §4), surfaced on `PlaceGeometryRef`
(`place.rs:1108`) with a root `pub use`. Do **not** carry it in `Confidence`.

### 8. `MultiPolygon`

Plainly: **the civil import is not possible without it, except by lying.** All three workarounds
fail. Keeping the largest part drops 166 of Kvitsøy's 167 islands. Asserting N same-dated `Polygon`s
on one place collides with §6 — `geometry_as_of` returns exactly one (`view.rs:168`), so the map
draws one island and hides the mainland while the R\*Tree indexes every part (`geo_index.rs:96`), so
bbox queries silently disagree with the render. Fabricating a child Place per island means 239,057
aggregates and a false hierarchy.

**Web counts:** Norway has **239,057 islands** and 81,192 skerries; **280 of 357 municipalities have
coastline**; Kvitsøy is 167 islands; Andøy is Andøya plus part of Hinnøya plus 189 islands; and there
are only ~4–5 true land exclaves (Himberg, an exclave of Sandefjord inside Larvik, is the most
populous). The multi-part requirement comes overwhelmingly from islands, not exclaves. The exact
count of municipalities whose administrative area is topologically disconnected is **UNVERIFIED** —
coastline count is the verifiable proxy — but it cannot plausibly be under ~200 of 357.

This touches the recorded decision **"`LineString` / `Multi*` geometry variants"**. It is not a
reopening *against* the decision: ADR 0024 §Out of scope reserves `Multi*` for "additive follow-ups
when a concrete need appears", and a majority-of-units need is that trigger.

**Verdict: Insufficient.** Add `MultiPolygon { parts: Vec<PolygonRings> }` to `PlaceGeometry`
(`geo.rs:70`) — the enum is `#[serde(tag = "kind")]`, so a new variant is append-only per ADR 0004
§4 and every historical `Point`/`Polygon` event stays decodable. Extend `rings()` (`geo.rs:88`),
`points()` (`geo.rs:96`), `representative_point()` (`geo.rs:133` — largest part, not an average
across parts) and the `geo_types` conversion (`geo_index.rs:14`). `LineString` is **not** needed
here; leave that half closed.

### 9. Vertex volume — the sharpest constraint

Bytes per coordinate pair in serde JSON with `Microdegrees(i32)`:
`{"latitude":60391262,"longitude":5321987},` ≈ **43 bytes** (Norwegian latitudes 57–71° and longitudes
4.5–31° are both 7–8 digits). Mean municipal coastline is 100,915 km / 280 ≈ 360 km (**web**, skewed
by Nærøysund at ~3,600 km); the table below takes a representative ~200 km coastline plus ~100 km
inland border, with vertex spacing per level as general knowledge:

| Level | Vertex spacing | Vertices | JSON per boundary |
| --- | --- | --- | --- |
| N50 (1:50k) | ~15 m | ~20,000 | **860 KB** |
| N250 | ~75 m | ~4,000 | 172 KB |
| N500 | ~175 m | ~1,700 | 73 KB |
| N5000 | ~1.5 km | ~200 (islands mostly gone) | 8.6 KB |

National totals — 356 *kommuner* + 15 *fylker*, each owning its own ring so shared borders are stored
twice: N50 ≈ **380 MB** (cross-checked from coastline: 100,915 km / 15 m × 43 B = 289 MB plus inland
borders ≈ 390 MB, same order), N250 ≈ 78 MB, N500 ≈ 26 MB, N5000 ≈ 4 MB. Historically, ~1,100
distinct municipal identities 1838–2024 at ~3 dated boundaries each gives N50 ≈ **2.8 GB**,
N250 ≈ 570 MB, N500 ≈ 190 MB.

Three effects, in increasing severity:

- **Event rows.** One `GeometryAsserted` is ~860 KB of `payload TEXT` at N50; replaying a *kommune*
  with three boundaries parses 2.6 MB. The measured 105.8 µs/event
  (`docs/research/performance-profiling.md:47`) was on tiny payloads; at 200–500 MB/s JSON parse this
  becomes 2–4 ms/event, 20–40× worse.
- **The `place_view` blob is the wall.** `place_view(view_id PK, version, payload TEXT)`
  (`schema.rs:67`) holds the whole `PlaceView`, hence *all* live geometries, and cqrs-es's view
  repository loads → applies → re-serializes → rewrites the entire blob per committed event. So every
  *subsequent* command on that place — a name, an enclosure, a tag — re-reads, re-parses and rewrites
  2.6 MB, making the import O(n²) *within* a place (~31 MB of JSON churn per *kommune* at N50). Across
  places it is worse: `find_view_by_human_id` and `next_human_id` run
  `json_extract(payload,'$.state.human_id')` over **every row** (`sqlite_query.rs:23,51`), so SQLite
  parses an 860 KB blob per place to pull a 5-character string, and `next_human_id` fires on every
  create — O(n²) across places. And `list_places` has no `LIMIT`/`OFFSET`, which is what
  `show_geography` calls (`geography.rs:128-131`): ~380 MB of JSON per Geography screen load at N50.
- **R\*Tree reindex.** `reindex_place` (`geo_index.rs:90`) runs on **every** place command: re-reads
  the whole view, deletes that place's rows, re-encodes every live geometry to WKB. The index itself
  is compact (WKB is 16 B/vertex); the cost is the repeated encode plus blob rewrite, and
  `place_geometry` has no index on `place_id` (`geo_index.rs:29`), so each delete full-scans it.

**Verdict: Insufficient at N50/N250; tractable at N500 or coarser.** Import the national sweep at
**N500** (~73 KB/place, ~26 MB current, ~190 MB historical), with N250 as the absolute ceiling. N50
is not storable in this design at all, and should not be forced in — sub-100-m boundary detail
belongs behind a separate non-event-sourced cache, which ADR 0024 §1 rightly forbids putting in the
log. No event change; this is a choice of input data, plus two cheap wins worth taking first: a
generated column and index on `$.state.human_id`, and an index on `place_geometry(place_id)`.

This is also a **genuine collision** with the recorded decision **"Snapshotting is decided, not
deferred-open"**. That decision rests on a measurement taken at 105.8 µs/event on small payloads;
megabyte geometry events invalidate the premise. At N500 it holds; if N50 were ever imported it must
be re-measured.

### 10. Identifier plurality

`code` is `Option<Attributed<Asserted<String>>>` — one value, last-writer-wins, **undated**
(`state.rs:50`, `view.rs:176`, DTO `place.rs:131`). **Web:** Bærum carries three *kommunenummer* with
no boundary change at all (0219 ≤2019, 3024 2020–2023, 3201 2024–), and a place additionally wants a
Kartverket/matrikkel id, an SSR *stedsnummer* and a Wikidata QID. Today, setting 3201 drops 0219 from
state (the log keeps it; no read surfaces it), there is no code lookup anywhere in `genealogy-db`, and
nothing distinguishes a *kommunenummer* from a QID.

Both referenced documents check out, and they point in different directions. data-model §7/§17 name
Place explicitly as a deferred `ExternalId` widening target. The decided item **"External ids have no
frontend entry point"** does not block that — it says external ids are importer bookkeeping, not
user-editable data, which is exactly the role of a Kartverket id or a QID. But it does mean routing
*kommunenummer* through `ExternalId` makes it **invisible in the UI**, and a *kommunenummer* is
precisely what a Norwegian researcher wants to see and search on. So the recommendation splits.

**Verdict: Insufficient.** Two changes: (1) `ExternalId` on Place for machine keys, as §5; (2) make
`code` dated and accumulating like `PlaceName` — `CodeSet` gains `#[serde(default)] date`
(additive), `PlaceState.code` becomes `Vec<Attributed<Asserted<PlaceCode>>>` with a `code_as_of`
mirroring `name_as_of`. (2) is a projection reshape, free per ADR 0010, touching `state.rs:50`, the
`evolve` fold, `view.rs:176`, `place.rs:131` and the UI.

### 11. Postgres parity

Traced, and worse than "only the map view":

- **`places_in_bbox`** — the only caller is a bench (`crates/genealogy-db/benches/store.rs`).
  `show_geography` deliberately does not use it (`geography.rs:6-10`, the filed "viewport-scoped
  loading" deferral). `Unsupported` here blocks nothing today.
- **`place_predecessors` / `place_successors`** — called at `place.rs:784-785` inside
  `show_place_resolved`, the shared body of **both** `show_place` and `show_place_as_of`, with `?`
  propagation. The Postgres branches return `Unsupported` (`store.rs:398`, `store.rs:431`). So on a
  Postgres workspace **every place-detail read fails** — the GUI place screen and `genealogy place
  show` are broken outright, not degraded. `list_places` is unaffected (it never populates those
  fields).
- **Writes are fine.** The index queries are appended only in the SQLite path
  (`sqlite_wire_place_indexes!`, `sqlite.rs:56-66`), so a Postgres import writes events and
  `place_view` normally and never touches the missing tables.

**Verdict: Insufficient — it blocks reading, not importing.** Immediate fix, app-layer only, no event
change: in `show_place_resolved` treat `DbError::Unsupported` as empty `predecessors`/`successors`,
exactly what `list_places` already returns, instead of propagating. Note the filed item sits under
*Performance & scale* and reads as an optimisation, when it is in fact a functional break of
`show_place`; it should be re-filed.

#### If Postgres + PostGIS is a first-class target

Treating Postgres with PostGIS as a supported deployment rather than a feature-gated option changes the
disposition here, though less than the phrase "Postgres unsupported" throughout this document suggests.
Four things are worth separating.

**Two of the three unsupported operations are not spatial at all.** `place_successors` and
`place_predecessors` are a plain relational join over `place_succession` and `place_succession_link`
(`place_succession_index.rs:150`). PostGIS is irrelevant to them; they are missing only because the
index modules are wired exclusively into the SQLite path (`sqlite.rs:56-68`, `:109`, `:115`, `:155`) and
`postgres.rs` contains **no place handling whatsoever**. So **PostGIS does not fix the `show_place`
break — ordinary Postgres parity does**, and that work should not wait on any spatial decision.

**PostGIS genuinely upgrades one thing: containment.** SQLite's R\*Tree is bounding-box-only, so §13's
`places_containing(point, as_of)` has to be two-stage — R\*Tree candidates, then an exact predicate in
Rust over decoded WKB. With PostGIS it is a single indexed `ST_Contains`/`ST_Covers` against a
`geometry(Geometry, 4326)` column on a GiST index. That is a real capability gain, not just a speed one,
and it is precisely the query the farm→parish use case needs. ADR 0024 §3 already anticipated this
("Postgres GiST later"); this makes *later* now, so it is an implementation follow-up within an accepted
decision rather than a new architectural direction.

**It does not change what the event log stores.** ADR 0024 §1 is emphatic that events carry the typed
`PlaceGeometry` over integer `Microdegrees` and that WKB/GeoJSON are boundary encodings only. A PostGIS
`geometry` column lives in the *projection*, rebuildable from the log, exactly parallel to the SQLite
`place_geometry` WKB table. Nothing about requiring PostGIS licenses storing PostGIS types in events, and
it must not be read that way.

**The cost is a second implementation of the same semantics.** Both index modules are hard-bound to
`Pool<Sqlite>` in every signature — struct field and every free function — so parity means mirroring them
for `Pool<Postgres>`, following the existing `sqlite_query.rs` / `postgres_query.rs` split rather than
attempting a generic-over-`sqlx::Database` refactor, which sqlx makes painful. More importantly, once
containment exists on both engines, the two must **agree** at boundary-touching points, shared edges and
ring orientation, where a Rust `geo` predicate and `ST_Contains` can legitimately differ. The
read-model contract should therefore be specified once, with a shared conformance corpus asserting both
backends return identical answers for the same fixtures — not two independently plausible
implementations. Getting that wrong produces a workspace whose parish membership changes when you switch
database engines, which would be far worse than the current honest `Unsupported`.

**Licensing, since the repo has a hard rule.** PostGIS is GPL-2.0-or-later, but it is a server-side
PostgreSQL extension reached over the wire protocol; no PostGIS code is linked into or shipped with our
binaries, so the workspace's `MIT OR Apache-2.0` licensing is unaffected and `cargo deny` never sees it.
The only way this would change is if we distributed PostGIS binaries ourselves, which is not proposed.
Worth stating explicitly so it is not re-litigated.

**What it does *not* change:** §8's `MultiPolygon` requirement is a core-model gap, not a storage one —
PostGIS handles multi-part geometry natively, but the event payload still cannot express it. And §9's
vertex-volume ceiling is driven by the `place_view` blob being rewritten per command and the O(n²)
`human_id` scans, both engine-independent. Postgres does soften the edges — TOAST compresses large
payloads transparently, and a `jsonb` expression index fixes the `human_id` scan more cleanly than
SQLite's generated column — but the ingest bottleneck is unchanged, and PostGIS addresses query
performance, not write throughput. **Do not expect PostGIS to raise the recommended N500 generalization
ceiling.**

### 12. Everything else the aggregate cannot express

- **No attributes on Place.** `PlaceState` (`state.rs:26-68`) has no `attributes` field, unlike
  Source/Citation/Media (data-model §7). Homeless: area, population at a census year, administrative
  centre, and *målform* (bokmål/nynorsk/neutral — a legally recorded per-*kommune* property).
  **Verdict: Insufficient, but plausibly YAGNI** — these are statistics, not evidence about a person.
  If the import carries them: `AddAttribute` → `AttributeAdded` reusing the existing
  `Attribute`/`AttributeType`, additive per ADR 0004 §4.
- **No sibling ordering or *hovedsokn* flag.** A *prestegjeld* is one *hovedsokn* plus *annekssokn*, a
  canonical order; nothing expresses it, and lists sort by `human_id` = allocation order.
  **Verdict: Insufficient** — fold a `primary: bool` into §1's `PlaceRef` change rather than adding a
  separate field.
- **Official versus vernacular name.** `PlaceName` has no name type, unlike `PersonName`. But
  `language` + `date` genuinely covers most of it: **web:** Lyngen's official names (Lyngen / Ivgu
  suohkan, North Sámi / Yykeän kunta, Kven) and Oslo's Sámi forms model perfectly today, as do
  Bokmål/Nynorsk pairs and Kristiania → Oslo. What is lost is official-versus-vernacular *within one
  language*. **Verdict: Sufficient-but-awkward.** `PlaceName.language` already existing is a real
  strength for this import.
- **Coats of arms.** Place has `media` and `AttachMedia` (`state.rs:54`, `command.rs:100`), so they
  store fine — but this collides with the decided item **"Media DTO convention split"**: `place-dto`
  has no `media` field in the WIT, so a coat of arms can never be *exported* by any plugin.
  **Verdict: Sufficient to store, insufficient to round-trip** (an accepted deferral, worth knowing
  before importing 356 crest images).
- **"Reference data, not user data."** Nothing flags a place as imported authority data — but
  `EventContext.operator` is already `AgentKind::Software` for every plugin assertion (ADR 0007 §7),
  the History tab shows it, and a Tag works today with zero model change. `Restriction::Locked` is the
  wrong subject (GEDCOM `RESN` privacy). **Verdict: Sufficient** — record it so it is not re-raised.
- **Bug found in passing, unrelated to Norway:** `show_geography` labels each marker with
  `place.names.first()` (`geography.rs:152`) — the first-asserted name, not the as-of-resolved one —
  while the *geometry* on the same marker **is** date-resolved. At slider year 1875 the pin reads
  "Oslo" while `generated_title` correctly says "Kristiania". One-line fix; should be filed.
- **No transactions and no resumability.** `genealogy-db` has none; each `execute_place`
  auto-commits, so a create → set-type → assert-enclosure → assert-geometry sequence over thousands
  of places can half-fail with no rollback and no progress marker. Consistent with the accepted
  "change-set is not atomic" stance, but at this scale a crashed import leaves orphans to be diffed
  by hand — which is what §5's `ExternalId` idempotency is for.

### 13. Should a *place* be distinct from an *area*?

The question is whether point-Places and area-Places are different kinds of thing — a building or a farm
yard does not move, while a farm's extent grows and shrinks — and whether we should be able to ask which
area a point falls inside, across each hierarchy.

**Do not split the aggregate, and do not add a point/area type.** The same real-world thing is routinely
both: SSR publishes farms as a `representasjonspunkt`, so a farm arrives as a point and may later acquire
a polygon; a *kommune* is an area but is plotted as a pin on the map view. If point-vs-area were a type or
an aggregate boundary, then *acquiring a polygon* would force a type change — which in an event-sourced
model is either a rewrite (forbidden) or a `SuccessionAsserted` (a lie: nothing changed identity, we
merely learned more). `PlaceType` already carries the semantic distinction (`Farm`/`Building` versus
`Municipality`/`Parish`/`County`), and dated `AssertGeometry` already models an extent that changes over
time (ADR 0024 §2). The premise that a point "will not move" is also only true of buildings: a farm's
representative point legitimately moves when its extent is redrawn, and the model handles that as another
dated assertion.

**But two real gaps sit underneath the question.**

*First, extent versus stand-in.* §7 found this half-expressible: `PlaceGeometry::Point` is legal on a
`Municipality` (`geo.rs:71`), so nothing distinguishes "this place **is** a point" from "this is the only
locator we have for an area", and `representative_point` (`geo.rs:133`) returns a true centre for one and
a guess for the other with no way to tell them apart. This is one additive field, orthogonal to §7's
`method`: `#[serde(default)] role: Option<GeometryRole>` on `PlaceGeometryAssertion`, with
`GeometryRole = Extent | Representative` — additive per ADR 0004 §4. It also gives the map view something
honest to render: an extent draws as a boundary, a representative point as a pin.

*Second, there is no containment query at all.* `places_in_bbox` (`geo_index.rs:133`) answers only "which
places' bounding boxes intersect this rectangle", it is SQLite-only, and its Postgres branch is
`Unsupported` (§11). Nothing answers **"which areas contained this point as of date D"**, which is what
this question actually requires. The pieces exist: the R\*Tree yields candidates cheaply, and an exact
point-in-polygon test over the stored WKB needs a predicate from the `geo` crate that ADR 0024 §5 already
sanctions. The date dimension comes free from `geometry_as_of`. Proposed shape:
`places_containing(point, as_of) -> Vec<PlaceId>`, plus its inverse for the area case. On a
PostGIS-backed workspace this collapses to one indexed `ST_Contains` against a `geometry(Geometry, 4326)`
column (§11), which is the strongest argument for treating PostGIS as a first-class target — but the two
backends must then be held to a shared conformance corpus, since a Rust `geo` predicate and `ST_Contains`
can differ on boundary-touching points and shared edges.

**The derivation must be evidence, not a conclusion — and that is a genuine refinement of §1.** This
document endorsed "never derive hierarchy from geometry", because administrative parentage is a legal fact
recorded in sources and containment tests will disagree with it. A containment query does not violate
that, provided the result is treated as **evidence for a claim an operator makes**, never as a
projection-time inference. Concretely: a point-in-polygon hit against SSB's `Kommuner 1838` layer should
produce an ordinary `AssertEnclosedBy` carrying `EventContext.rationale` naming the method and dataset
version, a citation to that dataset as a `Source`, and a deliberately low `confidence` — exactly the
provenance envelope the plugin design section already specifies. What must never happen is the hierarchy
appearing in a projection because two shapes overlap, with no assertion and no operator behind it. That
distinction is the whole evidence/conclusion architecture, and it is what makes the feature safe.

**Area-in-area is not a predicate but a degree, and Norwegian farms prove it.** A farm could straddle a
parish or municipality boundary, which is precisely why the 1838 *matrikkel* records `PRESTEGJELD` and the
1886 one records `Sogn` as **text per farm** rather than leaving it to be derived. So for farms we have
two independent routes — a recorded membership and a derivable one — and the recorded one wins, with the
geometric test serving as a cross-check that can flag disagreements worth a researcher's attention. Where
only geometry exists, the honest output is an overlap fraction, not a boolean.

This surfaces one further limit that survives §1's fix: straddling means a place can have **two parents
within a single relation** at the same date, not merely one parent per relation. `enclosed_by_as_of`
returns exactly one (`view.rs:143`), so even with `PlaceRef.relation` added, a farm in two parishes cannot
be represented faithfully. Whether to model that is a judgment call — it is rare, and the cost is making
every hierarchy read handle a set rather than an option — but it should be a recorded decision rather than
an accident.

**Verdict: Sufficient as an aggregate — the point/area split is correctly *not* a type distinction —
Insufficient on the read side.** Smallest fixes: `GeometryRole` on `PlaceGeometryAssertion` (additive
event change), a `places_containing(point, as_of)` read-model query (no event change), and a recorded
decision on same-relation multi-parenthood.

### Verdict summary

**Insufficient:** §1 parallel hierarchies, §2 cessation, §3 ecclesiastical `PlaceType`, §6 geometry
resolution, §8 `MultiPolygon`, §9 at N50/N250, §10 identifier plurality, §11 Postgres `show_place`,
§12 Place attributes and sibling order, §13 on the read side (no containment query, extent versus
stand-in). **Sufficient-but-awkward:** §4 `SuccessionKind`, §7 accuracy versus method, §12 name kind.
**Sufficient:** §5 the identity rule itself — it matches Norwegian administrative practice exactly —
§12 media storage and reference-data flagging, and §13 as an aggregate question: point-versus-area is
correctly *not* a type distinction.

The cheapest bundle that unblocks the import, every item additive per ADR 0004 §4:
`PlaceGeometry::MultiPolygon`; `PlaceRef.relation`; `PlaceGeometryAssertion.accuracy`; `ExternalId`
on Place (one registry line); dated `AssertEnclosedBy` and `SetCode`; three `PlaceType` variants; plus
two pure read-side changes with no event impact — `existed_as_of` cessation filtering and
`Unsupported`-tolerant succession joins.

## Data sources — the civil hierarchy

### The change history is machine-readable back to 1838 — the geometry is not

This is the distinction the original question elided, and the one every chat got wrong in one
direction or the other. **1838 is real, but it is the floor for the *entity and change register*, not
for boundary polygons.**

Verified directly for this document (`curl`, HTTP 200, no auth):

| KLASS classification | Id | Versions | Range |
| --- | --- | --- | --- |
| Standard for kommuneinndeling | 131 | 141 | **1838-01-01** … 2026-01-01 |
| Standard for fylkesinndeling | 104 | 12 | **1842-01-01** … 2026-01-01 |

The earliest 131 entry is literally `Kommuneinndeling 1838`. So **1838, not 1837** — the *law* is
1837, the classification's first in-force division is 1838-01-01, which settles the terminological
split among the chats and fixes the `valid_from` literal an importer should write. The `changes`
endpoint returns real old→new code mappings with dates: `changes?from=1838-01-01&to=1850-01-01`
returns HTTP 200 / 8,899 bytes, beginning
`{"oldCode":"0103","oldName":"Fredrikstad","newCode":"0102","newName":"Sarpsborg","changeOccurred":"1839-01-01"}`.
`correspondsAt?targetClassificationId=131&date=…` on 104 maps *fylke* to *kommune* codes on a given
date. Licence for the API and its data is **CC BY 4.0** (*"I likhet med resten av ssb.no benytter
API-ene lisensen CC BY 4.0"*).

**Two operational limits an importer must handle.** The full 188-year range *does* work in one call —
`changes.json?from=1838-01-01&to=2026-01-01` returns HTTP 200, **302,162 bytes, 2,093 change
records** from 1839-01-01 to 2024-01-01 — but it takes **47 seconds**, and an earlier attempt at the
same query returned **HTTP 502**. So the range is not capped, it is merely slow and intermittently
flaky. Chunk by decade anyway: not to stay under the 8 MiB response cap (302 KB is nowhere near it)
but so the `progress` capability can report and honour cancellation instead of blocking on one
47-second call that may fail. And there is **no change-code vocabulary**: the live OpenAPI schema's
`CodeChangeItem` is
`{oldCode, oldName, oldShortName, newCode, newName, newShortName, changeOccurred}` with no
`changeType`/`endringskode` field anywhere. The chats' assumption of an SSB change-code enum in the API
is **REFUTED at schema level**, so `SuccessionKind` cannot be populated straight from KLASS: change
*kind* must be inferred from the cardinality of the code mapping or read from SSB's free-text narrative.
The model's §4 vocabulary is adequate; the *source* does not carry the distinction.

One refinement, though: a change-type vocabulary **does** exist outside the API. SSB's own methodology
notat documents a `TypeOfChange` variable with the values *Deling*, *Sammenslåing*, *Kodeendring*,
*Navneendring* and *"Deling, sammenslåing"*, plus finer source-level labels including *Fraskilt*,
*Overføring*, *Endring av kommunenummer* and *Kommune byttet fylke* — while stating outright that it
"brukes ikke i KLASS". These are Norwegian labels, not numeric codes. So there is a documented target
vocabulary to map onto even though no endpoint emits it, which makes the §4 mapping a defensible
translation rather than a guess.

**A format trap on the KLASS side.** Every `codes`/`codesAt`/`corresponds`/`changes` link template
accepts a `csvSeparator` parameter, and CSV is genuinely more compact — `codesAt` for all 2020
municipalities is 24,301 bytes as CSV against 63,770 as JSON, 2.6× smaller. But the CSV is served
`charset=ISO-8859-1` and is **not valid UTF-8**: the first `ø` (byte `0xf8`, in "overført") makes a naive
`String::from_utf8` fail outright. Prefer JSON, which is UTF-8; if CSV is chosen for size, decode
Latin-1 explicitly — `encoding_rs` covers ISO-8859-1, unlike the CP865 case in the SOSI section.

The canonical pre-1999 register, **"Historisk oversikt over endringer i kommune- og
fylkesinndelingen"** (Dag Juvkam, SSB *Rapporter* 1999/13, 90 pp., ISBN 82-537-4684-9), covers
"kommuneinndelingen 1838–1998 og … fylkesinndelingen 1660–1998" and is **PDF-only**. SSB itself
designates KLASS as the machine-readable form ("Alle regionale inndelinger er dokumentert i
klassifikasjonssystemet KLASS"), and KLASS 131 does cover the whole 1838–2026 span. The 1660–1841
*fylke*/*amt* pre-history exists **only** in that PDF — KLASS 104 starts at 1842.

### Geometry — `kart.ssb.no` is the answer, and it reaches 1838

The most consequential find in this research, and one no chat came near. **SSB runs a real, documented,
unauthenticated OGC API – Features service at `https://kart.ssb.no/api/core/v1/ogc/features/`** (409
feature collections; OpenAPI spec at `/api/core/swagger/v1/swagger.json`). Geonorge does not host these
files — its record for the boundary dataset is a pure redirect, and Geonorge's own download broker
returns 404 for it — so anyone looking only at Kartkatalogen (as every chat did) misses this entirely.

Verified directly for this document:

- **129 per-year collections titled `Kommuner YYYY`, spanning 1838 to 1996.** The years are exactly the
  years a boundary changed, matching KLASS 131's version list (1838, 1839, 1840, 1841, 1842, 1843,
  1845, …).
- **`Kommuner 1838`** (`068d24a0-01bd-76a5-8000-7f51f6a3530a`): `count` → **395 features**, matching
  KLASS's 395 units at 1838-01-01 exactly. Fields are `komm_nr`, `komm_navn`, `ogc_fid`, `geom`. A
  sampled feature is `komm_nr 1553`, `komm_navn Eide`, with a **`MULTIPOLYGON` of 656 vertices**.
- **The whole 1838 year is 4,613,865 bytes fetched in 0.85 s** — comfortably inside the plugin `net`
  capability's 8 MiB cap, so one request per year works for the early series. Asking for EPSG:4326 makes
  it **smaller, not larger** — 2,167,190 bytes — since degrees carry fewer digits than metre
  coordinates. So the reprojected form is the cheaper fetch as well as the more useful one.
- **Provenance matters for how these are recorded.** The pre-1997 geometry is **back-dated**, not
  surveyed: SSB rebuilt it by stepping backwards from the ABAS *Grunnkretsfil* of January 2019, and only
  **land** boundaries were back-dated — decisions adjusting maritime boundaries were not applied. The
  series comes out of the Eurostat-funded HistGeoStat work that extended KLASS 131 back to 1838. Treat
  every pre-1997 boundary as *reconstructed jurisdictional extent* rather than a measured border, which
  is exactly what §7's `PositionalAccuracy { method: Derived }` is for. It also explains why no chat
  found it: SSB's own methodology notat of 2025-02-11 still described only the 1986–2019 maps as
  existing, so this series was published after February 2025.
- **A second series, 22 collections, "Historiske kommunegrenser YYYY med forbedret nøyaktighet",
  covering 1986–2019** (some spans merged where nothing changed: `1992til1993`, `2013til2016`,
  `2008til2011`). Richer schema (`NAVN`, `KOMMUNENR`, `Stat_aar`, `Shape_Area`, `Shape_Leng`) and much
  heavier geometry: a full year is **~16.6 MiB, over the cap**, so this series must be paged.
  `limit`/`offset` paging is real and honoured, but per-feature size varies roughly 30× (13 KB to
  448 KB — fjord-heavy coastal municipalities are enormous), so a fixed feature limit is not a byte
  guarantee; page conservatively or retry on size.
- **`storageCrs` is EPSG:25833, but the service advertises CRS84, 4326, 4258, 3857, 25832/33/35 and
  the UTM32/33/35 codes — and reprojection works.** Requesting
  `items.json?crs=http://www.opengis.net/def/crs/EPSG/0/4326` returns `7.7309 62.97779` for that same
  Eide feature — correct lon/lat. **This removes client-side reprojection from the civil import
  entirely.**
- Current boundaries are served separately as `kommuner_2024` and `fylker_2024`, both described as
  simplified to ~100 m tolerance and automatically updated.
- **No historical *fylke* series exists.** Searching all 409 collection titles finds only the two
  current county collections. So county geometry before 2024 is unavailable here, and county history
  must be imported as entities from KLASS 104.
- Licence is **NLOD 1.0** per the Geonorge-side declaration; the API exposes no machine-readable
  licence field of its own (`/v1/legal/terms` requires auth, unlike the geodata reads).

The gap between the two series is 1997–1985 — that is, the plain series stops at 1996 and the improved
series starts at 1986, so they overlap rather than leaving a hole, and together they cover **1838–2019
continuously**. 2020–2026 comes from `kommuner_2024` plus KLASS changes.

### Geometry — other sources, now largely redundant

- **SSB, "Historiske kommunegrenser med forbedret grensenøyaktighet 1986-2019"** — verified
  independently on data.norge.no (dataset `268bc92d-…`): publisher Statistisk sentralbyrå, **6
  distributions** (Parquet, **gpkg**, **geojson**, GML, FileGeodatabase, Shapefile) plus one API,
  licence **NLOD 1.0**. Annual (1 January) municipal boundary polygons, built on "prinsippet om
  nyeste innmålt grense" from SSB's own change register plus Kartverket's historical series. **This
  refutes six of the seven chats**, which asserted SSB is codes-only; the one chat that claimed it
  was right. GeoJSON among the distributions makes it directly usable in-component.
- **Kartverket, "Administrative enheter – historiske versjoner"** — **it exists**, settling the
  head-on chat conflict (one chat asserted Kartverket publishes only current boundaries). Verified in
  Kartkatalogen, UUID `9bc064e3-6c34-4c3a-8421-00290052e9c0`, plus a WMS twin (`efc76ccd-…`). The
  DCAT-AP JSON-LD behind its data.norge.no record lists **five distributions — SOSI, FGDB, GML,
  GeoJSON and SQL/PostGIS — every one licensed CC BY 4.0**, so GeoJSON *is* available here (unlike
  Kartverket's N-series). Note the licence differs from SSB's NLOD 1.0.
  **Its coverage floor is 1997** — resolved, after several dead ends. The answer is in the
  `SerieDatasets` array of `kartkatalog.geonorge.no/api/**getdata**/{uuid}` (not the `metadata`
  endpoint, which omits it, and not the download-broker paths, which all 404 for this UUID): **35
  members titled "Administrative enheter – historiske data YYYY", spanning 1997–2025 with 1999
  absent.** The earliest, 1997, is **SOSI-only and EPSG:25833-only** at 1:50 000; from 2019 the series
  splits into per-*kommune* and per-*fylke* datasets carrying the full format set. The WMS twin is
  shallower still — its `GetCapabilities` (at `wms.geonorge.no/skwms1/wms.adm_enheter_historisk`; the
  service name is `adm_enheter_historisk`, which is why a plausible guess at
  `administrative_enheter_historisk` returns HTTP 500) exposes only `kommuner_2017`…`2025` and
  `fylker_2017`…`2025`, 2022 absent. Ignore the record's `dcat:startDate` of 2020-07-10: it is a copy of
  the metadata-modified date, not a coverage start.
  Practically, this series matters far less than it seemed: SSB covers 1838–2019 in GeoJSON, so
  Kartverket's value is authoritative *current* geometry and the 2020–2025 years.
- **Kartverket generalized series** N50–N5000 exist but are distributed as **FGDB/GML/PostGIS/SOSI
  only — no GeoJSON**, which given the format table below means none of them is directly readable
  in-component.
- **`github.com/robhop/fylker-og-kommuner`** — current *fylke*/*kommune* GeoJSON at three
  generalization tiers (Norge-S/M/L), coastline-clipped MultiPolygon, "Basert på Kartverkets data
  under CC BY 4.0", all tiers under 8 MiB. This is the pragmatic path for **current** boundaries: it
  is the only artifact found that is simultaneously GeoJSON, sub-8-MiB, coastline-clipped and
  permissively licensed. Third-party, so pin a commit and record provenance.
- **Raster fallback** — Kartverket *historiske kart* (amtskart, rektangelmålinger, gradteigskart)
  exist as georeferenced scans. These are digitization input, not data; out of scope for a plugin.

### Sikt / NSD Kommunedatabasen

**Exists, but is not what the chats claimed.** `kommunedatabasen.sikt.no` holds ~543,000 *statistical*
variables per municipality — demography, economy, elections, health — with coverage varying by subject
group (Befolkning **1769–2024**, Valg 1829–2017, Arbeidsliv 1835–2017), not a single 1838→present
range. It provides a population-proportional **conversion** mechanism to re-base historical values
onto a target year's boundaries, which is a statistical reallocation, **not geometry**. No boundary
polygons were found on its `/about`, `/api`, `/calculation` or `/usage` pages, and no licence statement
either. Sikt's service page does claim *"I tillegg inneheld tenesta historiske kommunekart"* — but the
sentence carries **no hyperlink**, and no `.geojson`/`.shp`/`.gpkg` reference appears anywhere on that
page or on `kommunedatabasen.sikt.no`. It reads as an in-app map view, not an exportable geometry
product. So the chat claim that Sikt publishes 1838-onward GIS reconstructions is **not supported** —
"no evidence found" rather than a hard refutation, since the app's JS bundles were not inspected. The
claim is also now moot: SSB serves exactly that geometry openly, so nothing depends on resolving it.
Its own API page states the *formannskapslover* were "i 1937", which is a typo for 1837 on Sikt's page
and should not be repeated.

### Kartverket's current datasets, and the real field names

The current authoritative boundaries are **two** datasets, not one: "Administrative enheter kommuner"
(`041f1e6e-bdbc-4091-b48f-8a5990f3cc5b`) and "Administrative enheter fylker"
(`6093c8a8-fa80-11e6-bc64-92361f002671`), at 1:5000, updated annually, in FGDB/GeoJSON/GML/PostGIS/SOSI.
Licence is **CC BY 4.0** (`"Åpne data"`, `"Ingen begrensninger på bruk er oppgitt. Se forøvrig lisens."`)
— **not** NLOD; NLOD is what SSB's 1986–2019 series carries. CRS options are 25832/25833/25835 for every
format with 3035 and 4258 for GeoJSON and GML only; **EPSG:4326 is not offered by the download service**,
though the WFS will reproject to it.

**All four chat guesses at the attribute field names are wrong.** The delivered GeoJSON carries
`objtype`, `kommunenummer`, `kommunenavn`, a structured `administrativenhetnavn[{navn, rekkefolge,
sprak}]` array, `identifikasjon.Identifikasjon.{lokalId, navnerom, versjonId}`, `gyldigFra`, `gyldigTil`,
`oppdateringsdato`, `opphav`, and on the boundary layer `avgrensningstype` plus
`kvalitet.Posisjonskvalitet.{målemetode, nøyaktighet}`. So `kommunenummer`/`kommunenavn` are right,
`fylkesnummer`/`fylkesnavn` exist only on the *fylker* layer, and `fylkesnr`/`kommunenr`/bare `navn` are
refuted for Kartverket (`komm_nr`/`komm_navn` is SSB's historical schema — a different source). Two
practical notes: every Geonorge download is a **ZIP**, and the inner `.geojson` carries a **UTF-8 BOM**,
which will break a naive `serde_json::from_slice` on the first byte. The observed
`kvalitet.Posisjonskvalitet.nøyaktighet` of 1500 (1.5 m) is also a useful reality check — the source data
is far coarser than the 0.11 m microdegree quantum, so the datum shortcut in the CRS section is
comfortably below the noise floor.

### Identifier stability

*Kommunenummer* is **not** a durable identifier, and this is documented by SSB itself: codes are
reused across eras (0516 Heidal 1908–1964 / Nord-Fron 1977–2019; **1806 Svolvær / Narvik 2020–**;
0704 Åsgårdstrand / Tønsberg 1988–), and units are renumbered without any boundary change (Bærum 0219
→ 3024 → 3201). Combined with §5, the practical rule for the importer is that the key must be
`(kommunenummer, valid_from)`, never the number alone — keying on the number alone would merge
Svolvær into Narvik.

The no-reuse *policy* is explicit — the 2017 ministry decision states reuse "er ikke ønskelig", with
named exceptions for Oslo, Bergen, Trondheim and Stavanger — but it is aspirational rather than
historically true, since the post-1970 renumbering caused real reuse in 1977 and 1988, and Kartverket's
own codelist deliberately mixes live and retired numbers (`0101 Halden (Utgått 2020-01-01)`). Note the
structural reason reforms cascade: the first two digits of a *kommunenummer* **are** the *fylkesnummer*,
so merging counties forces renumbering of every municipality inside them — which is why 2020 renumbered
wholesale and 2024 renumbered again.

Three genuinely stable identifiers do exist, and they are what `ExternalId` should carry:

- **Kartverket `lokalId`** — a UUID on every feature, scoped by `navnerom`, with a companion `versjonId`,
  guaranteed unique within the namespace. Whether a merged municipality inherits or is assigned a fresh
  one is **UNVERIFIED**.
- **SSR `stedsnummer`** — the strongest guarantee found: *"Stedsnummeret er unikt og kan ikke brukes om
  igjen."* Applies to places generally rather than to administrative units.
- **Wikidata** — unexpectedly good historical coverage. A SPARQL query returned **883 Norwegian
  municipality items, of which 523 carry a dissolution date (P576) and 879 carry a municipality number
  (P2504)** — roughly 2.5× the 357 current units, so most abolished municipalities are already modelled
  and numbered there. Useful as a cross-check on the succession graph and as a link target.

## Data sources — the ecclesiastical hierarchy

This is the part the seven chats did not research: *bispedømme* and *prosti* have **zero** mentions
across all of them, *sokn* has three, and the only named candidate for parish geometry was a
single-sourced, URL-less "Digitalarkivet / Historiske Administrative Grenser (HAG)". The findings
below invert that picture — the *entity* hierarchy is far better served than any chat suggested, and
the *geometry* far worse.

### The entity hierarchy is fully machine-readable in SSB KLASS

Verified directly for this document against `https://data.ssb.no/api/klass/v1/classifications/<id>`
(HTTP 200, no auth, JSON):

| Classification | Id | Versions | Range of `validFrom` |
| --- | --- | --- | --- |
| Standard for bispedømme | 510 | 22 | **1070-01-01** … 2005-01-01 |
| Standard for prosti | 651 | 58 | **1865-01-01** … 2025-01-01 |
| Standard for prestegjeld | 655 | 8 | 1997-01-01 … 2005-01-01 |
| Standard for sokn | 644 | 7 | 2020-01-01 … 2026-01-01 |

So diocese history is coded back to **1070** and deanery history to **1865** — a far better temporal
reach than anything the chats imagined, and better than the *civil* fylke classification (104, which
starts 1842). `codesAt?date=2020-01-01` on 644 returns **1165 sokn** in 185 KB, with the 8-digit
*soknenummer* scheme visible in the codes (`01010104` = 2 digits diocese + 2 prosti + 2 prestegjeld +
2 sokn), e.g. `01010105 Oslo Domkirkes sokn`.

Two caveats that matter for the import design. **`prestegjeld` (655) covers only 1997–2005** — the
tail before the level was abolished (out of church administration 2004, out of law 2012), *not* the
1838–1900 period genealogical sources actually use. And **`sokn` (644) starts only in 2020**, so the
current parish list is coded but its history is not. The same `changes` endpoint that works for
municipalities is available on these classifications, and the same operational limit applies (see
below): chunk the range.

### Geometry: current-only, official; one unofficial historical layer

- **Soknegrenser** (Kartverket, Kartkatalogen UUID `289d459c-0390-4000-84f3-88982f2cdb0c`) —
  **current** *sokn* and *bispedømme* boundary geometry, "Åpne data" under Kartverket's blanket
  CC BY 4.0 terms (attribution "© Kartverket"; dataset owner Barne- og familiedepartementet),
  refreshed annually. The closest real thing to the chats' phantom "HAG", and no chat mentioned it.
  The file was downloaded and parsed for this research, which settles several things the metadata does
  not say:
  - **SOSI 4.5 only, EPSG:25833 only** — the download API's format codelist returns exactly one entry,
    and `Distributions` is empty, so there is **no WMS, WFS, GeoJSON or GeoPackage**. One national file
    (`Basisdata_0000_Norge_25833_Soknegrenser_SOSI.zip`), **5,524,444 B zipped → 23,373,459 B**. The
    zip fits the plugin's 8 MiB `net` cap; the inflated file does not matter, since parsing is
    streaming.
  - Coordinates are `ENHET 0.01` (centimetres) with **northing-first `NØ` axis order** — two easy
    ways to get a boundary silently mirrored or scaled by 100.
  - Object counts: **1,154 `Sokn` FLATE (1,146 unique `SOKNENUMMER`, so 8 are multi-part)**, **11
    `Bispedømme`**, 15,247 `Soknegrense` curves, 1,506 `Bispedømmegrense`, plus `Riksgrense` and
    `Territorialgrense`. The 1,146 figure matches kirken.no's own count of geographic *sokn* exactly
    (they also list 10 non-geographic congregations — deaf parishes, Svalbard, Sørsamisk).
  - **Zero `Prosti` and zero `Prostigrense` objects**, despite the SOSI object model defining both —
    consistent with the abstract's "Prostitilhørighet kan leses ut av soknekode". So *prosti* polygons
    do not exist as data but are **losslessly reconstructable** by dissolving the 1,154 *sokn* on
    digits 3–4 of `SOKNENUMMER` (91 distinct prefixes in the January 2025 vintage).
  - Every *sokn* carries an `ORGANISASJONSNUMMER` and exactly **one** `KOMMUNENUMMER` — so *sokn* do
    nest inside *kommuner* today, and the registry join below works.
  - Geometry is **topological**: each `FLATE` references signed curve ids in `..REF` (negative =
    reversed), and only **4** *sokn* use inner-ring syntax. That makes a minimal SOSI reader a
    **bounded job** rather than a blocker — it is a line-oriented text format, and this import needs
    only `.FLATE`/`.KURVE`/`..REF`/`..NØ` plus five attribute paths.
  - **No historical vintages exist.** Geonorge's Atom feed for this dataset carries exactly one entry,
    overwritten annually (`api/search?text=soknegrenser` → 1 result; `text=geistlig` → 0).
- **"Kirkebygg – forenklet"** (UUID `eea87664-d936-478e-897a-c38ca7c478a0`, owner Barne- og
  familiedepartementet) — 2,289 church *buildings* as points, and unlike Soknegrenser it ships
  **GeoJSON, GeoPackage, GML and FGDB** in EPSG:4258 among others, "No conditions apply to access and
  use". Not parish polygons, but a trivially parseable per-*sokn* anchor point, and the successor to
  Kirkebyggdatabasen (decommissioned December 2024).
- **Brønnøysundregistrene gives *sokn* a stable, non-reused identifier.** Verified directly:
  `api/enheter?organisasjonsform=KIRK` returns **1,418** entities (*sokn* plus *kirkelige fellesråd*),
  and `api/enheter/976993829` returns `SULDAL SOKN`, `organisasjonsform.kode: "KIRK"`, registered
  1997-01-27 — an *organisasjonsnummer* byte-identical to the one inside the Soknegrenser polygon. This
  is the single best idempotency key found in this research for any level of either hierarchy, and it
  is **NLOD** with bulk JSON/CSV. Caveat: *prostier* and *bispedømmeråd* lost their
  *organisasjonsnummer* when the church left state administration, so this covers only the *sokn* tier.
- **Geonorge SOSI object catalogue "Geistlig inndeling"** formally defines
  Bispedømme/Prosti/Prestegjeld/Sogn/Prestegjeldgrense object types — but the catalogue entry is
  stamped *Erstattet* (superseded, v4.0) and no live historical product corresponds to it. For an
  official *historical* ecclesiastical boundary dataset this is a **hard "does not exist"**, not a
  search miss.
- **`github.com/atlefren/norske_prestegjeld`** — the only published historical parish polygons found
  anywhere. Verified via the GitHub contents API: `prestegjeld.geojson` (**25,370,403 bytes**),
  `prestegjeld-kystlinje.geojson` (23,868,927 bytes, coastline-clipped), plus nine georeferenced
  `.tiff` sheets. Title "Norske Prestegjeld 1801"; licence quoted from the README: *"Norske
  Prestegjeld by Atle F. Sveen, Jørgen H. Marthinsen is licensed under a Creative Commons
  Attribution 4.0 International License"* — so **CC BY 4.0, redistributable with attribution**. Its
  provenance is fragile: it derives from a Digitalarkivet tool at `digitalarkivet.no/norkart/` which
  now **404s**, and Norwegian Wikipedia cites it only through a 2010 Wayback snapshot. CRS not stated
  in the README. Note both files exceed the plugin `net` capability's 8 MiB response cap threefold.

### Registries and textual histories

- **Digitalarkivet has no public catalogue API.** No OAI-PMH, no JSON endpoint, no developer docs;
  the only pages named "API" are B2B archive-deposit services for institutions. Sweden's Riksarkivet
  has real OAI-PMH; Norway has no equivalent. What does exist is a **stable, decodable URL-parameter
  scheme** on the human search UI — `clerical_parishes=0818P` for a *prestegjeld* and
  `parishes=0818S1` for a *sokn* — whose code lineage matches Kartverket's official *soknenummer*
  scheme. That is scrapeable HTML, not a published contract, and should not be treated as an API.
  **And scraping it is ruled out on Digitalarkivet's own terms:** its `robots.txt` lists 137 named
  user-agents — including `anthropic-ai`, `CCBot` and `ChatGPT Agent` — each with `Disallow: /`, before
  a generic `User-Agent: *` with `Crawl-delay: 5`. Whatever a generic client is technically permitted
  to do, the site has explicitly excluded AI agents, so building a plugin that harvests it would run
  against the publisher's stated wishes. An API demonstrably exists (histreg.no consumes it with
  immediate propagation) but is partner-only. **The correct move is to ask Arkivverket for access**,
  which is also the cheapest possible route to a complete ecclesiastical entity tree.
- **Nasjonalarkivet's "Historikk for prestegjeld og sogn"** — 19 per-*fylke* pages listing every
  *prestegjeld* as a heading. The taxonomy (fylke → prestegjeld names) is extractable from static
  HTML; the actual boundary-change prose is accordion-rendered client-side and needs a headless
  browser. No identifiers on the page at all, only names. Editorial, not data.
- **`prestehistorie.no`** — "Norske prestegjeld 1530–1900", the fullest inventory found: per-*prosti*
  tables under each *stift*, with founding/split/rename lineage embedded as annotated text inside
  cells, covering Båhuslen, Jemtland/Härjedalen and Idre/Särna for the periods they were Norwegian.
  Actively maintained (last updated 2026-07-16). **Licence: none stated — "Utarbeidet av R. Rostrup©
  2022".** Copyright-only, so not redistributable; usable as a research reference, not as import
  input.
- **`lokalhistoriewiki.no`** — a live standard MediaWiki API at `/api.php` (MediaWiki 1.39.4);
  `Kategori:Prestegjeld` holds 181 articles, and `/Prestegjeld` is a genuinely structured sortable
  wikitable (Prestegjeld | Skrivemåte 1814 | Sokn | Dagens kommune(r)) for the 1814-election-era
  list, which is stated to be near-identical to the 1837 *formannskapslov* list.
- **RHD / UiT (HistLab) publishes the farm→parish crosswalk, and it is the key historical find.** The
  **1838 and 1886 *matrikkel*** are downloadable as **34 per-county archives** (`Matr<NN>-1838.zip`,
  `maf<NN>.zip`, counties 01–19). Two were downloaded and parsed for this research: the 1838 file
  carries a **populated `PRESTEGJELD` column** (plus a sparse `SOGN_NR`) alongside
  `FYLKE, KNR, NMNR, LNR, GMNR, GNAVN, BNAVN`, owner names, valuation, a stable per-record `EID` and a
  `BILDENAVN` linking to the scan — 6,278 rows for Rogaland alone; the 1886 file carries **`Sogn`**.
  So parish *membership* per farm is published data, not something to be guessed at with a Voronoi.
  Volumes are small — 1838 ≈ 10.1 MiB and 1886 ≈ 12.0 MiB in total, largest single file 1,285,702 B,
  so **every file is under the 8 MiB `net` cap** — and the formats (`.xlsx` OOXML, legacy `.xls`
  BIFF8) are readable by the pure-Rust `calamine` crate, needing no GDAL.
  **The licence is genuinely unclear and must be resolved before any redistribution.** Neither
  *matrikkel* page states a licence at all; a sibling RHD dataset page carries terms reading
  *"Kopiering … bare tillatt til eget bruk. Ervervsmessig bruk eller publisering … bare tillatt etter
  skriftlig samtykke fra HistLab, Universitetet i Tromsø"* under a confusing "(Public domain)" label.
  Treat as **not redistributable pending written clarification from HistLab**. Coverage caveats:
  Finnmark is absent (unmatriculated), towns are excluded from both, *husmannsplasser* are excluded as
  not independent economic units, and the material is partly OCR-derived and corrected but not
  guaranteed.
- **Modern farm→parish linkage is available and openly licensed** via a different route:
  **"Matrikkelen – Adresse"** carries `sokn` (as `SokneId { soknenummer, organisasjonsnummer,
  soknenavn }`) alongside `matrikkelnummer`, distributed as **CSV** among other formats — CSV being
  the one format needing no GDAL. This gives a present-day gnr/bnr → *sokn* crosswalk.
- **SSR (Sentralt stedsnavnregister)** REST API verified at `https://api.kartverket.no/stedsnavn/v1`
  (the old `ws.geonorge.no/stedsnavn/v1` now proxies it), no auth, plain JSON, with `stedsnummer` as
  a stable integer id, ~260 `navneobjekttyper` including farm-relevant types (`gard`, `navnegard`,
  `bruk`, `historiskBosetting`), and explicit historical-name status codes (`historisk`, `sidenavn`).
  Licence **CC BY 4.0**, "Åpne data". Paging caps at 5000 hits per query. Two limits: the bulk
  download is **GML-only** (confirmed against the download API's format codelist, which returns
  exactly one entry, GML/EPSG:4258 — no GeoJSON), and **SSR does not record *sokn* on ordinary place
  records** — only `fylker` and `kommuner`; *sokn* exists in SSR merely as its own place type. So SSR
  cannot supply farm→parish membership; the *matrikkel* must.
- ***Gårdsnummer* continuity — the chats' claim is refuted for 1838.** **There is no *gårdsnummer* in
  1838 at all**: that *matrikkel* used `matrikkelnummer` + `løpenummer`, confirmed directly by the
  parsed file's columns (`NMNR`, `LNR`, `GMNR`, and no Gnr/Bnr). Gnr/bnr were introduced by the
  1863–83 revision published as the **1886** *matrikkel*, which is precisely what makes that file
  valuable — it prints **both** systems, so it is itself the bridge back to 1838. Even from 1886 the
  values are not stable: gnr is unique only within a *kommune* and is renumbered at every merger (the
  2020 Stavanger merger shifted Finnøy by +100 and Rennesøy by +200). So the chats' "largely
  continuous back to the 1886 and traceable to the 1838 matrikkel" is half right — traceable **via the
  1886 file's own columns**, not by numbering continuity.

### Refuted or absent

- **"Historiske Administrative Grenser" / "HAG" — REFUTED.** No dataset, product or acronym by that
  name exists at Arkivverket, Nasjonalarkivet, Digitalarkivet, Kartverket, Geonorge or
  data.norge.no. Arkivverket publishes no boundary geometry at all. The single chat that named it
  (`gemini.md:196`) invented it, most likely by conflating Kartverket's "Administrative enheter –
  historiske versjoner" with Digitalarkivet.
- **IPUMS / NAPP parish geography — REFUTED by IPUMS's own documentation.** NAPP does carry Norwegian
  census microdata for 1801/1865/1875/1900/1910, but the variable page for `GEO1_NO1801_1910` states
  it "is harmonized by name and does not take into consideration changing province boundaries. It is
  **not mappable**." Non-mappable at *fylke* level, so a fortiori no parish shapefile.
- **histreg.no bulk geography — REFUTED.** A person index; geography appears only as a search filter
  drawn from Digitalarkivet's taxonomy. No bulk download, no geodata API, no farm→parish table.
- **RHD's claimed digital 1801 parish-boundary maps — UNVERIFIED.** Thorvaldsen & Holden (2023,
  CC BY 4.0) state "The UiT extended the municipality map backwards to 1801 with the parish boundary
  maps from the transcribed 1801 census", but no public RHD page, download or API exposing them could
  be located. Claimed to exist; not publicly available. A direct enquiry to HistLab is the only way
  to settle it.

### Unit counts, churn, and whether the hierarchies nest

Counts from `data.ssb.no/api/klass/v1/…/codesAt`, so they are the import volume:

| Date | Bispedømme | Prosti | Prestegjeld | Sokn | Kommuner |
| --- | --- | --- | --- | --- | --- |
| 2026-01-01 | 11 | 92 | 614 (frozen) | 1,147 | 358 |
| 2004-01-01 | 11 | 103 | 617 | n/a | — |
| 1900-01-01 | 6 | 83 | 0 | 0 | 594 |
| 1865-01-01 | 6 | 77 | 0 | 0 | 481 |
| 1838-01-01 | — | — | — | — | **395** |

The zeroes are the honest shape of the data, not missing rows: *sokn* is coded only from 2020 and
*prestegjeld* only from 1997, so KLASS returns nothing for them in 1865 or 1900 even though the units
existed. Churn is concentrated in *prosti* (58 versions since 1865; peaked at 106 units in 2008–2012,
92 today) and in *kommuner* (141 versions, 2,093 change records, of which 467 fall in 1838–1900).
Total import volume across both hierarchies and all vintages is on the order of **1,200 current
polygons + ~1,900 tabular ecclesiastical units + ~2,100 municipal change events**, plus ~150,000
historical farm records if the *matrikkel* is ever ingested — all tractable.

**Do the hierarchies nest? Currently yes, historically only approximately.** Every one of the 1,154
current *sokn* polygons carries exactly one `KOMMUNENUMMER`, and *prosti*/*bispedømme* nest by
construction of the *soknenummer*. For 1838 the chats' claim that *formannskapsdistrikter* were drawn
on the *prestegjeld* is **verified but must be quantified**: the 1837 law took the *prestegjeld*
division as its basis (§1), but an eligible *sokn* could vote itself a separate district, so 1838 had
**320 *prestegjeld* against 355 *formannskapsdistrikter*** — roughly **90% identity, not 100%**. The
equivalence then degraded gradually from the 1840s (splits usually divided both the *prestegjeld* and
the *kommune*, but not always), and broke decisively with the purely civil **1963–65 mergers** — which
is the stated reason *prestegjeld* were abolished, the boundaries having drifted too far apart.
Practical guidance for the importer: an 1838 *kommune* reconstruction is a usable *prestegjeld* proxy
to roughly 1870, degrading through 1930, and **worthless after 1963**. Any such derived geometry must
be recorded with low positional accuracy and a rationale saying it is a proxy.

### Verdict per ecclesiastical level

| Level | Entity list / history | Geometry | Best artifact |
| --- | --- | --- | --- |
| Bispedømme | **Coded 1070–2005** (KLASS 510, 22 versions) | **Published vectors** — 11 polygons | KLASS 510 + Soknegrenser |
| Prosti | **Coded 1865–2025** (KLASS 651, 58 versions) | **Reconstructable** — dissolve *sokn* on code digits 3–4 (91 prefixes); no published layer | KLASS 651 + Soknegrenser dissolve |
| Prestegjeld | Coded **1997–2005 only** (KLASS 655, 614 frozen units); rich 1530–1900 history at `prestehistorie.no` (no licence); farm-level membership in the 1838 *matrikkel* (`PRESTEGJELD` column, licence unclear) | **One unofficial 1801 layer**, CC BY 4.0, dead upstream; or an 1838 *kommune* proxy at ~90% identity | KLASS 655 + atlefren 1801 + RHD 1838 |
| Sokn | Coded **2020–2026 only** (KLASS 644, 1,147 units); stable `organisasjonsnummer` per *sokn* in Brønnøysund (NLOD) | **Published vectors** — 1,154 polygons / 1,146 unique codes | Soknegrenser + KLASS 644 + Brønnøysund |

The shape of the answer: **the ecclesiastical hierarchy is importable as entities with real dated
history at the top two levels, and as a current snapshot at the bottom two — but historical parish
geometry is essentially unavailable**, with one CC BY 4.0 1801 exception whose upstream has vanished.
The genealogically most valuable level, *prestegjeld* 1838–1900, is exactly the one with neither
coded history nor open geometry.

## Chat-claim audit

Every substantive claim from the seven transcripts, with its status. Cited as `file:line`. This table
exists so that refuted material — including two fabricated endpoints — is not reused by the next
reader.

| Claim | Asserted by | Status | Evidence |
| --- | --- | --- | --- |
| Kartverket publishes current *fylke*/*kommune* boundaries | all seven | **VERIFIED** | Kartkatalogen datasets `6093c8a8-…` (fylker), `041f1e6e-…` (kommuner) |
| An "Administrative enheter – historiske versjoner" product exists | `claude.md:14`, `grok.md:53-55` | **VERIFIED** | Kartkatalogen `9bc064e3-6c34-4c3a-8421-00290052e9c0` + WMS twin |
| "Kartverket generally publishes only **current** boundaries" | `chatgpt.md:294` | **REFUTED** | the dataserie above exists |
| SSB publishes historical boundary **geometry**, 1986–2019 | `grok.md:22,56` | **VERIFIED** | data.norge.no `268bc92d-…`, 6 distributions incl. GeoJSON, NLOD 1.0 |
| "SSB is codes-only, no geometry" | the other six | **REFUTED** | same dataset |
| KLASS classification **131** = kommune, **104** = fylke | `claude.md:20` | **VERIFIED** | live API, exact titles |
| KLASS coverage starts "roughly mid-20th century" | `claude.md:20` | **REFUTED** | 131 earliest version is `Kommuneinndeling 1838` |
| KLASS municipality codes tracked back only to "the 1990s" | `deepseek.md:166-169` | **REFUTED** | 141 versions, 1838-01-01 onward |
| SSB has a machine-usable change list "since 1837" | `qwen.md:77`, `copilot.md:486` | **VERIFIED** (as 1838) | `changes` endpoint, HTTP 200 from 1838-01-01 |
| The 1837/1838–1974 register is text-only | `claude.md:20` | **VERIFIED** for *Rapporter* 1999/13 (PDF, 90 pp.); superseded by KLASS for the machine-readable form |
| An SSB *endringskode* vocabulary exists | implied by `qwen.md:104` | **REFUTED at schema level** | `CodeChangeItem` has no `changeType` field |
| Sikt/NSD *Kommunedatabasen* records every change 1838→present, with partial GIS reconstructions to ~1838 | `gemini.md:186,194` | **REFUTED as described** | it is ~543 k statistical variables (Befolkning 1769–2024) with population-proportional conversion factors; no polygons found |
| "Digitalarkivet / Historiske Administrative Grenser (HAG)" maps parish boundaries | `gemini.md:196` | **REFUTED — does not exist** | no such dataset/acronym at Arkivverket, Nasjonalarkivet, Digitalarkivet, Kartverket, Geonorge or data.norge.no |
| Earliest open vector polygons: ~2018 / ~1997 / 1998 / 1980 / "1980s" / a 1960–2010 gradient | `claude.md:74`, `grok.md:53`, `deepseek.md:26-28`, `gemini.md:186`, `qwen.md:75`, `chatgpt.md:350-360` | **ALL REFUTED** | `kart.ssb.no` serves per-year municipal polygons from **1838**; verified `Kommuner 1838` = 395 `MULTIPOLYGON` features |
| "Polygons are **not** systematically published as open vector data" for 1838→2018 | `claude.md:74` | **REFUTED** | 129 per-year collections, NLOD 1.0, unauthenticated |
| 1838 (or 1837) is the floor for municipalities *as entities* | `claude.md:74`, `gemini.md:178` | **VERIFIED** — the correct literal is **1838-01-01** — and it is also the floor for *geometry* | KLASS 131 + `kart.ssb.no` |
| Academic projects and SSB reconstructed borders back to 1837 | `qwen.md:76` | **VERIFIED for SSB**, unverified for the academic projects | SSB itself serves 1838 onward; no university artifact located |
| `kartkatalog.geonorge.no/api` with dataset ids `"fylker-2024"`/`"kommuner-2024"` | `deepseek.md:84-91` | **REFUTED — fabricated** | no such id scheme; real ids are UUIDs |
| `wfs.geonorge.no/skwms1/wfs.adresser` for administrative boundaries | `copilot.md:447-451` | **REFUTED — wrong layer**, self-flagged in the transcript | it is the address service |
| `wfs.geonorge.no/skwms1/wfs.administrative_enheter` | `grok.md:18` | **UNVERIFIED** | not fetched |
| SOSI needs FYBA and GDAL usually lacks the driver; request GML/GeoPackage instead | `claude.md:54`, `qwen.md:47` | **VERIFIED, and stronger than stated** | no viable pure-Rust SOSI parser exists at all |
| Norway has ~240,000 islands | `claude.md:15` | **VERIFIED** | 239,057 islands, 81,192 skerries |
| "PostGIS. This is non-negotiable" | `qwen.md:101` | **REFUTED for this repo** | ADR 0024 fixes typed `Microdegrees` geometry in the event log with a SQLite R\*Tree projection |
| Store GeoJSON / f64 coordinates as the persistence form | `chatgpt.md`, `deepseek.md:40-64`, `gemini.md` | **REFUTED** | ADR 0024 §1 — GeoJSON is interchange only |
| GeoPackage/SpatiaLite is the pragmatic desktop target | `claude.md:69` | **Partly VERIFIED** | correct instinct, but the repo already ships R\*Tree-in-SQLite; and in-component GeoPackage is impractical (see formats) |
| "Never derive hierarchy from geometry — parentage is a legal fact" | `claude.md:47` | **Endorsed** | §1 and §5; containment tests would disagree with the record and most historical places have no polygon |
| Use `ST_Contains` to find a place's parent municipality | `gemini.md:150-156`, `qwen.md:20` | **Rejected** | same reasoning |
| One `places` row per code with `ON CONFLICT (code) DO UPDATE` | `gemini.md:279` | **REFUTED** | codes are reused (1806 Svolvær → Narvik); this silently merges distinct units |
| Treat each legally distinct municipality as its own Place, linked by predecessor/successor | `chatgpt.md:391` | **VERIFIED as correct** | matches ADR 0026 §2 and Norwegian practice (re-established units get new numbers) |
| Parallel civil/ecclesiastical/judicial hierarchies that do not nest; `relation` on the parent link | `claude.md:37,46` | **VERIFIED and adopted** | §1 — the single most valuable modelling insight in the corpus |
| "For genealogy the ecclesiastical hierarchy is the one your sources actually use" | `claude.md:46` | **VERIFIED** | Digitalarkivet's church books are keyed by *prestegjeld*/*sokn* |
| Geometry needs `method` ∈ surveyed/derived/digitised/approximated + `accuracy_m` | `claude.md:78` | **VERIFIED and adopted** | §7 |
| Same place needs several geometries keyed by validity *and* generalisation level | `claude.md:45` | **VERIFIED and adopted** | §6 — the repo has the date axis but not the level axis |
| 17th-century polygons are often "factually wrong, not merely imprecise" (borders undefined; Sweden 1751, Russia 1826) | `claude.md:76-78` | **Endorsed, unverified in detail** | consistent with the total absence of pre-1801 vector data |
| *gårdsnummer* is "largely continuous back to the 1886 (and traceable to the 1838) matrikkel" | `claude.md:17` | **Partly REFUTED** | the *scheme* is continuous since the 1863–83 revision, but values are renumbered at every merger (Finnøy +100, Rennesøy +200 in 2020) |
| SSR exposes historical and alternate name forms via a REST API | `claude.md:16` | **VERIFIED** | `api.kartverket.no/stedsnavn/v1`, `navnestatus` includes `historisk`; CC BY 4.0 |
| Build parish extents bottom-up from farm points + matrikkel membership + Voronoi | `claude.md:80` | **Feasible, licence-blocked** | RHD's 1838/1886 matrikkel does carry a `Sogn` column, but its terms are personal-use-only |
| Parish reconstructions exist in research datasets where "licensing may be restrictive" | `copilot.md:128` | **VERIFIED — prescient** | RHD terms; `prestehistorie.no` is copyright-only |
| "Norway lacks comprehensive historical GIS like UK's Vision of Britain" | `deepseek.md:34` | **VERIFIED** | especially true ecclesiastically |
| GADM is not open for commercial use | `claude.md:22` | **VERIFIED — unusable** | its licence reads *"freely available for academic use and other non-commercial use. Redistribution or commercial use is not allowed without prior permission."* |
| OSM `admin_level` 4 = fylke, 7 = kommune, current only | `claude.md:22` | **UNVERIFIED** | not fetched |

Two further points the corpus disagreed on internally, now settled: the *amt* era began **1662** and
*fylke* renaming took effect **1919** (`claude.md:76` and `gemini.md:187` over `grok.md:59`'s 1671/1918
— though note KLASS 104 itself begins only in 1842, so the 1660–1841 period is PDF-only); and
`gemini.md` contradicted itself on geometry storage between its two answers, which is moot here since
ADR 0024 already decided it.

## Formats and CRS inside a WASM component

The constraint is hard: a `wasm32-wasip2` component has no GDAL, no shell and no filesystem, so every
format must have a pure-Rust parse path and every dependency must be permissive. All versions,
licences and dates below verified directly against the crates.io API for this document.

| Format | Crate | Version | Licence | Verdict |
| --- | --- | --- | --- | --- |
| GeoJSON | `geojson` | 1.0.0 | MIT/Apache-2.0 | **Use this.** 9.98 M downloads |
| WKB | `wkb` | 0.9.2 | MIT OR Apache-2.0 | Fine (GeoRust, 1.2 M downloads) |
| Shapefile | `shapefile` | 0.9.0 | MIT | Viable, 682 K downloads |
| FlatGeobuf | `flatgeobuf` | 6.0.1 | BSD-2-Clause | Viable; skip the optional `reqwest` features |
| GML | `quick-xml` 0.41.0 (hand-rolled) | — | MIT | Viable but you write the schema handling |
| GeoPackage | `rusqlite-gpkg` / `geopackage` / `oxigeo-gpkg` | 0.0.8 / 0.5.0 / 0.2.1 | MIT / MIT OR Apache-2.0 / Apache-2.0 | **Avoid** — see below |
| FGDB | `geonative-filegdb` | 0.4.0 | MIT OR Apache-2.0 | **Avoid** — contradictory signals |
| SOSI | none | — | — | **No crate exists** — but see below |

Three conclusions that shape the plugin design:

1. **SOSI has no crate, but it is not a dead end.** No pure-Rust parser exists in any usable state, and
   every real implementation is C (Kartverket's own FYBA, and GDAL's SOSI driver which requires it),
   C++ (`sosicon`) or GPLv3 Python. That matters because **Soknegrenser — the only official
   ecclesiastical geometry — is SOSI-only**. But having parsed the actual file, the job is bounded
   rather than blocked: SOSI is a line-oriented text format, and this import needs only `.FLATE`,
   `.KURVE`, `..REF` and `..NØ` plus five attribute paths, over 1,165 flater with just 4 inner-ring
   cases, at a fixed `ENHET 0.01` and `NØ` axis order. A minimal reader is a contained piece of work.
   Three options in order of preference: write that reader; ask Kartverket to add GeoJSON/GeoPackage to
   the Soknegrenser distribution as the sibling "Kirkebygg – forenklet" dataset already has; or ship a
   pre-converted GeoJSON as a plugin asset and refresh it annually, since the source changes once a
   year. Note that the SOSI path is also the one place **reprojection is genuinely required** (EPSG
   25833 → WGS84), whereas the SSR API and "Kirkebygg – forenklet" both hand over EPSG:4258 degrees
   that need no projection step at all.

   Three practical details for whoever writes that reader. **Licensing of reference implementations
   matters here:** `sosicon` is **GPLv3**, so its logic must not be ported — the same copyleft trap
   `CLAUDE.md` already flags for Gramps — whereas **Kartverket's own FYBA is MIT-licensed**, making it a
   legally clean C reference to study for the topology-assembly algorithm. **Character encoding:** the
   `TEGNSETT` header selects the codepage, and `DOSN8` is **CP865 ("DOS Nordic")** — confirmed by
   Kartverket's object catalogue giving the exact Æ/Ø/Å byte codes (146/157/143) and by Unicode.org's
   mapping table being named `cp865_DOSNordic`. `encoding_rs` does **not** cover CP865 (it targets the
   WHATWG web set), so use `oem_cp` 2.1.2 (MIT), which has a dedicated `Cp865` type; the header itself is
   ASCII-safe, so read it raw, then decode the body. **Unzipping:** `zip` 8.6.0 (MIT) reads from a
   `Cursor<&[u8]>` with no filesystem, which suits a byte buffer from `net`; disable default features and
   enable only DEFLATE to keep the dependency tree pure-Rust.
2. **GeoPackage is technically reachable but should not be reached for.** `rusqlite` does compile for
   `wasm32-wasip2` (its wasm bindings-swap is gated on `target_os = "unknown"`, so WASI falls through
   to bundled SQLite; there is a working precedent project), but it needs a wasi-sdk C toolchain at
   build time *and* an in-memory VFS since the component has no filesystem. The reader libraries on top
   are all too new to depend on: `rusqlite-gpkg` 0.0.8 (352 downloads), `geopackage` 0.5.0 (**created
   2026-07-24 — five days before this research**, 80 downloads, and it bundles C SQLite so it inherits
   the toolchain cost rather than avoiding it), and `oxigeo-gpkg` 0.2.1 (63 downloads), which is
   architecturally the only one that avoids C entirely — a from-scratch pure-Rust SQLite b-tree/WAL
   reader — but sits inside a 75-crate single-author workspace whose own support table admits GeoPackage
   writes handle "point feature tables only, single-page B-tree", and whose wasm evidence is for
   *browser* `wasm32-unknown-unknown`, a different target from a WASI component. Cost and risk both far
   exceed the benefit now that the civil data arrives as GeoJSON over HTTPS.
   The same caution applies to **FGDB**: `geonative-filegdb`'s generated docs describe specific,
   spec-literate modules, but its own workspace README states the `geonative-*` crates are "currently
   placeholders to reserve the namespace". That contradiction is unresolved, and the crate describes
   itself as mmap-backed, which does not map onto a component with no filesystem. Do not depend on it
   without first running it against a real `.gdb`.
3. **Reprojection is mostly unnecessary, and solvable in pure Rust where it is needed.** The civil path
   needs none at all: `kart.ssb.no` reprojects server-side to EPSG:4326 on request, and both the SSR API
   and "Kirkebygg – forenklet" hand over EPSG:4258 degrees. Client-side projection is required only for
   the SOSI/EPSG:25833 Soknegrenser file. For that, `proj4rs` 0.1.10 (MIT OR Apache-2.0, 874 K downloads)
   implements the PROJ.4 API including `utm`/`tmerc`, takes proj-string literals, and needs no
   `proj.db` or grid files — unlike the `proj` and `gdal` crates, which link C libraries and are
   therefore blocked outright. `geodesy` 0.15.0 is a credible backup. The projection *math* is a
   non-issue at our precision (Karney-class Transverse Mercator is accurate to nanometres, nine orders
   below the 0.11 m microdegree quantum). The real cost is the **datum** step: EUREF89/ETRS89 was
   fixed to the Eurasian plate at epoch 1989.0 and diverges from current-epoch WGS84 at roughly
   1–3 cm/year, so by 2026 the accumulated offset is plausibly decimetre-to-metre — **larger than our
   storage quantum**. For plotting places on a map this is an acceptable simplification, but it must
   be documented rather than claimed away, and expressed explicitly in the proj string (an identity
   `+towgs84=0,0,0,0,0,0,0`) so the choice is visible in code. The exact current offset figure is
   **UNVERIFIED** — no single authoritative scalar was found.

## Plugin design

### World and role

Neither existing bulk world fits cleanly. `bulk-import` (ADR 0013) assumes the user hands over a
file, and `assisted-import` (ADR 0017) assumes a user-driven, record-at-a-time wizard with `present`
suspensions. What this task actually is — "create Norway if absent, then populate the administrative
hierarchy from named public registers" — is **reference-data seeding**: no user file, no per-record
decisions, one fetch-and-populate run whose input is a version of a public dataset rather than a
document the user owns.

**Recommendation: a new `reference-data` world importing `log`, `query`, `commands`, `progress` and
`net`.** Because every civil source turns out to be fetchable over HTTPS inside the 8 MiB cap (see *Data
delivery* below), there is no file for the user to supply and so no reason to take `import-source` —
under `bulk-import` it would be dead weight. That world also imports `log`, `commands`, `progress`,
`import-source` and **not `query`**, so idempotency checks against existing places would be impossible
in it as it stands. Declaring a new world is additive per ADR 0011 §1, which makes each role a world
importing only what it needs — exactly this case.

If a new world is judged too much for a first cut, the fallback is `bulk-import` **plus `query`**, with
the user downloading one GeoJSON by hand. That works, but it trades one additive world declaration for a
manual step and an unused capability.

### Required WIT additions

`host-api@0.21.0` cannot express this import. Grepping `host.wit` for `geometry`, `coord` and
`geojson` returns nothing, and there is no succession verb. The minimum additive set, which would
make this **`host-api@0.22.0`** (additive, so a minor bump per ADR 0011 §1):

| Verb | Purpose | Notes |
| --- | --- | --- |
| `set-place-enclosed-by-dated(place, enclosing, date)` | dated hierarchy | or add an `option<date>` param; the existing undated verb stays |
| `add-place-name-dated(place, name, language, date)` | dated names | Kristiania → Oslo |
| `assert-place-geometry(place, geometry, date, accuracy)` | boundaries | needs a WIT `place-geometry` record mirroring `PlaceGeometry` incl. `multi-polygon` |
| `assert-place-succession(place, from, to, kind, date)` | mergers/splits | mirrors `PlaceSuccessionInput` |
| `set-place-code-dated(place, code, date)` | *kommunenummer* history | pairs with §10 |
| `resolve-or-create-place(name, place-type, external-id)` | idempotency | mirrors the existing `create-person` shape returning `import-result { human-id, created }` |
| `assert-place-dissolution(place, date)` | cessation | only if §2's additive event is adopted |

Each of these also needs an app use-case and a `pub use` in `genealogy-app/src/lib.rs`, and each
raises a CLI/GUI surface question — the CLI has `place assert-succession` but no geometry subcommand,
and `PlaceEdit::AddEnclosing` carries no date. Those surfaces are out of scope here but should be
filed alongside, or the plugin becomes the only way to write data the UI cannot then edit.

### Ingest shape and cost

Per place: `resolve-or-create` (2 commits today — create + name), `set-place-type` (1),
external id (1), one dated enclosure per era (~2), one dated code per era (~3), geometry (~1–3), and
successions on the anchor (~1). Call it **~10 commands / ~12 commits per place**.

- **Current civil only:** 15 *fylker* + 356 *kommuner* = 371 places ≈ 4.5 k commits.
- **Historical civil 1838–2026:** ~1,100 distinct municipal identities plus ~35 county identities
  ≈ **14 k commits**.
- **Ecclesiastical entities:** 1,165 *sokn* + ~100 *prosti* + ~11 *bispedømme* ≈ 15 k commits.

Now multiply by the known cost drivers from §9. `next_human_id` scans and JSON-extracts **every**
`place_view` row per create (`sqlite_query.rs:23`), and each `human_id` resolve is a second full scan
(`sqlite_query.rs:51`). At N500 a place's blob is ~73 KB, so by the time 1,100 places exist a single
create scans ~80 MB. Summed over 1,100 creates that is `1100²/2 × 73 KB ≈ 44 GB` of JSON extraction,
and enclosure/succession writes add two or three more full scans each. At a generous few hundred MB/s
that is **order tens of minutes for a one-time run**, with both derived indexes reindexed on every
one of the 14 k commands and no transaction anywhere to batch them.

#### A batched command, not a fused event

The obvious optimisation is a special-purpose bulk assertion that does everything at once. Split that
into two proposals, because one is precluded and the other is available today.

**A fused event is precluded by ADR 0021 §1**, which moved deliberately in the opposite direction.
`ChildAdded` used to pack every per-parent link into a single assertion, so one `AssertionId` covered
"child of this family" *and* "birth child of P1" *and* "step child of P2" — meaning an adoption link
could not be retracted or re-cited without retracting the child's membership. That ADR split the fat
event into fine-grained ones, bumped `ChildAdded` to `"2.0"`, accepted that old fat events no longer
decode, and grounded the rule in Polygenea's *principle of sensible disbelief*: it should be sensible to
disbelieve a node without disbelieving the nodes it references. A
`BulkAsserted { name, place_type, code, enclosed_by, geometry, … }` event would carry **one
`AssertionId` over five unrelated claims**, so a wrong boundary could not be retracted without also
retracting the name and the *kommunenummer*. That breaks ADR 0004 §2 corrections, re-introduces exactly
the shape ADR 0021 removed, and would duplicate every `evolve` arm — two ways to reach one state, against
the repo's no-dual-formats rule.

**A batched command needs no new event shape at all, and captures most of the win.**
`decide(state, command) -> Result<Vec<Event>, Error>` already returns a `Vec`, and cqrs-es appends that
whole vector in one commit while dispatching queries **once with the event slice**. So a single `execute`
emitting ten fine-grained events collapses ten aggregate loads to one, ten commits to one, ten
`place_view` blob read-parse-rewrite cycles to one, ten `reindex_place` runs *per derived index* to one
each, and ten optimistic-concurrency CAS attempts to one — while every event keeps its own `AssertionId`
and `evolve` stays untouched.

The concept already exists half-built: `commit_place_change_set` (`place_change_set.rs:52`) is exactly
this idea, but issues up to four separate `execute` calls, one commit each. So the change is to make the
change-set emit **one** command, and delete the multi-call path rather than shim around it.

Two design details worth settling up front. **Assertion ids come from outside the pure core** (ADR 0004
§3), and today `PlaceCommandEnvelope { meta: AssertionMeta, command }` carries exactly one `AssertionMeta`
while each `Envelope` carries one `assertion_id` — so a batch command needs the app layer to supply *N*
assertion ids, either as a `Vec` on the envelope or per item inside the batch. And the batch should carry
**assertions only, not `CreatePlace`**: creation needs the `next_human_id` round trip in the app layer
regardless, and excluding it makes `decide`'s guards uniformly "the place must exist", which is what makes
the command repeatable — the property that matters for re-runs and for later corrections, not just for
first import.

**What it does not fix.** This is a constant-factor win, roughly 10×, not a complexity fix: the O(n²)
`next_human_id` and `find_place`-by-`human_id` scans happen in the app layer *before* the command is
built, so only the index in recommendation 4 addresses them. Replay cost is also unchanged — ten events
still replay as ten, so `rebuild_projections` sees no benefit. And it grants no cross-aggregate
atomicity: cqrs-es 0.5 commits per aggregate, which is why this works cleanly here and only here —
every assertion in the batch targets one Place.

**Verdict: a slow one-time import is acceptable; an O(n²) one is not, and this one is O(n²) twice
over.** The prerequisite is not a batching API — it is two cheap, local fixes: a **generated column
plus index on `$.state.human_id`** (which converts both full scans into index probes) and an index on
`place_geometry(place_id)`. A sequence table for `human_id` allocation would remove the remaining
scan. None of these touch the event model; all three are `genealogy-db` changes. With them, the run
becomes linear and the estimate drops to minutes.

### Idempotency

Recurring imports must add no duplicate records and emit no events when nothing changed. This is
achievable, and most of the machinery is already decided — but it resolves into four distinct layers, and
Place is missing the prerequisites for the third.

**Layer 1 — no duplicate aggregates.** `ExternalId` resolve-or-create (§5). The core already supports the
no-op half of this: `PersonCommand::AddExternalId` and its Family twin return `Ok(Vec::new())` when the
identifier is already present, carrying the comment *"Idempotent: re-adding the same identifier emits
nothing, so re-import is a no-op"* (`person/decide.rs:143`, `family/decide.rs:150`). Place needs the same
command and the same guard.

**Layer 2 — no events for unchanged values.** `decide` returning an empty vector is an established idiom
here, not a novelty: person, family and `research_note` all do it. Extending it to Place's assertions —
emit nothing when the proposed value equals the current live one — is pure, deterministic, unit-testable,
and applies to every frontend rather than just the importer. This is the direct answer to "no new events
if not needed", and it belongs in the core rather than in the plugin, because a read-then-skip in the
plugin is both duplicated per caller and racy. One implementation note: confirm the derived-index
`dispatch` does not run `reindex_place` when handed an empty event slice, or the saving is partly lost.

**Layer 3 — changed values reconcile by timestamp, per ADR 0029.** That ADR already fixes the rule: if the
live assertion's `EventContext.occurred_at` is *after* the incoming file's export date the import is
stale and nothing happens; if at or before, the importer supersedes through the existing correction path
with a generated rationale naming the source and its date; and a missing export date falls back to
additive-only. `begin-import(file-asserted-at)` is how the vintage arrives.

**But ADR 0029 §4 covers `Person.sex` only, and it enumerates exactly the prerequisites `Source` lacked —
Place lacks the same three.** (i) No `ExternalId` resolve-or-create, so a re-run cannot even find the
prior record; (ii) no WIT verbs for most Place fields (§*Required WIT additions*); and (iii) **no read
path exposing a field's live `AssertionId` together with that assertion's `occurred_at`** — `PlaceView`
has `*_with_assertions()` accessors, but the timestamp needed for the gate lives in the envelope's
`EventContext`, and `PlaceSummary` does not surface it. That third gap is the one most likely to be
underestimated: without it the timestamp comparison cannot be evaluated at all.

**Layer 4 — the matching problem, where Place is better placed than `Fact`.** ADR 0029 deliberately
restricts itself to genuinely single-valued fields, excluding `PersonName` and `Fact` because "the same
fact updated" versus "a new fact of the same type" is an unresolved matching question. Place's important
fields are all multi-valued accumulating vectors — `names`, `enclosed_by`, `geometries`, `successions`
(`place/state.rs:38-45`) — so on its face the rule does not extend to them. **The date rescues it:** each
of these assertions carries an effective-from `date`, which supplies the natural identity the `Fact` case
lacks. A geometry asserted for 1838 *is* the same claim as another geometry asserted for 1838. So the
match keys are `(date)` for geometry, `(relation, date)` for enclosure, and `(text, language, date)` for
names — and reconciliation becomes well-defined per key. Note that §10's recommendation to make `code`
dated and accumulating moves it from the single-valued class into this one, which is an argument for
doing both pieces together.

#### Two hazards not covered by ADR 0029

**Retraction resurrection — the serious one.** ADR 0029 compares against the *live* assertion, and a
retracted assertion is not live. So if a researcher retracts an imported boundary because they judged it
wrong, the next import run sees no live value, re-asserts it, and silently overrides that editorial
decision — **every run, forever**. This is worse than a duplicate record, because it destroys human
judgement rather than adding noise, and it will be attributed to `AgentKind::Software` so it looks
routine in the history. The fix is to treat "asserted by this authority and since retracted" as a
tombstone the importer honours. That requires a read path over retracted assertions keyed by originating
authority, which does not exist today. **Any recurring import must solve this before it is turned on.**

**False-change churn on geometry.** `PlaceGeometry` derives `Eq` over integer `Microdegrees`
(`geo.rs:68`), so exact comparison is cheap and deterministic — but exactness is the problem. A re-fetch
can differ from the stored value by ring starting vertex, ring winding order, server-side reprojection
rounding, or upstream re-generalization, none of which change the boundary meaningfully while all of them
break structural equality. The result is a fresh `GeometryAsserted` on every run: technically a change,
semantically noise, and it would bloat the log of the largest payloads in the workspace. Mitigations:
canonicalize before comparing (fix ring start and winding), and pin the request — same `crs` parameter,
same dataset vintage — so the server performs the same computation each time. Useful nuance: the
per-year historical collections are immutable upstream, so this risk is concentrated almost entirely in
the *current* layer (`kommuner_2024`, which SSB describes as automatically updated), not in the 1838–1996
series.

For the ecclesiastical side there is a genuinely stable key —
`(authority = "brreg", value = organisasjonsnummer)` — which is not reused, is NLOD-licensed, and
appears byte-identical inside the Soknegrenser geometry, so parish identity needs no composite key at
all. For the civil side no such key exists, so it must be `(authority = "ssb-klass",
value = kommunenummer, valid_from)` — never the bare number, since codes are reused — and expose
`resolve-or-create-place` returning `import-result { human-id, created }`. `begin-import(file-asserted-at)`
should carry the dataset's version date so ADR 0029's timestamp gating decides whether a re-run
supersedes existing assertions or leaves them alone.

### Data delivery, and the licence question

Three options, and the licences decide it:

1. **Fetch at runtime over `net`.** Now the primary path for **both** the entity history and the civil
   geometry. KLASS JSON is tiny (`codesAt` for all historical *kommune* codes is 64 KB; a decade of
   `changes` is 9–46 KB), and `kart.ssb.no` serves a whole year of 1838-era boundaries in 4.4 MiB —
   inside the 8 MiB cap, in one HTTPS GET, already reprojected to EPSG:4326 on request. The
   improved-accuracy 1986–2019 series is ~16.6 MiB per year and must be paged with `limit`/`offset`;
   because per-feature size varies ~30×, page conservatively rather than trusting a fixed feature
   count. Both hosts must go in the `NetPolicy` allowlist (`data.ssb.no`, `kart.ssb.no`). The 8 MiB cap
   only becomes a blocker for the ~24 MB `atlefren` parish files and the 5.3 MB-zipped Soknegrenser
   (which fits, but inflates to 22 MB).
2. **Bundle pre-processed data in the signed plugin bundle.** Licence-permitted for the data we would
   want: SSB KLASS is **CC BY 4.0**, SSB's 1986–2019 boundaries are **NLOD 1.0**, Kartverket's
   historical series is **CC BY 4.0**, `robhop/fylker-og-kommuner` is CC BY 4.0, and `atlefren`'s 1801
   parishes are CC BY 4.0 — all redistributable **with attribution**. Blocked for exactly two:
   **RHD's 1838/1886 *matrikkel*** (personal use only; redistribution needs written HistLab consent)
   and **`prestehistorie.no`** (copyright, no licence). Bundle size is the real cost, and it argues
   for shipping generalized GeoJSON at N500-equivalent, not full resolution.
3. **User supplies the file** through `import-source`. Zero licence exposure, zero bundle weight, at
   the cost of a manual step and a documented download.

**Recommendation: (1) for everything civil.** The plugin fetches KLASS for the dated skeleton and
`kart.ssb.no` per year for boundaries — no user file, no bundled data, no licence redistribution
question, and no conversion step. Keep (3) as the fallback for the ecclesiastical layer, whose only
official geometry is SOSI. Revisit (2) only if offline seeding becomes a requirement; bundling CC BY or
NLOD data obliges us to display attribution somewhere in-app, which is a UI decision, not a plugin one.

This also changes the world choice above: with geometry arriving over `net` rather than through
`import-source`, the `bulk-import` world is no longer the natural fit — it would be imported purely for
`progress`. A **`reference-data` world importing `log`, `query`, `commands`, `progress` and `net`** is
the honest shape, and it is a small additive addition to the WIT rather than a new mechanism.

### Provenance

Every write is already attributed to `AgentKind::Software { name, version }` (ADR 0007 §7), which
answers "who", but not "from what". Since boundaries are *evidence-bearing claims* — a reconstructed
1801 parish outline is a much weaker claim than a 2024 surveyed *kommune* border — the dataset should
become a real `Source` plus `Citation` (title, publisher, version date, licence, URL, checksum), with
each `GeometryAsserted` carrying that citation in its `EventContext`. That is what the envelope is
for (ADR 0020), it makes the CC BY attribution obligation a data fact rather than a UI afterthought,
and it lets a user see *why* two boundaries for the same year disagree. `rationale` should name the
dataset version; `confidence` should be left alone in favour of §7's `accuracy` field.

### Ecclesiastical variant

Conditioned on the verdict table above: **import the entity hierarchy, not the geometry.** KLASS
510/651 give dated *bispedømme* and *prosti* history (1070 and 1865 onward) and 644 gives 1,165
current *sokn* with the 8-digit `soknenummer` encoding the whole parent chain — so the hierarchy can
be reconstructed from the code itself, which is exactly the *civil/ecclesiastical* `relation` case
from §1. Attach current *sokn* geometry from Soknegrenser only if the SOSI blocker is solved outside
the component. *Prestegjeld* 1838–1900, the level genealogical sources actually cite, gets entities
from `lokalhistoriewiki`'s structured 1814 table (licence permitting) and geometry only from the one
CC BY 4.0 1801 layer. This is worth doing precisely *because* it is entity-only: a researcher needs
"which *prestegjeld* held this farm in 1865" far more often than a polygon.

### Out of scope, confirmed

Map rendering stays out of the plugin — WASM components cannot draw, and ADR 0025 §1 already fixes
rendering in framework code. This is also **not** the deferred `map-provider` world of ADR 0025 §4:
that supplies tile/style descriptors and geocoding over `net`, whereas this writes domain aggregates
through `commands`. They are unrelated, and conflating them would put place-writing capability into a
provider plugin that has no business holding it.

## Feasibility verdicts

| Target | Data tier | Model sufficient | Plugin feasible | Verdict |
| --- | --- | --- | --- | --- |
| Norway (country) | trivial | yes | yes | **Ship** |
| *Fylker*, current | published vectors (CC BY 4.0) | after `MultiPolygon` | yes | **Ship** |
| *Kommuner*, current | published vectors (CC BY 4.0, GeoJSON via `robhop`) | after `MultiPolygon` | yes | **Ship** |
| *Kommuner*, geometry **1838–1996** | **published vectors, per-year, NLOD 1.0** (`kart.ssb.no` OGC API, 129 collections) | after `MultiPolygon` + dated enclosure | yes — 4.4 MiB/year, server-side reprojection | **Ship** |
| *Kommuner*, geometry 1986–2019 (improved accuracy) | published vectors, annual, NLOD 1.0 | after `MultiPolygon` + `accuracy` | yes, with paging (~16.6 MiB/year) | **Ship** |
| Civil change history 1838–2026 (entities, names, codes, successions) | machine-readable (KLASS 131/104) | after dated enclosure + dated `code` | yes | **Ship** |
| *Fylker*, geometry, historical | **none published** — only current `fylker_2024` | — | — | **Not feasible** (entities only) |
| *Fylker*/*amt* pre-1919 | tabular only; 1660–1841 is PDF-only | entities yes | entities yes | **Ship reduced** (entities, no geometry) |
| *Bispedømme* | coded 1070–2005; **11 published polygons** (SOSI) | after `Diocese` type | yes, needs a SOSI reader | **Ship** (entities + current geometry) |
| *Prosti* | coded 1865–2025 (58 versions); geometry reconstructable by dissolve | after `Deanery` type | yes | **Ship reduced** (entities; geometry derived, so mark accuracy) |
| *Prestegjeld* | coded 1997–2005 only; 1801 CC BY 4.0 polygons; 1838 farm membership (licence unclear); 1530–1900 history unlicensed | after `District` type | yes | **Ship reduced** (1801 layer + thin entity history) |
| *Sokn* | coded 2020–2026 (1,147); **1,154 published polygons**; stable Brønnøysund orgnr | yes | yes, needs a SOSI reader | **Ship reduced** (current snapshot only; no history) |

The headline: **the civil hierarchy is fully shippable as dated entities back to 1838 and as geometry
back to 1986; the ecclesiastical hierarchy is shippable as entities with real depth at the top but
only as a current snapshot at the bottom.** The original question's implicit goal — dated boundary
polygons since 1838 — is not achievable from open data for the 1838–1985 window, and quite possibly
not at all.

## Recommendations

Ordered by whether they block the import.

1. **Add `PlaceGeometry::MultiPolygon`** — blocks any real Norwegian boundary import (§8). Vehicle:
   new ADR (0031) recording the reopening of the `Multi*` half of the decided item, since ADR 0024
   named "a concrete need" as the trigger. Size **M**. No dependencies.
2. **Add `PlaceRef.relation`** and surface the full enclosure set on `PlaceSummary` — blocks the
   ecclesiastical import and silently corrupts hierarchy titles if skipped (§1). Vehicle: ADR 0031 or
   its own ADR; it changes a resolver contract, not just a field. Size **M**.
3. **Date the enclosure, name and code write paths** — `assert_place_enclosed_by` and
   `add_place_name` take a date; `SetCode` gains a date and `code` becomes accumulating (§4, §10).
   Vehicle: the existing issue "Dated name/enclosure use-cases", widened. Size **M**. Blocks any
   dated hierarchy.
4. **Index `$.state.human_id`** (generated column) and `place_geometry(place_id)` — turns the import
   from O(n²) to linear (§9, plugin cost). Vehicle: issue under *Performance & scale*. Size **S**.
   No event impact. Do this before any bulk run.
5. **Widen `ExternalId` to Place** — the only route to a re-runnable import (§5). Vehicle: existing
   deferred widening in data-model §17. Size **S** (one registry line plus an additive event).
5b. **Add a batched Place command** that emits many fine-grained events from one `execute`, and rewire
   `commit_place_change_set` onto it, deleting its four-call path (plugin design, *A batched command, not a
   fused event*). Collapses per-assertion commits, view-blob rewrites and derived-index reindexes by
   roughly 10× on import. **No new event variants and no fused assertion** — ADR 0021 §1 forbids the
   latter. Requires deciding how *N* assertion ids reach the core (ADR 0004 §3). Size **M**. Complements
   recommendation 4 rather than replacing it: this fixes the constant factor, the index fixes the
   complexity class.
5c. **Make recurring import idempotent**, in four pieces (plugin design, *Idempotency*): `AddExternalId`
   on Place with the existing `Ok(Vec::new())` guard; value-equality suppression in `decide` for Place's
   assertions; a read path exposing each field's live `AssertionId` **with** its `occurred_at` so ADR
   0029's timestamp gate can actually be evaluated (the prerequisite that ADR 0029 §4 found missing for
   `Source` and which Place lacks identically); and per-key matching for the multi-valued fields, keyed on
   the effective-from `date`. Size **M**. **Blocks turning on any recurring import** — see the retraction
   hazard in issue 18c.
6. **Add `PositionalAccuracy { metres, method }` and `GeometryRole { Extent | Representative }` to
   `PlaceGeometryAssertion`**, plus a deterministic same-date tie-break in `geometry_as_of` — fixes the
   generalization ambiguity, the provenance gap, and the "is this a boundary or a pin" ambiguity
   (§6, §7, §13). Size **S**. Depends on 1 only for ordering convenience.
6b. **Add `places_containing(point, as_of)`** to the read model, over R\*Tree candidates plus an exact
   `geo` predicate on the stored WKB (§13). No event change. Size **S**. This is what makes "which parish
   held this farm in 1865" answerable from geometry — as *evidence* for an assertion, never as a
   projection-time inference. Note it inherits §11's Postgres gap.
7. **Add `Diocese`, `Deanery`, `District` to `PlaceType`** with Fluent labels — otherwise the
   ecclesiastical import leaks raw Norwegian into a localized UI (§3). Size **S**.
8. **Add `existed_as_of` and use it in `show_geography` and `summarize_as_of`** — stops dissolved
   units rendering forever on the map (§2). Pure read-side, no event change. Size **S**.
9. **Make `show_place_resolved` tolerate `DbError::Unsupported`** — fixes an existing outright break
   of the place screen and CLI `place show` on Postgres (§11). Size **XS**. Unrelated to Norway; file
   and fix independently. This is a **stopgap, not the destination** — see 9b.
9b. **Bring Postgres to parity, then add PostGIS spatial support** (§11). Two separable pieces, in this
   order: (i) mirror `place_succession_index` for `Pool<Postgres>` so `show_place` genuinely works rather
   than degrading — **non-spatial, needs no PostGIS**, size **M**; (ii) add a PostGIS
   `geometry(Geometry, 4326)` projection column with a GiST index backing `places_in_bbox` and
   `places_containing`, size **M–L**, and gated on a shared conformance corpus proving both engines agree.
   Follow the existing `sqlite_query.rs`/`postgres_query.rs` split rather than genericising over
   `sqlx::Database`. If Postgres + PostGIS is a supported deployment, this moves from filed follow-up to
   in-scope, and the requirement itself deserves recording — ADR 0002 gated Postgres by Cargo feature and
   says nothing about required server extensions.
10. **Fix the geography marker label** to use the as-of-resolved name (`geography.rs:152`) — existing
    bug, one line (§12). Size **XS**.
11. **`host-api@0.22.0`** with the seven additive verbs, plus `query` added to the `bulk-import`
    world for idempotency checks. Vehicle: ADR 0011 §1 covers additive evolution; no new ADR needed.
    Size **M**. Depends on 1, 3, 5, 6.
12. **Build the plugin** as a `reference-data` component: KLASS over `net` for the dated skeleton,
    `kart.ssb.no` over `net` for per-year boundaries already reprojected to EPSG:4326, each dataset
    version recorded as a `Source` + `Citation`. Size **L**. Depends on everything above.
13. **Confirm the `kart.ssb.no` access contract** (open question 1) before shipping a plugin that
    depends on unauthenticated reads. Size **XS**.
14. **Refresh `docs/data-model.md`** §10 and the diagram to include the Phase 9 commands and events.
    Size **S**. Independent of everything else.
15. **Decide the reference-data question** (open question 2) before importing thousands of places into
    a user's workspace. Size **S** if it is a Tag convention; **L** if it needs a real separation.

## Phased plan

**P0 — Unblock and clean up (no Norwegian data involved).** Recommendations 4, 9, 10, 14. Exit: the
`human_id` scans are index probes, `place show` works on Postgres, the map label matches the slider,
and the data model documents what the code does. Independently valuable; nothing here is speculative.

**P1 — Model changes.** Recommendations 1, 2, 3, 5, 6, 7, 8, gated by **ADR 0031** (reopening
`Multi*`, adding the hierarchy `relation`, and fixing positional accuracy as a distinct concept from
`Confidence`). Exit: a hand-written test can assert a dated civil *and* ecclesiastical parent on one
place, a multi-part island municipality, a dissolved unit that stops resolving, and a re-runnable
external-id lookup. **This is the real gate** — everything after it is plumbing.

**P2 — Host surface.** Recommendation 11: `host-api@0.22.0`, the app use-cases and `pub use` entries
behind each verb, plus the CLI/GUI surfaces for anything the plugin can now write that the UI cannot
edit. Exit: the fixture plugin can assert a dated multi-polygon boundary and a succession.

**P3 — Civil import, current only.** Recommendation 12 scoped to present-day *fylker* and *kommuner*
from `kommuner_2024`/`fylker_2024`, plus KLASS 131/104 current codes. Exit: 373 places with correct
hierarchy, correct multi-part geometry, and a re-run that creates nothing new. This is the smallest
end-to-end slice that exercises every new model field.

**P4 — Civil history 1838→2026.** Both halves, since the geometry turned out to be available: KLASS
`changes` chunked by decade for dated names, dated codes and successions (inferred from code-mapping
cardinality, documented as lossy since there is no `endringskode`), **and** per-year boundaries from
`kart.ssb.no` — the 1838–1996 series at one request per year, the 1986–2019 improved series paged. Exit:
the time slider moves through 188 years of both entity *and* boundary history with correct
predecessor/successor links. Note this is the phase where import volume and the §9 performance work
actually bite: 129 year-collections is far more geometry than P3.

**P5 — Ecclesiastical entities.** KLASS 510/651/644, hierarchy derived from the 8-digit
*soknenummer*, Brønnøysund *organisasjonsnummer* as the *sokn* external id, all typed with the new
`PlaceType` variants and the `Ecclesiastical` relation. Exit: a *sokn* resolves its *prosti* and
*bispedømme* parents without disturbing the civil chain.

**P6 — Optional, conditional.** Current *sokn*/*bispedømme* polygons, which requires the minimal SOSI
reader (or a pre-converted asset); the 1801 *prestegjeld* layer with explicit low positional accuracy;
the 1838 *kommune*-as-*prestegjeld* proxy, if the ~90% identity is judged honest enough to publish with
a rationale. Each is independently droppable.

Placement against `docs/roadmap.md`: Phases 0–11 are shipped and only 12 (DNA) and 13 (server/web)
remain, so this is **a new phase**, and P0–P2 are ordinary maintenance and platform work that need no
Norwegian justification. P3 onward is arguably a **first-party plugin deliverable** rather than a core
phase, which fits the roadmap's treatment of import/export as plugins.

## Potential issues

| # | Issue | L | I | Mitigation |
| --- | --- | --- | --- | --- |
| 1 | Historical *fylke* geometry does not exist at all — only current `fylker_2024` among 409 SSB collections | H | M | Import county history as entities from KLASS 104; do not promise county boundaries |
| 1b | `kart.ssb.no`'s OpenAPI declares global `Bearer` security even though the geodata reads work unauthenticated; the terms endpoint requires auth | M | H | Treat unauthenticated access as undocumented-but-working; pin the dataset, and expect the plugin to break if SSB enforces auth later |
| 1c | The 1986–2019 improved series is ~16.6 MiB/year with ~30× per-feature size variance, so a fixed page limit is no byte guarantee | H | M | Page conservatively (≈50 features) with a byte-aware retry; the 1838–1996 series needs no paging at 4.4 MiB/year |
| 2 | `MultiPolygon` absence makes ~200+ of 357 municipalities unrepresentable without lying | H | H | Recommendation 1 before any geometry import |
| 3 | Mixed civil/ecclesiastical hierarchy chains produce factually false generated titles, silently | H | H | Recommendation 2; never import a second parent before `relation` exists |
| 4 | No `endringskode` in KLASS means merge/split/boundary-adjustment must be inferred from code-mapping cardinality — some changes will be mislabelled | H | M | Document the inference as lossy; prefer omitting a `SuccessionKind` over guessing; cite the SSB narrative in the envelope |
| 5 | O(n²) `human_id` minting and resolution makes a 14 k-command import take tens of minutes | H | M | Recommendation 4 (index) before any run |
| 6 | Dissolved municipalities render forever, overlapping their own successors on the map | H | M | Recommendation 8 |
| 7 | Same-date geometries at different generalization levels resolve non-deterministically by import order | M | M | Recommendation 6's tie-break |
| 8 | `place_view` blob is re-read and rewritten per command; megabyte geometries invalidate the snapshotting decision | M | H | Import at N500 or coarser; re-measure snapshotting if finer data is ever attempted |
| 9 | Importing ~1,100 reference places drowns a user's own handful of places in every list and picker | H | H | Open question 2 — decide before P3, not after |
| 10 | CC BY 4.0 and NLOD attribution obligations have no in-app surface today | H | M | Record each dataset as a `Source` + `Citation` (plugin design, Provenance) and design one attribution view |
| 11 | RHD's 1838/1886 *matrikkel* — the published farm→parish crosswalk — states **no licence at all**, while a sibling RHD dataset is personal-use-only | H | M | Do not bundle or redistribute; contact HistLab for written clarification before using it as import input |
| 11b | Digitalarkivet's `robots.txt` explicitly `Disallow: /` for `anthropic-ai`, `CCBot`, `ChatGPT Agent` and 134 other agents | H | M | Do not build a scraping plugin against it; request API access from Arkivverket instead (histreg.no already consumes such an API) |
| 12 | `prestehistorie.no`, the richest *prestegjeld* history, is copyright-only | H | M | Use as a research reference only; never as import input |
| 13 | The one historical parish polygon set derives from a Digitalarkivet tool that now 404s; provenance is unverifiable and its CRS is undocumented | M | M | Import only with explicit low `PositionalAccuracy`; pin the commit; record the dead upstream in the citation |
| 14 | Soknegrenser — the only official ecclesiastical geometry — is SOSI/EPSG:25833 only, and no pure-Rust SOSI crate exists | H | M | Write a minimal reader (bounded: 1,165 flater, 4 inner rings, 5 attribute paths), or ship a pre-converted GeoJSON asset refreshed annually, or ask Kartverket to add GeoJSON as its sibling dataset already has |
| 14b | SOSI's `ENHET 0.01` (centimetres) and northing-first `NØ` axis order are two easy ways to mirror or 100×-scale every boundary silently | M | H | Assert a known *sokn*'s bounding box against its `representasjonspunkt` from SSR in a test before trusting any parsed geometry |
| 15 | `kommunenummer` reuse (1806 Svolvær → Narvik) silently merges distinct units if keyed on the number alone | H | H | Key on `(number, valid_from)`; recommendation 5 |
| 16 | `net`'s 8 MiB response cap excludes the ~24 MB `atlefren` parish files and the inflated Soknegrenser payload | M | M | Civil geometry fits (4.4 MiB/year); for the ecclesiastical extras use `import-source` or a bundled asset rather than raising the cap |
| 17 | `bulk-import` world lacks `query`, so the plugin cannot check what already exists | H | M | Additive world change in recommendation 11 |
| 18 | No transactions: a crashed 14 k-command import leaves orphans with no progress marker | M | M | `ExternalId` idempotency makes a re-run safe; report progress through the `progress` capability; recommendation 5b reduces the number of partial-failure points ~10× by making each place one commit |
| 18c | **Retraction resurrection**: a researcher retracts an imported boundary as wrong, and every subsequent import re-asserts it, because ADR 0029 compares only the *live* assertion and a retracted one is not live | H | H | Treat "asserted by this authority and since retracted" as a tombstone; needs a read path over retracted assertions keyed by authority, which does not exist. **Resolve before enabling recurring import** — this destroys human judgement rather than merely adding noise, and is attributed to `Software` so it looks routine |
| 18d | Geometry re-fetch differs by ring start vertex, winding, reprojection rounding or upstream re-generalization, so exact `Eq` reports a change every run and the largest payloads in the workspace churn | M | M | Canonicalize ring start and winding before comparing; pin the `crs` parameter and dataset vintage. Concentrated in the auto-updated current layer, not the immutable per-year historical collections |
| 18b | A batched command is implemented as a fused event, collapsing several claims under one `AssertionId` | M | H | ADR 0021 §1 forbids it and gives the reasoning; keep one event with its own `AssertionId` per assertion and batch only at the command layer |
| 19 | `PlaceType::Custom` leaks raw Norwegian strings into a localized UI | H | M | Recommendation 7 for the three real levels; record the `Custom` tail as an explicit ADR 0003 §14 exception |
| 20 | `list_places` has no `LIMIT`/`OFFSET`, so the Geography screen deserializes every place | M | H | Existing filed items (`ListPane` paging, viewport-scoped loading) become load-bearing at this scale |
| 21 | Postgres users get an outright broken place screen today | M | H | Recommendation 9 as a stopgap, 9b(i) as the fix — non-spatial, needs no PostGIS |
| 21c | Containment answers diverge between the Rust `geo` predicate and PostGIS `ST_Contains` at boundary-touching points and shared edges, so parish membership could change with the database engine | M | H | Specify the read-model contract once; gate 9b(ii) on a shared conformance corpus run against both backends |
| 21d | PostGIS is a server-side dependency: `CREATE EXTENSION postgis` needs privileges the app may not hold, and it is absent from some minimal/managed Postgres offerings | M | M | Detect the extension at workspace open and fail with an actionable message rather than at first spatial query; keep SQLite the default per ADR 0002 |
| 20b | A geometric containment result gets stored as hierarchy without an assertion behind it, bypassing the evidence layer | M | H | Containment is evidence only: it must surface as an `AssertEnclosedBy` with rationale, dataset citation and low confidence, never as a projection-time inference (§13) |
| 20c | A farm straddling two parishes cannot be represented — `enclosed_by_as_of` returns one parent per relation even after the §1 fix | L | M | Prefer the *matrikkel*'s recorded `Sogn`/`PRESTEGJELD` text over derived containment; record a decision on same-relation multi-parenthood rather than leaving it implicit |
| 21b | KLASS CSV output is ISO-8859-1 and not valid UTF-8, so `String::from_utf8` fails on the first `ø` | M | L | Use JSON (UTF-8), or decode Latin-1 explicitly with `encoding_rs` |
| 22 | ETRS89/EUREF89 treated as WGS84 carries a growing (~1–3 cm/yr since 1989) offset that now exceeds the 0.11 m storage quantum | H | L | Acceptable for map pins, but state it in the ADR and make the identity datum step explicit in the proj string |
| 23 | Dataset schema drift or withdrawal between runs (Kartverket's harvest record was last updated 2020-07-10) | M | M | Pin the dataset version in the citation; checksum the input; never re-derive silently |
| 24 | Third-party GeoJSON repositories (`robhop`, `atlefren`) can move or change | M | M | Pin a commit SHA and record it; prefer the official NLOD series where coverage allows |
| 25 | Coats of arms can be attached to places but never exported (`place-dto` has no `media`) | L | L | Accepted deferral; know it before importing 356 crest images |
| 26 | `gårdsnummer` renumbering at mergers breaks farm-level linkage across eras | M | M | Treat gnr as a within-era key; never as a durable identifier |

## Decisions this research would reopen

Two of the recorded *Decided — no action needed* items, and one only partially:

- **"`LineString` / `Multi*` geometry variants"** — reopen the **`Multi*`** half only. This is not an
  argument against the decision: ADR 0024 §Out of scope explicitly reserved `Multi*` for "additive
  follow-ups when a concrete need appears", and roughly 200+ of 357 Norwegian municipalities being
  topologically disconnected is that need. `LineString` stays closed; nothing here wants it.
- **"Snapshotting is decided, not deferred-open"** — reopen **only if** geometry finer than N500 is
  ever imported. The decision rests on a measurement of 105.8 µs/event taken on small payloads;
  megabyte geometry events invalidate the premise, not the reasoning. At the recommended N500 the
  decision holds and should stay closed.
- **"Explicit `[from, until)` validity intervals"** — **do not reopen**, even though this research
  found the gap the decision anticipated. Cessation needs an *end to a place's lifetime*, not an
  interval on every assertion; a read-side `existed_as_of` plus (for the no-successor case) one
  additive `DissolutionAsserted` event closes it at a fraction of the cost. Recorded here so the next
  reader does not mistake §2 for a reason to reopen it.

Also worth noting, though not a *Decided* item: **"External ids have no frontend entry point"**
remains correct and is compatible with widening `ExternalId` to Place — but it is precisely *why*
`code` must be fixed separately, since a *kommunenummer* is something a Norwegian researcher expects
to see and search on.

## Open questions for the maintainer

1. **Is unauthenticated `kart.ssb.no` access something we can depend on?** Its OpenAPI spec declares
   global `Bearer` security, yet every read used here works with no credentials, and the endpoint that
   would state the terms (`/v1/legal/terms`) requires auth. The data is NLOD 1.0 per Geonorge, so the
   licence is not in doubt — the *access contract* is. Worth one email to SSB before shipping a plugin
   that depends on it. (Kartverket's own historical coverage floor, previously the open question here, is
   now academic: its service exposes nothing before 2017, and SSB covers 1838–2019.)
2. **Is reference data separable from user data?** Importing ~1,100 municipalities and ~1,165
   parishes into the same place list as a user's dozen family farms is a product decision, not a
   technical one. Options: a Tag convention (works today, zero model change); a workspace-level
   filter; or a genuine reference-data partition (large). This should be settled *before* P3, because
   it is much harder to retrofit.
3. **Bundle the data, or require a download?** Every dataset we would want is redistributable with
   attribution, so the blocker is bundle size and the absence of an attribution surface — not
   licence. Worth deciding alongside question 2.
4. **How faithful must the identity model be?** The repo's rule yields five aggregates for
   Troms/Finnmark 1919–2024 and matches Norwegian practice exactly, but a user searching "Troms" will
   get three hits. Accept, or add a merged presentation layer?
5. **Generalization level.** N500 is the recommendation (§9). Confirm, or accept N250 with the
   performance consequences and a re-measured snapshotting decision.
6. **Is the ecclesiastical entity-only import worth building** given that historical *prestegjeld*
   geometry is unavailable and *sokn* history starts only in 2020? This research says yes — "which
   *prestegjeld* held this farm in 1865" is a more common genealogical question than any polygon — but
   it is a scope call.
7. **Should a new dataset vintage asserting an unchanged value emit a corroborating assertion, or
   nothing?** These pull against each other, and the answer is a policy call rather than a technical one.
   Strict "no new events if nothing changed" says emit nothing. But this document's plugin design records
   each dataset version as a `Source` plus `Citation`, so the 2027 vintage of SSB agreeing with the 2026
   vintage is genuinely *new evidence for the same conclusion* — and suppressing it discards provenance a
   researcher might value ("two independent vintages agree"). Recommendation: **suppress by default**,
   treating the value plus its citation set plus confidence as the comparison key, so a re-run of the same
   vintage is silent while a genuinely new source can still corroborate if that is later judged worth
   recording. Worth deciding explicitly, because retrofitting corroboration later means the log is
   silent about vintages that were already checked.
8. **Is PostGIS required for every Postgres workspace, or optional?** The distinction decides whether
   spatial reads may *assume* it. If required, the app should verify the extension when opening a
   workspace and refuse with a clear message, and the requirement belongs in an ADR, since ADR 0002 gates
   Postgres by Cargo feature and is silent on server extensions. If optional, then `places_in_bbox` and
   `places_containing` need a non-PostGIS Postgres fallback — most likely the same bbox-plus-Rust-refine
   path SQLite uses — which is a third implementation to keep in agreement. Recommendation: **require it**,
   and keep SQLite as the default engine so the requirement only binds deployments that opt into Postgres.
9. **Three enquiries worth making, each cheap and each potentially decisive.** None can be settled by
   further searching:
   - **Sikt** — a 1982 NSD paper asserts "coordinate matrices for all commune boundaries in Norway
     since 1800 have been computerized", and sikt.no advertises "historiske kommunekart", yet the
     Kommunedatabasen API has zero geometry endpoints. If that lineage survives as a downloadable
     product it would answer open question 1 outright and push civil geometry back a century. Highest-
     value open lead in this research.
   - **Arkivverket** — a Digitalarkivet catalogue API exists (histreg.no consumes it) but is
     partner-only, and scraping is excluded by their `robots.txt`. Access would give the complete
     *prestegjeld*/*sokn* entity tree with identifiers, which is most of the ecclesiastical import.
   - **HistLab (UiT)** — written clarification of the 1838/1886 *matrikkel* licence, and whether the
     1801 parish boundary maps referenced in Thorvaldsen & Holden (2023) can be released.
   - **Norsk Regnesentral, about HISTGEO / Histabas** — the highest-value lead of all, and it lines up
     almost exactly with this document's §1 recommendation. NFR project 322231 (2022–2029, NR with
     RHD/UiT) is building three linked databases, of which **Histabas** is described as
     *"kartrelaterte koordinatpolygoner for de administrative områdene"* covering **secular,
     ecclesiastical *and* judicial** administrative areas **since ~1660** — i.e. precisely the parallel
     non-nesting hierarchies that motivate `PlaceRef.relation`, with the geometry this research
     otherwise could not find for the ecclesiastical side. Verified as a funded project;
     **UNVERIFIED as an obtainable artifact** — delivery is to researchers on application and it is
     still under construction. Worth asking about timing before investing in a bottom-up
     reconstruction that this project may supersede.
