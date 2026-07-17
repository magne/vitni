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

## Deferred core / app / round-trip work

Migrated from the retired `remaining-work.md` (now under `docs/archive/`). Genuine, code-verified
gaps that are **not** owned by a roadmap phase (phase-owned work — trust-tier/signing → Phase 8,
DNA rich visualizations → Phase 9, GEDCOM/Gramps round-trip breadth → Phase 6/7, migration story →
future — stays in [`roadmap.md`](roadmap.md), not duplicated here).

### Core (no backing yet)

- **DnaTest fields** — `account`, `date_tested`, `snp_count` are absent from `DnaTestState`.
- **DnaMatch depth** — no terminal-SNP, no fully-identical-regions (segment lineage only partially
  present via `ChromosomeSide` + `snps`). (Distinct from the UI-side "DNA payload columns" above.)
- **DNA citation collections** — both DNA aggregates hardcode `citations: Vec::new()`; provenance
  is stubbed empty.
- **`remove_translation` verb** — note-translation retract is Edit-only (no core verb to remove a
  single translation).

### App / UI

- **GUI Import-GEDCOM command** — the CLI imports; `genealogy-ui-dioxus` has no import flow.
- **`prepare_import_target`** — still inline in the CLI; lift it into
  `genealogy-app::workspace_registry` (the rest of `init` already delegates).
- **Pedigree** — no `Restriction` chart cue; focus/relationship pickers are plain `human_id` text
  (want name-autocomplete).
- **`ListPane` DOM virtualization** — `master_detail.rs` mounts every row (and a `MountedEvent`
  per row). Render only a scrolled window with a `store.count`-sized spacer and make the
  roving-focus `nodes` bookkeeping window-aware. If server-side windowing is chosen instead, add
  `list_view_page(table, offset, limit)` (+ Postgres mirror) and a generated column + index on
  `$.state.human_id` in `genealogy-db`.

### Round-trip (interchange plugins)

- **RichText translator** GEDCOM/Gramps round-trip (display is already backed; no standard tag).
- **`Address.original_text`** round-trip — the core field exists; the format crates don't carry it.

### Plugin-UI vocabulary tail (ADR 0022 out-of-scope)

Repeating groups / nested forms; `List`/detail descriptions + plugin-driven navigation; per-field
validation vocabulary; plugin-prefilled field values; the `query` capability for `ui-panel`;
long-running / streaming actions; multi-panel pages.
