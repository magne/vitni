# Genealogy Issues

Outstanding work, grouped by **area**. [`roadmap.md`](roadmap.md) owns phase detail, sequencing, and
progress state — this file owns open bugs, scoped residuals, and unscheduled backlog, and does not
restate phases. Completed work lives in
[`archive/completed-work.md`](archive/completed-work.md).

Two conventions:

- The `###` area names are the organizing unit; each maps to one `area/*` GitHub label. A bullet being
  worked on carries its issue number (`— #142`); the rest are unfiled by design. See
  [`issue-tracking.md`](issue-tracking.md) for the labels, milestones, triage loop, and what happens to
  a bullet when the work lands. `cargo xtask issue-sync` keeps the two honest.
- [**Decided — no action needed**](#decided--no-action-needed) at the end collects deliberate
  non-tasks: things recorded so they are not re-raised or misread as unfinished. Check it before
  filing anything.

## Bugs

Two open defects, both found by code reading rather than use, and both tracked under the area they
affect: *Postgres place-detail reads fail outright* under *Performance & scale*, and *map markers label
with the first-asserted name* under *Geography & map*. The five Phase 9 map/geometry bugs are fixed and
archived; two of those fixes ship without test coverage, also tracked under *Geography & map*.

## Records & data model

### Person & Family

No open items. The area keeps its heading so `area/records/person-family` stays a live label and the
issues already filed against it keep resolving their [`#person--family`](#person--family) anchor.

### Places

Most of the model items below come from [`research/gis-norway.md`](research/gis-norway.md), which
assessed the aggregate against Norwegian administrative history; it holds the per-question reasoning,
the additivity check against ADR 0004 §4, and the data behind each claim. All of them are additive —
none needs an event rewrite — and the first three are gated by an unwritten **ADR 0031**.

- **Dated name/enclosure use-cases** — `add_place_name` / `assert_place_enclosed_by` don't accept a
  date param, so map/UI enclosure edits can't be dated (geometry edits already can); the map-edit
  provenance form doesn't yet default its date to the active time-slider year. This is also what makes
  a dated hierarchy unwritable from the app layer at all: a county transfer or a 19th-century parish
  boundary can only be expressed today by building a raw `PlaceCommand` and calling
  `Store::execute_place`, which is what `crates/genealogy-app/tests/place_temporal.rs` does and
  documents as a gap.
- **`MultiPolygon` geometry variant** — `PlaceGeometry` is `Point`/`Polygon`, so a place whose area is
  topologically disconnected cannot be expressed: 280 of 357 Norwegian municipalities have coastline and
  Kvitsøy alone is 167 islands. Asserting several same-dated `Polygon`s does not substitute —
  `geometry_as_of` returns exactly one while `geo_index` indexes every part, so the rendered boundary and
  the bbox query disagree. Reopened from *Decided* on the trigger ADR 0024 named for itself.
- **Parallel hierarchies need a relation on `PlaceRef`** — a farm sits in a *kommune* **and** a *sokn*
  **and** a *prestegjeld*, and these do not nest. `PlaceRef` has no relation kind, so every parent lands
  in one undiscriminated `Vec`, `enclosed_by_as_of` returns whichever import order put first, and
  `hierarchy_chain`/`generated_title` can emit a factually mixed chain. `PlaceSummary` has no field for
  the live enclosure set either, so a second parent is invisible to the frontend and its `AssertionId`
  never reaches the UI to be retracted.
- **Geometry accuracy and role** — `PlaceGeometryAssertion` carries only `{geometry, date}`, so nothing
  distinguishes a surveyed boundary from a reconstructed one, nor "this place **is** a point" from "this
  is the only locator we have for an area". Two same-dated geometries at different generalization levels
  are also indistinguishable, and `geometry_as_of` then picks by assertion order. `Confidence` is the
  wrong carrier — it is a surety scheme whose ordinals ADR 0027 §3 makes relabelable per workspace, so
  metres would inherit a user-chosen label.
- **Ecclesiastical `PlaceType` variants** — `Diocese`/`Deanery`/`District` are missing, which is the
  whole hierarchy above the parish and the one Norwegian genealogical sources are organised by.
  `Custom(String)` is not a substitute: `genealogy-ui/src/i18n.rs` renders it verbatim, so a raw
  Norwegian string reaches every list, breadcrumb, picker and map label, against data-model §14.
- **Dissolved places resolve forever** — each dated assertion is read as effective-*from* with no way to
  say "ceased", so a municipality dissolved in 1964 still resolves a name, a parent and a geometry as of
  today, and `show_geography` plots it overlapping its own successors. `SuccessionAsserted` does not fix
  it: nothing in the resolution path reads successions. A read-side `existed_as_of` derived from the
  succession payload closes it with no event change; only cessation with *no* successor needs an
  additive `AssertDissolution`, since `decide` rejects an empty `to`.
- **Dated, accumulating `code`** — `code` is single-valued and last-writer-wins, so a place's identifier
  history is lost: Bærum carried 0219, then 3024, then 3201 with no boundary change. Making it dated and
  accumulating like `PlaceName` is a projection reshape, free per ADR 0010.
- **`places_containing(point, as_of)`** — there is no containment query at all; `places_in_bbox` answers
  only bounding-box overlap. This is what "which parish held this farm in 1865" needs. Containment must
  surface as *evidence* for an `AssertEnclosedBy` — rationale, dataset citation, low confidence — never
  as a projection-time inference, or geometry silently overrides the recorded legal fact.
- **Optional DB `place_parent` index** — a Gramps precedent for scaling the hierarchy walk; a later
  follow-up, not needed at current volumes.

### Notes & research notes

- **`remove_translation` core verb** — note-translation retract is Edit-only; there is no verb to
  remove a single translation.

### Media

- **"Add file to media library" action.** The media-save dialog and the pure naming logic
  (`suggest_filename`/`slugify`) ship and are SSR-tested; the app-layer copy use-case that writes an
  external file into `media/<target>` and creates the Media record is deferred.

### DNA

Sequencing is roadmap-owned — see [`roadmap.md` Phase 12](roadmap.md#phase-12--dna-breadth--depth).
Remaining work is DNA **depth**, not views: the match screens shipped (`screens/dna_match.rs` has
Segments and Ancestors tabs with per-row edit/retract, covered by `tests/dna_match_detail.rs`), so the
long-standing "DNA match views in the UI" item is closed.

- **DnaTest fields** — `account`, `date_tested`, `snp_count` are absent from `DnaTestState`.
- **DnaMatch depth** — no terminal-SNP, no fully-identical-regions (segment lineage only partially
  present via `ChromosomeSide` + `snps`).
- **DNA citation collections** — both DNA aggregates hardcode `citations: Vec::new()`; provenance is
  stubbed empty.
- **DNA payload columns (UI)** — haplogroup lineage / terminal-SNP / per-row source (VM has 2 of 6);
  shared-ancestor relationship-to-A/B + per-row confidence/source (2 of 5).
- **DNA depth (research):** Y/mtDNA markers, haplogroup detail, triangulation groups.

### Cross-aggregate

- **Configurable surety-scheme *cardinality*** — ADR 0027 shipped relabeling the five fixed ordinals;
  re-scaling the scheme (GENTECH's full generality) stays deferred behind its own gating ADR, no
  consumer need demonstrated yet.
- **`run_checks` is dead code, and there is no CLI data-quality command** — its body is `list_persons`
  \+ `check_persons` (`checks.rs:53-56`), and the GUI's Data Quality view calls `check_persons` directly
  with the person list it already holds, so GUI coverage is *equivalent* — nothing is missing there.
  Either delete the wrapper, or keep it to back a `genealogy check` subcommand so quality findings are
  scriptable.
- **Data-quality checks are person-only** — both `CheckKind`s are `DeathBeforeBirth` and
  `PossibleDuplicates`. Widening checks to the other twelve aggregates is its own item.
- **Repository media refs (U31)** — should Repository carry media refs (e.g. archive photos)? A
  data-model question.

## Frontend & interaction

### Shell, tabs & notifications

- **Toast notifications.** Show a toast at the bottom of the work area, auto-dismissed after a set
  time. — #208
- **Remember the open record's tab.** Record-detail view should restore the last-shown tab while the
  record stays open, and forget it once closed. — #209
- **A `SidePanel`'s background is not `inert`.** The panel now traps and restores focus like `Modal`
  (#247), so neither `Tab` nor the pointer can reach the shell behind it, but assistive tech still
  can: `shell/root.rs` inerts `.app` for the overlays and the close/quit confirm, and every
  `SidePanel` renders *inside* `.app`, so inerting the shell would inert the panel with it. The fix is
  a layer the panel can render into as a sibling of `.app` (what the overlays already use), not
  another `inert` clause.
- **Only one unsaved new record per category can exist.** A second `⌘N` (or tabstrip `+`) for the same
  category re-focuses the open draft instead of starting another (`NavState::open_create`), so two new
  people cannot be sketched side by side. Deliberate today and test-locked, because the identity is the
  category: `OpenTab::Draft(Category)` carries no draft id and `edit_key()` maps it to
  `EditKey::draft(category)`, which is also the stash key, the create pane's component key, and how
  `commit_draft`/`cancel_draft`/`note_save_finished` find their target — two drafts would share one
  parked buffer and one component instance. Lifting the limit means threading a draft id through all of
  those; the tabstrip only needs its label disambiguated. — #260
- **Record pickers and the command palette never see a record created while they stay open.** Both
  load their rows once per open (`use_existing_picker` in `screens/shared.rs`; the palette's
  `use_resource` in `shell/palette.rs`) and never subscribe to `NavState::data_version` — the deferred
  half of #207, which fixed the Explorer list but not these. — #266

### Lists, search & scale

- **Long-list / overflow specimen (U30)** — no tab demonstrates a long-list or overflow state;
  deferred as low-fidelity in a static mockup (the a11y real-app walkthrough covers it).
- **`ListPane` DOM virtualization** — `master_detail.rs` mounts every row (and a `MountedEvent` per
  row). Render only a scrolled window with a `store.count`-sized spacer and make the roving-focus
  `nodes` bookkeeping window-aware. If server-side windowing is chosen instead, add
  `list_view_page(table, offset, limit)` (+ a Postgres mirror) and a generated column + index on
  `$.state.human_id` in `genealogy-db`. Overlaps the `list_*` pagination item under *Performance &
  scale*.
- **Saved searches** — nothing in the palette, list toolbars, or app layer; the 100k-scale research
  workflow argues for it. Needs a design + use-case decision.
- **Column chooser** — `list.rs` has no column state though PR3's text claims "columns". Decide
  whether to build it or amend the PR3 description.

### Keyboard & shortcuts

Residuals from the shortcuts work (ADR 0030); see
[`archive/completed-work.md`](archive/completed-work.md). Deliberate non-goals are under *Decided*.

- **The unsaved-work confirm's remaining webview pass is timing only.** #238, #239, and #240 shipped
  the close/quit confirm, the per-record edit stash, and Save / Save all under SSR; two `cargo xtask
  gui-pass` scenarios now drive them in the real webview. `unsaved-close-confirm` dirties a record,
  raises the confirm with `⌘W`, tabs the three-button footer full circle, cancels it with both `Esc` and
  the scrim without losing the edit, restores a parked edit after navigating away and back, and saves
  from the dialog. `unsaved-quit-confirm` covers the blocked Save (a `⌘N` draft: the disabled button is
  not a tab stop, so the ring is two buttons) and the `⌘Q` dialog's `ul.stack` over two unsaved tabs.
  What stays human: whether a freshly activated pane mounts fast enough that Save looks instant, that a
  `⌘Q` Save-all run reaches `QuitManager` after the last save (clicking Save all kills the window the
  screenshots come from), and the slide-in motion. The 2026-07-31 pass could not run the Save-all half:
  the button was disabled by an unrelated draft tab, so that check is still outstanding and this bullet is
  blocked on #261. Everything else the pass covered — the map half — closed as #203. — #244
- **One unsavable tab disables Save all for every other tab.** `quit_confirm_copy`
  (`shell/close_confirm.rs`) walks the unsaved tabs and keeps the *first* blocked reason it finds, and
  the button is disabled whenever any reason exists — so an untouched `⌘N` draft (unsaved by
  definition, savable never, reason "Nothing has been filled in for …") blocks saving records that are
  perfectly valid, with no hint that the reason belongs to a different tab than the one you were
  editing. Enabling the button alone is not the fix: `save_all_then_quit` queues by the same
  `tab_has_unsaved` predicate, so the run would reach the draft, fail `can_save()`, and
  `abandon_save_run` mid-flight with the earlier records already saved. Gate on "no unsaved tab is
  savable", queue on `tab_is_savable`, and say what will be left open. — #261
- **`⌘S` lives outside the shortcut map.** Save is wired directly in `screens/record_form.rs` (with
  its own `Esc` to cancel), and shown in `docs/mockups/shortcuts.html`, but is not a `ShortcutAction` —
  so it is neither listed by the `?` overlay nor rebindable, and it does not go through
  `NavState`/`resolved_shortcuts` at all. — #206
- **The "Jump back in" recent-list write has no close/quit hook.** `shell/window_geometry.rs` flushes
  window geometry on `WindowEvent::CloseRequested`; the recent-list persistence effect in
  `shell/root.rs` has no equivalent, so a keyboard quit can race the debounced write. — #205
- **Chord entry is a typed canonical string, not live key capture.** `keydown` is inert under SSR and
  `cargo xtask input-guard` forbids a raw form element outside the primitives, so the Preferences
  rebind field takes `mod+shift+alt+key` text rather than a press-the-keys capture widget.
- **No chord sequences beyond the existing `g`-prefix** — `resolved_shortcuts` resolves single chords
  only.
- **The framework-free `Key` enum (`genealogy-ui::shortcuts`) is still closed** — no function keys, so
  `e`/`F2` (the within-screen edit chord) could not be rebound even if that group were opened up.
- **No keyboard topic in the in-app Help browser.** `genealogy-ui::help.rs`'s `HelpSection::Reference`
  is documented as "Lookup material (shortcuts, glossaries)" and `Run::Kbd` is unused — no authored doc
  covers shortcuts; the `?` overlay is the only in-app reference today.

### Pedigree & charts

- **`Restriction` chart cue** on the pedigree chart.
- **Name-autocomplete pickers** for the focus / relationship inputs, which are plain `human_id` text
  fields today.

### Geography & map

- **Map markers label with the first-asserted name, not the resolved one.** `show_geography` builds each
  `PlaceMarker` label from `place.names.first()` (`genealogy-app/src/geography.rs`) while the geometry on
  that same marker *is* date-resolved, so at slider year 1875 the pin reads "Oslo" while
  `generated_title` beside it correctly reads "Kristiania" — the map contradicts the record. Use the
  as-of-resolved name; `PlaceView::name_as_of` already exists. — #232
- **`geography_toolbar` takes 9 args** (`#[expect(clippy::too_many_arguments)]`) after the picker +
  fit + draw-target state were threaded in — bundle them into a struct. Cosmetic cleanup.
- **The Geography tool's Point tool cannot save at all.** The Place Map editor has a "Use this point"
  confirm; the Geography tool has no equivalent, and contrary to what this bullet used to say it does
  not commit on click either — it only paints a red draft dot. `open_geometry_panel`
  (`screens/geography.rs`) is called from `on_finish_polygon` alone, whose geometry is always a
  `Polygon`, so its `PlaceGeometry::Point` branch and the whole `GeoPanel::CreateHere` variant are
  unreachable. Either give the tool a confirm step that reaches that branch, or delete the dead
  variant.
- **Polygon vertices are never drawn, and cannot be moved.** `geo-draft-point` filters the draft
  source to `Point` geometry (`screens/map_shared.rs`) while a ring is emitted as a `LineString`
  under three vertices and a `Polygon` at three or more, so no vertex handle is ever rendered: the
  first click shows nothing at all, the second only a hairline segment, and a finished ring has
  outline plus fill with no corners. A draft vertex layer is the prerequisite for the drag-to-move
  and mid-ring insertion under *In-map editing depth* below. — #259
- **Zoom is invisible and unbounded.** No zoom readout, no `NavigationControl`, no scale bar, and no
  `minZoom`/`maxZoom` on the map or `maxzoom` on the raster source (`screens/map_shared.rs`) — so
  MapLibre's 0–22 default applies while `tile.openstreetmap.org` only serves to z19, and zooming past
  it silently yields blank tiles. `maxZoom` on the map plus `maxzoom: 19` on the source, and a zoom
  readout beside the provider select. (`fitBounds`' own `maxZoom: 15` is the only bound today, and it
  bounds Fit alone.) — #253
- **OSM attribution is never shown.** The map is created with `attributionControl: false` and the
  surface's own attribution line renders an empty string (`screens/map_shared.rs`), while
  `MapProvider::attribution` (`genealogy-app/src/config.rs`) is never read — so the tile source's
  required credit is absent, which the OSM tile-usage policy does not allow. — #254
- **The Geography place list is undocumented and geometry-only.** The rail lists places that resolved
  a geometry *as of the slider year*, narrowed by the toolbar picker's live query
  (`screens/geography.rs`) — so a place without geometry can never be selected there (making it
  unreachable as a draw target), and the list silently shrinks as the year moves. It carries no label
  saying any of that. Either list every place and mark the plotted ones, or label the list and offer
  the unplotted ones as draw targets. — #256
- **Geometry saved from Geography is undated; the Place Map tab stamps its slider year.** The
  Geography panel saves `year: None` (`screens/geography.rs`) while the Place Map tab saves
  `Some(year())` from its own independent slider, defaulting to 1900 (`screens/place.rs`). A point
  saved from the Place tab therefore disappears from the Geography map for every year below 1900 —
  `resolve_as_of` (`genealogy-core/src/temporal.rs`) ignores assertions dated after the target and
  falls back only to an *undated* one, and a place with neither drops out of the marker feed
  entirely. Decide one dating policy (an explicit dated/undated choice in both panels), and never
  drop a marker silently — show "no geometry as of \<year\>" instead. — #257
- **The Place Map tab cannot render an undated geometry.** Its view-model resolver filters to
  assertions dated at or before the year and otherwise falls back to `geometries.first()`
  (`genealogy-ui/src/view_model/place.rs`) — the first *item*, where core's `resolve_as_of` falls back
  to the first *undated* item. With a point dated 1900 ahead of an undated polygon in the list, the
  polygon is unreachable at every year, which is why a polygon drawn in Geography never appears on the
  record's own map even though the *Geometry over time* table lists it. Reuse core's rule rather than
  restating it client-side. — #258
- **The draft is never cleared after a successful save either.** `geo_edit_panel`'s `onsaved`
  (`screens/geography.rs`) sets `panel`/`reload`/`toast` but not `draft`, so the red draft ring sits on
  top of the newly-saved boundary until the user clicks Clear by hand. Found while fixing #255 — the
  same "keep the draft, let the user clear it" behaviour is right for a *refusal*, wrong for a *save*.
- **Switching provider repaints nothing.** The toolbar's provider select writes `[map]` config for
  `osm-raster` and is an explicit no-op for the other two options, but the tile URL is hardcoded in
  `maplibre_init_script` and nothing ever calls `setStyle`, so no choice can change the map. Pairs
  with the *Provider sub-forms* item below — the sub-forms are pointless while the map ignores the
  setting.
- **In-map editing depth** — true mouse-drag reposition and mid-ring vertex insertion (today: click to
  drop/move a point and click to add polygon vertices), pin-click selection on the canvas (today:
  select via the rail list), and polygon-drawn creation of a *new* place (today: point-drop creation is
  wired; polygon draws onto an existing place).
- **Provider sub-forms** — `osm-raster` is switchable from the toolbar; `maplibre-style` / `google` are
  declared in `[map]` config and round-trip but have no toolbar sub-form to collect a style URL /
  API-key-env yet.
- **Tile caching** — tiles are fetched by the webview, and `main.rs` never gives dioxus-desktop a data
  directory, so they land in WebKit's default unmanaged cache at a path the app neither controls nor
  can bound. A viewport-only, size- and TTL-bounded disk cache is possible without leaving Rust: serve
  the raster source from a `use_asset_handler("tiles", …)` route, read/write
  `project_dirs()?.cache_dir()` (no cache-dir helper exists in `genealogy-app/src/config.rs` yet), and
  fall through to `reqwest` with a real `User-Agent`. That seam is also the natural place to enforce
  `MapConfig::net_allowlist`. Constraint: `research/geography-rendering.md` commits to caching no more
  than the browser does, so bulk or offline prefetch stays out and the wording needs amending along
  with any change here. — #262
- **`map-provider` plugin world + geocoding** — the declarative provider ships and the Geography
  toolbar search is now a `RecordPicker` over existing places (search + jump); geocoding a *new*
  real-world address to a coordinate stays deferred. A WASM `map-provider` world supplying geocoding
  \+ custom tile-source descriptors over `net` is the ADR 0025 §4 follow-up (supplies data/descriptors,
  never pixels).

### GUI ⇄ CLI parity

Every per-aggregate verb the CLI exposes has a GUI counterpart (audited
`genealogy-cli/src/{main.rs,commands/*.rs}` against `genealogy-ui/src/{navigation.rs,intent.rs}`); each
reuses an existing `genealogy-app` use-case rather than a new core verb. One parity gap remains, filed
in its own area: research notes (*Notes & research notes*). The one gap running the other way:

- **Restrictions cannot be set from the CLI on any aggregate** — `restrictions_tag`
  (`genealogy-cli/src/i18n/person.rs:35`) renders them on record output, but no command writes them:
  no `Restriction` `ValueEnum`, no `set-restrictions` verb. The app use-cases exist per aggregate and
  the GUI wires all thirteen, so privacy is GUI-only.
  *Shape:* one shared `ValueEnum` plus a `set-restrictions` subcommand per aggregate over the existing
  use-cases and `restriction_label`. Not a pre-1.0 gate — the milestone rule is GUI reachability. — #225

## Import, export & plugins

### Bulk import, export & sync

- **Gramps `<header created>` is parsed but never threaded to `begin-import`** — ADR 0029's
  timestamp-gated reconciliation is only wired on the GEDCOM side
  (`plugins/gedcom-import/src/lib.rs:41`). `plugins/gramps-import/src/lib.rs` goes straight from
  `parse` to the person loop and never reads `db.header`, though `genealogy-gramps-xml/src/parse.rs:400`
  does parse the date — so a Gramps re-import gets no timestamp gating at all. Found while verifying
  the Phase 10 completion claim, which described both formats as wired.
- **Source merge/sync reconciliation prerequisite** — Source resolve-or-create (`ExternalId` dedup) +
  `set-source-title`/`set-source-abbrev` WIT verbs + a field-level `AssertionId`/`occurred_at` read
  path. Unlike Person/Family, a standalone Source has no `ExternalId` resolve-or-create today (a second
  import of the same file duplicates the Source aggregate), so the ADR 0029 timestamp-gated rule can't
  target it yet. `Person.sex` reconciliation shipped without these; widening the rule to Source's
  bibliographic fields (`title`/`author`/`pub_info`/`abbrev`) is blocked on them.
- **Place merge/sync reconciliation prerequisite** — the same three gaps ADR 0029 §4 recorded for Source,
  and Place has them identically: no `ExternalId` resolve-or-create (Place is absent from
  `for_each_db_external_id_aggregate!`, and `import.rs` has no place path, so a second import duplicates
  every place), no WIT verbs for most Place fields, and no read path exposing a field's live
  `AssertionId` **together with** that assertion's `occurred_at` — without which the timestamp gate
  cannot be evaluated at all. Place's dated multi-valued fields do have the natural match key `Fact`
  lacks: the effective-from `date`. See [`research/gis-norway.md`](research/gis-norway.md).
- **Retraction resurrection blocks recurring imports** — ADR 0029 compares the incoming value against the
  *live* assertion, and a retracted assertion is not live. So a value a researcher retracted as wrong is
  re-asserted by the next import run, and every run after it, attributed to `AgentKind::Software` so it
  reads as routine in the history. This destroys editorial judgement rather than merely duplicating data,
  and must be resolved before any import is made recurring. Needs a tombstone rule over retracted
  assertions keyed by originating authority.
- **Lift `prepare_import_target`** into `genealogy-app::workspace_registry` — still inline in the CLI
  (the rest of `init` already delegates).
- **No merge/conflict mockup for reconciled fields** — the Phase 10 plan required a merge/conflict view
  in `docs/mockups/import.html` showing a reconciled field's audit trail (who/when/why, not an
  interactive picker), and listed it in the Gate-2 exit criteria. `import.html` has no such view: the
  ADR 0029 supersede path is invisible in the mockups, so there is no agreed design for showing a user
  that an import overwrote one of their values.

### Assisted import

Follow-ups left open when the Digitalarkivet flow shipped; each is scoped, none blocks the flow.

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
- **Politeness delay for `net`.** The archive `robots.txt` requests `Crawl-delay: 5`; `net` enforces
  a timeout and size cap but no inter-request delay (the assisted flow is interactive and low-volume
  today). A politeness delay is a follow-up if usage grows.
- **`AiProvider::Plugin` is declared but unsupported** — the variant exists in `[ai]` config and
  round-trips, but `genealogy-plugin-host/src/ai.rs:73` returns `AiError::InvalidInput` for it, so a
  workspace configured with `kind = "plugin"` fails only at first use. Either implement it or reject it
  at config-load time.

### Round-trip gaps

- **`SUBM`/other `HEAD` metadata** — deferred to its own future ADR-gated item; no core-domain concept
  (a document-level submitter/owner) exists to map it onto yet.
- **RichText `translations`** (text+language, distinct from `translator`) — GEDCOM 7 has a real target
  (`NOTE.TRAN`), but `genealogy-gedcom` has no structured `Note` model yet (notes are bare strings);
  blocked on that prerequisite rewrite. Gramps has no equivalent construct.
- **`Address` on the Gramps side** — `genealogy-gramps-xml` has no `Address` concept at all, so
  `Address.original_text` (which round-trips on the GEDCOM side now) has nowhere to go there; and
  `original_text` has no Gramps DTD equivalent even once an Address type exists.

### Plugin-UI vocabulary

ADR 0022 out-of-scope tail:

- Repeating groups / nested forms.
- `List`/detail descriptions + plugin-driven navigation.
- Per-field validation vocabulary.
- Plugin-prefilled field values.
- The `query` capability for `ui-panel`.
- Long-running / streaming actions.
- Multi-panel pages.

### Plugin trust & capabilities

ADR 0014 out-of-scope:

- **Marketplace / registry / auto-update** — bundles are installed manually into a layer; discovery and
  remote distribution/fetch of third-party plugins is future work.
- **Transparency-log / online revocation** — revocation is by a binary release dropping a key from the
  embedded trust-root set (ADR 0014 §6); no sigstore-style online revocation.
- **Sub-capability grants** — e.g. a user-editable per-host `net` allowlist. `net`'s allowlist stays the
  grant-site `NetPolicy` (ADR 0017); the user-editable override ADR 0017 deferred here remains a
  follow-up, not built.
- **Per-plugin resource-budget overrides** in config — fuel/memory limits stay host-set (ADR 0011 §4).
- **Host-binary signing / build-provenance attestation** — signing covers plugin bundles, not the app
  binary itself.

## Platform & operations

### Performance & scale

From [`research/performance-profiling.md`](research/performance-profiling.md):

- **`list_*` projections lack `LIMIT`/`OFFSET`** — a full scan + JSON decode (61.7 ms at ~52.6k persons);
  pagination is the real interactive scaling lever before 100k scale. Overlaps **`ListPane` DOM
  virtualization** under *Lists, search & scale*.
- **Research-note reverse lookup is a `json_each` scan, not a materialized index** — fine now (~2 ms at
  ~2250 notes); a materialized side-index is a follow-up only if note volume grows.
- **Postgres spatial mirror** — `places_in_bbox` returns `Unsupported` on Postgres (SQLite R\*Tree only);
  the native geometry + GiST index is a later feature-gated follow-up. It would also back the
  `places_containing` query under *Places*, and once containment exists on both engines the two must
  agree at boundary-touching points and shared edges — so a shared conformance corpus, not two
  independently plausible implementations, or parish membership would change with the database engine.
- **Index `$.state.human_id`** — `next_human_id` reads and JSON-extracts every row of a `*_view` table to
  take the max, and every `human_id` lookup is a second full scan, so a bulk import is O(n²) twice over
  before any domain work happens. A generated column plus index turns both into probes. Overlaps
  **`ListPane` DOM virtualization** under *Lists, search & scale*, which names the same change as one of
  its options; this is the standalone version, wanted independently of virtualization. — #233
- **Viewport-scoped loading** — `show_geography` loads every place with a resolved geometry rather than
  calling `places_in_bbox` for the current viewport; wire the spatial query in when place counts grow
  (needs a Postgres fallback given the row above).

### Packaging & release

- **Cross-platform packaging** — 1.0 is Linux-first (tarball + `.deb` + AppImage). macOS/Windows
  bundles and **OS-level code-signing / notarization** (Gatekeeper, Authenticode) are a later cycle
  (ADR 0014 §Out of scope). — #215
- **`.deb` needs `GENEALOGY_PLUGIN_DIR`** — the embedded plugin layer has no default *system* path, so a
  distro-installed binary needs `GENEALOGY_PLUGIN_DIR=/usr/lib/genealogy/plugins` (the AppImage sets it
  via `AppRun`; the tarball resolves the fleet beside the binary). Teaching the embedded layer a default
  system path so an installed `.deb` finds the fleet with no env var is the follow-up (see
  [`release.md`](release.md)). — #212
- **Real release keys not yet generated** — only the deterministic **DEV** signing key exists (Sanctioned
  in debug builds only), so `embedded_sanctioned_keys()` is `None` in a release build until one is
  configured. Before the first real release, generate the release ed25519 keypair, set the private half
  as the `GENEALOGY_PLUGIN_SIGNING_KEY` repo secret, and embed the public half via
  `GENEALOGY_PROJECT_PUBLIC_KEY` (ADR 0014 §6; procedure in [`release.md`](release.md)). — #210
- **The embedded plugin-dir resolver is duplicated *and* divergent** — the ADR 0014 §4 *layering* is
  shared (`genealogy_app::plugin_layers`), but each frontend still resolves the embedded layer itself
  and the two disagree on the dev fallback: `genealogy-ui-dioxus/src/app.rs:326` uses
  `CARGO_MANIFEST_DIR/../../target/plugins` (source-tree-absolute) while
  `genealogy-cli/src/commands/io.rs:83` uses a bare `target/plugins` **relative to the working
  directory**. So a CLI invoked from anywhere but the repo root silently finds no embedded fleet while
  the GUI always finds it. The Phase 11 plan called for replacing this duplication; only the layering
  half landed. Fold both into one `genealogy-app` resolver — the same change that would give an
  installed `.deb` a default system path (item above). — #213
- **No `[profile.release]` section** — the Phase 11 plan's "strip/optimize release profile" was not
  done: the root `Cargo.toml` has no `[profile.release]`, so shipped binaries carry full debug symbols
  and default codegen settings. `strip = true` plus a considered `lto`/`codegen-units` is the cheapest
  size win available before the first tag. — #214
- **`release.yml` unverified end-to-end** — GitHub Actions billing is currently blocked, so the release
  workflow is zizmor / YAML / `bash -n` verified and its build/package steps reproduced locally, but has
  never run a full tag → AppImage → GitHub Release cycle. The first real tag needs a live verification
  when billing is active. — #211

### Dependencies blocked upstream

- **`sqlx` 0.9** — `sqlite-es` / `postgres-es` 0.5.0 (their latest) pin `sqlx` 0.8, so a bump splits the
  tree into two `sqlx` versions and every `Pool<Sqlite>` / `Pool<Postgres>` handed to the event stores
  fails to typecheck (17 mismatches). 0.9 also replaces `&str` query input with `SqlSafeStr`
  (38 `sqlx::query(&format!(…))` call sites in `genealogy-db` need `AssertSqlSafe`). Re-evaluate when the
  `*-es` crates release on `sqlx` 0.9.
- **`ed25519-dalek` 3.0.0** — needs `curve25519-dalek` ^5.0.0, while `russh` (via the `testcontainers`
  dev-dependency) needs `curve25519-dalek` =5.0.0-pre.6. Cargo resolves the conflict by downgrading to
  `russh` 0.60.1, which does not compile against the `pkcs5` 0.8.1 that ed25519-dalek 3.0.0 pulls in.
  Held at `=3.0.0-pre.6` (comment in `Cargo.toml`); re-evaluate when `testcontainers` ships a `russh` on
  stable `curve25519-dalek`.

### Docs & repo tooling

The `area/docs` label already existed with no `###` home; this is it.

- **`issue-sync` rejects the bug bullets that `issue-tracking.md` says are correct.** `offline_problems`
  in `xtask/src/issue_sync.rs` fails any bullet whose section has no `###` area — "is directly under
  `## X` with no `###` area, so it has no area/\* label to inherit" — but
  [`issue-tracking.md`](issue-tracking.md) §2 states "**`## Bugs`** has no H3s by design: a bug takes its
  `area/*` from whichever area it affects, plus `type/bug`". So the documented shape for an open bug is
  one the tool refuses. Latent today only because `## Bugs` currently holds prose, not bullets: the two
  open defects were deliberately routed under their area H3s to avoid it. Whichever way it is resolved —
  exempt `## Bugs` in the parser, or drop the by-design claim from the doc and require bugs to live under
  an area H3 — the two must agree, because `cargo xtask check` is a prek hook and a docs-only commit gets
  no CI (`paths-ignore` covers `docs/**`), so this is the only gate that would catch it. — #235
- **`gui-pass` occasionally grabs a blank first shot.** Once in roughly a dozen runs the first `shot` of
  a scenario comes back uniform (`… is blank (standard deviation 0) — the webview painted nothing`) and
  the run aborts, passing on a re-run with nothing changed. The startup handshake in
  `xtask/src/gui_pass.rs` waits for the window to map and then settles once; a paint the harness can
  actually observe (a shot that must not be blank, retried) would make it deterministic instead of
  making the first assertion of every scenario flaky.

## Decided — no action needed

Deliberate non-tasks, recorded so they are not re-raised or read as unfinished work. Each is a
decision, not a gap.

### Keyboard & shortcuts (ADR 0030 §2, §3, §Out of scope)

- **`Ctrl+W`/`Ctrl+Q` bubble out of focused text inputs, by design.** `focus_trap.rs`'s
  `keep_typing_local` lets every primary-modifier chord bubble to the shell except native `⌘Z`/`⌘⇧Z`
  text undo/redo — so typing in a field and pressing `⌘W` closes the tab mid-edit, the same as `⌘K`/
  `⌘N` already did.
- **`Ctrl+W` closes the active tab even when its strip isn't shown.** The record tabstrip mounts only
  for entity destinations (`shell/root.rs`'s `entity_category`), but `NavState::active_record` can stay
  `Some` while the Dashboard or a tool is active — a deliberate simplification.
- **Within-screen and `g`-prefix keys are not rebindable** — widget-owned (roving focus / the
  `g`-prefix state machine), fixed by design.
- **No per-platform keymaps.** `Modifier::command` abstracts ⌘/Ctrl by design, so a binding that must
  differ between macOS and Linux cannot be expressed.
- **Bindings are global-only, no per-workspace override.** `[shortcuts]` lives in
  `~/.config/genealogy/config.toml`, consistent with `[map]`/`[ai]`/`[plugin_trust]` — a keymap is
  machine/user-local, not a dataset property.
- **No VS Code-style *when* context.** No context predicates are defined; a design question if a real
  need surfaces, not a missing implementation.

### Model & interchange

- **`LineString` geometry variant** — the model ships `Point`/`Polygon`; `LineString` is additive-later
  per ADR 0024 (grow the enum append-only when a concrete need appears), and nothing wants it yet.
  **`Multi*` is no longer deferred** — the concrete need ADR 0024 named for itself arrived, so it is an
  open item under *Places*.
- **Explicit `[from, until)` validity intervals** — the effective-from resolution rule (ADR 0026)
  ships; add intervals additively only if gaps/overlaps prove ambiguous in real data. Still closed after
  the Norwegian-geography review, which found the gap intervals would address (a dissolved place
  resolving forever) but concluded they are the wrong-sized fix: only a place's *lifetime* needs an end,
  not every assertion, so the open *Places* item is a read-side `existed_as_of` instead.
- **RichText `translator`** — permanently non-round-trippable: neither GEDCOM 7 nor the Gramps DTD has
  any tag for who translated a note's text.
- **Media DTO convention split** — `person-dto`/`family-dto`/`event-dto`'s `media` is `list<media-ref>`
  (host-api 0.21.0), but `source-dto`/`citation-dto`/`place-dto` have no `media` field at all (no
  exporter reads media off those three today); widening them is deferred until a real consumer appears
  (YAGNI).
- **External ids have no frontend entry point** — person `add_external_id` (module-level, not even
  root-exported) and `add_family_external_id` are used only inside `import.rs` for resolve-or-create.
  External ids are importer bookkeeping, not user-editable data.

### Geography map

- **The map needs no explicit `map.resize()` call.** #252 read the blank canvas as a missing resize:
  arming the Polygon tool inserts a Finish/Clear row that shrinks `.map-surface`, and nothing in our
  code calls `resize()`. Measured on the real webview, MapLibre's own `trackResize` observer does
  re-measure and resize the canvas correctly — the frame taken right after the shrink is pixel-identical
  to the settled one. What it fails to do under WebKitGTK is get that frame to the compositor, so the
  fix is a repaint (`redraw()` on the animation frame after a container resize, `screens/map_shared.rs`),
  not a resize. Adding a resize would only re-clear the drawing buffer and fire spurious `move` events.
  `preserveDrawingBuffer: true` was tried and changed nothing.
- **Only a layout change ever blanked the canvas, not "any re-render".** #252's title and body said
  clicking *Pan*, *Drop / move a point* or *Draw polygon* all empty the canvas, and blamed the
  container's `class` / `data-armed` writes. Measured on the unfixed tree, neither of the first two
  blanked anything: arming Point left the canvas region byte-identical to the frame before it, and
  re-clicking the active Pan tool likewise. Only Polygon — the one tool that inserts a row and shrinks
  the surface — went flat. A class write on the container is harmless; the trigger was always the
  resize. The `map-repaint` scenario keeps both non-blanking cases asserted so a future change cannot
  quietly make them true.

### Architecture

- **Snapshotting is decided, not deferred-open** — measured and **not** warranted at target scale;
  ADR 0004's deferral stands, no follow-up ADR. The measurement was taken on small payloads, so it holds
  for boundary geometry imported at N500-equivalent generalization or coarser (the level
  [`research/gis-norway.md`](research/gis-norway.md) recommends); only importing finer geometry, which
  that research argues against on other grounds, would put megabyte events in the log and require
  re-measuring.
- **Server backend + web frontend** — roadmap-owned and deliberately unscheduled; see
  [`roadmap.md` Phase 13](roadmap.md#phase-13--beyond-10-server-backend--web-frontend). Builds on the
  config split: the server adds the `ConfigStore` **database** backend so the operator and
  client/presentation scopes persist per authenticated user, while the embedded build keeps the file
  backend.
