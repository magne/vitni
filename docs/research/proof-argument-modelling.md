# Research — modelling the proof argument (Phase 10, gates ADR 0028)

- **Status:** Findings informing ADR 0028 (`ResearchNote`/`Argument` aggregate).
- **Date:** 2026-07-23

## Question

data-model §17 names GEDCOM X's `Document(Analysis)` and "the GENTECH/GPS process wants research
questions and tasks" as the two prior-art threads behind a future `ResearchNote`/`Argument` aggregate.
Before adding a 13th aggregate, this asks: what, precisely, does each thread model, and which one (or
both) does a minimal slice need?

## The Genealogical Proof Standard (GPS) chain

FamilySearch's own explainer of how GEDCOM X maps to the GPS process (a primary source for the
conceptual model, not a third party) names five concepts in a strict pipeline:

> - **Question** — the well-defined research question.
> - **Evidence** — "a tentative Answer to a research Question that is the product of using
>   Information to answer" it.
> - **Analysis** — "notes or narrative text about the result of two processes: (a) recognizing the
>   Information items a Source contains that are likely to answer a research question; (b)
>   considering the characteristics, purpose, and history of a Source... to determine their likely
>   accuracy."
> - **Conclusion** — "an accepted Answer to a research Question; a Hypothesis that has passed
>   testing and for which conflicts can be resolved."
> - **Proof** — "a Conclusion explained in writing; an explanation that demonstrates the five GPS
>   elements" (reasonably exhaustive search, complete and accurate source citation, analysis and
>   correlation of the collected information, resolution of conflicting evidence, a soundly reasoned
>   written conclusion).

GEDCOM X's conceptual model (`conceptual-model-specification.md`) implements *Analysis* and *Proof*
with **one shared construct**: a `Document` whose `type` is `http://gedcomx.org/Analysis` — "the
document is an analysis done by a researcher; a genealogical proof statement is an example of one
kind of analysis document." Every `Conclusion` (so every `Fact`, `Name`, `Gender`, and by extension
every fact-shaped claim our own model has) carries an *optional* `analysis` property: "Reference to a
document containing analysis supporting this conclusion... MUST resolve to an instance of
`Document` of type `Analysis`." So in GEDCOM X the link is asserted **from the conclusion side** — a
conclusion points at the document that argues for it — and one analysis document can be the target of
several conclusions' `analysis` references (a single proof argument commonly resolves more than one
fact at once, e.g. "same person across these two records").

This is the narrower of the two threads: a **long-form written artifact**, argued once, that a
conclusion cites as its justification. It is evidence-layer content — narrative text with its own
provenance — not a workflow object.

## The research-task / log thread

The second thread — "research questions and tasks" — is a different, project-management concept,
confirmed by how the two shipping tools built around GPS actually split it:

- **RootsMagic's Tasks page** models a task as `{ name, goal, result, type: research | to-do |
  correspondence, priority, status }`, linkable to a person/family/fact/place/source/media, and
  organizable into **folders** with a dedicated "Research Log" view per folder. A task tracks *work to
  be done or done*, not a conclusion's justification — RootsMagic's own evidence-analysis tool
  (source/information/evidence quality flags, matching our `EvidenceAnalysis`) is a *separate* feature
  from Tasks.
- **Evidentia**, built explicitly to "guide you through the Genealogical Proof Standard," ships a
  distinct "Research Summary report — the status of my current research, including identification of
  gaps," alongside its "Genealogical Proof Report — a detailed analysis and justifiable conclusion,"
  again as two different outputs from two different internal concepts (a claims/evidence catalogue
  feeding the proof report; a gap analysis feeding the research summary).

Both tools therefore keep "what am I still trying to find out" (a task/log, oriented toward future
work) structurally separate from "here is my written case for this conclusion" (an analysis/proof,
oriented toward evidence already collected). Neither tool's task/log model resembles GEDCOM X's
`Document(Analysis)` — there is no first-class "research question" object in GEDCOM X at all; the GPS
*question* stays implicit in the analysis text.

## Implication for the aggregate

The roadmap's single bullet ("`ResearchNote`/`Argument` aggregate for proof arguments... recording the
reasoning that ties evidence to a conclusion") describes the **first** thread only — GEDCOM X's
`Document(Analysis)`, a written case for a conclusion. The second thread (tasks/goals/status/folders) is
a materially different, orthogonal feature — it tracks unfinished work, not evidence — and neither
GEDCOM X nor our own evidence/conclusion architecture (data-model §4) has anywhere to hang a "status"
or "goal" that isn't itself a claim about the past. ADR 0028 therefore scopes the new aggregate to the
analysis/proof-argument thread and defers the research-task/log thread entirely (it would more
naturally sit beside `Tag`/`Note` as its own future aggregate with its own lifecycle — a to-do, not an
assertion).

## References

- GEDCOM X conceptual model, §2.6 "The `Document` Data Type" and the `analysis` property of
  `Conclusion` — <https://github.com/FamilySearch/gedcomx/blob/master/specifications/conceptual-model-specification.md>.
- "GEDCOM X and the Genealogical Research Process" (the Question/Evidence/Analysis/Conclusion/Proof
  chain, in FamilySearch's own words) —
  <http://gedcomx.org/GEDCOM-X-and-the-Genealogical-Research-Process.html>.
- RootsMagic 8 Tasks page — <http://wiki.rootsmagic.com/wiki/RootsMagic_8:Tasks_Page> and "Adding a
  task" — <https://help.rootsmagic.com/RM9/adding-a-task.html>.
- Evidentia's own account of its report set —
  <https://evidentiasoftware.com/what-do-you-expect-your-genealogy-software-to-produce/>.
- `docs/data-model.md` §2.3 (GEDCOM X survey), §4 (evidence/conclusion architecture), §17 (deferred
  item).
- `docs/adr/0020-evidence-citations-live-in-the-envelope.md`, `0021` — the envelope-as-evidence-channel
  and uniform-projection conventions the new aggregate must respect.
