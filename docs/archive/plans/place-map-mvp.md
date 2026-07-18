# Plan — Place map MVP (read-only point)

- **Status:** Proposed
- **Roadmap home:** Phase 6 (near-term; before the configuration split)
- **Mockup:** the **Map** tab of [`../../mockups/place.html`](../../mockups/place.html)
- **ADR:** none — deliberately kept small (see *Non-goals*)

## Context

A `Place` already stores a single point coordinate (`GeoCoordinates { latitude, longitude }`,
`crates/genealogy-core/src/geo.rs`), but the UI only shows it as two text fields
(`crates/genealogy-ui-dioxus/src/screens/place.rs`, `place_coordinate_fields`). Users cannot *see*
where a place is. This MVP adds the smallest useful visual: a **read-only map with one marker** at the
existing coordinate. It ships value early and de-risks embedding a JS map library in the WebKitGTK
webview before the much larger geography phase ([`places-geography-temporal.md`](../../plans/places-geography-temporal.md))
takes it on.

## Goal

On the Place screen, show the place's point coordinate on a small map (one marker, pan/zoom), with a
clean empty state when the place has no coordinate. Read-only — the map writes nothing.

## Non-goals (deferred to Phase 9)

Polygons/areas, boundaries over time, editing/drawing from the map, event pins, a time slider,
provider choice/config, geocoding, offline tiles, and any change to the event log or geometry model.
No new ADR: the only architectural novelty is that the app makes an **outbound tile request** — noted
below, not gated.

## Approach

- **Library:** **Leaflet** (raster tiles) — smallest option, one marker + attribution is a few lines.
- **Provider:** a single hardcoded **OpenStreetMap** raster tile source
  (`https://tile.openstreetmap.org/{z}/{x}/{y}.png`), with the required
  `© OpenStreetMap contributors` attribution always visible and a descriptive `User-Agent`
  (OSM tile policy). No provider selection — that is Phase 9.
- **View-model (framework-free, `genealogy-ui`):** add a small `MapPointVm { lat, lon, label }`
  option to `PlaceDetail` (`crates/genealogy-ui/src/view_model/place.rs`), derived from the existing
  `coordinates` the DTO already carries. No framework types (ADR 0008).
- **Renderer (`genealogy-ui-dioxus`):** a `PlaceMap` component beside `place_coordinate_fields`
  (`crates/genealogy-ui-dioxus/src/screens/place.rs`) that mounts Leaflet in the webview and drops one
  marker; renders the empty state when `MapPointVm` is `None`. Leaflet's JS/CSS are bundled as local
  assets (mirroring how the mockup skin is embedded via `include_str!`) so the app does not fetch the
  library over the network — only the tiles.
- **Localization:** the "No coordinates yet" copy and the Map tab label are Fluent message IDs
  (ADR 0003); OSM attribution is a proper-noun string shown verbatim.

## Files

- `crates/genealogy-ui/src/view_model/place.rs` — add `MapPointVm`, populate from `coordinates`.
- `crates/genealogy-ui-dioxus/src/screens/place.rs` — `PlaceMap` component + a "Map" tab/card.
- `crates/genealogy-ui-dioxus/src/assets/` — bundled Leaflet JS + CSS (local, offline-safe).
- `crates/genealogy-ui/i18n/{en,no}/genealogy-ui.ftl` — new message IDs (keep the `no` catalogue
  complete; `cargo xtask i18n-check`).

## Privacy / network note

The map fetches raster tiles from an external server the first time a located place is viewed. This is
the app's **first outbound network request** (plugins are still `net`-denied). The MVP: single fixed
provider, attribution shown, tiles cached per HTTP headers, no telemetry. A user-facing
offline/opt-out control and provider choice are Phase 9 (presentation-config, after the config split).

## Verification

- `cargo run -p genealogy-ui-dioxus`; open a place **with** coordinates → marker renders at the point;
  open a place **without** → empty state.
- View-model unit/SSR test that `MapPointVm` is `Some` iff `coordinates` is set (mirror existing
  `genealogy-ui-dioxus/tests` patterns).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo xtask i18n-check`,
  `cargo xtask css-check` all clean.
