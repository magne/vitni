# 26. Place succession and temporal resolution

- **Status:** Accepted
- **Date:** 2026-07-18

## Context

A place is not fixed in time. Its **name** changes (Oslo → Christiania → Kristiania → Oslo), its
**jurisdiction** changes (a municipality moves between counties; administrative levels are added or
removed), its **boundary** grows and shrinks, and its very **identity** can change — municipalities
**merge** (Aker + Kristiania → Oslo, 1948) and counties **split**.

The model already covers the first three without change (confirmed in `genealogy-core`):
`PlaceName { value, date, language }` (`place_name.rs`) and `PlaceRef { place_id, date }`
(`place_ref.rs`) both **accumulate dated assertions**, and ADR 0024 makes `PlaceGeometry` dated and
accumulating too. What is missing is (a) a defined rule for reading those dated assertions *as of a
date*, and (b) any way to record an **identity change** — today there is no succession/merge concept
for Place (unlike Person's `PersonsMerged`/`AssociationAsserted`).

This ADR fixes those two gaps. It builds on ADR 0024 (geometry & storage) and is consumed by the
geography view (ADR 0025); it does not restate the event-sourcing contract (ADR 0004) or the
projection schema (ADR 0009). It gates the full geography phase (roadmap Phase 9).

## Decision

1. **Date-aware resolution rule.** A place's name, enclosing parent, and geometry are each resolved
   *as of a target date* by treating each dated assertion's `date` as **"effective from"** and
   selecting the assertion with the latest effective date **≤ target** (the Gramps approach), falling
   back to the undated/primary assertion when none qualifies. Assertions remain unordered in the log;
   resolution is a pure read over the accumulated set. This single rule drives the geography time
   slider, the generated place title, and the transitive hierarchy walk. An explicit `[from, until)`
   **validity interval** on an assertion is deliberately **not** adopted first — the effective-from
   rule needs no schema change and matches how sources record "renamed in year Y"; an interval can be
   added additively later if gaps/overlaps prove ambiguous.

2. **Rename stays on the same aggregate; succession is for identity change.** A place that is merely
   renamed keeps its aggregate and gains a new dated `PlaceName` (§1). A **succession** is recorded
   only when a place's *identity* changes — it ceases, is born, or is absorbed. This keeps the common
   case (rename) cheap and reserves succession for genuine lifecycle events.

3. **Succession is a dated, typed link between Place aggregates.** A new
   `PlaceCommand::AssertSuccession` → `PlaceEventBody::SuccessionAsserted { from, to, kind, date }`,
   where `from`/`to` are lists of `PlaceId` and
   `kind = Merged | Split | Absorbed | Elevated | Renamed`. Cardinality carries the meaning: merge is
   many→one, split is one→many, absorb/elevate/rename are one→one. It is modelled like Person
   `AssociationAsserted` — an `Attributed<Asserted<…>>` claim carrying its `AssertionId`, confidence,
   and citations (ADR 0004 §2, ADR 0020/0021), correctable by retract/supersede. The event is
   self-contained (both endpoint ids in the payload; ADR 0002) and additive (ADR 0004 §4).

4. **Succession is projected as a navigable, symmetric relation.** `genealogy-db` projects
   succession so a place exposes both its predecessors and its successors (a merged-away place links
   to what it became, and the survivor links back), enabling "what happened to this place?" and
   date-scoped resolution across an identity boundary. The projection is derived and rebuildable from
   the log (ADR 0009/0010). Existence of the referenced places is validated against the projection —
   the "aggregate tax" pattern (data-model §9), like `UnknownPlace`.

## Consequences

### Positive

- Historical records resolve to the correct name, jurisdiction, and boundary for their date, and the
  map can be shown "as of" any year with one shared rule.
- Municipality mergers and splits — pervasive in Nordic administrative history — are first-class and
  navigable, without destroying either the old or the new place (audit intact).
- No schema churn for the temporal read: the effective-from rule reuses the dated assertions that
  already exist.

### Negative / costs

- A new command/event and a symmetric projection on the Place aggregate, plus resolution helpers used
  in several layers (title, hierarchy walk, time slider).
- The effective-from rule leaves gaps/overlaps implicit; if real data needs precise intervals, an
  additive `[from, until)` follow-up is required.
- Deciding merge/split cardinality UX (assigning territory/coordinates to successors) is real UI work
  in the geography phase.

## Out of scope

- **The geometry types, spatial index, and interchange** — ADR 0024.
- **The geography view, in-map editing, provider, and geocoding** — ADR 0025.
- **Explicit validity intervals** — an additive follow-up only if the effective-from rule proves
  ambiguous in practice.

## References

- ADR 0002 / 0004 — self-contained versioned events; the pure `decide` path; `AssertionId`
  corrections.
- ADR 0009 / 0010 — projection schema; derived, rebuildable projections; the "aggregate tax" check.
- ADR 0020 / 0021 — assertion granularity and evidence-in-the-envelope (`Attributed<Asserted<T>>`).
- ADR 0024 — `PlaceGeometry` (dated, accumulating) this resolution rule reads.
- ADR 0025 — geography view + time slider that consume this rule and succession links.
- `docs/data-model.md` §7 — Place, `PlaceName`/`PlaceRef` (dated); §9 the aggregate tax.
- `docs/archive/plans/places-geography-temporal.md` — the phase this ADR gates.
