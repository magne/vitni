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

No open defects here. The two this preamble used to name are fixed and their bullets deleted per §6 of
[`issue-tracking.md`](issue-tracking.md) — *Postgres place-detail reads fail outright* with #231 and
*map markers label with the first-asserted name* with #232 — but the preamble was not updated with
them, so it went on advertising bullets the file no longer held. The five Phase 9 map/geometry bugs are
fixed and archived; the two of those fixes that ship without test coverage now have the bullet
[`archive/completed-work.md`](archive/completed-work.md) always claimed they had, under *Geography &
map*.

Open bugs take their `area/*` from whichever area they affect, plus `type/bug`, and live under that
area's `###` heading.

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

- **A `SidePanel`'s background is not `inert`.** The panel now traps and restores focus like `Modal`
  (#247), so neither `Tab` nor the pointer can reach the shell behind it, but assistive tech still
  can: `shell/root.rs` inerts `.app` for the overlays and the close/quit confirm, and every
  `SidePanel` renders *inside* `.app`, so inerting the shell would inert the panel with it. The fix is
  a layer the panel can render into as a sibling of `.app` (what the overlays already use), not
  another `inert` clause.
- **Back/forward cannot return to a draft tab.** `NavLocation.record` is `(Category, String)`, a saved
  record's key, so a draft is never recorded in the history and `⌘←`/`⌘→` step past it to the last saved
  record instead. True before several drafts per category (#260) and unchanged by it, but a strip that
  can now hold four drafts makes it easier to notice. `NavLocation` lives in `genealogy-ui`
  (framework-neutral, ADR 0008) and a history entry naming a draft goes dead when the draft is cancelled
  *or* committed, so the variant needs a rule for that first.
- **A save run whose target leaves the strip hangs.** `advance_save_run` arms the next queued `EditKey`
  and waits for `note_save_finished` to report it; nothing reports if that tab was closed or cancelled
  while armed, or if the record was saved by hand and then failed its own `can_save()` gate, so
  `save_request` stays set and the run's quit/close never fires. Pre-existing — a rename and a committed
  draft are both re-keyed (`rekey_save_run`), which covers the two identity changes, not disappearance.
  Several drafts per category (#260) make a long queue, and so a lost target, more reachable.
- **A draft tab reads "New People" while its pane heading reads "New person".** The tab takes the *rail*
  label (`nav-people`, plural, shared with the rail and the Explorer) and the create pane takes
  `person-new-title` (singular), so one screen shows both forms of the same thing. Needs 13 singular
  entity keys — the tab is the one that should change, since the pane heading reads correctly.
- **`draft-tab-label = Ny { $entity }` is grammatically wrong in `no` for every category.** The rail
  labels are plural ("Personer", "Steder"), so the draft tab reads "Ny Personer" instead of "Ny person".
  Pre-existing, and the singular entity keys above are the same fix seen from the other side.
- **A docked split mounts two detail panes, breaking `NavState`'s single-mount assumptions.**
  `edit_drafts`, `save_request`/`save_queue` and `pending_step` each document their design as safe
  *because* "only the active tab's pane is mounted" (`shell/nav_state.rs`), which a dock makes false.
  Audited one by one, only `pending_undo` is actually broken: `use_record_undo` (`screens/shared.rs`) is
  called by all 13 detail panes and latches a *per-pane* `seen` against a *shell-wide* counter, so one
  `⌘Z` retracts the newest undoable assertion of **both** records and two "Nothing to undo" notices race
  for `nav.notice`. `use_save_on_request` already compares an `EditKey`, and `use_record_step` has a
  single caller in `shell/explorer.rs`, so save and `[`/`]` stepping are correct — their stated reasons
  are not. The class is what matters: a shell-wide ticket a *detail pane* consumes must be addressed,
  not bumped. — #284

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

- **Two shipped map fixes have no test coverage.** The marker load-race stash (`__geoPending`,
  `screens/map_shared.rs`) and the zoom-interpolated `circle-radius` both live entirely inside
  `format!`-built JavaScript that no test inspects, so neither the SSR suite nor `gui-pass` would notice
  a regression in either. Recorded by the 2026-07-27 verification note in
  [`archive/completed-work.md`](archive/completed-work.md), which says the gap "is now tracked under
  *Geography & map*" — it was not, until this bullet. It is also the only cited evidence for the
  `type/test-gap` label in [`issue-tracking.md`](issue-tracking.md) §2.
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
- **Viewport-scoped loading** — `show_geography` loads every place with a resolved geometry rather than
  calling `places_in_bbox` for the current viewport; wire the spatial query in when place counts grow
  (needs a Postgres fallback given the row above).

### Packaging & release

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
  load `<workspace>/.env` then `~/.config/genealogy/.env` at startup in `genealogy-app`, never
  overriding an already-set variable, both gitignored — the key stays out of config files, logs and
  the event log either way, which is the whole point of naming a variable rather than storing a
  secret. — #296
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
- **`⌘S` targets the active record, falling back to the docked one, never both.** With a split open,
  `NavState::request_save_active` saves whichever of the two has something savable — the active pane
  first, the docked pane only when the active one has nothing to save. It never saves both at once.

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
