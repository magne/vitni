# Genealogy Issues

A prioritized backlog: quick wins (bugs, then ease-of-use) first, then an unscheduled UI/app
backlog, then roadmap-phase work ordered exactly as [`roadmap.md`](roadmap.md) sequences it. The
roadmap remains the source of truth for phase detail — the phase sections below are short summaries
that link back to it.

## Ease of use

- **Quit / close-tab keys.** `Ctrl+Q` to quit the application; `Ctrl+W` to close the current tab
  (entity).
- **Customizable keyboard shortcuts** as user/client (presentation) configuration; belongs to the
  Phase 6 config split. Would also enable a general VS Code-style *when* context, beyond the
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

- **Transitive place-hierarchy walk** — the enclosed-by chain is read one level deep, so the
  Hierarchy tab and the Overview enclosing line show only a place's *direct* enclosers, never the walk
  up to a country. Add a **cycle-aware** walk in `genealogy-app/src/place.rs` (`PlaceLookups::load` /
  `summarize`, the hierarchy loop) that follows the **primary** (first) `PlaceRef` up to a top-level
  place, producing the full chain for the breadcrumb (`genealogy-ui-dioxus/src/screens/place.rs`,
  `place_hierarchy_table`) and a generated title ("Saint Petersburg, Russia"). It must be
  **date-aware** — when a place is shown for an event, pick the enclosing link valid at the event date
  (a place can move between jurisdictions over time; `PlaceRef.date` exists for exactly this); use the
  primary/latest link otherwise. Guard cycles with a visited set + depth cap. Flows through
  `PlaceSummary.enclosing` → `PlaceDetail.hierarchy`. An optional DB `place_parent` index (Gramps
  precedent) is a later scale follow-up. No new invariant — no ADR needed. Folded into the Phase 9
  geography work below (or landed just before it).

- **Map & geometry — now scheduled as roadmap phases.** The owner split the map work by size; it is no
  longer an unscheduled item:
  - **Phase 6 — Place map MVP** (read-only single point; Leaflet + OpenStreetMap, no editing, no model
    change). Plan [`plans/place-map-mvp.md`](plans/place-map-mvp.md); mockup
    [`mockups/place-map.html`](mockups/place-map.html).
  - **Phase 9 — Places: geography & temporal model** — point / polygon / multi-polygon geometry, dated
    boundaries, place **succession** (merge / split), the date-aware resolution rule, a time slider,
    in-map editing, and the pluggable provider. Gated by
    [ADR 0024](adr/0024-place-geometry-and-spatial-storage.md),
    [ADR 0025](adr/0025-geography-view-and-pluggable-map-provider.md), and
    [ADR 0026](adr/0026-place-succession-and-temporal-resolution.md). Plan
    [`plans/places-geography-temporal.md`](plans/places-geography-temporal.md); mockup
    [`mockups/geography.html`](mockups/geography.html).

### Pedigree

- **`Restriction` chart cue** on the pedigree chart.
- **Name-autocomplete pickers** for the focus / relationship inputs, which are plain `human_id` text
  fields today.

### Local import & internal cleanup

- **GUI Import-GEDCOM command** — the CLI imports; `genealogy-ui-dioxus` has no import flow. (This is
  local file import, distinct from the Phase 7 *assisted* import.)
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

## Phase 6 — Configuration split & storage

Roadmap-owned; see [`roadmap.md` Phase 6](roadmap.md#phase-6--configuration-split--storage). Pulled
forward from the server/web prerequisite: split the entangled config into three scopes and give it a
storage seam. Gated by **ADR 0015**.

- **Three scopes** — workspace-functionality (id_formats, operators, privacy, data-language, surety;
  shared / server-side); operator/user (operator identity + per-user prefs); client/presentation (UI
  locale, theme, view prefs, keyboard shortcuts, endpoint or local `database_url`; local to the
  client).
- **Storage seam** — a `ConfigStore` abstraction with a file backend now (`workspace.toml` +
  `~/.config/genealogy/config.toml`); the database backend (operator + presentation, per
  authenticated user) lands in Phase 11 with the server.
- **Fix inverted env-var precedence via the seam.** A bare env var (`LANGUAGE`) currently outranks
  config files; it should not. Model env vars as two read-only `ConfigStore` seams bracketing the
  file store — intended order, lowest → highest: plain env var < config files < `GENEALOGY_`-prefixed
  env var (e.g. `GENEALOGY_LANGUAGE`). (Presentation-config precedence; was a standalone bug.)
- Unblocks the Ease-of-use presentation-config items above (env-var precedence, customizable
  shortcuts, theme/view prefs).

## Phase 7 — Assisted import & external search (Digitalarkivet)

Roadmap-owned; see [`roadmap.md` Phase 7](roadmap.md#phase-7--assisted-import--external-search-digitalarkivet).
Online, record-by-record assisted import gated by **ADR 0017** (assisted-import host capabilities):

- New host capabilities: `net` (allowlisted outbound HTTP), `media-store` (host writes + checksums
  downloaded bytes under the workspace `media/`), and a pluggable multi-provider `ai`.
- A `digitalarkivet-import` plugin + pure `genealogy-digitalarkivet` crate (parse census/churchbook
  pages, resolve the scan URL chain).
- Interactive present-and-confirm: show the interpreted record and scan before import (CLI renders
  the image inline; the same capability backs the GUI).

## Phase 8 — Research rigor & import sync

Roadmap-owned; see [`roadmap.md` Phase 8](roadmap.md#phase-8--research-rigor--import-sync). The
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

## Phase 9 — 1.0 hardening

Roadmap-owned; see [`roadmap.md` Phase 9](roadmap.md#phase-9--10-hardening).

- Plugin **signing, trust tiers, capability-grant UX, and three-layer loading** (workspace > app-dir
  > embedded) — **ADR 0014**.
- Performance profiling.
- Packaging and distribution.

## Phase 10 — DNA breadth & depth

Roadmap-owned; see [`roadmap.md` Phase 10](roadmap.md#phase-10--dna-breadth--depth). Pulled together so
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

## Phase 11 — Beyond 1.0: server + web

Roadmap-owned; see [`roadmap.md` Phase 11](roadmap.md#phase-11--beyond-10-server-backend--web-frontend).
Backend server, web frontend, and server-connected workspaces — deliberately additive, not scheduled.
Builds on the **Phase 6** config split: the server adds the `ConfigStore` **database** backend so the
operator + client/presentation scopes persist per authenticated user, while the embedded build keeps
the file backend.

## Completed

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
  remains future work under customizable shortcuts (Phase 6).
