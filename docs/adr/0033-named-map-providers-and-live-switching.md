# 33. Named map providers and live switching

- **Status:** Accepted
- **Date:** 2026-08-11

## Context

Issue #283: the Geography toolbar's provider select was a control that lied. Picking a value wrote
`[map]` config for `osm-raster` and was an explicit no-op for `maplibre-style`/`google`
(`geography.rs`, pre-fix); nothing ever repainted, because the mount script added one raster source
once inside `map.on('load')` and no code ever called `setStyle`; and the provider `use_memo` had no
reactive dependency, so it was read once at mount regardless.

A second, deeper defect the issue did not name: both screens reached config through
`FileConfigStore::for_workspace(dir)`, whose `config_path` is `None`, while `[map]` lives in the
**global** config (ADR 0005). `store_map_config`/`load_map_config` therefore always returned `Err`
— the write was silently dropped (`let _ =`), and a hand-edited `[map]` section was never read either
(it logged and fell back to the built-in default). The config round-trip had never actually happened.

A third: the surface's credit always fell back to OSM's own URL *and* attribution for both vector
kinds (deliberately, per the pre-fix `rendered_credit` doc comment) — a configured `maplibre-style`
or `google` provider silently rendered OpenStreetMap tiles under someone else's name.

ADR 0025 §3 named the provider as a `[map]` descriptor in client/presentation config, and named
`osm-raster` / `maplibre-style` / `google` as the three kinds; it did not specify how more than one
provider could be configured at once, and its MVP-era code never resolved a kind into anything a
renderer could mount other than the one hardcoded raster source. This ADR fixes both: the config
shape, and how a resolved provider actually reaches the running map.

## Decision

1. **`[map]` holds named providers, mirroring `[ai.providers]` (ADR 0017 §4).** `MapConfig` becomes
   `{ provider: Option<String>, providers: BTreeMap<String, MapProvider>, net_allowlist }` — an active
   choice by name plus the full inventory, so switching away from a provider and back never loses its
   style URL or API-key env name. The reserved empty-string key resolves to the built-in OSM default
   (`BUILT_IN_MAP_PROVIDER`); `MapConfig::resolve(name)` mirrors `AiConfig::resolve` except the
   fallback is that built-in provider, never an error, and `MapConfig::choices()` returns exactly what
   the toolbar select may offer — the built-in default first, then configured providers in name order
   — so the control can never show an option it cannot also resolve. No back-compat shim for the old
   `provider = { kind = … }` shape (repo convention: workspaces/config are disposable).
2. **Entering a provider's parameters (style URL, API-key env name) is config-file-only in this
   change; no toolbar sub-form.** The select only ever chooses among what `[map.providers.*]` already
   declares. A follow-up may add an in-app form; recorded as a backlog item (`docs/issues.md`), not
   built now.
3. **`genealogy-app` resolves a `MapProvider` into a `MapSource` — the frontend-neutral thing a
   renderer actually mounts — and this is where map network I/O lives (ADR 0008).** `MapSource` is a
   `MapBasemap` (`Raster { tile_url, tile_size, max_zoom }` or `Style { style_url }`) plus the
   attribution to display. `resolve_map_source`:
   - `osm-raster` → `Raster` verbatim, no I/O;
   - `maplibre-style` → `Style`, substituting `{key}` in the style URL from the named env var
     (`AppError::Config` if the env var naming is inconsistent with the URL — set with no `{key}`
     placeholder, or unset);
   - `google` → mints a Map Tiles API session (`POST .../v1/createSession`), builds the
     `.../v1/2dtiles/{z}/{x}/{y}?session=…&key=…` template, and reports Google's own `tileWidth` and
     a fixed `maxZoom` of 22. The session is cached in memory per API key (a token is valid two
     weeks; one mint per process run is the target, not one per provider switch).
   `genealogy-ui-dioxus` never fetches; it calls `resolve_map_source`/`refresh_map_attribution`
   through `genealogy-app` and renders whatever comes back.
4. **A provider switch calls `setStyle`, it does not remount.** The mount script now defines
   `el.__geoInstall(map)` once — the marker/event/draft `GeoJSON` sources and layers, plus (only for a
   `Raster` basemap) the tile source itself, keyed off a runtime descriptor stashed on
   `el.__geoBasemap` — and calls it from `map.on('load')`. `apply_map_source` re-points
   `el.__geoBasemap`, calls `map.setStyle(<style url | a blank style-8 document>, { diff: false })`
   (unconditional, so `style.load` always fires even for a same-looking style), and re-runs the same
   `__geoInstall` on `map.once('style.load', …)`, followed by a forced `redraw()` for the `WebKitGTK`
   compositor reason `push_draft_script` already documents. One definition serves both the initial
   mount and every later switch, so the overlay-layer logic cannot drift between the two paths.
5. **Google's Map Tiles terms require a live, per-viewport attribution string, which the map's own
   `moveend` feeds.** The mount script's `moveend` handler reports the settled camera
   (`MapMessage::Moved`); the Geography screen forwards it to `refresh_map_attribution`, which is a
   no-op for every provider but Google and otherwise fetches `.../tile/v1/viewport?...` for the
   current camera's `copyright` string. The renderer's `attribution` is therefore a signal independent
   of the resolved `MapSource.attribution` — seeded from it at mount/switch, then updated live — not a
   value baked into the mount closure once.
6. **A resolve or persist failure keeps the previous provider.** The toolbar's `onchange` resolves the
   candidate and persists the config change before moving the active-provider signal or calling
   `apply_map_source`; on either failure the signal never moves, a toast reports the technical detail,
   and a generation counter forces the `<select>` to remount so its displayed value is pulled back —
   the native control has already shown the picked option by the time the failure is known.

## Rationale

- **Named providers over a single descriptor** is what "config file only" needs to be coherent at
  all: without an inventory, switching to the built-in default and back would erase whatever the
  configured provider's URL/env name had been.
- **Resolution in the app layer, not the renderer**, keeps ADR 0008's one-way dependency intact and
  gives the Google adapter one place to hold a raw API key and its session token — never passed to
  `genealogy-ui-dioxus`, so a future renderer for a second framework never needs it either.
- **One `__geoInstall`, called from both the initial `load` and every later switch**, is what makes
  "switch without remount" safe: the alternative (a second, switch-specific copy of the overlay-layer
  logic) is exactly the kind of duplication that drifted before (#254's stale-credit bug was one
  symptom of the mount script hardcoding a single source).
- **`setStyle({ diff: false })`, not `diff: true`** — the default diffed reload can no-op when
  MapLibre judges the incoming style close enough to the current one, which would silently break a
  switch between two visually similar raster kinds. Unconditional reload costs one extra style parse
  per switch, negligible next to a user-initiated action.

## Consequences

### Positive

- The toolbar's provider select does what it shows: every option it offers actually renders, and
  picking one repaints the live map and survives a restart (config round-trips through the file it
  is actually stored in).
- A `MapLibre` style or Google provider now renders its own tiles under its own credit, never OSM's.
- The mount/switch script has one definition of "what a basemap needs," not two that can diverge.

### Negative / costs

- The Google adapter adds `reqwest` to `genealogy-app` and two outbound endpoints
  (`tile.googleapis.com`) with no automated test against the live service — it needs a billed key
  (`docs/issues.md` records this).
- No toolbar sub-form for entering a style URL or API key: a provider must already exist in
  `[map.providers.*]` before it can be chosen, which is a deliberate scope cut, not an oversight.
- `ai_config`'s identical `for_workspace` config-path bug (ADR 0017 §4) survives only because its
  global-config fallback happens to paper over it; `[map]`'s own version of that bug had no such
  fallback, which is why #283 was visible and `ai_config`'s twin was not. Recorded, not fixed here
  (`docs/issues.md`).

## Out of scope

- **A toolbar (or Preferences) sub-form for entering a style URL / API-key env name** — config-file
  only in this change; a real backlog item.
- **`ai_config`'s dead `for_workspace` store path** — a latent twin of #283's root cause, named above,
  fixed separately.
- **Geocoding, a `map-provider` plugin world, offline/self-hosted tiles** — ADR 0025 §4 already
  deferred these; unchanged here.

## References

- ADR 0025 — the geography view and the three provider kinds this ADR keeps; this ADR replaces its
  single-descriptor `[map]` shape and its "renderer hardcodes one raster source" mount, neither of
  which ADR 0025 itself specified beyond naming the kinds.
- ADR 0017 §4 — `[ai.providers]`, the named-provider shape `[map.providers]` mirrors, and the
  `for_workspace` config-path bug `ai_config` shares with #283's root cause.
- ADR 0008 — the one-way `genealogy-app → genealogy-ui → genealogy-ui-<framework>` dependency that
  places map network I/O (and the Google session/API key) in the app layer, never the renderer.
- ADR 0005 — the global vs. per-workspace config split; `[map]` is global/client-scope, which is what
  made `for_workspace`'s missing `config_path` a silent no-op instead of a load-time error.
