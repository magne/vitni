# Genealogy Issues

A prioritized backlog: quick wins (bugs, then ease-of-use) first, then an unscheduled UI/app
backlog, then roadmap-phase work ordered exactly as [`roadmap.md`](roadmap.md) sequences it. The
roadmap remains the source of truth for phase detail — the phase sections below are short summaries
that link back to it.

## Ease of use

- **Quit / close-tab keys.** `Ctrl+Q` to quit the application; `Ctrl+W` to close the current tab
  (entity).
- **Customizable keyboard shortcuts** as user/client (presentation) configuration; belongs to the
  Phase 7 config split (delivered — see Completed). Would also enable a general VS Code-style *when*
  context, beyond the
  structural input guard already in place (see Completed).
- **Live list updates on create.** Creating an entity should immediately insert it into the matching
  entity list, with no manual refresh.
- **Toast notifications.** Show a toast at the bottom of the work area, auto-dismissed after a set
  time.
- **Remember the open record's tab.** Record-detail view should restore the last-shown tab while the
  record stays open, and forget it once closed.

## UI & app backlog (unscheduled — need a design or product call)

Not owned by any roadmap phase; grouped by area, roughly easy → hard.

### Lists, search & scale

- **Long-list / overflow specimen (U30)** — no tab demonstrates a long-list or overflow state;
  deferred as low-fidelity in a static mockup (the a11y real-app walkthrough covers it).
- **`ListPane` DOM virtualization** — `master_detail.rs` mounts every row (and a `MountedEvent` per
  row). Render only a scrolled window with a `store.count`-sized spacer and make the roving-focus
  `nodes` bookkeeping window-aware. If server-side windowing is chosen instead, add
  `list_view_page(table, offset, limit)` (+ a Postgres mirror) and a generated column + index on
  `$.state.human_id` in `genealogy-db`.
- **Saved searches** — nothing in the palette, list toolbars, or app layer; the 100k-scale research
  workflow argues for it. Needs a design + use-case decision.
- **Column chooser** — `list.rs` has no column state though PR3's text claims "columns". Decide
  whether to build it or amend the PR3 description.

### Places

- **Transitive place-hierarchy walk** — ✅ *done* (Phase 9, see Completed). The cycle-aware,
  date-aware primary-`PlaceRef` walk landed in `genealogy-app/src/place.rs` (`hierarchy_chain` /
  `generated_title` in `place_hierarchy.rs`), flowing through `PlaceSummary.enclosing` →
  `PlaceDetail.hierarchy`. An optional DB `place_parent` index (Gramps precedent) remains a later scale
  follow-up.

- **Map & geometry — delivered as roadmap phases.**
  - ✅ **Phase 6 — Place map MVP** *(done — see Completed)*: read-only single point; Leaflet +
    OpenStreetMap, no editing, no model change. Plan
    [`archive/plans/place-map-mvp.md`](archive/plans/place-map-mvp.md); mockup the **Map** tab of
    [`mockups/place.html`](mockups/place.html).
  - ✅ **Phase 9 — Places: geography & temporal model** *(done — see Completed)*: point/polygon
    geometry, dated boundaries, place **succession** (merge/split), the date-aware resolution rule, a
    time slider, in-map editing, and the pluggable provider. Gated by
    [ADR 0024](adr/0024-place-geometry-and-spatial-storage.md),
    [ADR 0025](adr/0025-geography-view-and-pluggable-map-provider.md), and
    [ADR 0026](adr/0026-place-succession-and-temporal-resolution.md).

### Pedigree

- **`Restriction` chart cue** on the pedigree chart.
- **Name-autocomplete pickers** for the focus / relationship inputs, which are plain `human_id` text
  fields today.

### Local import & internal cleanup

- **GUI Import-GEDCOM command** — the CLI imports; `genealogy-ui-dioxus` has no import flow. (This is
  local file import, distinct from the Phase 8 *assisted* import.)
- **Lift `prepare_import_target`** into `genealogy-app::workspace_registry` — still inline in the CLI
  (the rest of `init` already delegates).
- **Record-picker scroll-listener cleanup** — `PickerSearch::watch_scroll_close`
  (`components/record_picker.rs`) arms a `window` `scroll`/`resize` listener (via `document::eval`)
  per mount to close the floating picker on pane scroll, but never removes the JS-side listener on
  unmount, so each clear/re-search cycle leaves one inert listener behind (bounded by that, not by
  keystrokes or scroll events). Remove it on unmount, or arm it once at a higher scope.
- **`Modal`/`SidePanel` overlay follow-ups** — `Modal` (`components/layout.rs`) still has no backdrop
  scrim or `onclose` prop (harmless today since it has no callers); and neither overlay has a
  dedicated focus trap or slide-in motion beyond what the existing keyboard layer already provides.

### Media & assisted import (Phase 8 residuals)

Follow-ups left open when Phase 8 shipped (see Completed); each is scoped, none blocks the flow.

- **Assisted session survives navigation.** The assisted-import session is screen-local — navigating
  away from the `Tool::Import` wizard cancels the run (the documented cancel-on-navigate path, ADR
  0017 §5). A root-owned driver that keeps the invocation alive across navigation is a design
  follow-up.
- **`ai` decode in the confirm stage.** The `ai` capability, its `[ai]` config, and the
  `command`/`vision-api` providers ship and are tested, but the `digitalarkivet-import` plugin does
  not yet invoke them (census HTML is reliable; the church-book path has no resolvable scan). A
  user-triggered "interpret with AI" action in the confirm stage is the natural next step, especially
  for gothic church books.
- **IIIF scan resolution.** The new `nye.digitalarkivet.no` IIIF single-page viewer carries no
  permanent image, so the church-book path imports without a scan. Resolving the IIIF image belongs
  in the crate's documented `api` seam (research doc §IIIF).
- **`attach-citation-media` WIT verb.** The wizard's census-line crop lands on the *person's* media
  ref because there is no `attach-citation-media` command in the WIT. Citation-level attach is a
  follow-up (`genealogy-core`/`genealogy-app` already model citation `MediaRef`s).
- **Interactive Set/Clear region on every owner.** The interactive region viewer is wired on the
  Person screen only; the other five media owners show the read-only rich gallery. The
  `SetMediaRegion` intent and dispatch exist for all six — extending the viewer wiring is mechanical.
- **"Add file to media library" action.** The media-save dialog and the pure naming logic
  (`suggest_filename`/`slugify`) ship and are SSR-tested; the app-layer copy use-case that writes an
  external file into `media/<target>` and creates the Media record is deferred.
- **Politeness delay for `net`.** The archive `robots.txt` requests `Crawl-delay: 5`; `net` enforces
  a timeout and size cap but no inter-request delay (the assisted flow is interactive and low-volume
  today). A politeness delay is a follow-up if usage grows.

### Records & data-model

- **Repository media refs (U31)** — should Repository carry media refs (e.g. archive photos)? A
  data-model question.

### Notes

- **`remove_translation` core verb** — note-translation retract is Edit-only; there is no verb to
  remove a single translation.

### Plugin-UI vocabulary tail (ADR 0022 out-of-scope)

Repeating groups / nested forms; `List`/detail descriptions + plugin-driven navigation; per-field
validation vocabulary; plugin-prefilled field values; the `query` capability for `ui-panel`;
long-running / streaming actions; multi-panel pages.

## Phase 9 residuals (geography & temporal model)

Phase 9 shipped (see Completed); these are scoped follow-ups, none blocking:

- **`LineString` / `Multi*` geometry variants** — the model ships `Point`/`Polygon`; the other
  variants are additive-later per ADR 0024 (grow the enum append-only when a concrete need appears).
- **Explicit `[from, until)` validity intervals** — the effective-from resolution rule (ADR 0026)
  ships; add intervals additively only if gaps/overlaps prove ambiguous in real data.
- **Postgres spatial mirror** — `places_in_bbox` / `place_predecessors` / `place_successors` return
  `Unsupported` on Postgres (SQLite R\*Tree only); the native geometry + GiST index is a later
  feature-gated follow-up.
- **Viewport-scoped loading** — `show_geography` loads every place with a resolved geometry rather than
  calling `places_in_bbox` for the current viewport; wire the spatial query in when place counts grow
  (needs a Postgres fallback given the row above).
- **In-map editing depth** — true mouse-drag reposition and mid-ring vertex insertion (today: click to
  drop/move a point and click to add polygon vertices), pin-click selection on the canvas (today:
  select via the rail list), and polygon-drawn creation of a *new* place (today: point-drop creation is
  wired; polygon draws onto an existing place).
- **Provider sub-forms** — `osm-raster` is switchable from the toolbar; `maplibre-style` / `google` are
  declared in `[map]` config and round-trip but have no toolbar sub-form to collect a style URL /
  API-key-env yet.
- **Dated name/enclosure use-cases** — `add_place_name` / `assert_place_enclosed_by` don't accept a
  date param, so map/UI enclosure edits can't be dated (geometry edits already can); the map-edit
  provenance form doesn't yet default its date to the active time-slider year.
- **`map-provider` plugin world + geocoding** — the declarative provider ships; a WASM `map-provider`
  world supplying geocoding + custom tile-source descriptors over `net` is the ADR 0025 §4 follow-up
  (supplies data/descriptors, never pixels).

## Phase 10 — Research rigor & import sync

Roadmap-owned; see [`roadmap.md` Phase 10](roadmap.md#phase-10--research-rigor--import-sync). The
evidence/conclusion model's research-quality layer (data-model §17):

- **Configurable surety scheme** — the fixed five-level `Confidence` ships first; a gating ADR
  precedes making it configurable.
- **`ResearchNote`/`Argument` aggregate** — record the reasoning tying evidence to a conclusion.
- **Import true merge / sync** — re-import is additive-only today; true merge reconciles divergent
  values without overriding facts asserted after the file's export date.
- **Remaining round-trip gaps** — GEDCOM `REPO` records/pointer, `FAM`-level `SOUR`/`OBJE`/`NOTE`,
  place `MAP`/coordinates, multi-`NAME`, `FAMS`/`FAMC` back-refs, event-level witnesses, `SUBM`,
  media `FORM`, citation `CALN`, Gramps `<tagref>`, plus:
  - **RichText translator** GEDCOM/Gramps round-trip (display is already backed; no standard tag).
  - **`Address.original_text`** round-trip — the core field exists (`genealogy-core` `address.rs`);
    the format crates don't carry it yet.
  - **Gramps `<region>` export** — media-crop *import* is proven end-to-end, but the read DTOs keep
    media as `list<string>`, so gramps-**export** does not yet reproduce `<region>` from a workspace.
    Carrying the crop out needs the query-side DTO crop (PR #157).

## Phase 11 — 1.0 hardening

Roadmap-owned; see [`roadmap.md` Phase 11](roadmap.md#phase-11--10-hardening).

- Plugin **signing, trust tiers, capability-grant UX, and three-layer loading** (workspace > app-dir
  > embedded) — **ADR 0014**.
- Performance profiling.
- Packaging and distribution.

## Phase 12 — DNA breadth & depth

Roadmap-owned; see [`roadmap.md` Phase 12](roadmap.md#phase-12--dna-breadth--depth). Pulled together so
the DNA match model and its views land as one slice (data-model §17). Homes the migrated DNA gaps:

- **DNA match views** in the UI (moved from Phase 5).
- **DnaTest fields** — `account`, `date_tested`, `snp_count` are absent from `DnaTestState`.
- **DnaMatch depth** — no terminal-SNP, no fully-identical-regions (segment lineage only partially
  present via `ChromosomeSide` + `snps`).
- **DNA citation collections** — both DNA aggregates hardcode `citations: Vec::new()`; provenance is
  stubbed empty.
- **DNA payload columns (UI)** — haplogroup lineage / terminal-SNP / per-row source (VM has 2 of 6);
  shared-ancestor relationship-to-A/B + per-row confidence/source (2 of 5).
- **DNA depth (research):** Y/mtDNA markers, haplogroup detail, triangulation groups.

## Phase 13 — Beyond 1.0: server + web

Roadmap-owned; see [`roadmap.md` Phase 13](roadmap.md#phase-13--beyond-10-server-backend--web-frontend).
Backend server, web frontend, and server-connected workspaces — deliberately additive, not scheduled.
Builds on the **Phase 7** config split: the server adds the `ConfigStore` **database** backend so the
operator + client/presentation scopes persist per authenticated user, while the embedded build keeps
the file backend.

## Completed

- **Places: geography & temporal model (Phase 9).** *(Done — stacked branches
  `feat/place-geometry-storage` → `feat/place-succession-temporal` → `feat/geography-view`.)* Places
  were point-only, undated, and had no map beyond the read-only Phase 6 MVP. Delivered across three
  slices, one per gating ADR (all now **accepted**): **ADR 0024** — a typed `PlaceGeometry`
  (`Point`/`Polygon`) over integer `Microdegrees`, asserted **dated and accumulating**
  (`AssertGeometry`/`GeometryAsserted`), materialised as **WKB behind a SQLite R\*Tree** (`places_in_bbox`,
  rebuildable), with **GeoJSON** interchange closing the GEDCOM `PLAC.MAP` / Gramps `<coord>` round-trip
  (permissive GeoRust crates). **ADR 0026** — a pure **effective-from** resolver (latest dated assertion
  ≤ target, else undated/primary) shared by the generated title, the **transitive cycle-aware
  date-aware place-hierarchy walk** (the item above), and the time slider; plus
  `AssertSuccession`/`SuccessionAsserted { from, to, kind, date }` (`Merged`/`Split`/`Absorbed`/
  `Elevated`/`Renamed`) projected as a symmetric predecessor/successor relation with the aggregate-tax
  check (a rename stays a dated `PlaceName` on the same aggregate). **ADR 0025** — a framework-free
  `GeographyVm` and a vendored **MapLibre GL JS 5.24.0** component with a persistent `document::eval`
  click-stream; in-map editing emits geometry through the **existing** `PlaceEdit`/`PlaceChangeSetRequest`
  change-set path (no separate map-write); a time slider resolving as-of a year; and a declarative
  `[map]` client/presentation-config provider descriptor. Research:
  [`research/geography-rendering.md`](research/geography-rendering.md); plan (archivable):
  [`plans/places-geography-temporal.md`](plans/places-geography-temporal.md); mockup
  [`mockups/geography.html`](mockups/geography.html). Scoped residuals tracked above under *Phase 9
  residuals*.
- **Assisted import & external search — Digitalarkivet (Phase 8).** *(Done — branches/PRs
  #153–#160.)* There was no way to search an online archive and turn a found record into reviewable
  assertions. Added, under [ADR 0017](adr/0017-assisted-import-host-capabilities.md), four
  deny-by-default host capabilities (WIT `genealogy:host-api` 0.15.0 → 0.19.0): `net` (GET-only,
  HTTPS, allowlist re-checked per redirect hop, honest non-crawler User-Agent), `media-store`
  (SHA-256 checksums, path-safe writes under the workspace `media/` root, path+checksum dedup), a
  config-declared multi-provider `ai` (`command`/`vision-api`, client scope), and a suspending
  `present` carrying a typed, versioned assisted-import payload. On top: the `assisted-import` world
  with a `Confidence::Low` provenance template; a pure `genealogy-digitalarkivet` crate that parses
  census/church-book pages and resolves the scan-URL chain over verbatim fixtures (HTML-first — the
  research doc found no anonymous public API); media-crop plumbing (`MediaRef.crop`/caption through
  app, DTO, and WIT, with the Gramps `<region>` round-trip proven on import) plus a GUI crop tool,
  media viewer, and media-save dialog; a first-party `Tool::Import` wizard; and the
  `digitalarkivet-import` plugin with an idempotent end-to-end import test. Per the owner's decision
  (2026-07-19) the flow is **GUI-only** — the CLI inline-scan sketch (kitty/sixel) is dropped and
  `present` stays frontend-neutral. Scoped residuals are tracked above under *Media & assisted
  import* (session survives navigation, `ai` decode in confirm, IIIF scan resolution,
  `attach-citation-media` verb, region viewer on every owner, add-to-library action, `net` politeness
  delay) and the Gramps `<region>` export gap under Phase 10. Plan (archived):
  [`archive/plans/assisted-import.md`](archive/plans/assisted-import.md).
- **Configuration split & storage — three scopes + `ConfigStore` seam + env-var fix (Phase 7).**
  *(Done — branch `feat/config-split-storage`.)* Configuration is now grouped by owner into three
  scopes — operator, workspace-functionality, and client/presentation (ADR 0015) — behind a
  `ConfigStore` trait with a `FileConfigStore` backend over the two existing TOML files; the database
  backend (per authenticated user) is deferred to **Phase 13** with the server. Fixed the inverted
  env-var precedence: a workspace's configured `ui_language` now outranks a bare `LANGUAGE`/`LANG`,
  and `GENEALOGY_LANGUAGE` outranks both (plain env < config < `GENEALOGY_`-prefixed), via a pure
  language resolver wired into every localizer-building site (cli / ui / dioxus). The on-disk layout
  is unchanged (a clean break was permitted but no consumer needed one). Realized as a single typed
  resolver for the one env key that exists; a general `GENEALOGY_*`-over-config overlay is documented
  intent (ADR 0015). No new config fields.
- **Place map MVP — read-only point (Phase 6).** *(Done — branch `feat/place-map-mvp`.)* A Place's
  point coordinate was only shown as two text fields; there was no way to *see* where a place is. Added
  a read-only **Map** tab to the Place screen that renders one marker at the existing coordinate over
  OpenStreetMap raster tiles (Leaflet 1.9.4, vendored locally and injected into the webview `<head>` so
  only the tiles are fetched — the app's first outbound network request), with a dashed "No coordinates
  yet" empty state otherwise. A framework-free `MapPointVm` on `PlaceDetail`
  (`crates/genealogy-ui/src/view_model/place.rs`), parsed from the existing `coordinates` DTO string,
  gates the marker; the Leaflet map is initialised via `document::eval` (a no-op under SSR) with a
  `divIcon` marker and the `© OpenStreetMap contributors` attribution rendered in the server-side DOM.
  No `genealogy-core`, DTO, or event-log change; no ADR. New message ids are localized (`en` + `no`).
  Editing, polygons, boundaries-over-time, event pins, a time slider, provider choice, and geocoding
  remain **Phase 9** (the transitive place-hierarchy walk and geography items above are untouched).
- **Tall side panel overflows the viewport.** *(Fixed — branch
  `fix/side-panel-viewport-overflow`.)* A tall side panel (`edit-patterns.html`, b — a 6+ field or
  nested edit) pushed its Cancel/Save foot off-screen so the form couldn't be finished. Root cause:
  the shared `SidePanel` (`components/layout.rs`, composed by every screen's edit/attach/retract
  panel) rendered as an in-flow flex child of the `.detail` pane, which is `overflow:hidden`; the
  `.sidepanel` had no `position`, `max-height`, or overflow, so a panel taller than the pane was
  clipped with no way to reach the footer. Fixed structurally in the one shared component + shared
  CSS: `.sidepanel` is now positioned absolutely against `.detail` (`position:relative`), bounded to
  the pane's height with the head/foot pinned (`flex:none`) and only `.sp-body` scrolling
  (`flex:1; overflow-y:auto`), so the foot stays reachable; a click-away `.sidepanel-scrim` behind it
  (mirroring the record-picker/menu scrim) closes the panel and lets the record show through. The
  parallel `Modal` container got the same viewport cap (`max-height:92vh`, scrolling `.m-body`)
  pre-emptively. Because every screen composes the one `SidePanel`, all sites are fixed at once.
- **"Attach citation" dropdown won't close on blur.** *(Fixed — branch
  `fix/record-picker-floating-dropdown`.)* The reason-for-change citation picker kept its drop-down
  list of citations open after losing focus, and — a second, related bug — the list rendered in-flow,
  pushing the fields below it down rather than floating over them. Root cause: the shared record
  picker (`components/record_picker.rs`, composed by every entity selector — citation, source, place,
  person, note, DNA test/match) had no `position` on `.picker-results` and closed only on pick / clear
  / Esc. Fixed structurally, in the one shared component: the result list now floats as a
  `position:fixed` overlay, measured from the search input's on-screen box (so it escapes the
  `.detail` pane's `overflow:hidden` clip rather than being clipped or pushing siblings), and closes
  on a click-away scrim, on focus leaving the control, and on the pane scrolling — in addition to
  pick / clear / Esc. WebKitGTK's row-eat (a row click blurring the input, and closing the list,
  before the click lands) is avoided by `prevent_default` on every row/scrim/"+ New" `onmousedown`.
  Because every entity selector composes this one picker, all sites are fixed at once.
- **Global keys fire inside text controls.** *(Fixed — branch `fix/global-keys-shared-input`.)*
  Typing `g` (or any global-shortcut key) in a text field triggered the global `g`-prefix navigation.
  Root cause: the typing guard (`keep_typing_local`, which stops plain characters from bubbling to the
  shell's central key dispatcher) was opt-in per raw `<input>` and easy to omit — five fields had. Fixed
  structurally: every form control now composes one guarded behavior-core primitive
  (`components/text_input.rs` `TextInput`/`SelectInput`, `text_field.rs` `TextField`), so the guard is
  wired exactly once per element, and a `cargo xtask input-guard` lint (prek + CI) forbids raw form
  elements outside the primitives so it cannot regress. Field validation state moved into
  `genealogy-ui` view-models. A general VS Code-style *when* context was deliberately not built; it
  remains future work under customizable shortcuts (Phase 7).
