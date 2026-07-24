# 27. Configurable surety-scheme labels

- **Status:** Accepted
- **Date:** 2026-07-23

## Context

`Confidence` (data-model §7) is a fixed, closed five-variant enum — `VeryLow`/`Low`/`Normal`/`High`/
`VeryHigh` — carried in `EventContext.confidence` (every assertion) and `Citation.confidence`. It is
Gramps' own five-level scale, aligned with GEDCOM `QUAY 0-3` (lossy, one direction) and GEDCOM X's
`ConfidenceLevel`. data-model §17 names "making the scheme configurable" as deferred; ADR 0015 §Out of
scope names a configurable surety scheme as workspace-functionality-scope config with "no consumer yet
... not added (YAGNI)".

GENTECH's own model goes further than a fixed scale: `SuretyScheme`/`SuretySchemePart` (confirmed via
the GDMUML class model, a UML restatement of the GENTECH ER model) attaches an **arbitrary-cardinality,
project-owned** scale to assertions — "the researcher determines the appropriate level of surety...
the data model allows different schemes to be used... the scheme used with a particular project is
attached to the data it describes" (GENTECH GDM 1.1 primer). GenTle, an open implementation, lets a
user define any number of named schemes, each with an arbitrary number of parts. No mainstream shipping
product surveyed (Gramps, RootsMagic, Evidentia, GEDCOM X itself) offers this generality — every one
hardcodes a small fixed scale (full findings: `docs/research/surety-schemes.md`).

Two different things are conflated by "configurable surety scheme": **relabeling** the five existing
ordinals (a workspace's own wording for the same five positions) versus **re-scaling** their cardinality
(a workspace choosing 3, 7, or N levels — GENTECH's real generality). Re-scaling would change
`Confidence`'s shape — a type referenced in the payload of every already-persisted event across all
twelve aggregates, in the GEDCOM `QUAY` and Gramps confidence round-trip mappings, and in every UI
confidence-chip renderer — for a generality no surveyed shipping product's users are shown to need.

## Decision

1. **`Confidence`'s wire shape is unchanged.** The enum keeps its five fixed variants as the value
   stored in `EventContext.confidence` and `Citation.confidence`/`EvidenceAnalysis`. No event payload
   change, no version bump, no upcaster question — this ADR is presentation-only.

2. **A per-workspace `SuretyScheme` is workspace-functionality configuration** (ADR 0015 §1): exactly
   five ordered `SuretyLevel { label: String, description: Option<String> }` entries, index 0 =
   `VeryLow` .. index 4 = `VeryHigh`. It follows the `id_formats` precedent already in
   `workspace.toml` — a per-workspace override, falling back to a documented default (Gramps' own five
   labels — "Very Low"/"Low"/"Normal"/"High"/"Very High") through the same live `[workspace-defaults]`
   app-level fallback ADR 0005/0015 already established.

3. **Every surface that renders a `Confidence` resolves its label through the workspace's
   `SuretyScheme`, not a hardcoded Fluent id per variant.** One Fluent placeholder per ordinal position
   supplies the default wording; the workspace override substitutes at render time. GEDCOM `QUAY` and
   Gramps confidence import/export keep mapping against the fixed ordinals — labels never enter the
   round-trip, only the ordinal, exactly as documented today (data-model §16).

4. **Re-scaling cardinality is explicitly out of scope.** A workspace choosing a different *number* of
   surety levels is not built here: no demonstrated consumer need exists (mirroring ADR 0015's own
   deferral rationale), and it would touch a core type referenced by all twelve aggregates' already-
   persisted events. If a real research workflow demands it, a follow-up ADR revisits `Confidence`
   itself — this ADR does not foreclose that, it only declines to speculate ahead of the need.

## Rationale

- **Relabeling satisfies GENTECH's stated motivation in the one dimension that is actually about fit of
  meaning** — "the researcher determines the appropriate level of surety... rather than an arbitrary
  standard that may not fit the data" — without touching a type baked into every historical event.
- **Matches an existing precedent exactly.** `id_formats` is already a per-workspace, presentation-
  facing override with a documented default and a live app-level fallback; `SuretyScheme` is the same
  shape applied to a different field.
- **A scheme changed after assertions exist would otherwise silently reinterpret old claims.**
  Re-scaling under a mutable per-workspace cardinality risks exactly the ambiguity GDMUML's own
  commentary flags GENTECH left unresolved (per-axis surety within one assertion); relabeling has no
  such hazard because the ordinal — and therefore the meaning already recorded — never changes.

## Consequences

### Positive

- Closes the data-model §17 item with a minimal, safe slice: a workspace can adopt its own or an
  archive's surety vocabulary (or a translation of Gramps' own five labels) with no event-log, GEDCOM,
  or Gramps round-trip change.
- No blast radius on the twelve aggregates' `decide`/`evolve` code, the format plugins, or the
  cross-aggregate invariant checks — every consumer still matches five fixed variants.

### Negative / costs

- Does not deliver GENTECH's full generality (arbitrary schemes, arbitrary cardinality, per-axis
  surety); a workspace that genuinely wants a 3- or 8-level scale is not served by this ADR.
- A small, mechanical Fluent-resolution change is needed at every site that currently hardcodes a
  per-variant confidence label (person/family/event/place/source/citation/DNA screens and the CLI).

## Out of scope

- **Cardinality / re-scaling** the number of surety levels (GENTECH's full `SuretyScheme` generality).
- **Per-axis surety** within one assertion (GDMUML's own noted extension beyond the base GENTECH model)
  — `EvidenceAnalysis`'s three axes (data-model §7) already give source/information/evidence
  granularity; this ADR does not add a fourth.
- **Exporting the scheme definition itself** alongside data (GENTECH's own recommendation for a
  genuinely portable scheme) — moot while cardinality is fixed, since every workspace shares the same
  five ordinals; revisit only if cardinality ever becomes configurable.

## References

- ADR 0005 / 0015 — the workspace-functionality scope and the `id_formats`
  per-workspace-override-with-live-fallback precedent this ADR follows.
- `docs/data-model.md` §7 (`Confidence`), §8 (`EventContext.confidence`), §16 (the `QUAY`/Gramps
  confidence round-trip this ADR leaves unchanged), §17 (the deferred item this ADR closes).
- `docs/research/surety-schemes.md` — the GENTECH/GDMUML/GenTle findings and the relabeling-vs-
  rescaling distinction this decision rests on.
