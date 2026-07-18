# Plan — Places: geography view & temporal place model (full phase)

- **Status:** Proposed
- **Roadmap home:** Phase 9 (after Phase 8, the `net` capability)
- **Mockup:** [`../mockups/geography.html`](../mockups/geography.html)
- **Gating ADRs:** [0024](../adr/0024-place-geometry-and-spatial-storage.md) (geometry & spatial
  storage), [0025](../adr/0025-geography-view-and-pluggable-map-provider.md) (view & provider), and a
  new **[0026](../adr/0026-place-succession-and-temporal-resolution.md)** (place succession & temporal resolution)

## Context

The [MVP](place-map-mvp.md) shows a place's point on a read-only map. This phase makes places
**geographically and historically accurate**: they can be *areas*, their boundaries and jurisdictions
change over time, and their very *identity* can change (municipalities merge and split). It adds the
full geography view — editing, event pins, a time slider, and a pluggable provider with geocoding
(which needs the `net` capability from Phase 8). It sits after `net` for that reason; the model and
view work depend on nothing new and could start earlier if resequenced.

## What the model already supports (no change)

Confirmed in `genealogy-core`:

- **Name over time** — `PlaceName { value, date, language }` (`place_name.rs`) accumulates: Oslo →
  Christiania → Kristiania → Oslo is four dated name assertions on one aggregate. A **rename keeps the
  same aggregate**.
- **Enclosure / jurisdiction over time** — `PlaceRef { place_id, date }` (`place_ref.rs`) accumulates:
  a municipality transferred between counties, or a new/removed administrative level, is just
  different dated `enclosed_by` assertions.

## What this phase adds

### 1. Geometry (ADR 0024)

Typed `PlaceGeometry` over integer `Microdegrees`: extend the MVP's point with **Polygon**,
**MultiPolygon** (islands, exclaves/enclaves), and **LineString** (routes/rivers). Geometry
assertions are **dated and accumulate** (1801 boundary ≠ 1900 boundary). Projection materialises WKB
behind a **SQLite R\*Tree** for viewport queries; GeoJSON is the import/export interchange (closes the
GEDCOM `PLAC.MAP` round-trip gap).

### 2. Temporal resolution + succession (ADR 0026 — new)

- **Date-aware resolution rule.** Given a target date, pick the name / parent / geometry assertion in
  effect. Decide the validity model: treat each assertion's `date` as "effective from" and resolve by
  latest-date-≤-target (Gramps' approach), or add an explicit `[from, until)` interval. This shared
  selection logic drives the time slider, the generated title, and the transitive hierarchy walk.
- **Place succession links.** A dated, typed relationship between *different* Place aggregates for
  identity change, analogous to Person `AssociationAsserted`:
  `AssertSuccession { from: [PlaceId], to: [PlaceId], kind, date }` with
  `kind = Merged | Split | Absorbed | Renamed | Elevated`. Expresses "Aker + Kristiania → Oslo
  (1948)" (merge, many→one) and a county split (one→many). Distinct from a rename (same aggregate,
  dated `PlaceName`).

### 3. Geography view + editing (ADR 0025)

- Framework-free map view-model in `genealogy-ui` (markers, event-at-place pins, viewport, selected
  year, provider descriptor); a MapLibre GL JS component in `genealogy-ui-dioxus`.
- **Editing writes audited assertions:** dropping a point / drawing a polygon / dragging a vertex, and
  create-a-place-at-a-point, all emit the same `GeometryAsserted` (or succession/enclosure) events
  through the existing `PlaceEdit` / `PlaceChangeSetRequest` path
  (`crates/genealogy-ui/src/navigation.rs`) — with operator, confidence, and reason. Nothing bypasses
  the log.
- **Time slider** resolves names/boundaries/jurisdiction as of a year (rule from ADR 0026).
- **Transitive hierarchy walk** (the `docs/issues.md` item) lands here or just before it: cycle-aware
  primary-parent walk in `genealogy-app/src/place.rs`, date-aware.

### 4. Pluggable provider + geocoding (ADR 0025 + Phase 8 `net`)

- Provider as a declarative presentation-config descriptor (kind + tile/style URL + attribution +
  optional API key + `net` allowlist) — a client-scope setting from the Phase 7 config split.
- Geocoding (place-name → coordinates) and custom tile sources over the `net` capability; optionally a
  future `map-provider` plugin world (ADR 0025 §4). Rendering stays framework code — plugins supply
  data/descriptors, not pixels.

## Files (indicative — detailed per gating ADR)

- `genealogy-core`: `PlaceGeometry` value object; `AssertGeometry`/`GeometryAsserted`,
  `AssertSuccession`/`SuccessionAsserted` command/event pairs; date-resolution helpers
  (`place/`, `geo.rs`, `place_ref.rs`).
- `genealogy-db`: WKB projection column + SQLite R\*Tree virtual table + `places_in_bbox` query;
  succession projection.
- `genealogy-app`: DTO fields (geometry, succession, resolved-as-of-date); transitive walk in
  `place.rs`; map view-model feed.
- `genealogy-ui` / `genealogy-ui-dioxus`: map view-model + MapLibre component + draw/edit + time
  slider + provider descriptor; extend `PlaceEdit`.
- `genealogy-gedcom` / `genealogy-gramps-xml`: GeoJSON geometry round-trip (`PLAC.MAP`).
- New: `docs/adr/0026-place-succession-and-temporal-resolution.md`.

## Verification

- End-to-end via the geography view: create/edit point & polygon from the map → assertions land in the
  log with provenance; `genealogy rebuild` reproduces the spatial projection identically.
- Model: name/parent/geometry resolve correctly for a given year (Oslo/Christiania boundary case);
  a merge makes the successors reachable from the merged place and vice-versa.
- Round-trip: export → import a GeoJSON geometry with no diff (idempotent re-import).
- `cargo nextest run --workspace --all-features --all-targets`, clippy, `i18n-check`, `css-check`,
  `cargo deny check` (georust crates permissive) all clean.
