# Phase 5 — UI breadth: implementation plan

- **Status:** Draft
- **Audience:** anyone implementing the Phase 5 UI
- **Companion:** the HTML mockups in this folder ([`index.html`](index.html)) are the visual target;
  this file is the build sequence. Roadmap context: [`../roadmap.md`](../roadmap.md) Phase 5.

## Context

Phase 4 is done: all 12 aggregates are modelled, `genealogy-app` exposes Summary DTOs +
`list_*`/`show_*`/`create_*`/mutation use-cases for **every** aggregate, and the CLI has full CRUD.
The UI is only the Spike-D proof — `genealogy-ui` (framework-free) has Person view-models + the
plugin form vocabulary; `genealogy-ui-dioxus` renders one Person list→detail screen and a plugin
form over ~207 lines of embedded dark CSS. Eleven aggregates have no UI, there is no design system,
and the roadmap-mandated pedigree, evidence-editing, and merge screens do not exist.

This plan turns the mockups into screens **without re-implementing domain rules**: the UI consumes
the existing use-cases in `crates/genealogy-app/src/<aggregate>.rs`. New app-layer code is only the
few read/command paths that don't exist yet (change-log query, merge, config read/write, pedigree
traversal), called out per PR.

## Locked decisions

- **Navigation:** entity-category rail + master-detail; a separate Tools section (Pedigree,
  Compare/Merge, Plugins, Preferences) — entities and functions are not mixed. Global search +
  always-visible active-record breadcrumb/status bar.
- **Compare:** in-app record tabs; drag a tab to dock side-by-side; the merge wizard reuses the split.
- **Theme:** light & dark via `[data-theme]` tokens; toggle in Preferences.
- **Build order:** design system → app shell → generic master-detail → Person slice → remaining
  aggregates → functions.
- **Surface our strengths** on every screen, not in one showcase (see below).
- **Privacy is a `Restriction` set** (GEDCOM v7 `RESN`: Confidential/Locked/Privacy), modelled as a
  multi-select — see the dependency note.
- **Language & locale are preferences** with sane defaults (ADR 0003 nb/nn→no→en fallback).
- **Fully keyboard-operable + accessible from the start (WCAG 2.2 AA)** — see the next section.

## Keyboard navigation & accessibility (foundational — WCAG 2.2 AA)

A11y is a property of the foundation, verified at every step — not a late polish pass. Grounding:
Dioxus 0.7.8 supports keyboard events and `aria-*`/`role`/`tabindex` as standard RSX attributes; the
SSR test (`crates/genealogy-ui-dioxus/tests/interpreter.rs`) asserts via `html.contains(...)`, so a11y
attributes are unit-testable; `Chrome`/`Localizer` (ADR 0003) localize aria-labels and shortcut hints.
The crate has **zero** a11y infra today — clean slate.

- **Shortcut map (single source of truth, localized via `Chrome`; the `?` overlay renders it).**
  Global: `⌘K` palette, `⌘N` new (context-aware), `⌘F` find/filter, `⌘Z`/`⌘⇧Z` undo/redo, `⌘1..9`
  switch record tab, `?` help, `Esc` close/clear. Navigation (`g`-prefix): `g d/p/f/e/l/s/c/r/m/n/t` →
  Dashboard + the entity categories. Within a screen: ↑/↓ move selection, Enter open, `[`/`]`
  prev/next record, ←/→ + Home/End across detail tabs, arrows to walk the pedigree; on a fact row
  `s` add source, `e` edit. `⌘` on macOS = `Ctrl` elsewhere.
- **Keyboard model.** Roving `tabindex` for composite widgets (entity list = `listbox`/`option`;
  detail tabs = `tablist`/`tab`/`tabpanel`; pedigree = `tree`/`treeitem`) — Tab moves between regions,
  arrows within. Focus trap + restore for modals/side panels/palette. A "Skip to content" skip link.
  Reading/focus order matches visual order.
- **ARIA & semantics.** Landmarks (`nav` rail, `main`, `search`, `contentinfo` status bar);
  `role=dialog aria-modal` for overlays; `aria-selected`/`aria-current` for active record/nav item;
  `aria-live=polite` for toasts and async load/save; accessible names on every icon button and input;
  `aria-pressed` on the Restriction toggles.
- **Visual & motion.** WCAG AA contrast for both themes (audit tokens incl. the focus ring); color is
  never the only signal — confidence is dot **+ label**, evidence chips carry **text**, no-source is
  **icon + text**; strong `:focus-visible` ring; honor `prefers-reduced-motion` and
  `prefers-color-scheme`; support 200% zoom / reflow.
- **Acceptance gate (every screen PR):** keyboard-only walkthrough reaches every action; visible focus
  throughout; SSR test asserts the screen's roles/labels; an automated axe-core (or equivalent) pass is
  clean; contrast verified.

See [`shortcuts.html`](shortcuts.html) for the full map, focus model, and per-pattern ARIA contract.

## Binding constraints (ADRs)

- **ADR 0008** — dependency direction `genealogy-app → genealogy-ui → genealogy-ui-<framework>`; no
  `dioxus::` type above the renderer. App screens use rich RSX over shared view-models; **only plugin
  screens** use the constrained serializable vocabulary.
- **ADR 0012** — plugin UI is the JSON form vocabulary (text/number/checkbox/select); labels are
  Fluent message IDs resolved by the frontend. Phase 5 must *complete* this vocabulary.
- **ADR 0003** — every user-facing string is a Fluent message ID (data catalogue in `genealogy-ui`,
  chrome catalogue in `genealogy-ui-dioxus`); no hardcoded literals; en + no kept complete (i18n-check).

## Differentiators to surface (cross-cutting)

Built once as components in **PR1**, placed by **PR4 (Person)**, reused by every aggregate slice and
the merge wizard — so the strengths are visible everywhere:

- **Audit trail** — per-record History tab + a global Activity view; who/when/why, with undo.
- **Evidence-first** — confidence badge + source count on every fact; a no-source flag on unsourced
  ones.
- **Research-grade citations** — the Evidence Explained axes (original/derivative · primary/secondary
  · direct/indirect/negative).
- **Non-destructive merge** — keeps the merged-from persona; reversible.
- **Provenance** — a "why we believe this" popover listing the assertions/citations behind a value.
- **Sandboxed, capability-gated plugins** + **full localization** (incl. plugin UI) — in the plugin
  manager and preferences.

## PR sequence

Each PR names the layers it touches and the existing use-cases it reuses.

| PR | Title | Touches | Notes |
| -- | ----- | ------- | ----- |
| **1** | Design system foundation (accessible components) | `genealogy-ui-dioxus/src/components/*`, `tokens.css` (replaces `app.css`) | Tokens (light+dark, **contrast-audited to AA**) + reusable components: Button, Input/Select/Checkbox/DatePicker, Card, Tabs, Table/ListRow, Badge/Chip, **ConfidenceBadge, EvidenceAxisChip, no-source flag**, SidePanel, Modal, Breadcrumb, EmptyState, Toast, StatusLine, **History timeline, provenance popover**. **Each ships accessible: roles/labels, `:focus-visible`, keyboard operability, color-not-alone, `prefers-reduced-motion`.** Refactor the existing Person + plugin screens onto them; extend the SSR test to assert roles/labels. Keep it green. |
| **2** | App shell + navigation + keyboard foundation | `genealogy-ui::navigation::{Screen,Intent}`, `genealogy-ui-dioxus` shell | Entity rail (entities vs Tools), global search box (stub), active-record breadcrumb + status bar, in-app tab container with drag-to-split, landmarks + skip link. **The keyboard layer lives here: central shortcut dispatcher (the localized map), roving-tabindex + focus-trap helpers, the `⌘K` palette, the `?` help overlay, `g`-prefix nav.** Chrome i18n keys for nav labels, shortcut hints, aria-labels. |
| **3** | Generic list + master-detail framework | `genealogy-ui`, `genealogy-ui-dioxus` | Reusable list (search/filter/sort/columns) + detail container with a related-item tab strip, driven by per-aggregate descriptors. Adding an aggregate = view-models + a tab config, not a bespoke screen. |
| **4** | Person vertical slice (reference) | `genealogy-ui/src/view_model.rs`, `genealogy-ui-dioxus`, reuses `app::person`/`app::event` | View-models for the Person tabs (Names/Facts/Events/Associations/Families/Citations/Media/Notes/Tags/History); full list + detail; inline + side-panel editing; inline source + confidence on facts. The copy-template for the rest. |
| **5** | History / change-log query | new query use-case in `genealogy-app`, `genealogy-ui` | Per-aggregate event stream → DTOs (operator/when/summary). Renders the History tab + the global **Activity** view + undo. The event-sourced differentiator; reused by every aggregate. |
| **6** | Citation slice (evidence axes) ✅ done (#57) | per-aggregate `view_model`/screens, reuse `app::citation` | View-models + list + detail tabs + edit wiring; the Evidence Explained axes. Shipped in #57. |
| **7** | Family slice | per-aggregate `view_model`/screens, reuse `app::family` | View-models + list + detail tabs + edit wiring. |
| **8** | Event · Place slices | per-aggregate `view_model`/screens, reuse `app::event`/`app::place` | View-models + list + detail tabs + edit wiring. |
| **9** | Source · Repository slices | per-aggregate `view_model`/screens, reuse `app::source`/`app::repository` | View-models + list + detail tabs + edit wiring. |
| **10** | Media (gallery) · Note (rich text) slices | per-aggregate `view_model`/screens, reuse `app::media`/`app::note` | View-models + list + detail tabs + edit wiring. |
| **11** | Tag · DnaTest · DnaMatch slices | per-aggregate `view_model`/screens, reuse `app::tag`/`app::dna_test`/`app::dna_match` | View-models + list + detail tabs + edit wiring; the small ones grouped. |
| **12** | Pedigree / tree view | new traversal query in `genealogy-app`, `genealogy-ui-dioxus` | Ancestor/descendant chart over Person/Family; view switcher (List/Pedigree/Descendants/Relationships). |
| **13** | Compare / merge | new `merge_persons` use-case + duplicate-detection query in `genealogy-app`, `genealogy-ui-dioxus` | Split-view compare + non-destructive merge wizard. The `MergePersons` event exists in core; no app/CLI path yet. Undo via the change log. |
| **14** | Preferences / configuration | new config read/write use-cases in `genealogy-app` (ADR 0005), `genealogy-ui-dioxus` | Operator identity, Appearance/theme, **Language & locale (sane defaults via the ADR 0003 chain)**, date/number format, workspace defaults. Surface the override layers and the resolved values. |
| **15** | Plugin manager | reuse `genealogy-plugin-host`, `genealogy-ui-dioxus` | List installed plugins, enable/disable, show declared capabilities. Trust tiers read-only (full UX is Phase 8). |
| **16** | Complete plugin-UI vocabulary + submission | `genealogy-ui` vocabulary + per-framework interpreter | Extend beyond a single form to lists/tables and wire form submission/actions. **Needs a follow-up ADR** (ADR 0012 left submission out). |
| **17** | Second-framework readiness check | `genealogy-ui` (test/guard), docs | Guarantee `genealogy-ui` carries zero framework types (a compile/test guard) and document the checklist a second renderer follows to reuse it unchanged (ADR 0008). Not a full second renderer. |

### Per-aggregate slice → PR map

| PR | Aggregate(s) | Rationale |
| -- | ------------ | --------- |
| 6 ✅ | Citation | evidence axes (done, #57) |
| 7 | Family | largest graph entity (relationships, child refs) — own PR |
| 8 | Event · Place | events occur at places |
| 9 | Source · Repository | source held by repository |
| 10 | Media · Note | gallery + rich text |
| 11 | Tag · DnaTest · DnaMatch | the small ones grouped |

Cross-cutting, folded into the PRs above (not separate): the keyboard layer + command palette + `?`
overlay (built in PR2; every screen registers its contextual shortcuts), **accessibility to WCAG 2.2
AA as a per-PR acceptance gate** (keyboard-only walkthrough, visible focus, roles/labels, SSR aria
assertions, axe-core pass, contrast), and i18n keys added with every screen and kept complete
(i18n-check).

## New ADRs flagged

- **Plugin-UI vocabulary expansion + submission** — gates PR16.
- Merge needs **no** new ADR (the model already supports non-destructive merge); it needs an app/CLI
  use-case.

## Dependency note — privacy `Restriction` set ✅ resolved

The privacy control (multi-select Confidential/Locked/Privacy, GEDCOM v7 `RESN`) needed the core/app
change from `private: bool` to a `Restriction` enum set first. **Done:** `Restriction` is a uniform
`BTreeSet<Restriction>` on all 12 aggregates with a `SetRestrictions` command/`RestrictionsChanged`
event, exposed through the Summary DTOs and a `set_restrictions` app use-case per aggregate, and
round-tripped through the GEDCOM/Gramps plugins for person/family (host-api 0.9.0). The UI read-path
maps it to the existing `RestrictionKind` (`From<Restriction>`); PR4 wires the `RestrictionSet`
toggle→intent editing flow onto `set_restrictions`. See data-model §6/§7/§16/§17 and roadmap Phase 2
item 7.

## Dependency note — cross-aggregate joins need stable ids ⚠️ open

App `*Summary` DTOs reference related aggregates by **`human_id` string** only
(`CitationSummary.source: Option<String>`, `.media`/`.notes: Vec<String>`, etc. —
`crates/genealogy-app/src/citation.rs:48`; same pattern across all 12 `*Summary` DTOs). The
only id carried is `TagRef.id` (for the detach command, never rendered). The UI therefore
**cannot join aggregates by stable id** — clicking a related record (citation→source, fact→event)
has no id to request the target aggregate by, only a display label.

**Either way the DTO must surface the stable aggregate id** (alongside the `human_id` it already
carries) so a navigation target exists.

**Decision: join in the app/db layer.** Use-cases assemble the joined view server-side and return
DTOs that carry related summaries + their stable ids; the UI navigates by id and fetches on demand.
This keeps join logic out of presentation and avoids N+1 queries from the renderer. *(Rejected:
UI-layer stitching — more round-trips, join logic leaks into the renderer.)* This adds a
per-aggregate app-layer task to PRs 7–11: extend the relevant `*Summary` DTOs with stable ids plus
any joined-view use-case the detail tabs need.

**PR7 (Family) status:** done for Family — `FamilySummary` carries stable ids and the joined view
(partners/children/events with names, surety, source counts; tags as `TagRef`; media captions). It
also landed the two domain features the mockup assumes: per-partner child relationships
(`ChildEntry.relationships`, GEDCOM `_FREL`/`_MREL`) and family events
(`LinkFamilyEvent`/`FamilyEventLinked`).

## Follow-up — GEDCOM/Gramps round-trip of the new Family fields ⚠️ open

PR7's core/app/UI work landed, but the import/export plugins still drop the new fields: per-partner
child relationships (`_FREL`/`_MREL`, Gramps `mrel`/`frel`) and the explicit `FamilyEventLinked`
link. Existing round-trip is unchanged (plain `CHIL` → no per-partner relationship; marriages still
flow as `Event` aggregates via the participant-set heuristic). Completing it needs: the
`genealogy-gedcom`/`genealogy-gramps-xml` family models + parse/emit, a host-api WIT bump
(per-partner `add-child` + `link-family-event` + `family-dto` children/events), the four
`plugins/{gedcom,gramps}-{import,export}` glue paths, and round-trip test assertions.

## Verification (per PR)

- `cargo nextest run --workspace --all-features --all-targets`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo xtask i18n-check`
- Run `genealogy-ui-dioxus` (desktop feature) and exercise each new screen against the matching mockup.
- **A11y gate:** keyboard-only walkthrough (every action reachable, focus visible, `Esc`/arrows/`g`-prefix
  work); SSR test asserts the screen's roles/labels; automated axe-core (or equivalent) pass clean;
  contrast checked for both themes.
