# Genealogy Issues

## UI Issues

- Tag name (new/edit) won't accept 'g'. This has been fixed before. We need a generic way of _not_ processing global
  keys in a control. Maybe something like VS Code's 'When' (some kind of context).
- I want the keys '<Ctrl+Q>' to quit the application, and '<Ctrl+W>' to close the current tab (entity).
- Creating an entity should update the corresponding entity list with the new entity.
- A side panel (@docs/phase5/edit-patterns.html, b) will, if high enough, hide parts of the form below the window so
  that it can't be finished.
- 'Attach citation' (Reason for this change) will not close it's drop-down list of citations when losing focus.
- A notification (toast) should display at the bottom of the work area, and be automatically dismissed after a set time.
- Record detail view should remember the tag that was displayed when last seen. Don't remember when closed.

## Phase 5 UI review — deferred / open items

Parked from the completed Phase 5 UI review (`docs/archive/phase5/ui-review.md` +
`ui-review-plan.md`). Never in scope for PR33–45; each needs a design or product/model call.

- **Long-list / overflow specimen (U30)** — no tab demonstrates a long-list/overflow state;
  deferred as low-fidelity in a static mockup (the a11y real-app walkthrough covers it).
- **Repository media refs (U31)** — should Repository carry media refs (e.g. archive photos)?
  Data-model question.
- **DNA payload columns** — haplogroup lineage / terminal-SNP / per-row source (VM has 2 of 6
  columns); shared-ancestor relationship-to-A/B + per-row confidence/source (2 of 5).
- **Transitive place-hierarchy walk** — hierarchy shows direct links only; no transitive walk.
- **Saved searches** — nothing in the palette, list toolbars, or app layer; 100k-scale research
  workflow argues for it. Needs a design + use-case decision.
- **Column chooser** — `list.rs` has no column state though PR3's text claims "columns". Decide
  whether to build it or amend the PR3 description.
- **Map / geography view for places** — coordinates exist, no visual; open product question.
