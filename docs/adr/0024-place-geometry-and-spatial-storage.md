# 24. Place geometry: typed shapes, spatial storage, and interchange

- **Status:** Proposed
- **Date:** 2026-07-17

## Context

A `Place` today stores a single centre point — `GeoCoordinates { latitude, longitude }` over
`Microdegrees` (a scaled `i32`, chosen so the value keeps `Eq` and a byte-stable serialization;
`crates/genealogy-core/src/geo.rs`), asserted last-writer-wins and **undated** via
`PlaceCommand::AssertCoordinates` → `PlaceEventBody::CoordinatesAsserted` (data-model §7,
`GeoCoordinates` at `docs/data-model.md:262`; Place row at `:217`).

A point is not enough to represent a place accurately. Most places are **areas** — a parish, county,
state, or country has an extent, not a coordinate — and a building or burial plot is a **point**.
Boundaries also **move over time**: a parish's 1801 extent differs from its 1900 extent, mirroring the
dated enclosure links `PlaceRef { place_id, date }` the model already carries (`place_ref.rs`). The
GEDCOM `PLAC.MAP` / Gramps place-coordinate round-trip is an explicit deferred gap
(`docs/data-model.md:1015`, the research-rigor round-trip list), and there is no spatial query surface for a
future geography view (ADR 0025).

This ADR fixes **how geometry is modelled in the event log, indexed in the projection, and exchanged
across the import/export boundary**. It does not restate the event-sourcing contract (ADR 0004), the
projection/read-model schema (ADR 0009), or the import/export contract (ADR 0013); it extends them for
place geometry. The geography *view* and its pluggable map provider are ADR 0025.

## Decision

1. **The event log carries a typed `PlaceGeometry`, not an encoded blob.** A new value object
   `PlaceGeometry` over the existing `Microdegrees` integer coordinate, serialised as serde
   internally-tagged JSON like every other event payload (ADR 0002 / 0004 §4):
   - First cut: `Point(GeoCoordinates)` and `Polygon` (an exterior ring plus optional holes, each ring
     a `Vec<GeoCoordinates>`). `Point` subsumes today's `GeoCoordinates`.
   - `LineString` (tracks, migration routes, rivers) and the `Multi*` variants are **additive later**
     (YAGNI) — the enum grows append-only, so historical events stay decodable.
   Raw **GeoJSON / GML / WKB are never stored in an event**: the event log is the assertion layer and
   holds typed domain claims; those formats are boundary encodings only (points 3–4). Keeping the
   integer `Microdegrees` representation (not floats) preserves the `Eq` + byte-stable-serialization
   property the value object was designed for.

2. **Geometry assertions are dated and accumulate.** A new
   `PlaceCommand::AssertGeometry` → `PlaceEventBody::GeometryAsserted { place_id, geometry, date? }`
   carries an optional `GenealogicalDate`, mirroring `PlaceRef`. Dated boundaries **accumulate** (a
   place holds many geometry assertions over time) rather than last-writer-wins, so the 1801 and 1900
   boundaries coexist and the right one is selected by date. `CoordinatesAsserted` becomes the
   undated `Point` case, superseded additively (old events still fold). Each assertion stays an
   `Attributed<Asserted<PlaceGeometry>>` — it carries its `AssertionId`, confidence, and citations,
   and is correctable by `AssertionId` (ADR 0004 §2, ADR 0020/0021) like any other claim.

3. **The projection materialises geometry as WKB behind a SQLite R\*Tree index.** In `genealogy-db`,
   the place read model stores each geometry as **Well-Known Binary** (compact; the GeoPackage
   geometry encoding) and maintains a **SQLite R\*Tree** virtual table on its bounding box, so a
   geography view can query `places_in_bbox(min_lat, min_lon, max_lat, max_lon)` without scanning
   every place. SQLite ships the R\*Tree module, so no extension is required; the layout is
   GeoPackage-compatible (WKB blob + R\*Tree is exactly what a `.gpkg` uses). The Postgres mirror
   (ADR 0002, feature-gated) uses its native geometry + GiST index later. This is a derived index,
   rebuildable from the log (ADR 0010).

4. **GeoJSON is the import/export interchange.** Import and export (ADR 0013) read and write geometry
   as **GeoJSON** (RFC 7946): JSON-native, human-readable, WGS84 lon/lat, and the widest tooling.
   This closes the deferred GEDCOM `PLAC.MAP` / Gramps place-coordinate gap. **GML** is not adopted —
   verbose XML with no benefit here; a specific future source that only speaks GML would convert at
   its own plugin boundary.

5. **Geometry work uses the permissive GeoRust crates.** `geo-types` (primitives), `geojson`,
   `wkb`, and `geozero` (convert `geo-types` ↔ WKB ↔ GeoJSON, including GeoPackage WKB), with
   `rstar` / `geo` available for in-memory indexing and containment. All are `MIT`/`Apache-2.0`,
   keeping the workspace's `MIT OR Apache-2.0` license clean (`cargo deny check` enforces this).
   `genealogy-core` depends only on the pure primitive/serde parts; WKB and the index live in
   `genealogy-db`; GeoJSON lives at the import/export and view boundaries.

## Consequences

### Positive

- A place can be a point *or* an area, and area boundaries are dated — the model finally represents
  parishes/counties/states accurately, and can pick the boundary valid at an event's date.
- The projection gains a real spatial query surface (bounding-box viewport queries) that ADR 0025's
  geography view builds on, with an interoperable GeoPackage-compatible layout.
- The GEDCOM/Gramps coordinate round-trip gap closes; geometry exports as standard GeoJSON.
- The event log stays typed, self-contained, and additive; no encoded blobs leak into the assertion
  layer, and the integer-coordinate `Eq`/byte-stability property is preserved.

### Negative / costs

- A new value object, command, and event across the Place aggregate (core), the projection + R\*Tree
  (db), the DTO/view-model chain (app/ui), and both format crates (import/export) — a broad change.
- Accumulating dated geometries needs a date-selection rule wherever a single geometry is shown
  (shares the logic the transitive-hierarchy date-aware walk needs).
- Adds the GeoRust dependency family to the build and license graph.

## Out of scope

- **The geography view, interactive geometry editing, and the pluggable map provider** — ADR 0025.
- **`LineString` / `Multi*` geometry variants** — additive follow-ups when a concrete need appears.
- **Coordinate reference systems other than WGS84** and on-the-fly reprojection — WGS84 lon/lat only.
- **Widening `ExternalId` and other deferred Place fields** — unrelated (data-model §17).

## References

- ADR 0002 / 0004 — cqrs-es storage; the pure `decide()` path, self-contained versioned JSON events,
  `AssertionId` corrections.
- ADR 0009 / 0010 — read-model/projection schema; projections are derived and rebuildable from the log.
- ADR 0013 — the import/export contract this geometry interchange rides on.
- ADR 0020 / 0021 — assertion granularity and evidence-in-the-envelope (`Attributed<Asserted<T>>`).
- ADR 0025 — geography view and pluggable map provider (consumes this geometry + spatial index).
- `docs/data-model.md` §7 — `Place`, `GeoCoordinates`, `PlaceName`/`PlaceRef` (dated); §17 deferred
  work; `:1015` the deferred `PLAC.MAP` gap.
