# Decided — no action needed

Deliberate non-tasks, recorded so they are not re-raised or read as unfinished work. Each is a
decision, not a gap. Open work lives in [GitHub Issues](https://github.com/magne/vitni/issues); when
an issue closes as by-design, impossible or deferred indefinitely, the reasoning lands here in the same
change ([`issue-tracking.md`](issue-tracking.md) §4). Check here before filing or "fixing" something.

## Keyboard & shortcuts (ADR 0030 §2, §3, §Out of scope)

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

## Assisted import

- **Two Digitalarkivet records of one individual import as two persons, until record matching.** A
  person resolves by the `digitalarkivet` `ExternalId`, which is the record id: a re-run of the same
  record is a no-op (`re_running_the_same_url_imports_no_duplicates`), but the 1900 census, the 1910
  census and a church-book entry for one individual carry three ids, so they make three persons.
  Recognising them as one is the 0.10 record-matching milestone (ADRs 0037–0040), not an import fix.
  #388 reported this as a re-import duplicate. The reporter could not reproduce it, and none of the
  suspected paths is at fault: a stale plugin build (every build resolves by `ExternalId`), an
  `og:url` variant (the record id is the last path segment), the GUI's workspace wiring (the same
  directory the Explorer reads), or church-book participants sharing a `pd…` id (each has its own).

## Record matching

- **Name-culture packs are filed when a user's data needs one.** ADR 0038 §5 makes a pack a TOML file
  plus corpus cases, with no code change, so `pl` and `pl-en`, `sv`, `de`, `fi` … are not tracked ahead
  of need. An issue for one is filed when real data shows names the shipped packs do not handle.

## Model & interchange

- **`LineString` geometry variant** — the model ships `Point`/`Polygon`; `LineString` is additive-later
  per ADR 0024 (grow the enum append-only when a concrete need appears), and nothing wants it yet.
  **`Multi*` is no longer deferred** — the concrete need ADR 0024 named for itself arrived, so it is an
  open issue (#569).
- **Explicit `[from, until)` validity intervals** — the effective-from resolution rule (ADR 0026)
  ships; add intervals additively only if gaps/overlaps prove ambiguous in real data. Still closed after
  the Norwegian-geography review, which found the gap intervals would address (a dissolved place
  resolving forever) but concluded they are the wrong-sized fix: only a place's *lifetime* needs an end,
  not every assertion, so the open issue (#573) is a read-side `existed_as_of` instead.
- **RichText `translator`** — permanently non-round-trippable: neither GEDCOM 7 nor the Gramps DTD has
  any tag for who translated a note's text.
- **Media DTO convention split** — `person-dto`/`family-dto`/`event-dto`'s `media` is `list<media-ref>`
  (host-api 0.21.0), but `source-dto`/`citation-dto`/`place-dto` have no `media` field at all (no
  exporter reads media off those three today); widening them is deferred until a real consumer appears
  (YAGNI).
- **Repository carries no media refs (#584).** Neither interchange format has a slot: the Gramps DTD's
  `repository` is `(rname, type, address*, url*, noteref*, tagref*)` and the GEDCOM 7 `REPO` record has
  no `OBJE`, so repository media would be vitni-only data that every export drops. Media on a repository
  (a building photo, a logo) is not evidence for any conclusion, and the useful cases are already
  covered: a finding aid or reading-room page is a repository `url` or note, and a digitised catalogue is
  a Source with its own media.
- **External ids have no frontend entry point** — person `add_external_id` (module-level, not even
  root-exported) and `add_family_external_id` are used only inside `import.rs` for resolve-or-create.
  External ids are importer bookkeeping, not user-editable data.

## Lists

- **No long-list or overflow specimen in the mockups.** Closed at triage (#589, 2026-10-13): a long or
  overflowing list is low-fidelity in a static mockup, and the real app is where it is judged — the
  list virtualization work (#590) measures it there.
- **Tags and import runs are not paged.** #607 gave the twelve lists ordered by `human_id` a page read
  (`Store::list_*_page`, `vitni <kind> list --limit/--offset`). A tag has no `human_id` and its list is
  a vocabulary of tens, and the import-run list sorts by start time and needs every run to know which
  one a dataset resumes from. Both stay whole lists.

## Shell & panes

- **Long rail labels ellipsize rather than widen the rail.** #375 widened `--rail-w` from 208px to
  232px, enough for every English label in bold with its count, and made the label single-line
  (`.nav-label`, ellipsis, full text in `title`). A rail wide enough for the longest Norwegian labels in
  bold (`Forskningsnotater` with a 3-digit count, `Sammenlign / slå sammen`) would need about 250px, and
  every pixel comes out of the work area. So those two truncate by design. What must never happen again
  is a label wrapping or pushing its count out of the row; `rail-active-label-fits.toml` guards the
  wrap.

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

## Geography map

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

## Architecture

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
- **No PostGIS; Postgres indexes place geometry with its built-in `box` + GiST (#608).**
  [`research/gis-norway.md`](research/gis-norway.md) recommended requiring PostGIS. It would not make
  the engines agree: PostGIS's `&&` also compares 32-bit boxes, so a match still needs the exact `f64`
  check both engines now share (`crates/vitni-db/src/geo_index/mod.rs`). Its GEOS predicates could also
  answer boundary cases differently from SQLite, which has no GEOS. Requiring it would add a
  `CREATE EXTENSION` (superuser) step to every Postgres workspace. Containment (#575) follows the same
  pattern: the index finds candidates and one Rust predicate decides, on both engines.

## Repo tooling

- **`gui-pass` settles every step, not only before a `shot`.** Giving inputs a short fixed gap and
  waiting for a quiet screen only before a `shot` was weighed in #378 and declined. It would save about
  0.6 s per input step, which is little once scenarios run in parallel. The cost is a race: a click
  that depends on the previous click's async result (a panel opening, a picker mounting) could fire
  before that result lands.
- **`gui-pass` starts a fresh Xvfb per scenario.** Reusing one X server per worker was weighed in #378
  and declined. Startup is polled, so it costs about 0.1 s. A reused server would carry pointer position
  and input focus from one scenario into the next, so a first shot's hover state would depend on run
  order.

## Publication

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
  captures under `crates/vitni-digitalarkivet/tests/fixtures/` are third-party content.
- **History is not rewritten to remove the Digitalarkivet captures (2026-09-30).** Their terms were
  examined on 2026-09-30:
  - **No scan was ever committed.** `git rev-list --objects --all` finds only icons, brand art, README
    shots and concept art, and the viewer pages link to their scans rather than embed them.
    Digitalarkivet's own policy (`content/1570`) lets unrestricted scans be reused freely, with
    attribution.
  - **What is uncertain is the transcriptions in the page captures.** The 1920 census (source 73902)
    is owned by Nasjonalarkivet with no licence stated. The church-book baptism (source 1035) was
    transcribed by AMF under a licence labelled only "Ancestry, MyHeritage og FamilySearch", with no
    published terms. Both pages also carry Arkivverket's site markup.
  - **The captures left the tree in #446 and stay in existing commits.** A rewrite would
    change every SHA since 2026-07-19 and drop the GPG signatures on those commits, for a risk nobody
    has raised.
  - **If a rights holder asks, the takedown procedure does it** (ADR 0042 §5,
    [`development.md`](development.md#takedown-removing-a-file-from-history)). The matching
    corpus's `digitalarkivet.toml` holds transcribed facts (names, dates and source URLs), not copied
    pages, so it stays.
