# Research — configurable surety schemes (Phase 10, gates ADR 0027)

- **Status:** Findings informing ADR 0027 (workspace-configurable surety-scheme labels).
- **Date:** 2026-07-23

## Question

`Confidence` (data-model §7) is a fixed, closed five-level enum (`VeryLow`/`Low`/`Normal`/`High`/
`VeryHigh`) baked into `EventContext.confidence` and `Citation.confidence`. The roadmap (Phase 10,
data-model §17) names "making the scheme configurable" as deferred work. This asks: what do prior
models mean by a *configurable* surety scheme, and how much of that generality does a minimal,
ADR-first slice actually need?

## GENTECH's `SuretyScheme` / `SuretySchemePart`

The GENTECH GDM 1.1 primer already surveyed in data-model §2.4 puts surety on the assertion, not the
source. The full spec (verified via the GDMUML class model, a UML restatement of the 2000 GENTECH
ER model) goes further than data-model.md's summary: surety is not a fixed scale at all.

> "Every PROJECT has associated with it a SURETY scheme used to express how certain the researcher is
> of the data gathered. Although many people use the same scheme, as implemented in certain software
> or elsewhere, the data model allows different schemes to be used... the scheme used with a
> particular project is attached to the data it describes; this means that an export specification can
> send the surety scheme along with the data." — GENTECH GDM 1.1 primer, §"Assertions and Surety"

`SuretyScheme` is a **project-owned collection of an arbitrary number of `SuretySchemePart`s** — the
scheme itself travels with the project's data (so an export must carry the scheme definition, not just
its part-indices, for the numbers to mean anything to a reader). GDMUML's own commentary flags a
further generalization GENTECH left out: "It would be more useful to have `SuretySchemeParts` tied to
each of the elements of the Assertion, e.g. the date, place, and subjects" — i.e. per-axis surety
within one assertion, not one scalar per assertion.

**GenTle** (an open-source desktop implementation of the GENTECH model) realizes this literally: "User
can define as much [schemes] as she wants. Within each [scheme], user can define an arbitrary number of
[parts]. This ensures fine granularity, scalability and modularity." Cardinality and even the *number
of schemes in use at once* are both open.

## What other surveyed products do instead

None of the desktop products already surveyed in data-model §3 implement GENTECH's full generality:

- **Gramps** ships a fixed five-level confidence (`-1`/very-low .. `4`/very-high, essentially what our
  `Confidence` already mirrors) with no per-project override.
- **RootsMagic 8's evidence-analysis tool** layers Evidence Explained's three *qualitative axes*
  (source Original/Derivative, information Primary/Secondary, evidence Direct/Indirect/Negative — our
  `EvidenceAnalysis`, already modelled) on top of a fixed confidence value; the axes are not
  user-renamable or user-extensible either.
- **Evidentia** (built explicitly around the Genealogical Proof Standard) produces a "Genealogical
  Proof Report" from a fixed claims/evidence/analysis pipeline; its confidence vocabulary is likewise
  fixed by the tool, not the project.
- **GEDCOM X's `ConfidenceLevel`** is a small fixed enumeration (`http://gedcomx.org/Low` /
  `.../Medium` / `.../High`) — three levels, not configurable at all — confirming that "configurable
  surety" is a GENTECH-specific idea, not something the interoperability standards themselves adopted.

So the field is split: one research-grade model (GENTECH) that allows arbitrary per-project schemes
with per-axis granularity, and every shipping product surveyed instead hardcodes a small fixed scale.
No shipping product was found that lets an end user rename or resize the scale short of GenTle, which
is source-available research software, not a mainstream tool.

## What "configurable" should mean for this workspace, in a first slice

Two axes of "configurable" are conflated in casual use, and they have very different costs here:

1. **Relabeling** — a workspace chooses its own *wording* for the five existing ordinal levels (e.g.
   an archive-specific vocabulary, or a translation of Gramps' own five labels). This changes nothing
   about `Confidence`'s shape: the value stored in every already-emitted event is still one of five
   ordinals, and every place that matches on the five variants (GEDCOM `QUAY` mapping, Gramps
   confidence mapping, every UI confidence chip) is unaffected — only the *string* shown to a human
   changes, resolved the same way `id_formats` already resolves a per-workspace override with a
   documented default (ADR 0005/0015 precedent).
2. **Re-scaling (cardinality)** — a workspace defines its own *number* of levels (GENTECH's real
   generality). This changes `Confidence`'s shape, which is referenced in the payload of every
   already-persisted event across all twelve aggregates; every consumer that exhaustively matches its
   five variants would need to become data-driven against a per-workspace definition, and a scheme
   changed *after* assertions exist would silently reinterpret old ordinals under new labels/counts —
   exactly the ambiguity GDMUML's own per-axis-surety aside hints GENTECH's designers were still
   wrestling with in 2000.

Axis 1 is a presentation change with no blast radius on the event log or the round-trip format
mapping. Axis 2 is a core-type change with a blast radius across all twelve aggregates and both format
plugins, for a generality (arbitrary schemes, arbitrary axes) that no surveyed shipping product
actually offers its users — i.e. it is speculative relative to demonstrated need (YAGNI). ADR 0027
adopts axis 1 only, and documents axis 2 as an explicit, deferred follow-up gated on a real workflow
that needs it.

## References

- GENTECH GDM 1.1 primer — <https://genealogy.sourceforge.net/GENTECH_Primer.html>.
- GDMUML specification (§4.3–4.4, `SuretyScheme`/`SuretySchemePart`) —
  <http://freepages.rootsweb.com/~mitchellsharp/history/gdmuml/index.htm>.
- GenTle feature list (surety schemes) — <https://gentle.sourceforge.net/features.html>.
- RootsMagic 8 evidence-analysis walkthrough —
  <https://www.familyhistoryfanatics.com/rootsmagic-8-evidence-analysis-tool>.
- Evidentia (Genealogical Proof Standard software) — <https://evidentiasoftware.com/>.
- GEDCOM X `ConfidenceLevel` — <http://gedcomx.org/v1/Conclusion.html>.
- `docs/data-model.md` §7 (`Confidence`), §8 (`EventContext.confidence`), §17 (deferred item).
- `docs/adr/0015-configuration-split-and-storage.md` — the workspace-functionality scope and
  `id_formats` precedent this ADR follows.
