# Phase 5 mockup UI review — findings

- **Status:** Review findings — mockup fixes applied on this branch; code-side
  implementation planned in [ui-review-plan.md](ui-review-plan.md) (PRs 39–45)
- **Date:** 2026-07-12
- **Scope:** every page in `docs/phase5/` + `assets/`, reviewed for **aesthetics, usability,
  accessibility, feature completeness/IA, code truth, and common-tab convergence**. Distinct
  from the 2026-07-05 review ([review-findings.md](../archive/phase5/review-findings.md)), whose findings are
  settled and whose deferred list stays deferred — nothing there is re-litigated here.
- **Method:** six parallel expert reviews (visual design · usability/interaction ·
  accessibility · genealogist feature-completeness · code-truth vs `genealogy-ui-dioxus` ·
  common-tab component audit), each covering all 26 pages and every JS-wired tab (rendered
  via agent-browser + raw HTML), then synthesized and deduplicated here.
- **Code is the truth:** plan-2.md PRs 24–32 are shipped (draft-based creates, unified record
  form, correction model, structured dates, edit-gap sweep); PRs 33–38 are not. Mockup claims
  were checked against `crates/genealogy-ui-dioxus/`, `genealogy-ui`, `genealogy-app`.
  Mockups that lag shipped code are findings (§2); mockup features the code lacks are
  stage-2 plan inputs (§8), not mockup bugs.
- **Resolution column:** every finding is either fixed in the mockups on this branch or
  explicitly deferred with a reason.

## Verdict

The mockup set is in strong shape where it matters most: the evidence-first visual system
(per-claim confidence, source-count/no-source flags, provenance popovers, Evidence Explained
axes) is applied with genuine discipline across all 12 aggregate pages, both themes hold up,
and the big PR 24–32 implementation wave landed without the mockups drifting far. Three
clusters need attention: **(1) accessibility of the shared chrome** — the nav rail, help
modal, record-tab close, palette, and merge picker all have broken or absent ARIA/keyboard
contracts, which matters because these mockups are the reference implementers copy;
**(2) post-ADR-0021 staleness** — vital Facts, forced confidence, and a handful of phantom
columns describe a data model that no longer exists; **(3) unbuilt-vs-built honesty** —
several tool pages present PR 33–36 work as if wired, the exact pattern the dashboard already
solves with "deferred" badges. The common tabs are closer to converged than expected — Tags
and History are byte-identical across pages — with two real gaps (Family Citations tab,
Media/Notes row actions).

Findings ranked severity × cost-to-fix-later within each section.

---

## 1. Blockers

| # | Lens | Finding | Resolution |
| - | ---- | ------- | ---------- |
| U1 | code/feat | **person.html still asserts Birth/Death as Facts.** Overview "Vital facts" card, Facts-tab rows (with Edit/Retract implying `AssertFact`), and the Citations "Backs" column all show Birth/Death as Facts — but ADR 0021 §2 (PR #118) removed the vital variants from `FactType`; `fact_type_choices` (genealogy-ui `i18n.rs:1452`) offers 14 non-vital types only. The person's own Birth/Death events are meanwhile *absent* from the Events tab, and the Facts tab-count badge said 6 over 4 rows. A developer cannot build this screen against the real DTOs. | Fixed: Birth/Death moved to the Events tab as Primary-participant events; Facts tab keeps attribute-shaped rows (Occupation, Residence, Religion); counts corrected; Overview card re-labeled to draw from events. |
| U2 | a11y | **Nav rail items lose link semantics.** `shell.js` sets `role="listitem"` directly on the `<a class="nav-item">` anchors, overwriting the implicit link role — the 19-item primary nav is invisible to screen-reader link navigation on every page (WCAG 4.1.2). | Fixed: list/listitem scaffolding removed from anchors in `shell.js`. |
| U3 | a11y | **`?` help modal has no focus trap or inert background.** `aria-modal="true"` but Tab walks into the background rail; `.app` never gets `inert`/`aria-hidden` (ARIA APG modal pattern, WCAG 2.4.3). Escape-close and focus-return already work. | Fixed: Tab/Shift+Tab cycle within the sheet; `inert` set on `.app` while open. **Done (PR40):** app-side — `.app` gets `inert` + `aria-hidden` while the help/palette overlay is open (overlays are siblings of `.app`); Escape-close/focus-return kept. |
| U4 | a11y | **Record-tab "✕ Close" is a dead control.** `role="button"` + `aria-label` but no `tabindex` and no handler anywhere — announced as actionable, unreachable and inoperable (WCAG 2.1.1/4.1.2). | Fixed: focusable with Enter/Space/click handler that removes the tab. **Done (PR40):** app-side — record-tab close is keyboard-operable (`tabindex` + Enter/Space) with a contextual `aria-label`. |
| U5 | a11y/code | **Command palette has no combobox/listbox semantics — and is shown fully live though `palette.rs` is a stub (PR33).** Query input and result rows are bare text: no `combobox`/`listbox`/`option`/`aria-activedescendant`; the visual `.sel` highlight is invisible to AT. Nothing marks search/commands as not yet wired. | Fixed: full combobox/listbox/option ARIA added to the specimen; "live search + command execution are PR33" specimen note added. |
| U6 | a11y/visual | **Merge "which value wins" picker is decorative.** The central merge decision renders as plain spans (no role/tabindex/aria-checked, `title` only) — keyboard-inoperable, unannounced; and the Death row's radio rendered unfilled while its own caption says "take this value". | Fixed: real `role="radiogroup"`/`role="radio"` + `aria-checked` per field row; Death row radio filled to match its caption. |
| U7 | a11y/visual | **Switches/radios: no keyboard model, and On/Off render identically.** No arrow-key handler exists for `role="radio"` groups (focus lands on "Dark" and can never reach "Light"); plugin-manager's five `role="switch"` spans have no `tabindex` at all; and `components.css` colors `.resn` only via `[aria-pressed="true"]`, so `aria-checked` switches show no On/Off difference — design-system's Switch specimen faked it with an inline style. | Fixed: arrow-key roving-tabindex handler for radio groups + Space/Enter toggle and `tabindex="0"` for switches in `shell.js`; `.resn[aria-checked="true"]` rule added; inline hack removed. |
| U8 | visual | **Preferences sub-nav renders as unstyled run-on text.** `preferences.html` reuses `.nav-item` outside `.rail`, but every `.nav-item` rule is scoped `.rail .nav-item` — the page's primary navigation is illegible. | Fixed: sub-nav styling scoped to work outside the rail (`.subnav .nav-item`). |
| U9 | a11y | **Provenance popover trigger has no accessible wiring anywhere.** Every `.src-link` "❝ N sources" cue is a bare span (no role/tabindex/haspopup/expanded, no handler). The core evidence-first affordance offers implementers no reference for the trigger↔popover ARIA contract (beyond settled X7, which fixed only the visual pattern). | Fixed: one fully-wired accessible specimen (trigger button with `aria-haspopup`/`aria-expanded`, Escape/focus-return) added to design-system.html; other pages unchanged (static specimens by design). |

## 2. Mockup lags shipped code (code-truth)

| # | Finding | Resolution |
| - | ------- | ---------- |
| U10 | **Optional confidence (PR-C2) is invisible.** Every edit-specimen Confidence select forces a value; no read-mode row shows the shipped unset state, though `ConfidenceBadge` has a `.conf-unset` path and data-model §8 mandates rendering absence as "no judgment recorded". citation.html's axis selects show "—" but its Confidence select didn't; media.html pre-selected "Normal". | Fixed: leading "—  (no judgment recorded)" option added to every Confidence select; media's pre-selected Normal removed; unset-badge read-mode examples added on mechanical-event History rows (tag/media/repository). |
| U11 | **Participation payload (ADR 0019) invisible in tables.** Age/attributes/notes are fully wired in `ParticipationForm`, but person's Events tab and event's Participants tab show no age cue; person's Events tab also lacks the Confidence column family/event tables have. | Fixed: Age and Confidence columns added to person Events tab; Age column added to event Participants tab. |
| U12 | **citation.html Attributes tab shows a Confidence column that doesn't exist** (`CitationAttributeVm` is Type/Value only). | Fixed: column dropped. |
| U13 | **source.html Attributes tab kept the citation column ADR 0020 removed.** | Fixed: column dropped, matching citation/media. |
| U14 | **note.html shows Retract on Translation rows** — code deliberately has no retract-single-translation verb (`note.rs:579`, Edit-only). | Fixed: Retract removed from translation rows; section-note states Edit-only. |
| U15 | **dna-match.html edit specimen shows locked observation totals (shared cM/%/largest segment) as editable** — no `DnaMatchEdit` command can change them (`dna_match.rs:229`). | Fixed: rendered disabled/read-only like Test A/B; caption corrected. |
| U16 | **dna-match.html list shows placeholder "⛓" instead of the real `X%04d` human_id** + a stale "no short id" comment (the page's own head shows X0007). | Fixed: real ids in list rows; comment deleted. |
| U17 | **dna-match.html "Inferred relationship" card overstates what's built.** Code renders a flat predicted-relationship string + status chip; no Person-level cited-assertion path exists yet (no command lets a Person fact cite a DnaMatch). Mockup showed independently-cited rows with Person links. | Fixed: card marked with "deferred" badges on the unbuilt affordances (cited rows, view-on-Person link); underlying feature recorded in §8. |
| U18 | **merge.html summary says "6 relationships re-pointed"** — `merge.rs` has a test *enforcing* the "N other record(s) still reference" phrasing instead. | Fixed: wording matches the shipped string. |
| U19 | **plugin-manager.html shows a "first-party" trust tier the code never produces** (all plugins render "unsigned" until ADR 0014 ships). | Fixed: all rows unsigned; trust tiers noted as planned. |
| U20 | **design-system.html presents Switch/RadioGroup as shipped reusable components** — none exist in `components/*.rs`; screens hand-roll them (PR35). | Fixed: "not yet extracted — PR35" note added. |
| U21 | **shortcuts.html lists PR33-unwired shortcuts (⌘Z/⌘⇧Z, [ ], s) indistinguishably from shipped ones** (`keyboard.rs` handles k/f/n/digits only). | Fixed: unwired rows badged "planned — PR33". |
| U22 | **preferences.html Workspaces card implies a registry PR36 hasn't built** (Path/Engine columns, "+ Register workspace…"; shipped screen has name + Active/Switch-to only). | Fixed: registry affordances badged "planned — PR36" (kept as PR36's design target). |
| U23 | **plan-2.md PR32 note is stale**: says Event remove-participant uses `set_participant_role(remove=true)` — that command was deleted in PR-A2; code retracts the person-side `ParticipationAsserted`. | Fixed: plan-2.md note corrected (doc-only). |
| U24 | **pedigree.html shows a static "Focus … · 4 generations" label** where the shipped screen has interactive controls. | Deferred: cosmetic; the shipped screen is the truth and the mockup label communicates the same content. |

Also confirmed while auditing (no action): **the review-findings.md addendum about
instant-persist creates is fully stale** — `services.rs` uses `commit_*_change_set` for all
12 aggregates (PR26), and the DnaMatch zero-fill-on-bad-cM bug is fixed (drafts reject).

## 3. Common-tab convergence (matrix in Appendix A)

| # | Finding | Resolution |
| - | ------- | ---------- |
| U25 | **Family has no Citations tab** though data-model §6 lists `citations` on Family and the page's own Overview shows a "❝ 1" popover with no Detach path. | Fixed: Citations tab added matching the canonical Person/Place shape (Source · Page · Backs · Confidence · Evidence · Detach). |
| U26 | **The same person↔event participation uses three removal verbs** — Person "Retract", Event "Remove", Family "Unlink". | Fixed: "Remove" on Person Events + Event Participants (one assertion, one verb); "Unlink" kept for the genuinely different family-event link. |
| U27 | **Media and Notes collections have zero per-row actions** on every page carrying them — the one collection shape the X2/X3 sweep missed (`MediaRef` is a per-use attachment; detaching is a real auditable act). | Fixed: Detach action on media cards; Edit · Retract on note cards, all carrying pages. |
| U28 | **Detach tooltip wording differs** (person's long form vs the short form on event/place/media). | Fixed: short form everywhere. |
| U29 | **DnaTest/DnaMatch progressively truncate the canonical History section-note.** | Fixed: full three-clause sentence restored; DnaMatch's extra inference note kept as a second note. |
| U30 | **No tab demonstrates a default sort order or an empty state** (person's Events rows: 1876, 1880, 1878, 1920). An `EmptyState` component exists in design-system.html but is never used in situ; no long-list specimen exists either. | Partially fixed: person Events rows re-ordered chronologically + canonical sort documented per tab type (design-system.html); one in-situ empty-state specimen added (family Media tab). Long-list/overflow specimen deferred: low fidelity value in a static mockup; the a11y gate's real-app walkthrough covers it. |
| U31 | **Repository has no Media tab** — accurate to data-model §6 (no `media` on Repository), but an archive photo is a plausible use case. | Deferred: doc follow-up question for the data model, not a mockup defect (§9). |

## 4. Usability & IA

| # | Finding | Resolution |
| - | ------- | ---------- |
| U32 | **Same-type record links are dead** (`href="#"`): person's spouse/parents/associations, place's entire enclosed-by/jurisdiction chain — while every cross-type link on the same pages works. | Fixed: wired to their aggregate pages like every other cross-record link. |
| U33 | **Search palette demonstrates only Person results** despite "Search people, places, sources…" and 12 aggregates — the grouping-under-load question the specimen exists to answer. | Fixed: mixed-kind results (Place, Source rows) with kind grouping added to the specimen. |
| U34 | **The `?` overlay omits four working go-to shortcuts** (`g r/m/n/t` work via `NAV.entities`; the overlay's `SHORTCUTS` constant never lists them; shortcuts.html documents them correctly). | Fixed: four rows added to the overlay's Go-to group in `shell.js`. |
| U35 | **Dashboard "Review" buttons have no destination** while "Compare" links to merge.html. | Fixed: Review links target the relevant filtered list view, establishing the destination pattern. |
| U36 | **"⇄ Compare" appears on only 5 of 12 record heads** with no stated rule. | Fixed: scoping documented in design-system.html — Compare is offered on the aggregates with a duplicate/merge-compare story (Person/Family/Event/Place/Source); others omit it deliberately. |
| U37 | **No decided-state specimen for DnaMatch confirm/reject, no post-merge toast.** Both are consequence-heavy actions whose after-state feedback is unshown. | Fixed: confirmed-state head specimen added to dna-match.html; "✓ Merged · Undo" toast specimen added to merge.html, mirroring the Save toast. |
| U38 | **citation.html "Backs" chips are unlinked** — the from-citation-to-claim hop works from source.html but not from the citation's own page. | Fixed: chips link to the backing record, matching source.html's treatment. |
| U39 | **List toolbars show only a free-text filter** — PR3 ships search/filter/sort/columns, but no sort control or column chooser is depicted anywhere (mockup lags shipped code; also the 100k-scale design question). | Fixed: sort + column-chooser specimen added to person.html's list toolbar as the reference slice. |
| U40 | **Pedigree's Descendants and Relationships views have no content** — the four-view switcher (PR18, shipped) has two views that cannot be reviewed; the buttons do nothing. | Fixed: static specimens added for the Descendants tree and the Relationships/kinship result (per `RelationshipVm`/`kinship_summary`); switcher wired to swap specimens. |
| U41 | **Dashboard "Needs attention" tile rolls unbuilt checks into one bare number** while the table below correctly badges them deferred. | Fixed: tile shows the real check's count with deferred checks broken out/badged. |

## 5. Accessibility (non-blocker)

| # | Finding | Resolution |
| - | ------- | ---------- |
| U42 | **No `<h1>` on any of the 18 shell-driven pages** — the record title (`.detail-title`) is a `<div>`; heading navigation lands mid-content. | Fixed: `.detail-title` is `<h1>` on the 12 record pages; tool pages promote their lead heading (app-shell/plugin-manager/merge) or gain an `sr-only` `<h1>` where no title element existed (preferences/pedigree/search-palette). Card `<h3>`s stay (level skip noted as accepted — tabs act as the structural second level). **Done (PR40):** ported to the app — `.detail-title` is `<h1>` in the shared read/create headers (12 record screens); tool pages promoted or given an `sr-only` `<h1>`; SSR tests assert exactly one `<h1>` per screen. |
| U43 | **Data tables have no accessible name** (no caption/aria-label on any `table.tbl`; empty actions `<th>`). | Fixed: `sr-only` captions per table; visually-hidden "Actions" header text. **Done (PR40):** ported to the app — `caption` prop on the shared `Table` renders an `sr-only` `<caption>`; the empty actions header carries visually-hidden localized "Actions" text. |
| U44 | **Row-action buttons all announce as bare "Edit"/"Retract"** — the descriptive `title` never reaches the accessible name. | Fixed as pattern: per-row `aria-label` ("Edit participant: John Smith (groom)") applied on person.html + event.html as the reference exemplars; systemic rule documented in design-system.html for the remaining pages at build time. **Done (PR40):** contextual `aria-label`s completed app-side (dashboard/preferences row-action gaps filled; `row_actions_cell` already covered the rest). |
| U45 | **Chip "×" buttons are 18×24px** — under the WCAG 2.2 §2.5.8 24px floor `.btn` otherwise meets (inline `padding:0 4px` with no min-width). | Fixed: `min-width: 24px` on `.btn`. **Done (PR40):** ported to the app's bundled `components.css`. |
| U46 | **`--faint` text fails AA contrast in both themes** (2.71–4.05:1 for 11–12px counts/ids/timestamps). | Fixed: `--faint` adjusted per theme to clear 4.5:1 on `--bg` and `--panel2` (ratios re-computed). **Done (PR40):** ported to the app `tokens.css`; a Rust WCAG gate (`tests/contrast.rs`) locks the ratios. |
| U47 | **Evidence-axis and restriction chip text fails contrast in light theme** (1.97–2.89:1; dark borderline) — the 12% `color-mix` tint gives the colored text too little ground. | Fixed: tint/text values adjusted per theme until ≥4.5:1, re-computed. **Done (PR40):** `.ev`/`.resn` hues ported (dark base + full light overrides); the contrast gate asserts chip text ≥4.5:1 over its 12%-tint background in both themes. |
| U48 | **Confidence dots under 3:1 non-text contrast in light theme** (labels always present, so information is never lost). | Fixed: 1px darker border on light-theme dots. **Done (PR40):** ported per the mockup's actual token treatment — light `--conf-low`/`--conf-very-high` fills darkened (the mockup CSS carries no border ring); the contrast gate asserts each light dot ≥3:1 non-text. |
| U49 | **Merge conflict highlighting is color-only** — the tint is the sole "this differs" signal. | Fixed: "differs" text cue alongside the tint. **Done (PR40):** `differs` derived on `MergeFieldRowVm`; the screen renders `span.diff` plus a warn "differs" badge (aria-label/title). |
| U50 | **Pedigree `tree`/`treeitem` roles exist only in an HTML comment** — the prior review's "roles noted" resolution demonstrates nothing in markup. | Fixed: real `role="tree"`/`treeitem` markup on the chart; keyboard model noted as shipped-in-code. |
| U51 | **Six standalone doc pages have no `<main>` landmark.** | Fixed: `.doc` wrapped in `<main>`. |
| U52 | **person.html Facts "⚠ no source" badge wraps onto two lines** in the narrow Source column, separating icon from text. | Fixed: no-wrap on source badges in table cells. **Done (PR40):** `table.tbl .no-source, table.tbl .src-link { white-space: nowrap; }` ported to the app. |

## 6. Stage-2 plan inputs (mockup ahead of code — not mockup bugs)

Feed `ui-review-plan.md`; no mockup change unless noted.

- ~~**Person citations Page/Backs columns** — mockup (and the canonical tab shape) show them;
  `person_citations_table` renders Id/Source/Surety/Evidence only.~~ **Done (PR39):** person
  citations converged onto the shared `citations_table` with Page + Backs.
- ~~**place.html "Period" header** — the shipped screen says "Date"; the mockup's "Period" is
  the 2026-07-05 review's deliberate fix (one dated `PlaceName`). Code should adopt "Period".~~
  **Done (PR39):** place Names header now reads "Period".
- ~~**Repository `Address.fax`/`www`** — re-opened: the 2026-07-05 fix was mockup-only;
  `repository_addresses_cards` never renders them and addresses have no per-item
  `AssertionId`/edit/retract.~~ **Done (PR41):** cards render fax + www; each address carries its
  `AssertionId` (core `addresses_with_assertions` → `RepositoryAddressRef`/`RepositoryAddressVm`)
  with per-card Edit (supersede) / Retract via the PR29 correction model.
- ~~**Person-citable DNA inference** (U17's substance): no command lets a Person
  fact/association cite a DnaMatch; §12's evidence/conclusion split needs the verb.~~
  **Done (PR45):** the envelope's `CitationRef` widens to an `EvidenceRef` union (ADR 0023), so a
  Person/Family `FactAsserted`/`AssociationAsserted` cites a DnaMatch through `EventContext.citations`;
  the DNA-match screen renders the cited inferences (reading · confidence · source cue · back-link)
  and person/family assertion forms gain a "cite a DnaMatch" evidence picker.
- **DNA payload columns**: haplogroup lineage/terminal-SNP/per-row source (VM has 2 of the
  mockup's 6 columns), shared-ancestor relationship-to-A/B/confidence/source (2 of 5).
- ~~**Event `Address` surface** — data-model §17 says Address is wired on Event, but neither
  event.html nor (apparently) the DTO surface it; residence addresses drift into free text.~~
  **Done (PR42):** event.html gained an Addresses tab; the DTO carries per-address `AssertionId`
  (`EventView::addresses_with_assertions` → `EventAddressRef` → shared `AddressVm`) with per-card
  Edit (supersede) / Retract via the PR29 correction model. The card + form are shared
  (`address_cards`/`address_form`); Repository was refactored to reuse them.
- ~~**Source "Used by N records"** — computed (`SourceReliability.record_count`), never rendered.~~
  **Done (PR39):** Source overview renders "Used by N records" from `record_count`.
- ~~**Sex "Other…" free-text** — `SEXES` list excludes it.~~
  **Done (PR43):** the Sex select gained an "Other…" choice that reveals a free-text entry; a stored
  `Sex::Other(v)` now selects it and pre-fills `v` (fixing the prior mislabel as "Unknown").
- ~~**Event-screen participant add** can't set age/attributes/notes (Person-screen only).~~
  **Done (PR43):** the event add/edit-participant form renders the extracted shared `ParticipationForm`
  (role · age · attributes · notes · provenance) — person-screen parity, still writing the Person
  aggregate (the canonical participation owner). `EventEdit::AddParticipant` and `ParticipantVm` widened
  with the participant-scoped detail (app `ParticipantRef` surfaces attributes/notes).
- ~~**Place**: coordinate provenance popover unwired though VM data exists; "Code" field has no
  provenance~~; transitive hierarchy walk (direct links only).
  **Done (PR43):** the Coordinates and Code overview claims render their confidence badge + "Why we
  believe" popover (Code gained a `code_citations` app→VM data path); `place_overview`'s docstring no
  longer over-claims. The transitive hierarchy walk stays open.
- **Saved searches** — nothing in palette or list toolbars; nothing in the app layer. Needs a
  design + use-case decision (100k-scale research workflow).
- ~~**Person life-timeline** — merged Facts+Events chronological view (distinct from the History
  audit trail); Ancestry/FamilySearch parity, potential differentiator.~~
  **Done (PR44):** a read-only "Timeline" tab merges the person's facts and event participations into
  one list ordered by the `GenealogicalDate` sort key (undated last, stable tie-break), each row
  carrying the same confidence + source cue; a section-note distinguishes it from the History audit
  trail. Mockup specimen added to person.html.
- **Map/geography view for places** — coordinates exist, no visual; open product question
  (zero roadmap hits — neither promised nor deferred).
- **PR33–36 tail** (already planned, listed for completeness): undo/redo + `[`/`]` + `s`
  dispatch, live palette, global search, drag-tab-to-dock, workspace registry,
  Switch/RadioGroup primitives, surety→confidence catalogue cleanup, ambient files/net
  capability chips.

## 7. Doc follow-ups (not mockup issues)

1. **Repository media** — should Repository carry media refs (archive photos)? Data-model
   question (U31).
2. **review-findings.md** repository fax/www entry was resolved mockup-only; the code gap is
   recorded in §6 here.

## 8. Positive findings (no change — do not regress)

- **The evidence-first system is real and pervasive**: per-claim confidence + source-count or
  explicit "⚠ no source" on every fact row of every aggregate — denser than Gramps and every
  consumer tool surveyed in data-model.md §3.
- **Tags and History tabs are byte-identical across all carrying pages** — the convergence
  target the other tabs should meet. Tag's no-undo History is the one documented, correct
  exception.
- **`wireTabs` is a fully correct ARIA tabs widget** (roles, roving tabindex, full keyboard
  model) — the pattern U7's radio/switch fix mirrors.
- **Both themes fully worked out**; no hardcoded hex or one-off styles found on the 12
  aggregate pages.
- **Read/edit no-reflow model applied identically everywhere** — per-field reset, dirty-gated
  Save, provenance-on-save — matching the shipped PR27 form.
- **merge.html is unusually honest UI** (sequenced change-set wording, MergeConflict specimen,
  plain score badge) and matches shipped PR30 behavior.
- **dna-match.html's observed-vs-inferred split** is a precise evidence/conclusion rendering
  (better than surveyed consumer DNA tools) — U17 only re-badges the unbuilt half.
- **Neutral partner roles + per-parent child relationships** match GEDCOM 7 best practice.
- **Row actions never hover-gated**; `:focus-visible` ring, reduced-motion support, skip
  links, 24px button floor all present in the shared chrome.
- **PR24–32 landed cleanly**: draft creates, correction model, typed operator stamps,
  structured dates all faithfully reflected — staleness clustered only in pre-ADR-0021
  leftovers and PR33–36 previews.

## Appendix A — common-tab matrix (as-found, before fixes)

Aggregate × tab audit. "MISSING (model)" = the aggregate has no such field in data-model §6,
so absence is correct.

### Events

| Page | Columns | Row actions | Add | Count |
| ---- | ------- | ----------- | --- | ----- |
| Person | Event, Role, Date, Place, Source *(no Confidence — U11)* | Edit · Retract *(verb — U26)* | + Participate in event | yes |
| Family | Event, Date, Place, Confidence, Source | Unlink | + Add family event | yes |
| others | — (only Person/Family carry participations/linked events) | | | |

### Citations

| Page | Columns | Row actions | Add | Count |
| ---- | ------- | ----------- | --- | ----- |
| Person | Source, Page, Backs, Confidence, Evidence | Detach | + Attach citation | yes |
| Family | **MISSING (U25)** | | | |
| Event | Source, Page, Confidence, Evidence (no Backs — single-subject, OK) | Detach | + Attach citation | yes |
| Place | Source, Page, Backs, Confidence, Evidence | Detach | + Attach citation | yes |
| Media | Source, Page, Confidence, Evidence | Detach | + Attach citation | yes |
| Source | inbound view (Citation page, Backs record, Confidence, Evidence) — read-only, correct | — | — | yes |
| Citation | — (primary focus) · Repository/Note/Tag — (model) · DnaTest/DnaMatch — (deferred) | | | |

### Media

Grid-3 card gallery + "+ Attach media", identical on Person/Family/Event/Place/Source/
Citation; **no per-card action anywhere (U27)**. Missing on Repository/Note/Tag/DnaTest/
DnaMatch (model), Media (primary focus).

### Notes

Card list ("{Type} · {lang}" + body) + "+ Add note", identical on the 10 carrying pages;
**no per-card action anywhere (U27)**. Missing on Note (primary focus), Tag (model).

### Tags

Chip (`dot` + name + `×`, `aria-label="Remove tag …"`) + trailing "+ Add tag" —
**byte-identical on all 11 carrying pages**. Missing on Tag (primary focus). No changes.

### History

`tl-when/tl-what/tl-who/tl-why` + Undo-on-newest + canonical 3-clause section-note on 9
pages; Tag: no Undo + last-writer-wins note (correct, documented); DnaTest/DnaMatch:
truncated section-note (U29).

### Canonical shapes (target after fixes)

- **Events**: Event · Role (Person only) · Age (Person/Event participant rows) · Date ·
  Place · Confidence · Source; actions Edit · Remove. Sort chronological.
- **Citations**: Source · Page · Backs · Confidence · Evidence; action Detach ("Detach this
  citation — recorded in History"); "Backs" omitted where the subject is singular.
- **Media**: grid-3 cards + per-card Detach. **Notes**: cards + per-card Edit · Retract.
- **Tags**: as-is. **History**: as-is + full canonical note; Tag exception stands.
