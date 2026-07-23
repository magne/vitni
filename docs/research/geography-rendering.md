# Research — rendering the geography view (PR3, ADR 0025)

- **Status:** Findings from implementing ADR 0025 §1 (the framework map component).
- **Date:** 2026-07-22

## Question

ADR 0025 §1 names MapLibre GL JS as the recommended map library (Leaflet — the Phase 6 MVP's raster
library — as the fallback) but does not fix a version or the JS↔Rust interaction seam. This PR needed
both before writing code.

## MapLibre GL JS: version and packaging

- **Current stable: 5.24.0** (published 2026-04-23), BSD-3-Clause, on npm as `maplibre-gl`. Verified via
  the npm registry and the project's own `docs/index.md`.
- **v6 is pre-release** (`6.0.0-22` as of this research, first pre-release tagged 2026-06-09) and is a
  breaking packaging change: v6 ships **ESM-only** (`maplibre-gl.mjs`), drops the classic UMD
  `maplibre-gl.js` global-script build, and requires an explicit worker-URL setup
  (`setWorkerUrl`) for non-bundler consumption — deliberately more work for a vendored, no-bundler
  desktop webview than the problem justifies.
- **Decision: vendor v5.24.0's UMD build** (`dist/maplibre-gl.js` + `dist/maplibre-gl.css`), fetched from
  `unpkg.com/maplibre-gl@5.24.0/dist/`. This is a `<script>`-global build (`window.maplibregl`) — the
  same shape Leaflet's vendored `assets/leaflet.js` already is, so it drops into the existing
  `scripts_head()` `<head>`-injection pattern (`crates/genealogy-ui-dioxus/src/app.rs`) with **zero**
  new build tooling (no bundler, no worker-URL wiring, no ESM `import` at runtime).
- Vendored files: `crates/genealogy-ui-dioxus/src/assets/maplibre-gl.js` (~1.03 MB) and
  `maplibre-gl.css` (~70 KB). License is BSD-3-Clause (`dist/LICENSE.txt`, not vendored verbatim but
  cited in the `app.rs` doc comment, matching how Leaflet's BSD-2-Clause notice is handled).
- Only tile/style requests are fetched over the network at runtime, per each provider's own policy —
  the library itself is fully offline once loaded, exactly like the Leaflet MVP.
- `cargo deny check`: unaffected — MapLibre GL JS is a vendored JS asset, not a Rust crate, so it never
  enters the Cargo dependency graph or license graph.

## The JS↔Rust interaction seam

The map is a JS canvas library the Dioxus renderer cannot draw into directly (WebGL inside an
`<canvas>`), so every interaction crosses `document::eval`. Two shapes already exist in this codebase;
the geography view uses both, for different purposes:

1. **One-shot fire-and-forget scripts** (`init_leaflet_map` in the Phase 6 MVP, `place.rs`): call
   `document::eval(&script)`, `spawn` a task that awaits `eval.recv::<()>()` once, discard the result.
   Used here for mounting the map and for pushing updated marker/event/draft GeoJSON to the running
   map's sources whenever Rust-side state changes (`update_geography_data`/`update_geography_draft`
   in `screens/geography.rs`).
2. **A persistent JS→Rust event channel** (`watch_scroll_close` in
   `components/record_picker.rs`): a JS listener calls `dioxus.send(value)` on every event, and a
   `spawn`ed Rust task loops `while listener.recv::<T>().await.is_ok() { … }` for the component's
   lifetime — not a one-shot round trip. This is the shape the map's click handling needs: MapLibre's
   `map.on('click', e => dioxus.send(JSON.stringify([e.lngLat.lng, e.lngLat.lat])))` is armed **once**
   at mount (`init_geography_map`), and a single spawned loop turns every click into a draft
   point/vertex for as long as the screen is mounted. This is materially simpler than a one-shot
   `unproject`-per-click round trip (the alternative considered: capture a pixel coordinate via
   `element_coordinates()` on a Dioxus pointer handler, then `eval` a one-shot `map.unproject([x,y])`
   call per click) — MapLibre already resolves lng/lat for us in the click event, so there is no need
   to round-trip pixel coordinates through `unproject` at all.

Both shapes are guarded so a re-render racing the map's own async `'load'` event degrades gracefully:
`update_geography_data`/`update_geography_draft`'s scripts check `map.getSource('geo-markers')` etc.
before calling `setData`, silently no-op-ing if the style hasn't finished loading yet (the next
reactive update catches up).

## Attribution and tile policy

MapLibre's built-in `AttributionControl` is disabled (`attributionControl: false`, matching Leaflet's
`attributionControl: false` in the MVP) in favor of a static Rust-rendered `.map-attr` overlay showing
the *configured* provider's attribution string — this lets the same DOM/CSS attribution treatment work
for every provider kind (OSM raster, a MapLibre style, Google) without depending on each library's own
attribution-rendering conventions. The v1 provider defaults to OpenStreetMap raster tiles
(`tile.openstreetmap.org/{z}/{x}/{y}.png`), whose tile-usage policy requires visible attribution and
forbids bulk/offline scraping — satisfied by only ever fetching tiles the user's viewport requests,
never pre-fetching or caching beyond the browser's own cache.

## What this unblocked

With the version and seam settled, the map component (`crates/genealogy-ui-dioxus/src/screens/geography.rs`)
composes cleanly from parts this codebase already had precedent for: `onmounted` to arm the map
(`place.rs`'s Leaflet mount), a persistent `dioxus.send`/`recv` channel (`record_picker.rs`'s scroll
watcher), and GeoJSON `Value` construction via `serde_json::json!` (already a dependency). No new Rust
crate was needed for any of this.
