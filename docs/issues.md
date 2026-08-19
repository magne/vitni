# Vitni Issues

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

This is a pointer, not a place bullets live: an open bug takes its `area/*` from whichever area it
affects, plus `type/bug`, and sits under that area's `###` heading. `cargo xtask issue-sync` reports a
bullet placed directly here as misplaced.

The **2026-08-12 GUI walkthrough** is where most of the currently-open defects came from, and it filled
several areas at once — *Record detail & shared tabs* (new, and the largest), *Media*, *Tags*, *Shell,
tabs & notifications*, *Keyboard & shortcuts* and *Docs & repo tooling*. Walking the real GUI is still
the only thing that finds this class: every one of them passed the SSR suite.

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
  `Store::execute_place`, which is what `crates/vitni-app/tests/place_temporal.rs` does and
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
  `Custom(String)` is not a substitute: `vitni-ui/src/i18n.rs` renders it verbatim, so a raw
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

### Tags

The `area/records/tags` label has existed since the taxonomy was applied; this is its first `###` home.

- **The Tag screen diverges from `mockups/tag.html` in four places** — the header's colour badge is
  text-only (`screens/tag.rs:222`; `Badge` carries no dot, `components/data.rs:125-132`) where the
  mockup shows the swatch inside the badge (`tag.html:64`); the read-only overview stacks label above
  value (`tag.rs:312-337`, `.field label`/`.field .val` are `display:block`, `components.css:461-470`)
  where the mockup puts them on one line (`.fact-row`, `tag.html:93-95`); and the read-only colour
  section renders the hex string with neither swatch nor preview chip (`tag.rs:330-335` vs
  `tag.html:108-112`), though edit mode has both (`tag_edit_colour_card`, `tag.rs:467-508`). Edit mode
  should also put label and input on one line while **keeping** the Tag/Colour card split the mockup
  does not draw — so `tag.html`'s edit state is updated in the same change, per the mockup rule in
  [`CLAUDE.md`](../CLAUDE.md). The same stacked-`.field` divergence is on the Media record, under
  *Media*; fixing it once in `components/draft_field.rs` covers both. — #310

### Media

- **A media preview whose filename is not ASCII never loaded** — the `<img src>` carried raw UTF-8, the
  webview percent-encoded it in the request, and nothing decoded it, so `resolve_media_path` looked for a
  file whose name contains a literal `%C3%B8` and 404'd. Spaces failed the same way (`%20`); `#`, `?` and
  `%` failed before the request, taking on their URL meanings. Systemic, not exotic: `slugify`
  (`media_save.rs`) and the plugin host's `sanitize_component` both deliberately keep `æøå`, so the app's
  own naming conventions produced paths it could never serve. #301 shipped without catching it because
  `media-preview`'s fixture was `portraits/portrait.png` — ASCII, no spaces — and because the two causes
  that PR fixed (no inferred MIME, the doubled `media/` prefix) genuinely fire on that fixture, while the
  operator's own records have a recorded MIME and an unprefixed path, so neither explained the symptom
  that was reported. Fixed by making the `/media/<rel>` URL space encoded end to end
  (`media_url_path`/`media_url_decode` in `vitni_core::media_path`), with the gui-pass fixture gaining
  a second record named in the real data's alphabet.
- **"Add file to media library" action.** The media-save dialog and the pure naming logic
  (`suggest_filename`/`slugify`) ship and are SSR-tested; the app-layer copy use-case that writes an
  external file into `media/<target>` and creates the Media record is deferred.
- **A file stored outside the workspace media root has no preview** — an absolute `MediaPath::File` is
  legitimate (see the *Media edit mode* bullet: a record for a file that is not on this machine is
  legitimate too), but `media_asset_src` serves only the media root, so such a record shows the 📷
  placeholder. Honest, not useful: previewing it needs either a second asset-handler route scoped to a
  configured set of readable roots, or an explicit "copy into the workspace" action reusing the media
  save dialog. Neither is designed.
- **The Media record diverges from `mockups/media.html`** — the File card repeats the human id the
  header already shows (`media.rs:71-87` vs `:471`, and the mockup's File card starts at *File path*,
  `media.html:105-111`); `DraftText` stacks label above value where the mockup uses same-line
  `.fact-row` rows; the header passes no subtitle and no MIME badge (`media.rs:469-477` vs
  `media.html:61,64`); and the "Used by" rows are `.fact-row` divs whose columns do not line up
  (`media.rs:637-652`). Shares its label/value half with the Tag bullet under *Tags*. — #309
- **Media edit mode cannot check, fetch, or type a file** — *File path* is a plain `DraftText`
  (`media.rs:88-101`) with no existence check, and any check added must **flag, never block**: a
  record for a file that is not on this machine is legitimate. There is no download when *Web path*
  changes with a file path set (the mockup already draws `⤓ Download`, `media.html:73`), which needs an
  overwrite prompt on checksum mismatch. And nothing *records* a `mime`: the extension→MIME mapping now
  lives in `vitni_core::media_path::mime_for_path` and drives the display gate (an image with no
  recorded MIME still previews, #301), but no command writes an inferred value into the record, and the
  editor's MIME box stays blank. Writing one on save is the work left — deliberately not done as part of
  #301, because `MediaDraft::from_detail` seeds the editor from `mime` and `edits_against` diffs it, so
  an inferred value there would emit a spurious `SetMime` on every save.

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
  Either delete the wrapper, or keep it to back a `vitni check` subcommand so quality findings are
  scriptable.
- **Data-quality checks are person-only** — both `CheckKind`s are `DeathBeforeBirth` and
  `PossibleDuplicates`. Widening checks to the other twelve aggregates is its own item.
- **Repository media refs (U31)** — should Repository carry media refs (e.g. archive photos)? A
  data-model question.

## Frontend & interaction

### Shell, tabs & notifications

- **A `SidePanel`'s background is not `inert`.** The panel now traps and restores focus like `Modal`
  (#247), so neither `Tab` nor the pointer can reach the shell behind it, but assistive tech still
  can: `shell/root.rs` inerts `.app` for the overlays and the close/quit confirm, and every
  `SidePanel` renders *inside* `.app`, so inerting the shell would inert the panel with it. The fix is
  a layer the panel can render into as a sibling of `.app` (what the overlays already use), not
  another `inert` clause. — #312
- **Back/forward cannot return to a draft tab.** `NavLocation.record` is `(Category, String)`, a saved
  record's key, so a draft is never recorded in the history and `⌘←`/`⌘→` step past it to the last saved
  record instead. True before several drafts per category (#260) and unchanged by it, but a strip that
  can now hold four drafts makes it easier to notice. `NavLocation` lives in `vitni-ui`
  (framework-neutral, ADR 0008) and a history entry naming a draft goes dead when the draft is cancelled
  *or* committed, so the variant needs a rule for that first. — #313
- **A draft tab and the `+` menu read "New People" while the pane heading reads "New person".** The
  tab label (`shell/tab_label.rs:66`) and every item of the `+` `NewRecordMenu` (`shell/tabstrip.rs:190`)
  feed the *rail* label — `nav-people`, plural, shared with the rail and the Explorer — into
  `draft-tab-label = New { $entity }`, while the create pane takes `person-new-title` (singular), so one
  screen shows both forms of the same thing. The same substitution makes the Norwegian read "Ny
  Personer" instead of "Ny person", which is not merely inconsistent but ungrammatical for every
  category. One fix from three sides: 13 singular entity keys, used by the tab and the menu; the pane
  heading already reads correctly. — #308
- **Closing a pristine draft tab still raises the unsaved-work confirm.** `tab_has_unsaved`
  (`shell/nav_state.rs:948-956`) answers `true` for `OpenTab::Draft(_, _)` unconditionally — documented
  at `:944-946` as "nothing about it is stored yet, whether or not anything has been typed" — so `⌘W`
  or the tab `✕` on a `⌘N` nobody typed into asks whether to discard nothing. The same predicate drives
  the strip's `●` dot and `has_unsaved_work()`, so an untouched draft also makes `⌘Q` claim unsaved
  work. Dirtiness is already knowable: `draft_label(&tab.edit_key())` (`tab_label.rs:63`) is `Some`
  only once something naming the record has been typed, and `edit_drafts` is keyed per `EditKey`. — #307

### Record detail & shared tabs

The 13 detail screens share their tab **bodies** — `screens/tabs.rs` (citations, notes, tags,
addresses, history) and `screens/shared.rs` (media, the retract and attach side panels) — and re-implement
only the Overview and their entity-specific tables. So each item here lands on every aggregate at once,
which is what makes them worth fixing in the shared code rather than per screen. All came out of the
2026-08-12 GUI walkthrough.

- **A record tab cannot create the record it wants to attach.** The labels already split by capability
  — `action-attach-citation/-media/-note` versus `action-add-*` — and there is no split-button or
  menu-button primitive in `components/` to offer both. The cheaper answer than a split button is that
  the *picker* creates: the design system already specifies that affordance (`+ New person "ann"…`,
  `mockups/design-system.html:327-332`), so the tab button stays a single "Attach …" and the search
  field grows the create path. Confirmed: `RecordPicker` already wired the "+ New …" row and
  `.draft-card` rendering (used by the create form and the provenance block); the gap was narrower than
  it looked — the shared attach/link side-panel path (`screens/shared.rs`'s `attach_picker_form`) built
  its picker with `allow_new: false`, so every attach dialog was existing-only regardless. Fixed by
  routing every attach/link side panel through the find-or-create `use_attach_picker` +
  `attach_link_form` (issue #314). — #314
- **The 13 detail screens re-implement the shared tab arms instead of calling one shared frame.**
  `screens/tabs.rs` and `screens/shared.rs` hold the tab *bodies*, but each screen still spells out the
  arm that reaches them: `"history" =>` in 13 screens, `"tags" =>` in 12, `"notes" =>` in 10, `"media"
  =>` and `"citations" =>` in 6 each, `"research-notes" =>` in 4. The `tab_with_add` arms are
  character-identical across screens apart from the screen's own edit-form variant — `place.rs:703-729`
  against `event.rs:882-908` is 27 lines the same but for `PlaceEditForm::`/`EventEditForm::` — and the
  `DetailTab` → `TabItem` mapping above them is repeated verbatim in all 13 `*_detail` fns
  (`source.rs:446-453`). Neither #303 nor #304 turned out to need it, for the same reason: what varies
  across the 13 screens is the arm, not the body it reaches. #303's per-tab explanation resolves from
  `tab.id` inside `tab_frame` because `DetailTab` already carries the tab's identity and its action
  (issue #314), and #304's convergence landed as one `attached_table` in `tabs.rs` that the six
  attached-records tables call — one edit each, not 13. So the case for the struct is the arms'
  duplication on its own, not a shape change blocked behind it.
  Wanted: `shared_tab<E>(loc, tab_id, &SharedTabCtx<E>) -> Option<Element>` in `tabs.rs` — the
  collection slices, the four `E` form variants, the `on_retract`/`on_tag_remove`/`on_undo` callbacks
  and the `MediaTabState` in one struct — returning `None` for a tab the screen owns itself, so each
  `*_tab_content` keeps only its entity-specific arms; plus `impl From<&DetailTab> for TabItem`. — #322
- **The change-set commit path is written out 14 times in `services.rs`.** `services.rs:283-497` holds
  the 13 `commit_*_change_set` wrappers plus `commit_new_record`, whose bodies are the same four
  statements — `localizer()`, `open()`, `Session::new(config.operator_agent())`,
  `vitni_ui::dispatch_*(…).map_err(|e| loc.error(&e))` — differing only in the dispatch fn and its
  request type. The `vitni-ui` dispatchers they call do genuinely differ per aggregate (each maps its
  own fields), so the duplication is confined to this wrapper layer. The `save_*_edit` half of it is
  gone: `detail_save_wrappers!` generates all 12 from `for_each_detail_aggregate!`
  (`services.rs:260-281`, `detail_aggregates.rs`), and the four per-pane callbacks
  (`on_submit`/`on_undo`/`on_tag_remove`/`on_retract_confirm`) now come from `use_detail_commits`
  (`screens/detail_commits.rs`), leaving one hand-written confirm above the hook — `event.rs:506`,
  because retracting a participation dispatches a `PersonEdit` against the person it belongs to.
  Wanted: the same x-macro treatment for these 14, which needs a row carrying each aggregate's
  `*ChangeSetRequest` type; `commit_new_record` has no aggregate row to sit on, and the per-wrapper doc
  comments (what commits together, what the UI boundary parsed) would have to survive as macro
  arguments rather than be dropped.
- **Every detail pane spells out the 19 `IntentOutcome` variants it does not handle.** The terminal
  `match` of each pane ends in a catch-all arm naming every other variant so no wildcard is used — 21
  lines apiece at `note.rs:337-358`, `person.rs:781-802`, `tag.rs:165-186` and 11 more, 14 sites in
  all, every one of them `=> rsx! {}`. `IntentOutcome` has 22 variants
  (`vitni-ui/src/intent.rs:101-153`), so adding a 23rd is 14 mechanical edits producing 14 compile
  errors that carry no information: no pane has ever handled another aggregate's outcome. Wanted: a
  `detail_state<'a, T>(&'a Option<ScreenData>, pick: impl Fn(&'a IntentOutcome) -> Option<&'a T>)`
  helper in `screens/shared.rs` returning `Loading | Error | NotFound | Ready(&T) | Other`, each screen
  supplying a one-line `if let` for its own variant — the exhaustive listing then survives in exactly
  one place, which is what the no-wildcard rule is for.
- **The record editors' side-panel host is written 13 times, and its `footer` prop has no user.** Each
  `*_edit_panel` (`source.rs:763-798`, `note.rs:601-630`, `repository.rs:693-736` and 10 more) opens
  with the same `let loc` / `let Some(form) = editing() else { return }` and closes over the same six
  `SidePanel` props; only the two `match`es — the title and the body — are the screen's own. All 15
  `SidePanel` call sites in the crate pass `footer: rsx! {}`, every panel's Save sitting in the body
  instead, so `components/layout.rs:71-72`'s footer slot is dead weight at every one of them. Wanted:
  `edit_side_panel<E>(loc, title, editing, body)` in `screens/shared.rs` beside the
  `retract_side_panel` it mirrors, and `footer` given `#[props(default)]` or dropped. Worth doing
  before #312, whose fix is to render the panel into a sibling layer of `.app` the way
  `shell/root.rs:132-135` mounts the overlays — a change that reaches every construction site, since
  the overlay layer is a root-mounted component reading a signal rather than a portal.
- **There is no label/value row primitive, so `.fact-row` is hand-rolled 39 times.** Every read-mode
  row is written out as `div.fact-row` + `span.field-label` carrying an inline
  `style: "width:NNpx;margin:0"` + a `span.grow` — 39 sites across 11 files, 8 of them in
  `tabs.rs:198-233` alone — with the `unwrap_or_else(|| "—".to_owned())` empty-value fallback repeated
  36 times beside them. The per-site width is *not* the defect: `docs/mockups` uses nine different
  label widths across 169 specimens, so the width is a call-site decision by design. The gap is that
  there is no `FactRow { label, label_width, children }` component to make that decision *in*, which is
  also why #309/#310 have nowhere to land their shared half — the `components/draft_field.rs` change
  from stacked `.field` to same-line `.fact-row`.
- **Collection history nodes cannot be expanded and show no count.** `collapse_runs`
  (`vitni-app/src/history.rs:247-302`) folds a software run into one synthetic
  `ActivityDetail::ImportBatch { count }` row and **discards the children**, and `ActivityVm`
  (`view_model/history.rs:99-122`) has no count field either — the number survives only baked into the
  localized sentence. Wanted: keep the children, show the count muted beside the node, and make the
  node disclose — two levels on the Dashboard's Recent Activity (collection → record → that record's
  entries), one on a record's History tab. There is no disclosure primitive to build on: no
  `Disclosure` component and no `<details>` anywhere in `components/`, only ad-hoc `aria-expanded` on
  four unrelated widgets.

### Lists, search & scale

- **Long-list / overflow specimen (U30)** — no tab demonstrates a long-list or overflow state;
  deferred as low-fidelity in a static mockup (the a11y real-app walkthrough covers it).
- **`ListPane` DOM virtualization** — `master_detail.rs` mounts every row (and a `MountedEvent` per
  row). Render only a scrolled window with a `store.count`-sized spacer and make the roving-focus
  `nodes` bookkeeping window-aware. If server-side windowing is chosen instead, add
  `list_view_page(table, offset, limit)` (+ a Postgres mirror) — the `human_id` column and indexes
  it would order/page by already exist (ADR 0032). Overlaps the `list_*` pagination item under
  *Performance & scale*.
- **Saved searches** — nothing in the palette, list toolbars, or app layer; the 100k-scale research
  workflow argues for it. Needs a design + use-case decision.
- **Column chooser** — `list.rs` has no column state though PR3's text claims "columns". Decide
  whether to build it or amend the PR3 description.

### Keyboard & shortcuts

Residuals from the shortcuts work (ADR 0030); see
[`archive/completed-work.md`](archive/completed-work.md). Deliberate non-goals are under *Decided*.

- **Chord entry is a typed canonical string, not live key capture.** `keydown` is inert under SSR and
  `cargo xtask input-guard` forbids a raw form element outside the primitives, so the Preferences
  rebind field takes `mod+shift+alt+key` text rather than a press-the-keys capture widget.
- **No chord sequences beyond the existing `g`-prefix** — `resolved_shortcuts` resolves single chords
  only.
- **The framework-free `Key` enum (`vitni-ui::shortcuts`) is still closed** — no function keys, so
  `e`/`F2` (the within-screen edit chord) could not be rebound even if that group were opened up.
- **No keyboard topic in the in-app Help browser.** `vitni-ui::help.rs`'s `HelpSection::Reference`
  is documented as "Lookup material (shortcuts, glossaries)" and `Run::Kbd` is unused — no authored doc
  covers shortcuts; the `?` overlay is the only in-app reference today.

### Pedigree & charts

- **`Restriction` chart cue** on the pedigree chart.
- **Name-autocomplete pickers** for the focus / relationship inputs, which are plain `human_id` text
  fields today.

### Geography & map

- **The zoom-interpolated marker radius is still unasserted.** The other half of this bullet is closed:
  the marker load-race stash (`__geoPending`, `screens/map_shared.rs`) is now exercised by the
  `gui-pass` map scenarios, because a shot in which markers are painted at all can only have reached
  them through the stash. The zoom-interpolated `circle-radius` has no such witness — `map-zoom.toml`
  asserts that the camera moved, which tiles alone satisfy — so a regression that pins the radius would
  pass every scenario. Both live inside `format!`-built JavaScript no test inspects, which is why the
  proof has to be a shot rather than an assertion over markup. Cheapest closure: a `region` compare
  over a marker between two zoom levels. This bullet is the cited evidence for the `type/test-gap`
  label in [`issue-tracking.md`](issue-tracking.md) §2.
- **The map's draw-tool state machine sits above the ADR 0008 line.** `DrawTool`
  (`screens/map_shared.rs:27`), `MapDraft` (`:47`), `draft_actions` (`:94`), `draft_geometry` (`:114`,
  which encodes the ≥3-vertices-for-a-ring rule and the refusal of a tool/draft mismatch), `geo_point`
  (`:59`, the decimal-degree → `Microdegrees` rounding), `move_vertex` (`:355`), `parse_map_message`
  (`:315`), `shape_to_draft` (`:902`), `combined_bounds` (`:933`), `closed_ring` (`:1008`),
  `format_zoom` (`:477`) and `save_year` (`:1078`) name no framework type and are already unit-tested
  as pure functions (`map_shared.rs:1165`), while their siblings `clamp_zoom`, `clamp_slider_year`,
  `display_coordinates` and `MarkerShapeVm` live one layer down in
  `vitni-ui/src/view_model/geography.rs`. A second renderer would re-implement the editing rules
  rather than reuse them, against ADR 0008 §7 and
  [`second-renderer-checklist.md`](second-renderer-checklist.md) §2 ("the renderer holds no domain
  rules or coordination"). The cut is the state machine only — the GeoJSON builders, the `*_script`
  consts and everything taking a `Signal` are MapLibre/Dioxus-specific and stay — so this is a lift of
  roughly 200 lines out of 2155, not a split of the file. Counter-case on the record: ADR 0008 does
  put app screens in the renderer as per-framework view code, and no second renderer exists; what
  makes this more than size is that `draft_geometry` returns a domain `PlaceGeometry` and decides
  whether one is valid.
- **In-map editing depth** — mid-ring vertex insertion (today: a vertex can be dragged to a new
  position, but not inserted between two others), pin-click selection on the canvas (today: select via
  the rail list), and polygon-drawn creation of a *new* place (today: point-drop creation is wired;
  polygon draws onto an existing place).
- **Provider sub-forms** — the toolbar select switches among every provider `[map.providers.*]`
  declares (ADR 0033), live and persisted; there is still no in-app form to *enter* a style URL or
  API-key env name — a provider has to already exist in the config file before the select can choose
  it.
- **`ai_config`'s `for_workspace` config-store path is dead, like #283's root cause was.**
  `services::ai_config` (`services.rs`) reads through `FileConfigStore::for_workspace(dir)`, whose
  `config_path` is `None` — the same defect `map_config` had — but it survives unnoticed because its
  `Err` falls back to `services.config.ai` (the already-loaded global config), which happens to be
  correct. `set_plugin_grants`/`store_plugin_enabled` use the same store for genuinely
  workspace-scoped writes and are fine; only the client-scope reads/writes routed through it are
  affected. Found while fixing #283 (ADR 0033); not fixed here since nothing currently observes the
  bug.
- **The Google Map Tiles adapter (ADR 0033) is untested against the live service.** `resolve_map_source`'s
  session mint, tile-URL template, and `refresh_map_attribution`'s viewport-copyright fetch are unit-
  tested on their pure seams (URL builders, `{key}` substitution, response deserialization) but never
  against `tile.googleapis.com` itself — that needs a billed Google Maps API key nobody has wired into
  CI. Exercise it manually with a real key before shipping the Google kind as supported, or add a
  feature-gated integration test once a key is available. Note that Google's **Maps Demo Key**
  (`developers.google.com/maps/demo-key`) is not such a key: it covers the Maps JavaScript API and a
  handful of web services, not the Map Tiles API, so `createSession` answers it with `403 Forbidden`
  (confirmed 2026-08-11). The failure is now legible — the error carries Google's own message, not just
  the status — but the adapter still has no end-to-end proof.
- **Tile caching** — tiles are fetched by the webview, and `main.rs` never gives dioxus-desktop a data
  directory, so they land in WebKit's default unmanaged cache at a path the app neither controls nor
  can bound. A viewport-only, size- and TTL-bounded disk cache is possible without leaving Rust: serve
  the raster source from a `use_asset_handler("tiles", …)` route, read/write
  `project_dirs()?.cache_dir()` (no cache-dir helper exists in `vitni-app/src/config.rs` yet), and
  fall through to `reqwest` with a real `User-Agent`. That seam is also the natural place to enforce
  `MapConfig::net_allowlist`. Constraint: `research/geography-rendering.md` commits to caching no more
  than the browser does, so bulk or offline prefetch stays out and the wording needs amending along
  with any change here. — #262
- **`map-provider` plugin world + geocoding** — the declarative provider ships and the Geography
  toolbar search is now a `RecordPicker` over existing places (search + jump); geocoding a *new*
  real-world address to a coordinate stays deferred. A WASM `map-provider` world supplying geocoding
  \+ custom tile-source descriptors over `net` is the ADR 0025 §4 follow-up (supplies data/descriptors,
  never pixels).
- **The map view opens pre-fitted, and three `gui-pass` scenarios have failed since 2026-08-11.**
  `map-view`, `map-zoom` and `map-repaint` fail identically on `main` against the same fixture
  (re-confirmed 2026-08-12 while closing #301, and again 2026-08-13): `map-view` at RMSE 0.0032 for
  `01-map.png` vs `02-fitted.png`, `map-repaint` at 0.0268 twice, `map-zoom` at 0.0000 over the
  readout region — so the suite cannot be green, and every unrelated change has to re-establish that
  these three are not its fault. Not a tile failure: `map-view`'s first shot shows a fully painted OSM
  basemap already framed on the fixture's one place at z4.0, so **Fit has nothing left to change**, and
  neither `workspace.toml` nor the seeded config carries a camera — this is the map view's initial
  camera, not fixture state. The other two are separate symptoms on the same screen: the zoom readout
  does not follow the camera after a `NavigationControl` `+`, and clicking the already-active tool
  repaints the window when `map-repaint` asserts it changes nothing.

### GUI ⇄ CLI parity

Every per-aggregate verb the CLI exposes has a GUI counterpart (audited
`vitni-cli/src/{main.rs,commands/*.rs}` against `vitni-ui/src/{navigation.rs,intent.rs}`); each
reuses an existing `vitni-app` use-case rather than a new core verb. One parity gap remains, filed
in its own area: research notes (*Notes & research notes*). The one gap running the other way:

- **Restrictions cannot be set from the CLI on any aggregate** — `restrictions_tag`
  (`vitni-cli/src/i18n/person.rs:35`) renders them on record output, but no command writes them:
  no `Restriction` `ValueEnum`, no `set-restrictions` verb. The app use-cases exist per aggregate and
  the GUI wires all thirteen, so privacy is GUI-only.
  *Shape:* one shared `ValueEnum` plus a `set-restrictions` subcommand per aggregate over the existing
  use-cases and `restriction_label`. Not a pre-1.0 gate — the milestone rule is GUI reachability. — #225
- **`person add-participation` cannot cite the participation it asserts** — it takes `--role`, ages,
  attributes and notes, but no `--citation`/`--confidence`, while the model, the app use-case and the
  GUI's per-row *Cite* action all carry them. A person's Events and Timeline tabs read the
  *participation's* citations, not the event's, so everything the CLI links reads `⚠ No source` and
  `No judgment` however well the event itself is cited. Found while seeding the `screenshots` demo
  workspace (#328), which is why its person views are not in the README.
  *Shape:* the `--citation`/`--confidence`/`--rationale` trio `person add-name` already takes, threaded
  into the same participation command.

## Import, export & plugins

### Bulk import, export & sync

- **Gramps `<header created>` is parsed but never threaded to `begin-import`** — ADR 0029's
  timestamp-gated reconciliation is only wired on the GEDCOM side
  (`plugins/gedcom-import/src/lib.rs:41`). `plugins/gramps-import/src/lib.rs` goes straight from
  `parse` to the person loop and never reads `db.header`, though `vitni-gramps-xml/src/parse.rs:400`
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
- **Lift `prepare_import_target`** into `vitni-app::workspace_registry` — still inline in the CLI
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
  follow-up (`vitni-core`/`vitni-app` already model citation `MediaRef`s).
- **Politeness delay for `net`.** The archive `robots.txt` requests `Crawl-delay: 5`; `net` enforces
  a timeout and size cap but no inter-request delay (the assisted flow is interactive and low-volume
  today). A politeness delay is a follow-up if usage grows.
- **`AiProvider::Plugin` is declared but unsupported** — the variant exists in `[ai]` config and
  round-trips, but `vitni-plugin-host/src/ai.rs:73` returns `AiError::InvalidInput` for it, so a
  workspace configured with `kind = "plugin"` fails only at first use. Either implement it or reject it
  at config-load time.

### Round-trip gaps

- **`SUBM`/other `HEAD` metadata** — deferred to its own future ADR-gated item; no core-domain concept
  (a document-level submitter/owner) exists to map it onto yet.
- **RichText `translations`** (text+language, distinct from `translator`) — GEDCOM 7 has a real target
  (`NOTE.TRAN`), but `vitni-gedcom` has no structured `Note` model yet (notes are bare strings);
  blocked on that prerequisite rewrite. Gramps has no equivalent construct.
- **`Address` on the Gramps side** — `vitni-gramps-xml` has no `Address` concept at all, so
  `Address.original_text` (which round-trips on the GEDCOM side now) has nowhere to go there; and
  `original_text` has no Gramps DTD equivalent even once an Address type exists.
- **A citation's transcription (`SOUR.DATA.TEXT`)** — a transcribed source text is an attached
  `NoteType::Transcript` note (data-model §6), and both formats have a target for it: GEDCOM
  `SOUR.DATA.TEXT` and a Gramps citation note. Neither direction is wired. `vitni-gedcom`'s `Citation`
  (`crates/vitni-gedcom/src/model.rs:80-85`) carries only `source_xref` + `page`, so importing the text
  means parsing it and having `plugins/gedcom-import` create the Note aggregate and attach it (and the
  reverse on export). Shares the blocker the `NOTE.TRAN` bullet above names: no structured `Note` model
  in that crate yet. — #344

### Plugin-UI vocabulary

ADR 0022 out-of-scope tail:

- Repeating groups / nested forms.
- `List`/detail descriptions + plugin-driven navigation.
- Per-field validation vocabulary.
- Plugin-prefilled field values.
- **A plugin form's fields are uncontrolled.** `field_input` (`vocabulary_render.rs:95-179`) binds no
  `value:` on Text, Textarea, Number or Date, so any re-render of the enclosing `FormView` blanks them
  in the live webview (the volatile-`value` mechanism in `components/text_input.rs`'s header). Not
  reachable today — `FormView` reads `values` only in `use_signal`'s initializer and in the action
  button's `onclick`, so it subscribes to nothing the typing writes, and its props are identity-stable
  under an outer re-render — but one added read makes every plugin field lose what was typed. Fixing it
  is not the one-line binding the record pane needed: `values` holds the *parsed* JSON a plugin action
  submits, and round-tripping `Number` through it fights the typist ("1." re-renders as "1.0"), so the
  numeric field needs a raw-text buffer, and per-field hooks are ruled out by the dynamic field list.
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
- **Viewport-scoped loading** — `show_geography` loads every place with a resolved geometry rather than
  calling `places_in_bbox` for the current viewport; wire the spatial query in when place counts grow
  (needs a Postgres fallback given the row above).

### Packaging & release

- **The two `.deb`s cannot both be installed.** `vitni` and `vitni-cli` each list
  `["../../target/plugins/**/*", "usr/lib/vitni/plugins/", "644"]`, so the second `dpkg -i` fails with
  "trying to overwrite … which is also in package". Inherited from the `vitni`/`vitni-gui` split rather
  than introduced by ADR 0035, and only reachable by someone who wants the launcher *and* a headless
  CLI on one machine — but that is a reasonable thing to want. Either declare
  `Conflicts`/`Replaces`, or split the fleet into a `vitni-plugins` package both depend on (which is
  also where a default *system* embedded path would want to live — see the `VITNI_PLUGIN_DIR`
  item, #212).
- **Cross-platform packaging** — 1.0 is Linux-first (tarball + `.deb` + AppImage). macOS/Windows
  bundles and **OS-level code-signing / notarization** (Gatekeeper, Authenticode) are a later cycle
  (ADR 0014 §Out of scope). — #215
- **An API key can only reach the app through the launching shell.** Both places that take a key name
  one as an *environment variable* and read it with plain `std::env::var` at use time — `[map]`'s
  `api-key-env` (`map_source.rs`, ADR 0033) and `[ai]`'s `api_key_env` (`plugin_host/src/ai.rs`, ADR
  0017 §4) — and nothing in the workspace loads a `.env` file (no `dotenvy`/`dotenv` dependency
  exists). `VAR=… cargo run` works; a desktop launcher (`.desktop`, AppImage, `.deb`) starts the GUI
  with no shell profile, so a keyed MapLibre style, the Google provider, and every `vision-api` AI
  provider are unreachable from an installed build with no way to say so in config. Proposed shape:
  load `<workspace>/.env` then `~/.config/vitni/.env` at startup in `vitni-app`, never
  overriding an already-set variable, both gitignored — the key stays out of config files, logs and
  the event log either way, which is the whole point of naming a variable rather than storing a
  secret. — #296
- **`.deb` needs `VITNI_PLUGIN_DIR`** — the embedded plugin layer has no default *system* path, so a
  distro-installed binary needs `VITNI_PLUGIN_DIR=/usr/lib/vitni/plugins` (the AppImage sets it
  via `AppRun`; the tarball resolves the fleet beside the binary). Teaching the embedded layer a default
  system path so an installed `.deb` finds the fleet with no env var is the follow-up (see
  [`release.md`](release.md)). — #212
- **Real release keys not yet generated** — only the deterministic **DEV** signing key exists (Sanctioned
  in debug builds only), so `embedded_sanctioned_keys()` is `None` in a release build until one is
  configured. Before the first real release, generate the release ed25519 keypair, set the private half
  as the `VITNI_PLUGIN_SIGNING_KEY` repo secret, and embed the public half via
  `VITNI_PROJECT_PUBLIC_KEY` (ADR 0014 §6; procedure in [`release.md`](release.md)). — #210
- **The embedded plugin-dir resolver is duplicated *and* divergent** — the ADR 0014 §4 *layering* is
  shared (`vitni_app::plugin_layers`), but each frontend still resolves the embedded layer itself
  and the two disagree on the dev fallback: `vitni-ui-dioxus/src/app.rs:326` uses
  `CARGO_MANIFEST_DIR/../../target/plugins` (source-tree-absolute) while
  `vitni-cli/src/commands/io.rs:83` uses a bare `target/plugins` **relative to the working
  directory**. So a CLI invoked from anywhere but the repo root silently finds no embedded fleet while
  the GUI always finds it. The Phase 11 plan called for replacing this duplication; only the layering
  half landed. Fold both into one `vitni-app` resolver — the same change that would give an
  installed `.deb` a default system path (item above). — #213
- **No `[profile.release]` section** — the Phase 11 plan's "strip/optimize release profile" was not
  done: the root `Cargo.toml` has no `[profile.release]`, so shipped binaries carry full debug symbols
  and default codegen settings. `strip = true` plus a considered `lto`/`codegen-units` is the cheapest
  size win available before the first tag. — #214
- **`release.yml` unverified end-to-end** — no version tag has been pushed, so the release workflow is
  zizmor / YAML / `bash -n` verified and its build/package steps reproduced locally, but has never run a
  full tag → AppImage → GitHub Release cycle. The first real tag is that verification, and wants
  watching. — #211

### Dependencies blocked upstream

- **`sqlx` 0.9** — `sqlite-es` / `postgres-es` 0.5.0 (their latest) pin `sqlx` 0.8, so a bump splits the
  tree into two `sqlx` versions and every `Pool<Sqlite>` / `Pool<Postgres>` handed to the event stores
  fails to typecheck (17 mismatches). 0.9 also replaces `&str` query input with `SqlSafeStr`
  (38 `sqlx::query(&format!(…))` call sites in `vitni-db` need `AssertSqlSafe`). Re-evaluate when the
  `*-es` crates release on `sqlx` 0.9.
- **`ed25519-dalek` 3.0.0** — needs `curve25519-dalek` ^5.0.0, while `russh` (via the `testcontainers`
  dev-dependency) needs `curve25519-dalek` =5.0.0-pre.6. Cargo resolves the conflict by downgrading to
  `russh` 0.60.1, which does not compile against the `pkcs5` 0.8.1 that ed25519-dalek 3.0.0 pulls in.
  Held at `=3.0.0-pre.6` (comment in `Cargo.toml`); re-evaluate when `testcontainers` ships a `russh` on
  stable `curve25519-dalek`.

### Docs & repo tooling

The `area/docs` label already existed with no `###` home; this is it.

- **The mockups' record-picker specimen pins itself to the viewport's top-left.** Every mockup that
  includes a `.picker-results` list — `design-system.html:327-332`, `family.html:178`,
  `citation.html:193` — renders it as a floating box over the top-left corner of the page, which is
  where the walkthrough found an unexplained "Berg, Anna / Lovelace, Anna / + New person" dropdown.
  `mockups/assets/components.css:832-840` gives the class `position: fixed` at `--pk-top`/`--pk-left`,
  custom properties **only the renderer sets** (from `getBoundingClientRect`), so in a static page they
  default to 0. Introduced by `9e9c983`, which made the app's picker a floating dropdown and updated
  the shared sheet without the pages that use it; the specimen's own prose still promises "an in-flow
  result list (never a floater…)" (`design-system.html:319-322`), so the mockup now contradicts both
  the app and itself. The app rule at `components.css:997-1006` is correct and unaffected. — #311
- **`gui-pass` occasionally grabs a blank first shot.** Once in roughly a dozen runs the first `shot` of
  a scenario comes back uniform (`… is blank (standard deviation 0) — the webview painted nothing`) and
  the run aborts, passing on a re-run with nothing changed. The startup handshake in
  `xtask/src/gui_pass.rs` waits for the window to map and then settles once; a paint the harness can
  actually observe (a shot that must not be blank, retried) would make it deterministic instead of
  making the first assertion of every scenario flaky. A second flake class was observed on 2026-08-04:
  `map-polygon` failed its "a second vertex must draw the ring's first segment" `differ` at RMSE 0.0000
  during a full-suite run and passed on an immediate re-run of the same scenario — a draw that had not
  reached the canvas within the 4 s settle. Both classes are the same missing capability: the harness
  waits a fixed time instead of waiting for the paint it is about to assert on.
- **`main` is protected without required status checks, and that is a choice.** `ci.yml` filters
  `docs/**`, `*.md` and `LICENSE*` out of its triggers, so a documentation-only pull request starts no
  run at all — a required context would sit unfulfilled forever on exactly the changes this repository
  produces most. The protection therefore requires a pull request (0 approvals, admins included) and
  blocks force-push and deletion, but requires no check. Making the four CI jobs required means first
  dropping those `paths-ignore` filters and paying runner minutes on every doc PR; free on a public
  repository, so it is a live option, not a closed one.
- **Paid-plugin distribution is unshaped.** The plugin boundary is the only genuinely exclusive
  revenue line (a signed bundle whose source stays private, under its own EULA — selling licence
  keys over an OSI-licensed source does not hold), and it needs decisions before the first one
  exists: which ADR 0014 trust tier a paid first-party bundle gets, how a purchase becomes a
  downloadable signed bundle through a merchant of record, and how the free build stays
  unmonetised so it remains outside the CRA (Commission guidance C(2026) 5252 ¶52 — reporting
  duties arrive 11 September 2026, main obligations 11 December 2027). Also unresolved for
  `vitni-digitalarkivet` specifically: the National Archives' service name in a paid product's
  title, and the redistribution terms of the verbatim fixtures under its `tests/fixtures/`.

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
  `~/.config/vitni/config.toml`, consistent with `[map]`/`[ai]`/`[plugin_trust]` — a keymap is
  machine/user-local, not a dataset property.
- **No VS Code-style *when* context.** No context predicates are defined; a design question if a real
  need surfaces, not a missing implementation.
- **`⌘S` targets the active record, falling back to the docked one, never both.** With a split open,
  `NavState::request_save_active` saves whichever of the two has something savable — the active pane
  first, the docked pane only when the active one has nothing to save. It never saves both at once.
- **`⌘N` stays inert while the palette / help sheet / close-quit confirm is open.** `.app` carries the
  keydown dispatcher and is `inert` under any of those (`shell/root.rs`, the ARIA APG modal pattern), so
  no `Global` chord reaches it — deliberate, and the palette already offers `Create` commands as a
  from-under-a-modal path (#300).

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

### Shell & panes

- **A shell-wide ticket a detail pane consumes must be addressed, not bumped.** #284 filed the class:
  `pending_undo` was a counter every mounted pane latched a per-pane `seen` against, so with a record
  docked one `⌘Z` retracted the newest assertion of **both** panes' records. Fixed with #279 by making
  the ticket carry the `EditKey` it is aimed at — the address *is* the latch. `save_request`/`save_queue`
  and `edit_drafts` were already keyed and were never broken, only mis-documented as safe "because only
  the active tab's pane is mounted". Undo stays **active-record-scoped** with a split open, which is the
  locked decision; the rule to carry forward is that a second mounted pane makes any bare counter wrong.

- **Duplicate `id`/`aria-controls` cannot make a handler inert.** #279 read a docked split's dead tab
  clicks as the duplicate element ids two mounted `Tabs` strips emit. Measured against the interpreter:
  dioxus-desktop resolves an event target by walking up to the nearest `data-dioxus-id`, which is
  stamped per `ElementId` and unique by construction — the document `id` is never consulted, so
  duplicate ids cannot stop an `onclick` firing. The primary pane's subtree is not even re-diffed when
  a dock opens (`MasterDetail` hands the same `detail` `Element` back down, `VNode: PartialEq` is
  `Rc::ptr_eq`, and `diff_node` short-circuits), so its listeners are untouched. The real cause was
  layout: `.master-detail.split-2` collapsing at the default window width, and a tab strip that moves
  when a pane halves. The ids **were** scoped per pane anyway — two elements claiming one id is a real
  ARIA defect — but scoping them fixes accessibility, not the clicks. Do not reach for id collisions to
  explain a dead Dioxus handler.
- **The tab strip's hit region never sat above its painted row.** #285 read the dead clicks as a
  ~9–14px vertical offset between paint and hit test, and the two `docked-*` gui-pass scenarios were
  written aiming above each label because of it. Column-scanning the shots (`convert <shot> -crop
  1xH+X+Y +repage txt:-`) falsified that: the "works" coordinate and the "does nothing" coordinate were
  **inside the same painted button** — at 1800×1200 the row paints y 231–266 and both 240 and 249 are in
  it — and the strip paints at identical y single-pane, where a click at 249 works. A click ladder
  across the row then put the boundary between y=245 (lands on the tab) and y=247 (does not), with
  y=262 visibly *scrolling the strip sideways*. The mechanism was `.tabs`' own horizontal scrollbar:
  `WebKitGTK` hit-tests one over the bottom ~20px of a 36px row, and it is there whenever the strip
  overflows — a docked pane, or a single pane at the app's own 1280px default window — even when its
  indicator is not painted. Fixed by suppressing the scrollbar; the right-edge fade already signals the
  overflow. Two lessons: a dead click on a scrolling strip is a scrollbar before it is a hit-test bug,
  and a whole-pane `differ` cannot tell a tab switch from a sideways scroll of the strip, which is how
  the #279 scenarios passed while the defect was live. `tab-strip-overflow.toml` is the regression
  test, and it needs no dock at all. The record tabstrip's `.tabs-scroll` had the same defect on a
  34px row, measured rather than inferred from the shared rule — `record-tabstrip-overflow.toml`,
  where a tab's close `✕` was dead too. One trap worth carrying forward from writing it: a `✕` probe
  aimed at a *draft* tab proves nothing, because the unsaved-draft confirm paints over the same region
  the assertion measures, so it passes on the modal rather than on the close.

  **The overflow is the precondition, which is why a casual check does not see this.** A manual pass on
  a real GPU during the investigation found the tab hit box fine — expected: a wide single pane fits the
  whole strip, so there is no scrollbar and no dead band. It is not evidence of an Xvfb artifact, and
  the mechanism cannot be one — a paint/compositing misalignment cannot make the strip *scroll*, which
  is what the failing clicks did. To see it by hand, put a Place in a pane narrow enough to clip the tab
  row (dock a second record, or use the app's own 1280×840 default window) and click a label's centre.

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

### Publication

Recorded when the repository was made public, so none of it is re-audited.

- **The pre-publication scan found nothing to remove.** gitleaks over **all refs**
  (`--log-opts="--all --full-history"`, 649 non-merge commits) returned two hits, both false
  positives: a `"0123456789abcdef".repeat(4)` test literal in `vitni-ui/src/view_model/plugin.rs`, and
  the page token inside the verbatim Digitalarkivet capture at
  `crates/vitni-digitalarkivet/tests/fixtures/census/viewer.html`. No `.pem`/`.key`/`.p12` has ever
  been tracked, and no non-source file has ever been deleted from history. The in-tree
  `signing::DEV_SEED`/`DEV_PUBLIC_KEY` is a real ed25519 seed and deliberately committed — no scanner
  pattern matches it, and a release build never trusts it (ADR 0014 §6, `SECURITY.md` §What is not in
  scope). No personal genealogy data is committed: nothing tracked is a `.ged`, `.gramps`, `.csv` or
  database file, and the `gui-pass` and screenshot fixtures are seeded at runtime from invented data.
- **Two one-way doors were accepted at publication.** The author's commit address
  (`magne.rasmussen@gmail.com`) is now public across all 906 commits, and the verbatim Digitalarkivet
  captures under `crates/vitni-digitalarkivet/tests/fixtures/` are third-party content whose
  redistribution terms are unexamined — no blocker for publication, a real question if a paid importer
  ever ships.
