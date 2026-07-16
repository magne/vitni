# Phase 5 UI review — implementation plan

- **Status:** Draft
- **Date:** 2026-07-12
- **Implements:** the code-side findings of [ui-review.md](ui-review.md) — its §6 "stage-2
  plan inputs" plus the canonical common-tab shapes (Appendix A) the shipped screens must
  converge to. The mockup-side resolutions are already applied on this branch and are **not**
  re-planned here.
- **Companion:** [plan-2.md](../archive/phase5/plan-2.md) owns PRs 24–38. Its unshipped tail (PR 33–36) is not
  re-planned; three of its PRs pick up contracts from this review (noted below). PR numbers
  here continue from plan-2.md: **39–45**.

## Ground rules (inherited from plan.md/plan-2.md unchanged)

- Every PR leaves the workspace green: `cargo build --workspace` ·
  `cargo nextest run --workspace --all-features --all-targets` ·
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` ·
  `cargo xtask i18n-check` · `cargo xtask build-plugins` · `prek run`.
- **A11y gate per PR:** keyboard-only walkthrough, visible focus, SSR role/label assertions,
  axe-core pass, contrast in both themes.
- Every user-facing string via Fluent, en + no, in the tracked fragments.
- Feature branches, `--no-ff`, never direct to main.
- **App CSS is a superset of the mockup's** (shared rules live in both, app-only rules stay
  app-only) — every mockup `tokens.css`/`components.css` change from this review must be
  mirrored into the app's bundled CSS or it silently diverges.
- On completing a PR, check its box here and strike the matching §6 line in ui-review.md.

## Blast radius (traced, not guessed)

- **Person Events tab is thinner than every mockup/table peer:** `events_table`
  (`genealogy-ui-dioxus/src/screens/person.rs:1355`) renders Event · Role · Date only — no
  Place, no Age (payload shipped in ADR 0019), no Confidence. `EventRefVm` carries the join.
- **Family screen has no Citations tab:** `family_tabs`
  (`genealogy-ui/src/view_model/family.rs:325`) lists
  overview/children/events/media/notes/tags/history; data-model §6 lists `citations` on
  Family and the overview already renders per-claim citation cues (`family.rs:910,937`).
- **Person citations table** (`person.rs:1482`) renders Id · Source · Surety · Evidence — no
  Page, no Backs; the reverse index for Backs exists (`app/citation_usage.rs`).
- **Event addresses exist app-side, invisible UI-side:** `EventDetail.addresses`
  (`genealogy-app/src/event.rs:96`) + `add_event_address` (`:292`) are wired; zero `address`
  hits in `screens/event.rs`.
- **Repository fax/www** — re-opened from the 2026-07-05 review (fixed mockup-only):
  `repository_addresses_cards` never renders them; addresses expose no per-item
  `AssertionId`, so no per-card Edit/Retract. Pattern to copy: PR32's
  `partners_with_assertions` (core view accessor → app DTO field → VM).
- **Source "Used by N records"** — computed (`SourceReliability.record_count`), never
  rendered.
- **List sort exists in the VM** (`genealogy-ui/src/list.rs:62` `RowSort`, applied by
  `visible_rows`); whether a toolbar control exposes it is unverified — PR39 verifies and
  exposes. A column chooser does not exist in `list.rs` (decision item, below).
- **DNA inference:** `dna_match.rs:845` renders a flat predicted-relationship string; no
  command lets a Person fact/association cite a DnaMatch (data-model §12 promises exactly
  that shape) — needs a small design note before code (PR45).

## PR sequence

### PR39 — Common-tab/table parity sweep

Findings: U11 (code side), U25 (code side), U26, §6 Page/Backs, §6 Period label, §6 used-by
count. Crates: `genealogy-ui`, `genealogy-ui-dioxus`, `genealogy-app` (DTO fields), i18n
fragments en+no.

- [x] Person `events_table`: add Place, Age, Confidence columns (canonical Events shape,
      ui-review Appendix A); chronological default sort per the documented rule.
- [x] Event participants table: add Age column.
- [x] Family: add Citations tab (`family_tabs` + a citations table matching the canonical
      Source · Page · Backs · Confidence · Evidence · Detach shape); expose
      `FamilyDetail.citations` if the DTO lacks it.
- [x] Person citations table: add Page and Backs columns (Backs via the citation-usage
      reverse index). Converged onto the shared `citations_table` (`show_backs` flag);
      `person_citations_table` deleted.
- [x] Participation row verb: "Retract" → "Remove" in the Fluent keys (person Events;
      event Participants already "Remove"; family-event "Unlink" unchanged).
- [x] Place Names column label: "Date" → "Period" (the 2026-07-05 review's deliberate
      single-dated-`PlaceName` fix — code adopts the mockup).
- [x] Source overview: render "Used by N records" from `SourceReliability.record_count`.
- [x] Verify a sort control is exposed on list toolbars (`RowSort` exists); added the
      cycling "Sort: …" button matching the person.html specimen.

### PR40 — A11y + CSS parity port

Findings: U45–U49, U42–U44 (app side), U3/U4 equivalents if the app chrome has them.
Crates: `genealogy-ui-dioxus` (bundled CSS + screens + SSR tests).

- [x] Mirror the mockup CSS/token fixes into the app's bundled CSS: per-theme `--faint`
      contrast values, light+dark `.ev`/`.resn` chip colors, light `--conf-low`/
      `--conf-very-high`, `.resn[aria-checked="true"]` coloring, `.btn min-width: 24px`,
      no-wrap source badges in table cells. Applied as surgical deltas (the app CSS is a
      superset of the mockup's — the WebKitGTK font fix stays). The contrast script never
      existed in-repo, so it is replaced by a pure-Rust WCAG gate (`tests/contrast.rs`)
      asserting the ported values clear AA/non-text floors in both themes.
- [x] Screen sweep: `sr-only` caption (via a `caption` prop on the shared `Table` +
      visually-hidden "Actions" header) per data table; contextual `aria-label` on every
      row-action button (dashboard/preferences gaps filled); exactly one `h1` per screen
      (`.detail-title` → `<h1>` in the shared detail/create headers; tool pages promoted or
      `sr-only` `<h1>`). SSR tests assert the count is exactly one.
- [x] Merge compare: `differs` derived on `MergeFieldRowVm`; changed values render in
      `span.diff` plus a warn "differs" badge (aria-label/title).
- [x] Record-tab close made keyboard-operable (`tabindex` + Enter/Space) with a contextual
      `aria-label`; the shell background is `inert` + `aria-hidden` while the help/palette
      overlay is open (overlays moved to siblings of `.app`). Escape-close/focus-return kept.
- [x] SSR assertions added for each of the above. Axe pass: satisfied by the Rust SSR
      role/label/caption/h1 assertions (the repo has no JS toolchain; no axe harness added).

### PR41 — Repository address provenance (re-opened finding)

Findings: §6 Repository fax/www. Crates: `genealogy-core` (view accessor),
`genealogy-app`, `genealogy-ui`, `genealogy-ui-dioxus`.

- [x] Core: per-address `AssertionId` exposure — `RepositoryView::addresses_with_assertions`
      accessor (state already held `Vec<Attributed<Address>>`; mirrors `urls_with_assertions`).
- [x] App/UI: `RepositoryAddressRef`/`RepositoryAddressVm` carry the assertion id; cards render
      fax + www; per-card Edit (supersede) / Retract wired through the PR29 correction model.
- [x] Note in ui-review.md §6 struck; review-findings.md left untouched (immutable record —
      the re-open lives here).

### PR42 — Event address surface

Findings: §6 Event `Address`. Crates: `genealogy-ui`, `genealogy-ui-dioxus`
(app layer already shipped).

- [x] Event screen: Addresses card (read) rendering `EventDetail.addresses`, matching the
      repository address card treatment; add/edit/retract via `assert_event_address` +
      generic `undo_event_assertion` (retract path already existed). Per-address `AssertionId`
      surfaced via a new `EventView::addresses_with_assertions` accessor →
      `EventAddressRef` → shared `AddressVm`.
- [x] Edit specimen fields in the event form; i18n en+no. The card **and** form are extracted
      as shared components (`address_cards`/`address_form` in `screens/tabs.rs`) and Repository
      is refactored to reuse them (its bespoke card/form deleted) — common-tab convergence.
- [x] Mockup follow-up: Addresses tab + `grid-2` card pane added to event.html (between
      Overview and Participants), mirroring the repository treatment.

### PR43 — Small parity gaps: participant payload on Event screen, Sex "Other…", Place provenance

Findings: §6 items. Crates: `genealogy-ui`, `genealogy-ui-dioxus`.

- [x] Event-screen "add participant" panel upgrades to the full `ParticipationForm`
      (age/attributes/notes — person-screen parity; still writes the person aggregate).
- [x] Sex select gains "Other…" with free-text entry (the core type already carries it;
      `SEXES` list excludes it).
- [x] Place: wire the coordinate provenance popover (VM data exists); give the Code field
      provenance treatment.

### PR44 — Person life-timeline tab

Findings: §6 timeline. Crates: `genealogy-app` (or ui-only if DTOs suffice),
`genealogy-ui`, `genealogy-ui-dioxus`.

- [x] Read-only "Timeline" tab on Person: facts + event participations merged, sorted by
      the `GenealogicalDate` sort key; rows link to their record; per-claim confidence +
      source cues as elsewhere. Distinct from History (audit trail) — state that in the
      tab's section-note.
- [x] Mockup follow-up: add the Timeline tab specimen to person.html once the shape ships.

### PR45 — Person-citable DNA inference (needs a design note first)

Findings: §6 DNA inference (ui-review U17's substance). Crates: `genealogy-core`,
`genealogy-app`, `genealogy-ui`, `genealogy-ui-dioxus`; small ADR or data-model §12 note.

- [ ] Design note: how a Person `FactAsserted`/`AssociationAsserted` cites a DnaMatch
      (citation-shaped link vs a dedicated reference field) — data-model §12 promises the
      shape but no verb exists. ADR-sized decision; write it before code.
- [ ] Core/app: the chosen verb + reverse lookup (match → assertions citing it).
- [ ] dna-match screen: "view assertion on Person" link + cited inference rows; remove the
      "deferred" badges the mockup now carries (U17).

## Contracts folded into existing plan-2.md PRs (no new PR here)

- **PR33 (palette/keyboard):** implement the U5 combobox/listbox/option +
  `aria-activedescendant` contract now demonstrated in search-palette.html; drop the
  "planned — PR33" badges from search-palette.html/shortcuts.html when wired.
- **PR35 (Switch/RadioGroup):** adopt the arrow-key roving-tabindex + Space/Enter model now
  reference-implemented in the mockup `shell.js`; remove the design-system "not yet
  extracted" note.
- **PR36 (workspaces):** the preferences.html "planned — PR36" badges come off when the
  registry ships.

## Decision items (not planned as PRs — need a product/model call first)

- **Saved searches** — no design anywhere (palette, toolbars, app layer). Needs a small
  design round; 100k-scale research workflow argues for it.
- **Column chooser** — plan.md's PR3 text claims "columns"; `list.rs` has no column state.
  Decide whether to build or amend the PR3 description.
- **Map/geography view for places** — coordinates + dated hierarchy exist, no visual; zero
  roadmap presence. Product decision.
- **DNA payload columns** — haplogroup terminal-SNP/lineage stay deliberately deferred;
  decide whether shared-ancestor relationship-to-A/B + per-row confidence/source join them
  or get a payload PR.
- **Repository media refs** — data-model question (archive photos), ui-review §7.

## Dependency graph and suggested order

```text
PR39, PR40, PR41, PR42, PR43: independent
PR44: independent (after PR39 lands, reuse its Events-tab columns)
design note ──> PR45
```

Suggested sequence: **40 → 39 → 41/42/43 (parallel-friendly) → 44 → 45.**
PR40 first: it ports the review's token/CSS + ARIA groundwork every later screen change
builds on.

## Finding → PR matrix

| ui-review item | PR |
| --- | --- |
| U11/U25/U26 code side, Page/Backs, Period, used-by count, sort control | 39 |
| U42–U49 app side, CSS/token parity | 40 |
| Repository fax/www (re-opened) | 41 |
| Event Address | 42 |
| Event-screen participant payload, Sex Other…, Place provenance | 43 |
| Person life-timeline | 44 |
| Person-citable DNA inference (U17 substance) | 45 |
| U5 palette ARIA, keyboard tail | plan-2 PR33 |
| Switch/RadioGroup primitives + keyboard | plan-2 PR35 |
| Workspace registry badges | plan-2 PR36 |
| Saved searches, column chooser, map view, DNA columns, repository media | decision items |

## Verification (plan-level)

- Per PR: the green-gate list + the PR's own items + the a11y gate.
- After PR39: person Events tab shows Place/Age/Confidence chronologically; Family Citations
  tab detaches a citation end-to-end; place Names header reads "Period".
- After PR40: contrast script passes on the app's CSS in both themes; SSR tests assert
  captions/labels/h1; axe clean.
- After PR41: assert fax/www render and a single address edit supersedes by `AssertionId`.
- After PR42/43: exercise event address add→show→retract; add a participant with age from
  the event screen; person record shows it.
- After PR44: timeline order matches `GenealogicalDate` sort for mixed facts/events.
- After PR45: an inference asserted on Person cites the match; the match page links back.
- Each landed PR: check its box here + strike the ui-review.md §6 line; keep mockups in
  sync where a PR ships a surface the mockups deliberately omitted (event Address, person
  Timeline).
