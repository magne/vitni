# 25. Geography view and pluggable map provider

- **Status:** Accepted
- **Date:** 2026-07-17

## Context

Places carry coordinates but there is no map. The Places screen renders coordinates as two text
fields (`crates/vitni-ui-dioxus/src/screens/place.rs`, `place_coordinate_fields`); nothing plots a
place on a map, and a place can only be located by keying decimal degrees. Gramps' `GeoView` (built on
OsmGpsMap) is the parity target: it plots pins for places and for events whose place has coordinates,
and it can create or re-position a place from the map.

Two things make this an architecture decision rather than a screen. First, **rendering a slippy map**
means drawing tiles/vector data and handling pan/zoom/interaction — the Dioxus desktop shell is a
WebKitGTK **webview**, so the natural renderer is a JS map library in that webview, which must sit
below the framework-agnostic presentation layer (ADR 0008's one-way dependency rule). Second, **map
providers differ in licensing and capability**: the OpenStreetMap tile policy requires attribution and
forbids bulk/offline scraping; MapLibre GL JS (BSD-3) renders open vector tiles with swappable styles;
Google Maps is paid and closed. A single hard-coded provider is wrong — the provider must be
pluggable.

A **read-only, single-point** map ships first as a small near-term MVP (roadmap Phase 6,
`docs/archive/plans/place-map-mvp.md`) — Leaflet + one OpenStreetMap layer, no editing, no model change, **no
ADR**. This ADR governs the **full geography view** (roadmap Phase 9): area geometry, editing,
event pins, the time slider, and the pluggable provider.

This ADR fixes **how the geography view is rendered, how geometry is edited from the map, and how the
map provider is made pluggable**. It builds on ADR 0024 (the `PlaceGeometry` type and the SQLite
R\*Tree viewport index), ADR 0026 (dated resolution + place succession, which the time slider reads),
and respects ADR 0008 (Dioxus behind `vitni-ui`), ADR 0005/0006 (configuration scopes), and the
`net` capability (ADR 0007 §2, ADR 0011 §3; landed in the assisted-import phase).

## Decision

1. **The map is rendered by framework code behind a framework-free view-model — not a plugin.** A map
   view-model in `vitni-ui` (markers, the event-at-place pins, current viewport, selection, and a
   provider *descriptor*) carries **no framework types**; a per-framework map component in
   `vitni-ui-dioxus` embeds a JS map library in the WebKitGTK webview and binds it to that
   view-model. **MapLibre GL JS** (BSD-3, GPU vector tiles, styles swappable per provider) is the
   recommended library; Leaflet (raster) is the fallback. WASM plugins cannot render DOM/canvas and
   the plugin-UI vocabulary (ADR 0012/0022) has no map widget — so the renderer is framework code, not
   a plugin, exactly as ADR 0008 places framework types below the renderer crate.

2. **The map is an editor, not just a viewer.** The component supports setting/editing a place's
   geometry from the map: click to drop or move the point, draw and edit polygon vertices, drag to
   reposition, and create-a-place-at-this-point (Gramps GeoView parity). A map edit emits the picked
   `PlaceGeometry` (ADR 0024) back through the **existing** edit path — `PlaceEdit` /
   `PlaceChangeSetRequest` (`crates/vitni-ui/src/navigation.rs`) — so it produces the same audited
   `GeometryAsserted` event, with the same provenance envelope, as a typed-field edit. There is no
   separate "map write" path. Read-only marker/pin display is the baseline; editing layers on top.

3. **The provider is pluggable via presentation config in v1.** A declarative `[map]` descriptor in
   the **client/presentation** configuration scope (ADR 0005; the scope split is Phase 7 / ADR 0015):
   provider kind (`osm-raster` / `maplibre-style` / `google`), the tile or style URL, the attribution
   string to display, an optional API key, and an optional `net` host allowlist. Swapping providers is
   configuration, not code. Attribution is shown on the map per the provider's terms; the app does not
   bulk-download or cache tiles beyond a provider's policy.

4. **A `map-provider` plugin type is a planned follow-up, not built now.** Once the `net` capability
   lands (Phase 8 / ADR 0017, the assisted-import ADR), a WASM `map-provider` world can supply geocoding
   (place-name → coordinates, the Gramps place-completion parity) and custom tile-source descriptors
   over allowlisted outbound HTTP. It follows the established pattern (a new WIT world + `PluginRole`
   variant + `inspect` arm + deny-by-default grants; ADR 0011). It **supplies data/descriptors, never
   pixels** — rendering stays in the framework component (point 1), because rendering cannot cross the
   WASM boundary. A `Map` field/panel in the plugin-UI vocabulary (additive per ADR 0012) is part of
   the same follow-up. This ADR records the intent and the boundary so v1 does not over-build.

## Consequences

### Positive

- A geography view with place markers and event pins, plus in-map geometry editing, reaching Gramps
  GeoView parity — and locating a place by pointing instead of keying degrees.
- Provider choice is a config value, so OSM, a MapLibre style, or Google can be selected per
  deployment/user without code, honoring each provider's licensing and attribution.
- Map edits reuse the audited change-set path, so provenance and correction semantics are unchanged.
- The framework boundary (ADR 0008) is preserved; a second UI framework reuses `vitni-ui`'s map
  view-model with its own map component.

### Negative / costs

- Embedding a JS map library in the webview adds a front-end asset and a JS↔Rust interaction seam to
  maintain; interactive polygon editing is non-trivial UI.
- The v1 provider config depends on the Phase 7 configuration split (ADR 0015) for a clean client
  scope; until then it rides whatever presentation-config seam exists.
- Offline/self-hosted tiles are unsolved (OSM policy forbids scraping) — deferred.

## Out of scope

- **The geometry model, spatial index, and interchange format** — ADR 0024.
- **The dated-resolution rule and place succession (merge/split)** — ADR 0026.
- **The read-only single-point MVP** — `docs/archive/plans/place-map-mvp.md` (Phase 6, no ADR).
- **The `net` capability and the `map-provider` plugin world / geocoding** — ADR 0017 (Phase 8),
  named here only as the planned follow-up.
- **Offline / self-hosted tiles and tile caching** — future work.
- **Non-place map layers** (heatmaps, migration paths, DNA-match geography) — later, on top of this.

## References

- ADR 0005 / 0006 — configuration scopes and the app coordination layer the `[map]` descriptor lives
  in; ADR 0015 (Phase 7) splits out the client/presentation scope.
- ADR 0026 — dated-resolution rule + place succession the time slider and titles read.
- ADR 0007 §2 / ADR 0011 §3 — the deferred `net` (allowlisted `wasi:http`) capability the follow-up
  plugin needs; ADR 0011's world + deny-by-default grant pattern the plugin would follow.
- ADR 0008 — Dioxus behind the framework-agnostic `vitni-ui`; the one-way dependency rule that
  keeps the map library below the presentation layer.
- ADR 0012 / 0022 — the plugin-UI vocabulary a future `Map` widget extends additively.
- ADR 0024 — `PlaceGeometry` and the SQLite R\*Tree viewport index this view renders and edits.
