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

No open defects. The five Phase 9 map/geometry bugs are fixed and archived; two of those fixes ship
without test coverage, tracked below under *Geography & map*.

## Records & data model

### Person & Family

No open items. The area keeps its heading so `area/records/person-family` stays a live label and the
issues already filed against it keep resolving their [`#person--family`](#person--family) anchor.

### Places

- **Place succession can be read but never written** — `assert_place_succession` (ADR 0026 §3;
  `Merged`/`Split`/`Absorbed`/`Elevated`/`Renamed`) has no CLI subcommand and no `PlaceEdit` variant,
  while `show_place` / `show_place_as_of` already surface `PlaceSuccessionRef`s — so the Place screen
  can display a succession no user can create.
  *UI:* `PlaceEdit::AssertSuccession { to, kind, date }` and a "Succession" edit panel on the Place
  screen — target picked with the existing place `RecordPicker`, kind a `SelectInput`, date reusing the
  map-edit provenance date form. A `genealogy place assert-succession` subcommand is the cheaper first
  move if the CLI should stay the reference surface. — #196
- **Dated name/enclosure use-cases** — `add_place_name` / `assert_place_enclosed_by` don't accept a
  date param, so map/UI enclosure edits can't be dated (geometry edits already can); the map-edit
  provenance form doesn't yet default its date to the active time-slider year.
- **Optional DB `place_parent` index** — a Gramps precedent for scaling the hierarchy walk; a later
  follow-up, not needed at current volumes.

### Notes & research notes

- **`remove_translation` core verb** — note-translation retract is Edit-only; there is no verb to
  remove a single translation.

### Tags

- **Tag has no restrictions path** — `set_tag_restrictions` exists but no frontend calls it, and
  `TagChangeSetRequest` carries only name / priority / colour: the one aggregate of thirteen with no
  privacy control.
  *UI:* add `restrictions` to `TagChangeSetRequest` and reuse the shared restrictions field the other
  twelve screens already render. — #197

### Media

- **Interactive Set/Clear region on every owner.** The interactive region viewer is wired on the
  Person screen only; the other five media owners show the read-only rich gallery. The
  `SetMediaRegion` intent and dispatch exist for all six — extending the viewer wiring is mechanical. — #199
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

- **Workspace-scope surety labels are read but unwritable** — `save_surety_label_overrides`
  (`workspace.rs:589`, writes `manifest.surety`) has no caller; the ADR 0027 Preferences card writes
  `store_workspace_default_surety`, i.e. the *global* `[workspace-defaults]` table.
  `read_resolved_surety_labels` resolves manifest-over-global, so the per-workspace layer sits in the
  resolution chain with no way to populate it — what shipped is the global live fallback, not a
  per-workspace override.
  *UI:* add `store_surety_label_overrides` to `ConfigStore` (delegating to the existing function) and
  give the Surety card the same two-scope control the theme / id-format cards already use via
  `read_preference_layers` / `LayerKind`. — #198
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

- **Live list updates on create.** Creating an entity should immediately insert it into the matching
  entity list, with no manual refresh. — #207
- **Toast notifications.** Show a toast at the bottom of the work area, auto-dismissed after a set
  time. — #208
- **Remember the open record's tab.** Record-detail view should restore the last-shown tab while the
  record stays open, and forget it once closed. — #209
- **`Modal`/`SidePanel` overlay follow-ups** — `Modal` (`components/layout.rs`) still has no backdrop
  scrim or `onclose` prop. This was harmless while `Modal` had no callers; the close/quit confirm
  dialog (`shell/close_confirm.rs`) is now its first real caller and does **not** wire a focus trap
  (`shell/focus_trap.rs`'s `trap_tab` is not attached) or a click-away scrim — a keyboard user tabbing
  inside the dialog can reach the inert background, and there is no click-outside-to-cancel. Neither
  overlay has slide-in motion beyond what the existing keyboard layer already provides. — #201
- **Record-picker scroll-listener cleanup** — `PickerSearch::watch_scroll_close`
  (`components/record_picker.rs`) arms a `window` `scroll`/`resize` listener (via `document::eval`)
  per mount to close the floating picker on pane scroll, but never removes the JS-side listener on
  unmount, so each clear/re-search cycle leaves one inert listener behind (bounded by that, not by
  keystrokes or scroll events). Remove it on unmount, or arm it once at a higher scope. — #204

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

- **Dirty saved-record edits are not confirmed.** `Ctrl+W`/`Ctrl+Q`'s confirm fires on `OpenTab::Draft`
  only. An in-progress edit of an *already-saved* record lives in screen-local `RecordEditState`
  (`screens/record_form.rs`) and is invisible to `NavState`, so closing/quitting discards it silently.
  Lifting edit-dirtiness into shell state is the follow-up. — #200
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

- **Two shipped map fixes have no test coverage** — the marker load-race stash (`__geoPending` in
  `map_shared.rs`) and the zoom-interpolated `circle-radius` + white stroke both live entirely inside
  `format!`-built JavaScript that no test inspects, and `maplibre_init_script` is private with no test
  module. Both are verified present in code; neither would fail if regressed. Either assert the
  generated script text, or extract the paint expressions into testable Rust values. — #202
- **`geography_toolbar` takes 8 args** (`#[expect(clippy::too_many_arguments)]`) after the picker +
  fit state were threaded in — bundle them into a struct. Cosmetic cleanup.
- **Point tool has no confirm step in the Geography tool.** The Place Map editor added a "Use this
  point" confirm; the Geography tool's Point tool still has no equivalent (commits on click), a
  pre-existing inconsistency.
- **In-map editing depth** — true mouse-drag reposition and mid-ring vertex insertion (today: click to
  drop/move a point and click to add polygon vertices), pin-click selection on the canvas (today:
  select via the rail list), and polygon-drawn creation of a *new* place (today: point-drop creation is
  wired; polygon draws onto an existing place).
- **Provider sub-forms** — `osm-raster` is switchable from the toolbar; `maplibre-style` / `google` are
  declared in `[map]` config and round-trip but have no toolbar sub-form to collect a style URL /
  API-key-env yet.
- **`map-provider` plugin world + geocoding** — the declarative provider ships and the Geography
  toolbar search is now a `RecordPicker` over existing places (search + jump); geocoding a *new*
  real-world address to a coordinate stays deferred. A WASM `map-provider` world supplying geocoding
  \+ custom tile-source descriptors over `net` is the ADR 0025 §4 follow-up (supplies data/descriptors,
  never pixels).
- **Manual webview pass outstanding** — the interactive MapLibre canvas (pan/zoom, click-to-place feel,
  polygon vertex rendering, the toolbar picker) cannot be exercised by an SSR test; agents can't run
  libwebkit2gtk. — #203

### GUI ⇄ CLI parity

Every per-aggregate verb the CLI exposes has a GUI counterpart *except* the items below (audited
`genealogy-cli/src/{main.rs,commands/*.rs}` against `genealogy-ui/src/{navigation.rs,intent.rs}`).
Each reuses an existing `genealogy-app` use-case — none needs a new core verb. One more parity gap is
filed in its own area: research notes (*Notes & research notes*).

- **Projection rebuild is CLI-only** — `genealogy rebuild` → `Workspace::rebuild_projections`
  (`workspace.rs:712`, an ADR 0010 maintenance op). After a `genealogy-db` schema change there is no
  in-app way to run it.
  *UI:* a "Rebuild projections" button in a new **Maintenance** card in Preferences. It is a
  workspace-functionality op, not an aggregate, so it belongs on Preferences' documented
  direct-to-app path (like the Workspaces card), not behind `Intent`. Confirm in a `Modal`, disable
  while running, report the outcome through `NavState::notify`. — #192

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
- **Postgres spatial mirror** — `places_in_bbox` / `place_predecessors` / `place_successors` return
  `Unsupported` on Postgres (SQLite R\*Tree only); the native geometry + GiST index is a later
  feature-gated follow-up.
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

- **`LineString` / `Multi*` geometry variants** — the model ships `Point`/`Polygon`; the other
  variants are additive-later per ADR 0024 (grow the enum append-only when a concrete need appears).
- **Explicit `[from, until)` validity intervals** — the effective-from resolution rule (ADR 0026)
  ships; add intervals additively only if gaps/overlaps prove ambiguous in real data.
- **RichText `translator`** — permanently non-round-trippable: neither GEDCOM 7 nor the Gramps DTD has
  any tag for who translated a note's text.
- **Media DTO convention split** — `person-dto`/`family-dto`/`event-dto`'s `media` is `list<media-ref>`
  (host-api 0.21.0), but `source-dto`/`citation-dto`/`place-dto` have no `media` field at all (no
  exporter reads media off those three today); widening them is deferred until a real consumer appears
  (YAGNI).
- **External ids have no frontend entry point** — person `add_external_id` (module-level, not even
  root-exported) and `add_family_external_id` are used only inside `import.rs` for resolve-or-create.
  External ids are importer bookkeeping, not user-editable data.

### Architecture

- **Snapshotting is decided, not deferred-open** — measured and **not** warranted at target scale;
  ADR 0004's deferral stands, no follow-up ADR.
- **Server backend + web frontend** — roadmap-owned and deliberately unscheduled; see
  [`roadmap.md` Phase 13](roadmap.md#phase-13--beyond-10-server-backend--web-frontend). Builds on the
  config split: the server adds the `ConfigStore` **database** backend so the operator and
  client/presentation scopes persist per authenticated user, while the embedded build keeps the file
  backend.
