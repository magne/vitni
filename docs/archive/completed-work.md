# Completed work (archived)

- **Status:** **Extracted 2026-07-27** from [`docs/issues.md`](../issues.md), which now tracks only
  outstanding work grouped by area. Retained here as a historical record; every claim below was
  re-verified against the code at archive time.
- **Purpose (historical):** was the `## Completed` log (and the closed `## Bugs` list) of
  `docs/issues.md`.
- **Companion:** [`roadmap.md`](../roadmap.md) owns phase detail and sequencing.

## Verification note (2026-07-27)

Each entry below was checked against the code before archiving — symbol by symbol, not by trusting
the prose. All 8 bug fixes and all 7 phase/feature completions are genuinely implemented. Four
inaccuracies were found; the two that describe real gaps became outstanding items in
[`docs/issues.md`](../issues.md):

1. **The `## Bugs` preamble overstated test coverage.** It claimed "each is covered by an SSR/unit
   test". True for bugs 1, 3 and 5; **false for 2 and 4** — the marker load-race stash
   (`__geoPending`, `map_shared.rs`) and the zoom-interpolated `circle-radius` both live entirely
   inside `format!`-built JavaScript that no test inspects. The preamble below is corrected, and the
   coverage gap is now tracked under *Frontend & interaction → Geography & map*.
2. **Phase 10 / ADR 0029 was overstated.** The entry claimed `begin-import` threads "GEDCOM
   `HEAD.1 DATE` / Gramps `<header created>`". Only `plugins/gedcom-import` calls it;
   `plugins/gramps-import` never reads the parsed header date, so Gramps re-imports get no
   timestamp gating. Now tracked under *Import, export & plugins → Bulk import, export & sync*.
3. **Minor, no action.** Phase 11's "embedded project trust root" is build-time-injected and is
   `None` in a release build without a configured signing key — already covered by the outstanding
   "Real release keys not yet generated" item.
4. **Minor, historical.** Phase 6's "read-only Map tab" was accurate when written; Phase 9 later made
   that same tab writable. The entry scopes editing to Phase 9, so the two are consistent.

## Bugs (fixed)

Phase 9 map/geometry defects found in live GUI use — all fixed in
`crates/genealogy-ui-dioxus/src/screens/{map_shared,place,geography}.rs` (+ the Place VM). Bugs 1, 3
and 5 carry SSR/unit tests; bugs 2 and 4 are verified present in code but **untested** (see the
verification note above). The interactive MapLibre canvas behavior still needs a **manual webview
pass** (agents can't run libwebkit2gtk):

- ✅ Draw-tool clicks were blocked by the pointer-capture overlay — overlay removed, crosshair moved
  onto the map container so MapLibre receives the click.
- ✅ Place map showed no marker — the marker push raced MapLibre's async `load`; now stashed on the
  map element and re-applied in the `load` handler. *(No test coverage.)*
- ✅ Geography selection didn't centre the map — a `use_effect` now drives `fit_bounds` on selection.
- ✅ Marker too small / vanished when zoomed — zoom-interpolated `circle-radius` + white stroke.
  *(No test coverage.)*
- ✅ Dropping a point didn't update the shown lat/long — the Overview coordinate now derives from the
  resolved geometry point (`display_coordinates`), not only the scalar `CoordinatesAsserted`.

## Places (delivered)

- **Transitive place-hierarchy walk** — the cycle-aware, date-aware primary-`PlaceRef` walk landed in
  `genealogy-app/src/place.rs` (`hierarchy_chain` / `generated_title` in `place_hierarchy.rs`),
  flowing through `PlaceSummary.enclosing` → `PlaceDetail.hierarchy`. An optional DB `place_parent`
  index (Gramps precedent) remains a later scale follow-up, tracked under *Platform & operations →
  Performance & scale*.
- **Phase 6 — Place map MVP**: read-only single point; Leaflet + OpenStreetMap, no editing, no model
  change. Plan [`plans/place-map-mvp.md`](plans/place-map-mvp.md); mockup the **Map** tab of
  [`mockups/place.html`](../mockups/place.html).
- **Phase 9 — Places: geography & temporal model**: point/polygon geometry, dated boundaries, place
  **succession** (merge/split), the date-aware resolution rule, a time slider, in-map editing, and the
  pluggable provider. Gated by
  [ADR 0024](../adr/0024-place-geometry-and-spatial-storage.md),
  [ADR 0025](../adr/0025-geography-view-and-pluggable-map-provider.md), and
  [ADR 0026](../adr/0026-place-succession-and-temporal-resolution.md).
- **Place succession write path.** *(Done — `feat/place-succession-write`, closes #196.)*
  `assert_place_succession` (ADR 0026 §3) was reachable from neither frontend, so the Place screen could
  display a succession no user could create. Both frontends now write one:
  `PlaceEdit::AssertSuccession { human_id, from_extra, to, kind, date }` (`genealogy-ui`) dispatches to
  the use-case with the **anchor prepended** to the ceasing set — `human_id` is always one of the places
  that ceased, so `from_extra` names only the *other* ones (a merge's many side) and the app never sees
  the `SuccessionAnchorMismatch` it rejects on. In the GUI the Hierarchy tab's Succession card carries an
  "Add succession" action in **both** its populated and empty states, opening a side panel
  (`PlaceEditForm::Succession`) with a kind select over all five `SuccessionKind` variants, two
  repeatable place pickers (resulting / also-ceased, each excluding this place and the already-picked
  ids) accumulating deletable chips, a structured effective date, and the provenance block; Save is
  inert until a resulting place is picked. The CLI gained
  `genealogy place assert-succession <HUMAN_ID> --to <ID>… [--from <ID>…] --kind <KIND> [--year/--month/--day]`
  over a `SuccessionKindArg` `ValueEnum` mirror, promoting `gregorian_date` to the app's public surface.
  Editing an existing succession row is still out of scope — Retract stays the only row action.
- **Workspace-scope surety labels are writable.** *(Done — `fix/workspace-surety-write`, closes #198.)*
  `save_surety_label_overrides` (writes the manifest's `[surety]` block) had no caller: the ADR 0027
  Surety card only ever wrote the global `[workspace-defaults.surety]` table, so the per-workspace layer
  sat in `read_resolved_surety_labels`' manifest-over-global chain with no way to populate it. The card
  now carries a **scope selector** (*This workspace* / *Shared default*) governing where a Save goes —
  workspace → the new `ConfigStore::store_surety_label_overrides`, shared → the existing
  `store_workspace_default_surety` — routed by the pure `surety_save(scope, values)` helper. Switching
  scope re-seeds the five fields from that scope's *own* stored labels (`PreferencesData.surety_workspace`
  or `config.workspace_defaults.surety`), never the resolved blend, so the fields always show what a Save
  writes; blank still means "no override at this scope". Below the fields, `surety_layers` (five
  `LayerKind`s, since the resolver works per ordinal) drives one override-chain row per level, badged
  `wins` where this workspace pinned the ordinal itself. No CLI verb — config parity for the CLI stays
  tracked as #225.
- **Interactive Set/Clear region on every media owner.** *(Done — `feat/media-region-all-owners`,
  closes #199.)* The interactive crop viewer (click a gallery card → Set/Clear region → supersede the
  crop, ADR 0017 §GUI) was wired on the Person screen only; the other five owners (Family, Event,
  Place, Source, Citation) showed the read-only gallery, even though `SetMediaRegion` intent + dispatch
  already existed for all six. Wired the shared `media_tab`/`MediaTabState` helper (`screens/shared.rs`)
  on the remaining five screens' Media tab — each dispatches its own `{Family,Event,Place,Source,
  Citation}Edit::SetMediaRegion`, mirroring the Person wiring exactly. Added an SSR test per screen
  (`tests/{family,event,citation,place,source}_detail.rs`) asserting the gallery card opens the viewer,
  plus a dispatch-layer test per aggregate in `genealogy-ui/tests/dispatch_provenance.rs` proving the
  region supersede reaches the change log. Updated all six owner mockups to show the click-to-open
  affordance (Family's stays a deliberate empty-state specimen; its note now says so).

## Completed features & phases

- **Bulk import in the GUI.** *(Done — `feat/gui-bulk-import`, closes #191.)* `genealogy-ui-dioxus` had
  no local-file import flow — only the assisted online wizard — and the CLI's own target selection
  (`--new NAME PATH` / `--into NAME`, prompting when an existing target already holds persons) had no
  GUI shape. `Tool::Import` gained a front mode choice (`ImportModeSwitch`, `screens/import.rs`): **Bulk
  file import** (the new default) or **Assisted online import** (the untouched ADR 0017 flow). Bulk
  import is its own state machine (`BulkImportSession`/`BulkImportStage`, mirroring the shipped export
  wizard's `ExportSession` exactly, including the terminal-stage guard) with a Source → Running →
  Summary wizard (`screens/bulk_import.rs`): a `bulk-import`-role plugin picker, a source-file field
  parsed lexically by `ImportSourcePath` (a file-only counterpart of `ExportDestination`), and a target
  radio — an existing workspace, or a new one through the workspace-registration fields lifted out of
  Preferences into a shared `register_fields_form` (`screens/shared.rs`; `RegisterFields` and
  `database_url_field` moved with it, one `prefs-register-*` key set for both call sites). Importing
  into an *existing* non-empty workspace is confirmed first in a `Modal`, naming the workspace and its
  `list_persons` count (`services::count_workspace_persons`) — there is no `--yes` equivalent, the modal
  is the confirmation; a freshly registered workspace is always empty and never prompts. `services.rs`
  gained `start_bulk_import`/`BulkImportHandle` mirroring `start_bulk_export`; the progress sink and its
  buffer constant were renamed (`bulk_progress_sink`/`BULK_PROGRESS_BUFFER`) to reflect being shared by
  both bulk wizards rather than duplicated. The shared `NoticeStage`/`WizardNoticeTone` (renamed from
  `ExportNoticeTone`) covers the failure/cancelled terminal stages for both wizards, and the `.export-*`
  CSS classes were renamed to the neutral `.path-preview`/`.run-progress*` now that two wizards use
  them. A success into the workspace already open this session triggers `request_restart()` so its
  projections are not shown stale.
- **Quit/close-tab keys & customizable keyboard shortcuts.** *(Done — stacked branches
  `feat/quit-close-tab-keys` (PR #187) → `feat/customizable-shortcuts`, gated by
  [ADR 0030](../adr/0030-customizable-keyboard-shortcuts.md).)* `Ctrl+Q`/`Ctrl+W` did not exist, and the
  shortcut map had two independent sources of truth: `genealogy-ui`'s declarative map fed only the `?`
  help overlay (decorative), while `genealogy-ui-dioxus`'s dispatcher re-implemented the same matrix
  hardcoded, so the two could drift and no binding was user-changeable. **Quit/close-tab:** two new
  `Global` actions (`⌘Q`/`⌘W`); closing a saved tab is immediate, closing a draft (or quitting with one
  open) now arms a confirm dialog (`Modal`-based) instead of silently discarding it — the tabstrip `✕`
  and the keyboard shortcut share one `NavState::request_close_tab` path. Quit is a desktop-only
  `QuitManager` component mirroring `WindowGeometryManager`, so the SSR test target stays
  `dioxus::desktop`-free. **Customizable shortcuts:** `resolved_shortcuts(overrides)` is now the single
  map both the dispatcher and the `?` overlay read (the two-implementations problem is closed); only
  `Global`-group actions (11 total) are rebindable — within-screen and `g`-prefix keys stay fixed;
  `Modifier` became a 3-flag struct (`command`/`shift`/`alt`) so `Alt` composes; `Chord` gained a
  canonical `mod+shift+alt+key` `FromStr`/`Display`; a rejected override (unknown id, unparsable
  chord, non-`Global` action, or a conflict) is a typed error surfaced in the Preferences card, never a
  silent drop. `[shortcuts]` lives in the global `~/.config/genealogy/config.toml`, client scope only
  (mirrors `[ai]`/`[map]`/`[plugin_trust]`) — no workspace-manifest layer. A save takes effect live (a
  `Signal<ShortcutConfig>` held in shell context), no restart needed. Scoped residuals tracked in
  [`docs/issues.md`](../issues.md) under *Frontend & interaction → Keyboard & shortcuts*.
- **1.0 hardening (Phase 11).** *(Done — Gate 1 + a Gate-2 PR stack, PRs #176–#182:
  `docs/phase-11-gate-1` → `feat/plugin-bundle-signing` → `feat/plugin-layered-loading` →
  `feat/plugin-grants` → `feat/plugin-grant-ux` → `feat/perf-profiling` →
  `feat/packaging-distribution`.)* Plugins were unsigned, loaded from one flat directory with all grants
  hardcoded at the call site, the core's replay cost was unmeasured, and there was no way to ship the
  app. Delivered in three workstreams under **ADR 0014** (the last deferred plugin-system decision).
  **Plugin trust (ADR 0014):** a plugin is now a signed **bundle** (`plugin.toml` + `plugin.wasm` +
  `plugin.sig`, closing the deferred ADR 0007 §8 format) — an **ed25519** detached signature over a
  `sha2` digest of the manifest **and** the component, verified against an **embedded project trust
  root**; three **trust tiers** (*sanctioned* project key / *user-trusted* pinned publisher key in a
  client-scope store / *untrusted* unsigned — loadable, never auto-granted), a present-but-unverifiable
  signature **fails closed**; **three-layer loading** (workspace > app-dir > embedded) mirroring the i18n
  `AssetsMultiplexor`, id-keyed with a manifest↔component cross-check (inspected caps ⊆ declared,
  tree-shake-safe); **grant = declared ∩ user-approved** persisted per plugin in the workspace manifest,
  surfaced by a Dioxus plugin-panel (trust badge, per-capability toggles, pinned-publisher trust-store
  editor) and a CLI `plugin list|grant|revoke|trust …` group. Signing never widens the sandbox
  (ADR 0007 §12). **Performance profiling:** a criterion harness over a synthetic-workspace fixture
  (built through the pure `decide` → event-store path) measures projection **rebuild** (~73–106 µs/event,
  ~linear) and the hot query paths; **snapshotting is measured and not warranted** (rebuild is a
  maintenance op, per-aggregate streams stay tiny — ADR 0004's deferral stands, no follow-up ADR).
  **Packaging (Linux-first):** `cargo xtask package` assembles a signed tarball (CLI + GUI + the signed
  fleet as the embedded layer, re-verifying every signature), cargo-deb metadata for a `.deb`, and a
  tag-triggered zizmor-clean `release.yml` building tarball + `.deb` + AppImage with the release-signed
  fleet and the embedded release trust root. Research:
  [`research/plugin-signing-and-trust.md`](../research/plugin-signing-and-trust.md),
  [`research/performance-profiling.md`](../research/performance-profiling.md); plan:
  [`plans/phase-11-hardening.md`](plans/phase-11-hardening.md); release procedure:
  [`release.md`](../release.md).
- **Research rigor & import sync (Phase 10).** *(Done — stacked branches `docs/phase-10-gate-1` →
  `feat/surety-scheme-labels` → `feat/research-note-aggregate` → `feat/import-merge-sync` →
  `feat/round-trip-gaps`.)* Gate 1 delivered three gating ADRs (0027/0028/0029) + research docs; the
  Gate-2 PR stack then shipped one workstream per PR. **ADR 0027** — per-workspace surety-label overrides
  relabelling the five fixed `Confidence` ordinals (presentation-only, `id_formats` precedent), with a
  Preferences card; cardinality stays deferred. **ADR 0028** — a new 13th aggregate `ResearchNote` for
  GEDCOM X `Document(Analysis)` proof arguments, full mutable multi-subject (`SubjectRef` over
  Person/Family/Event/Place, `AddSubject`/`RemoveSubject`, non-empty invariant, `json_each` reverse
  index), CLI-first, zero change to the other twelve aggregates. **ADR 0029** — timestamp-gated import
  reconciliation reusing `AssertionSuperseded`/`occurred_at`: a `begin-import` verb threads the file's
  `HEAD.1 DATE` once per session (host-api 0.20.0) and supersedes a live single-valued assertion only
  when it is at-or-before the file's export date; first slice `Person.sex`. *(The Gramps
  `<header created>` side of that verb is parsed but never threaded — see verification note 2.)*
  **Round-trip gaps** (host-api 0.21.0, no ADR): GEDCOM `REPO`/`SOUR.REPO`, `FAM`-level `SOUR`/`OBJE`/
  `NOTE`, `FAMS`/`FAMC` back-refs, `OBJE.CAPT`, `Address.original_text` (+ a blank-`CONT` fix), Gramps
  `<tagref>`, Source `ABBR`/`<sabbrev>`, multiple `NAME` per person (fixing a silent clobber), Gramps
  `<region>` media-crop export, and source-repository `CALN`/`MEDI`; place `MAP`/coordinates,
  event-level witnesses, and media `FORM`/MIME were already shipped (ADR 0024/0019, host-api 0.10.0).
  Research under [`research/`](../research/); plan
  [`plans/phase-10-research-rigor.md`](plans/phase-10-research-rigor.md).
- **Places: geography & temporal model (Phase 9).** *(Done — stacked branches
  `feat/place-geometry-storage` → `feat/place-succession-temporal` → `feat/geography-view`.)* Places
  were point-only, undated, and had no map beyond the read-only Phase 6 MVP. Delivered across three
  slices, one per gating ADR (all now **accepted**): **ADR 0024** — a typed `PlaceGeometry`
  (`Point`/`Polygon`) over integer `Microdegrees`, asserted **dated and accumulating**
  (`AssertGeometry`/`GeometryAsserted`), materialised as **WKB behind a SQLite R\*Tree** (`places_in_bbox`,
  rebuildable), with **GeoJSON** interchange closing the GEDCOM `PLAC.MAP` / Gramps `<coord>` round-trip
  (permissive GeoRust crates). **ADR 0026** — a pure **effective-from** resolver (latest dated assertion
  ≤ target, else undated/primary) shared by the generated title, the **transitive cycle-aware
  date-aware place-hierarchy walk**, and the time slider; plus
  `AssertSuccession`/`SuccessionAsserted { from, to, kind, date }` (`Merged`/`Split`/`Absorbed`/
  `Elevated`/`Renamed`) projected as a symmetric predecessor/successor relation with the aggregate-tax
  check (a rename stays a dated `PlaceName` on the same aggregate). **ADR 0025** — a framework-free
  `GeographyVm` and a vendored **MapLibre GL JS 5.24.0** component with a persistent `document::eval`
  click-stream; in-map editing emits geometry through the **existing** `PlaceEdit`/`PlaceChangeSetRequest`
  change-set path (no separate map-write); a time slider resolving as-of a year; and a declarative
  `[map]` client/presentation-config provider descriptor. Research:
  [`research/geography-rendering.md`](../research/geography-rendering.md); plan:
  [`plans/places-geography-temporal.md`](plans/places-geography-temporal.md); mockup
  [`mockups/geography.html`](../mockups/geography.html).
- **Assisted import & external search — Digitalarkivet (Phase 8).** *(Done — branches/PRs
  #153–#160.)* There was no way to search an online archive and turn a found record into reviewable
  assertions. Added, under [ADR 0017](../adr/0017-assisted-import-host-capabilities.md), four
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
  `present` stays frontend-neutral. Plan: [`plans/assisted-import.md`](plans/assisted-import.md).
- **Configuration split & storage — three scopes + `ConfigStore` seam + env-var fix (Phase 7).**
  *(Done — branch `feat/config-split-storage`.)* Configuration is now grouped by owner into three
  scopes — operator, workspace-functionality, and client/presentation (ADR 0015) — behind a
  `ConfigStore` trait with a `FileConfigStore` backend over the two existing TOML files; the database
  backend (per authenticated user) is deferred to the post-1.0 server work with the server. Fixed the
  inverted env-var precedence: a workspace's configured `ui_language` now outranks a bare
  `LANGUAGE`/`LANG`, and `GENEALOGY_LANGUAGE` outranks both (plain env < config <
  `GENEALOGY_`-prefixed), via a pure language resolver wired into every localizer-building site
  (cli / ui / dioxus). The on-disk layout is unchanged (a clean break was permitted but no consumer
  needed one). Realized as a single typed resolver for the one env key that exists; a general
  `GENEALOGY_*`-over-config overlay is documented intent (ADR 0015). No new config fields.
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
  landed later in **Phase 9**.
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
  remains explicitly out of scope of customizable keyboard shortcuts
  ([ADR 0030](../adr/0030-customizable-keyboard-shortcuts.md) §Out of scope), unless a real need
  surfaces.
