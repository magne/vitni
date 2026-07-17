# Remaining work (archived)

- **Status:** **Superseded 2026-07-17** — the live outstanding-work items moved to
  [`docs/issues.md`](../issues.md); phase-owned work is tracked in
  [`roadmap.md`](../roadmap.md). Retained here as a historical record; claims below were
  corrected against the code at archive time.
- **Purpose (historical):** was the single index of work planned, deferred, or unfinished after the
  Phase 5 UI and data-model-review efforts closed. The completed planning docs live alongside this
  file under [`docs/archive/`](.) (Phase 5 `plan.md`/`plan-2.md`/`review-findings.md`, the
  data-model review + its plan, and the Phase 4 follow-ups).

## Phase 5 UI — deferred

From the archived [`plan-2.md`](phase5/plan-2.md) "Deliberately deferred" list and the PR delivery
notes.

### No core backing yet

- DnaTest **Account** / **Date tested** / **SNP count**.
- DnaMatch **terminal-SNP** / **fully-identical regions** (segment lineage only partially present).
- First-class **citation collections on DNA records**.

### Round-trip / read surface

- `RichText` **translator** GEDCOM/Gramps round-trip (display is already backed; no standard tag).
- `Address.original_text` round-trip — the core field exists (`genealogy-core` `address.rs`); the
  GEDCOM/Gramps format crates don't carry it yet.
- `ExternalId` read surface — the `AddExternalId`/`add_external_id` verb exists but is **import-only**
  (only `import.rs` calls it); no interactive CLI/UI caller.

### UI follow-ups

- Note-translation **Retract** needs a `remove_translation` core verb — Edit-only today (PR29).
- Inline **"+ New"** on the remaining attach side panels — place / citation→source / partner pickers
  already have it; the rest (DNA/repository/event refs) are existing-only (PR28).
- No GUI **Import-GEDCOM** command (CLI-only). *(The plugin-host `assert_fact` path handles structured
  dates; a GUI fact form would need to carry them.)*
- `prepare_import_target` not yet lifted into `workspace_registry` — still inline in the CLI (PR36).
- Pedigree `Restriction` **chart cue** and **name-autocomplete** focus/relationship pickers, which
  are plain `human_id` text inputs today (PR18).
- Manual desktop **drag-to-dock** walkthrough unverified — SSR cannot fire drag events (PR36); wire
  `with_disable_file_drop_handler` only if the native file-drop handler suppresses HTML5 DnD.

### Plugin-UI vocabulary (ADR 0022 out-of-scope, PR37)

Repeating groups / nested forms; `List`/detail descriptions + plugin-driven navigation; per-field
validation vocabulary; plugin-prefilled field values; the `query` capability for `ui-panel`;
long-running / streaming actions; multi-panel pages.

### Further out

- Full **trust-tier / signing UX** → roadmap Phase 8 (ADR 0014).
- DNA **rich visualizations** → roadmap Phase 9.

## Phase 5 UI — code-vs-mockup re-review (complete)

The 2026-07 re-review shipped as PRs 39–45 (all merged); findings and plan are archived at
[`phase5/ui-review.md`](phase5/ui-review.md) and [`phase5/ui-review-plan.md`](phase5/ui-review-plan.md).
Its deferred/decision residue lives in [`docs/issues.md`](../issues.md).

## Performance — deferred

- **List DOM virtualization.** The lightweight-row use-cases (`list_person_rows`/`list_family_rows`/
  `list_event_rows`) removed the heavy `Lookups`/`FamilyLookups`/`EventLookups` join cascade, so list
  data now loads cheaply. Still deferred: `ListPane`
  (`crates/genealogy-ui-dioxus/src/master_detail.rs`) mounts every row and a `MountedEvent` per row.
  A follow-up should render only a scrolled window with a `store.count`-sized spacer and make the
  roving-focus `nodes` bookkeeping window-aware. If server-side windowing is chosen instead of the
  current client-side search/sort over the full cached row set, add `list_view_page(table, offset,
  limit)` (+ Postgres mirror) and a generated column + index on `$.state.human_id` in
  `crates/genealogy-db`. *(Now tracked in [`docs/issues.md`](../issues.md).)*

## Phase 4 import/export — remaining (tracked in roadmap)

From the archived [`phase-4-followups.md`](phase-4-followups.md); the foundation (PR2 groups A–G)
shipped and the remainder is now owned by later roadmap phases.

- Smaller model round-trip gaps (data-model §17): multi-`NAME`, `FAMS`/`FAMC` back-refs, place
  `MAP`/coordinates, `SUBM`, media `FORM`, citation `CALN`, GEDCOM `REPO` records/pointer,
  `FAM`-level `SOUR`/`OBJE`/`NOTE`, Gramps `<tagref>` → **roadmap Phase 7**.
- True **merge / sync** (reconcile divergent values, not additive-only append) → **roadmap Phase 7**.
- **Digitalarkivet assisted importer** (ADR 0017) + interactive present-and-confirm → **roadmap Phase 6**.
- Plugin **signing / trust tiers / distribution** (ADR 0014) → **roadmap Phase 8**.
- Smaller deferred: Software-agent identity/version from the plugin manifest; localized `progress`
  steps (treat the `step` string as a Fluent id); wire Ctrl-C → `progress` `cancel`; epoch-based
  wall-clock timeout as the successor to fuel.

## Data-model review — complete

All 10 findings of the archived [`data-model-review.md`](data-model-review.md) are implemented or
documented (PRs #110–#119; ADRs 0019–0021). No remaining work — listed for closure.

## Note — ADR references after the archive move

The path mentions of `docs/data-model-review.md` and `docs/phase-4-followups.md` inside ADRs 0013,
0018, 0019, 0020, and 0021 were rewritten to their `docs/archive/…` locations. This is a path-only
edit — no decision or rationale text changed — recorded here so the exception to ADR immutability is
auditable.
