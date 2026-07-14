# Phase 5 mockup review — findings

- **Date:** 2026-07-05
- **Scope:** every page in `docs/phase5/`, reviewed for **consistency, feasibility, completeness**
  against `docs/data-model.md` (§6 entities, §7 value objects, §8 EventContext, §10 command
  catalog, §12 DNA), `docs/phase5/plan.md` (locked decisions + PR status notes), and the ADRs
  (0003, 0004, 0005, 0008, 0011, 0012).
- **Method:** four parallel reviews (person/family/event · place/source/citation/repository ·
  media/note/tag/dna-test/dna-match · shell/tool/pattern pages), each building an
  aggregate × §10-command completeness matrix, then synthesized and deduplicated here.
- **Resolution column:** every finding was either fixed in the mockups on this branch or
  explicitly deferred with a reason.

## Cross-cutting findings (fixed in `record-editing.html` + echoed on every aggregate page)

| #   | Severity      | Finding                                                                                                                                                                                                                                                                                                                                    | Violates                                                                        | Resolution                                                                                                                                                                                                                                                     |
| --- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| X1  | **blocker**   | **`EventContext.rationale` ("why") and citations-at-assertion-time had no capture surface.** The edit model captured only a per-field confidence `<select>`; History displays rationale quotes and the prov popover shows per-claim citations/evidence axes, but no form collected them. The who/when/why differentiator was display-only. | data-model §8, ADR 0004 §1                                                      | Fixed: `record-editing.html` gains §5b "Provenance on Save" (rationale field, citation attach, evidence axes, confidence); every aggregate page's new edit-mode specimen includes the provenance block.                                                        |
| X2  | **blocker**   | **Retract and supersede were unreachable.** No collection row had a Remove/Retract control (only Person Names had per-row Edit); nothing stated that editing an existing value emits `AssertionSuperseded` referencing the prior `AssertionId`. The only correction path was History "↩ Undo" on the newest entry.                         | data-model §10 (universal `RetractAssertion`/`SupersedeAssertion`), ADR 0004 §2 | Fixed: every collection table now carries per-row **Edit** (→ supersede) and **Retract**/**Remove**; tag chips get **×** (Untag); `record-editing.html` §8 states the supersede-by-`AssertionId` semantics.                                                    |
| X3  | gap           | **The Remove\* half of the catalog was absent everywhere**: `RemovePartner`, `RemoveChild`, `RemoveParticipantRole`, `Untag`, detach-citation.                                                                                                                                                                                             | data-model §10                                                                  | Fixed with X2 (per-row remove + chip ×).                                                                                                                                                                                                                       |
| X4  | inconsistency | "Surety" and "Confidence" used interchangeably for the same `Confidence` value object (table headers vs card/badge).                                                                                                                                                                                                                       | data-model §7                                                                   | Fixed: standardized on **Confidence** in all headers/labels.                                                                                                                                                                                                   |
| X5  | inconsistency | Evidence-axis chips abbreviated inconsistently ("orig"/"deriv" in tables vs full words in cards/popovers).                                                                                                                                                                                                                                 | data-model §7 `EvidenceAnalysis`                                                | Fixed: full words everywhere (original/derivative · primary/secondary · direct/indirect/negative).                                                                                                                                                             |
| X6  | inconsistency | Date editing was a single free-text input, sometimes concatenating date **and** place ("12 Apr 1850 · New York, USA"); nothing demonstrated the structured `GenealogicalDate` (calendar/modifier/quality/original text).                                                                                                                   | data-model §7.1                                                                 | Fixed: event.html edit specimen demonstrates the structured date fields; date and place are separate inputs; the Precision row shows the *actual* modifier/quality/calendar/original text (was: all options at once). Full DatePicker remains a PR1 component. |
| X7  | inconsistency | The "why we believe" `.prov` block rendered as a standing inline section on several pages; the locked pattern is an anchored per-claim **popover** triggered from a value's ❝ source link (and `.prov` is `position:absolute`, so it needs a positioned ancestor).                                                                         | plan.md (provenance popover)                                                    | Fixed: every inline `.prov` is now labeled as a popover **specimen** ("opens anchored to the ❝ source-link").                                                                                                                                                  |
| X8  | inconsistency | Edit-entry keybinding conflict: record-editing.html said `Enter`/`F2`, shortcuts.html + shell.js say `e` (and plan.md's locked map says `e` on a fact row). `Enter` also means "open" in lists and "commit" in edit mode.                                                                                                                  | plan.md keyboard map                                                            | Fixed: `e`/`F2` (or double-click) enters edit; `Enter` commits. record-editing.html updated; shell.js `?`-overlay gains the missing "Editing a record" group + `⌘S`.                                                                                           |

### User-added scope

Every per-aggregate record page now carries an **edit-mode specimen** (whole-record draft,
Save/Cancel, per-field reset, aggregate-specific field controls) including the X1 provenance
block — read mode and edit mode are both mocked on all 12 pages.

## Per-page findings

### person.html

| Severity | Finding                                                                                                  | Resolution                                                                                 |
| -------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| gap      | `AssertSex` had no affordance (sex displayed in head only).                                              | Fixed: Sex select (Male/Female/Unknown/Intersex/Other) in the edit-mode specimen.          |
| gap      | No way to attach a citation to an **individual name** (`NameAsserted` carries `EventContext.citations`). | Fixed: per-row "❝ Cite" action on the Names table; provenance block covers new assertions. |
| gap      | Facts/Events/Associations/Citations tables had no row actions (X2/X3).                                   | Fixed: Edit · Retract (Detach for citations) per row.                                      |

### family.html

| Severity | Finding                                                                                                                                                                                                | Resolution                                                                                                                             |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| gap      | `AddPartner`/`RemovePartner` had no affordance — the Partners card was read-only.                                                                                                                      | Fixed: "+ Add partner" + per-row Edit/Remove.                                                                                          |
| gap      | `RemoveChild` missing; children/events rows had no actions.                                                                                                                                            | Fixed: row actions added.                                                                                                              |
| polish   | Standalone "married" badge implied a stored relationship-type field that doesn't exist (partners carry neutral roles; marriage is a family-level *event*); "religious ceremony" is not an `EventType`. | Fixed: badge dropped (the sub-line derives "married 1876" from the linked event); Type row replaced with a link to the Marriage event. |

### event.html

| Severity      | Finding                                                                                                                                                           | Resolution                                                      |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| inconsistency | "Precision" row showed *all* modifier badges ("exact", "abt", "range", calendar) simultaneously — a `GenealogicalDate` has exactly one modifier/quality/calendar. | Fixed: shows this date's actual structure + original text (X6). |
| gap           | `RemoveParticipantRole` missing; participants/citations rows had no actions.                                                                                      | Fixed: row actions added.                                       |

### place.html

| Severity      | Finding                                                                                                                                           | Resolution                                                                |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| gap           | `AssertEnclosedBy` had **no affordance** — Hierarchy tab was read-only despite History showing an "Enclosed-by set" event.                        | Fixed: "+ Add enclosing place" (dated `PlaceRef`) + per-row Edit/Retract. |
| inconsistency | Names table split one dated `PlaceName` into independent From/To columns (a `PlaceName` has a single `GenealogicalDate` with a From/To modifier). | Fixed: single "Period" column ("from 1664" / "until 1664").               |

### source.html

| Severity | Finding                                           | Resolution                            |
| -------- | ------------------------------------------------- | ------------------------------------- |
| gap      | Repositories/Attributes rows had no actions (X2). | Fixed: Edit · Unlink/Retract per row. |

### citation.html

| Severity | Finding                                                                                                                        | Resolution    |
| -------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------- |
| gap      | Header missing the universal `Restriction` multi-select (`resn-set`) that Person/Family/Event/Place/Source show.               | Fixed: added. |
| polish   | Mock-data mismatch: head badge said `source: S0008` while the Overview/list point at the 1850 census = `S0003` on source.html. | Fixed: S0003. |

### repository.html

| Severity | Finding                                                                                                              | Resolution         |
| -------- | -------------------------------------------------------------------------------------------------------------------- | ------------------ |
| gap      | Header missing `resn-set`.                                                                                           | Fixed: added.      |
| gap      | Address cards omitted `fax` and `www` — part of the wired `Address` value object (only `original_text` is deferred). | Fixed: rows added. |

### media.html

| Severity      | Finding                                                                                         | Resolution                                            |
| ------------- | ----------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| gap           | Header missing `resn-set`.                                                                      | Fixed: added.                                         |
| gap           | No Attributes surface at all (`Media.attributes` / `AttributeAdded`; PR10 DTO exposes them).    | Fixed: Attributes tab + table with add/edit/retract.  |
| inconsistency | Record-level confidence badge in the head — confidence is per-assertion (§8), never per record. | Fixed: dropped.                                       |
| gap           | "Related media" card had no model backing (no media↔media relation or query).                   | Fixed: removed ("Used by" card already covers usage). |
| polish        | Head sub showed file size + pixel dimensions no DTO exposes.                                    | Fixed: dropped.                                       |

### note.html

| Severity      | Finding                                                                                                                                                        | Resolution                                                |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| gap           | Header missing `resn-set`.                                                                                                                                     | Fixed: added.                                             |
| inconsistency | Note identified by raw UUID in list + head; Note has a `human_id` (`N####`) and every other page leads with it (same principle as the tag-display convention). | Fixed: `N0001`-style id shown; UUID demoted to a tooltip. |

### tag.html

| Severity      | Finding                                                                                        | Resolution                                  |
| ------------- | ---------------------------------------------------------------------------------------------- | ------------------------------------------- |
| inconsistency | History showed "↩ Undo" — tags have no retraction (plan.md PR11: tag History is display-only). | Fixed: Undo removed; section-note reworded. |

### dna-test.html

| Severity        | Finding                                                                                                                                                                                         | Resolution                                                            |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| gap/feasibility | "⤓ Import raw", a raw-data file row, "318 total" matches, and a "Raw data imported" History entry had no model/use-case backing (§12 has no raw-genotype storage; only observed matches exist). | Fixed: removed; matches line shows observed matches only.             |
| polish          | Ethnicity card claimed the estimate "is stored as a cited assertion" — no such event exists yet.                                                                                                | Fixed: reworded to future tense (needs a future `EthnicityAsserted`). |
| polish          | Deferred mockup-only fields (Account, Date tested, SNPs tested — per plan.md) were indistinguishable from backed ones.                                                                          | Fixed: marked with a "deferred" badge.                                |

### dna-match.html

| Severity      | Finding                                                                                                                                                                                                                                                                                                                                   | Resolution                                                                                                                               |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| gap           | The **inferred relationship** was presented as a DnaMatch-owned, match-editable value, and History listed "Relationship inferred … ↩ Undo" as a DnaMatch event. §12 is explicit: the inference is a separate `FactAsserted`/`AssociationAsserted` on Person/Family that *cites* the match — there is no such verb in the DnaMatch stream. | Fixed: card is read-only with a "view assertion on Person" link; the History entry moved out (replaced by match-stream events).          |
| gap           | `MatchConfirmed`/`MatchRejected` had no affordance and no status display, though the use-cases + `MatchStatus` DTO exist (PR11).                                                                                                                                                                                                          | Fixed: status badge + Confirm/Reject actions in the head.                                                                                |
| gap           | Segments and Shared-ancestors tabs were read-only ( `add_segment`/`assert_shared_ancestor` use-cases exist).                                                                                                                                                                                                                              | Fixed: "+ Add segment" / "+ Link shared ancestor" + row actions.                                                                         |
| inconsistency | Head claimed "DNA matches have no short id — identified by their UUID v7"; the app assigns `human_id` format `X%04d` (`genealogy-app/src/aggregates.rs:60`).                                                                                                                                                                              | Fixed: `X0007`-style id shown. **Doc follow-up:** data-model §12 is silent on DnaTest/DnaMatch `human_id` — should state they carry one. |

### merge.html

| Severity      | Finding                                                                                                                                                                                                                            | Resolution                                                         |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| gap           | No reason/rationale input — §9 requires `PersonsMerged` to record operator + rationale + confidence.                                                                                                                               | Fixed: "Reason for merge" field before the Merge action.           |
| gap           | `MergeConflict` (§10.1) never surfaced — no blocked/irreconcilable state.                                                                                                                                                          | Fixed: conflict specimen (blocked merge with explanation).         |
| feasibility   | Merge framed as one atomic, reversible event; it is `MergePersons` + follow-up assertions across aggregates, committed per aggregate (cqrs-es). record-editing.html §6b states this correctly — the pages contradicted each other. | Fixed: reworded to a sequenced change-set (undo reverses the set). |
| inconsistency | ConfidenceBadge reused for the duplicate-detector match **percentage** — a match score is not the 5-level `Confidence`.                                                                                                            | Fixed: plain badge for the score.                                  |

### pedigree.html

| Severity      | Finding                                                                                                                                              | Resolution                                         |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| inconsistency | Nodes showed name + dates only — the shipped PR18 screen renders a ConfidenceBadge per node and `tree`/`treeitem` roles; the mockup lagged the code. | Fixed: confidence cue added to nodes; roles noted. |

### app-shell.html

| Severity | Finding                                                                                                                                                                 | Resolution                                                                  |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| gap      | The locked "in-app record tabs + drag-to-split" decision was demonstrated nowhere (no page set `MOCK.tabstrip`).                                                        | Fixed: app-shell now shows a populated record tabstrip + drag-to-dock hint. |
| polish   | Dashboard showed "Death before birth · 2" and "Possible duplicates · 14" as if built (plan.md: only unsourced-facts is real; implementation must not fabricate counts). | Fixed: deferred checks labeled.                                             |

### preferences.html

| Severity | Finding                                                                                                                                            | Resolution                                                                                |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| gap      | No workspace registry/list (ADR 0005 `[workspaces.<name>]` + default-by-name is in PR20's surface).                                                | Fixed: Workspaces card added.                                                             |
| polish   | The three-layer defaults stack collapsed ADR 0005's `[defaults]` (frozen at use) and `[workspace-defaults]` (live fallback) into one middle layer. | Fixed: middle layer labeled `[workspace-defaults]`; frozen `[defaults]` noted separately. |
| a11y     | Theme radios / toggles set both `aria-checked` and `aria-pressed` by reusing `.resn`.                                                              | Fixed: single correct ARIA state per role.                                                |

### plugin-manager.html

| Severity    | Finding                                                                                                                                                                                                                                                                 | Resolution                                                                   |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| feasibility | Capability chips showed a "media-store" interface that doesn't exist — ADR 0011 fixes the vocabulary as `log`/`query`/`commands` (+ ambient `files`/`net`, denied by default); gedcom-import lacked a chip gramps-import carried despite doing media round-trip (PR15). | Fixed: real capability vocabulary; per-plugin chips reconciled.              |
| a11y        | Enable switches set both `aria-checked` and `aria-pressed`.                                                                                                                                                                                                             | Fixed.                                                                       |
| polish      | Trust tiers "Phase 8" follows plan.md PR21, but ADR 0011 §6 places signing/trust in roadmap Phase 4 / ADR 0014.                                                                                                                                                         | **Doc follow-up** (plan.md ↔ ADR 0011 phase mismatch) — not a mockup change. |

### strengths.html / design-system.html / edit-patterns.html

| Severity | Finding                                                                                                                                  | Resolution                                                                  |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| polish   | strengths.html called the merged-from record a "Duplicate"; §9's term is **persona** (retained, non-destructive).                        | Fixed: terminology.                                                         |
| gap      | The attach dialog never surfaced the **role** a link carries (`ParticipantRole` on person→event, `ChildParentRelationship` per parent).  | Fixed: role select added to the attach-dialog pattern (edit-patterns.html). |
| gap      | design-system.html lacked Switch/RadioGroup primitives, which is why `.resn` was overloaded (see preferences/plugin-manager a11y fixes). | Fixed: Switch + RadioGroup specimens added.                                 |

## Completeness matrix — after fixes

All §10 commands now have a reachable affordance on their aggregate page (create via list
"+ New"; scalar assertions via whole-record Edit; collections via "+ Add" and per-row
Edit/Retract; `Tag`/`Untag` via "+ Add tag"/chip ×; `SetRestrictions` via the header `resn-set`
on all aggregates that carry it; `RetractAssertion`/`SupersedeAssertion` via per-row
Retract/Edit + History Undo; `MergePersons` via Compare → merge wizard; DnaMatch
confirm/reject via the head actions). Remaining intentional exceptions:

- **Tag** — no restrictions, no retraction (by design; §6, PR11).
- Deferred mockup-only fields are labeled "deferred" in place (see below).

## Deliberately deferred (per plan.md / data-model §17 — labeled, not fixed)

- DnaTest **Account / Date tested / SNPs tested**; DnaMatch **fully-identical regions**;
  haplogroup **Terminal SNP** column; first-class **citation collections on DNA records**.
- `RichText` **translator** GEDCOM/Gramps round-trip (display is backed).
- `Address.original_text` verbatim fallback.
- Dashboard **death-before-birth / possible-duplicates** checks (data-quality framework
  follow-up); merge duplicate detection is PR19.
- Full structured **DatePicker** (calendar/modifier/quality/original text) is a PR1 component;
  the mockups demonstrate the shape, not the widget.
- Pedigree privacy `Restriction` chart cue (PR18 deferral).
- `ExternalId` read surface on DnaTest/Media (import-only today; no §10 verb).

## Doc follow-ups (not mockup issues)

1. **data-model §12** — state that `DnaTest`/`DnaMatch` carry a `human_id` (`D%04d`/`X%04d`),
   matching the shipped app layer.
2. **plan.md ↔ ADR 0011** — reconcile the trust-tier/signing phase (plan.md PR21 says Phase 8;
   ADR 0011 §6 says roadmap Phase 4 / ADR 0014).

## Addendum (2026-07-05, post-review) — create flows persist-then-edit

**blocker · feasibility/consistency — missed by the review** (the review checked *affordance
presence* per screen, not create *semantics*); raised by the user afterwards and verified in code.

Only two aggregates create records the way the locked model requires (`record-editing.html` §6:
create = throwaway draft, **nothing written until Save**, Cancel discards): **Person**
(`commit_person_change_set` — validate-first, Source→Citation→Person sequenced) and **Tag**
(`commit_tag_change_set`). The other **10** persist an aggregate the moment "+ New" is pressed and
then edit the already-created record (`crates/genealogy-ui-dioxus/src/services.rs`):

- empty creates: `create_family_record` (:192), `create_place_record` (:243),
  `create_source_record` (:273), `create_repository_record` (:302, name `None`),
  `create_media_record` (:331, path `None`), `create_note_record` (:360, text `None`);
- **fabricated data**: `create_event_record` (:214) persists a placeholder `EventType::Birth` — a
  false assertion in the append-only log;
- partial dialogs that still instant-persist: `create_citation_record` (:158),
  `create_dna_test_record` (:401), `create_dna_match_record` (:423 — additionally zero-fills an
  unparseable shared-cM via `unwrap_or_else`).

Because the log is append-only, an abandoned "+ New" leaves a junk aggregate forever (retractable,
never removable) — Cancel-with-nothing-written is impossible. **Resolution:** planned as
[`plan-2.md`](plan-2.md) **PR 26** (generalize the person/tag change-set pattern to per-aggregate
draft creates; no placeholder payloads; cascading create per §6b).
