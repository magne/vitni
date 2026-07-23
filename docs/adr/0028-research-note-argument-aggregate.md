# 28. `ResearchNote`/`Argument` aggregate

- **Status:** Proposed
- **Date:** 2026-07-23

## Context

data-model §17 names GEDCOM X's `Document(Analysis)` — a first-class construct for written proof
arguments — and "the GENTECH/GPS process wants research questions and tasks" as the prior art behind a
future `ResearchNote`/`Argument` aggregate; the roadmap (Phase 10) carries the same bullet forward.

Research (`docs/research/proof-argument-modelling.md`) found these are two different concepts, split
consistently across the tools that implement each:

- **GEDCOM X's `Document(Analysis)`** — every `Conclusion` (a `Fact`/`Name`/`Gender`, in our vocabulary
  every fact-shaped claim) carries an optional `analysis` reference resolving to a `Document` of type
  `http://gedcomx.org/Analysis`: "notes or narrative text about the result of... recognizing the
  Information items a Source contains... [and] considering [their]... likely accuracy"; "a genealogical
  proof statement is an example of one kind of analysis document." This is a long-form written
  artifact, argued once, that a conclusion cites as its justification — evidence-layer content with its
  own provenance, not a workflow object. One document commonly resolves several conclusions at once
  (e.g. "these two records are the same person").
- **The research-task/log thread** — RootsMagic's Tasks (`{ goal, result, type, priority, status }`,
  organized in folders with a "Research Log" view) and Evidentia's "Research Summary report... gap
  analysis" both model *unfinished work*, structurally separate in both tools from their
  evidence-analysis/proof-report features. Neither GEDCOM X nor our own evidence/conclusion
  architecture (data-model §4) has anywhere to hang a "status" or "goal" that is not itself a claim
  about the past.

This ADR scopes the new aggregate to the first thread only (a written proof-argument note); the
research-task/log thread is a materially different, orthogonal feature (a to-do, not an assertion) and
is deferred (§Out of scope).

Every one of the twelve existing aggregates follows the same template (command/event/state/view/decide/
error) and is wired through three x-macro registries — `genealogy-app::aggregates::for_each_aggregate!`
/ `for_each_human_id_aggregate!`, the matching `genealogy-db` registry rows, and per-aggregate CLI
`.ftl` fragments (issue #38) — so adding a thirteenth aggregate is a template exercise, not new
architecture.

## Decision

1. **A new aggregate, `ResearchNote`, models GEDCOM X's `Document(Analysis)`.** ("Argument" describes
   its content — a written case for a conclusion — not a second aggregate; the roadmap's
   "`ResearchNote`/`Argument`" naming is one aggregate.) `ResearchNoteId` is UUID v7 (`ids.rs`); its
   `HumanId` default format is `A%04d` — the first Gramps-style single letter not already taken
   (`I`/`F`/`P`/`S`/`C`/`E`/`D`/`X`/`R`/`N`/`O`; `Tag` has no `HumanId`).

2. **State:** `{ id, human_id, subject: SubjectRef, title: Option<String>, body: Attributed<RichText>,
   restrictions }`. `restrictions` gets the universal `SetRestrictions`/`RestrictionsChanged` pair
   (data-model §7/§16) like every other aggregate. `SubjectRef` is a new closed value object,
   `Person(PersonId) | Family(FamilyId) | Event(EventId) | Place(PlaceId)` — the four conclusion-
   bearing entity kinds a proof argument is written about. This is deliberately narrower than the
   general `EvidenceRef` union (ADR 0023: `Citation | DnaMatch`, the things an assertion *cites*):
   `SubjectRef` names what an argument is *about*, and Source/Citation/Repository/Media/Note/Tag/
   `DnaTest`/`DnaMatch` are not conclusions a proof argument concludes about. One `ResearchNote` names
   exactly one subject at creation; a multi-subject argument (e.g. one analysis resolving a
   shared-ancestor question across two Persons — GEDCOM X's own common case) is additive future work
   (§Out of scope) — in v1 the researcher writes one `ResearchNote` per subject and cross-references
   the others by naming them in the body text.

3. **Commands/events mirror the existing template one-to-one:**
   `CreateResearchNote { human_id, subject, title }` → `ResearchNoteCreated`;
   `SetBody { body: RichText }` → `RichTextSet` (mirrors `Note`'s own `RichTextSet`);
   `Tag`/`Untag` → `Tagged`/`Untagged`; `SetRestrictions` → `RestrictionsChanged`;
   `RetractAssertion`/`SupersedeAssertion` → `AssertionRetracted`/`AssertionSuperseded` (the universal
   pair, data-model §10). The note's own creating/body-setting assertions carry
   `EventContext.citations` exactly like every other claim — no new evidence channel (ADR 0020): a
   proof argument's citations are the sources it reasons over, recorded the same way every claim
   records its evidence.

4. **Cross-aggregate check.** `subject` must resolve against the target aggregate's projection at
   creation — the aggregate-tax pattern (`UnknownPlace`/`UnknownSource` precedent, data-model §10.1) —
   a new `ResearchNoteError::UnknownSubject`. The rest of the error taxonomy mirrors `Note`'s
   (`NotFound`, `RetractsMissingAssertion`, `EmptyRequiredField` for an empty body).

5. **Reverse lookup, not a two-sided attach.** A subject aggregate does **not** gain a
   `ResearchNoteAttached` event or an id-list field — unlike `MediaAttached`/`NoteAttached`, which are
   two-sided because one `Media`/`Note` is reusable across many owners. A `ResearchNote` points *at*
   its one subject; "which arguments exist about this Person" is answered by a reverse query
   (`list_research_notes_for_subject`) over the `ResearchNote` projection — the same reverse-index
   shape ADR 0020's consequences already establish ("the reverse citation index... covers facts").
   This keeps the change local to the new aggregate: **zero** event/state changes to Person, Family,
   Event, or Place.

6. **Wiring is mechanical, through the existing registries.** A new row in `for_each_aggregate!` /
   `for_each_human_id_aggregate!` (`genealogy-app/src/aggregates.rs`), the matching `genealogy-db`
   registry rows, `genealogy-app/src/lib.rs` re-exports (the DTO/use-case surface every frontend
   consumes), and a new CLI `.ftl` fragment (`research_note.ftl`) plus `research-note` subcommands in
   `for_each_cli_command!`. No change to the registry macros themselves.

7. **No GEDCOM/Gramps round-trip in this slice.** Neither format has a stable equivalent — GEDCOM has
   no analysis-document construct at all, and GEDCOM X's own `Document(Analysis)` has no GEDCOM
   5.5.1/7 mapping — mirroring the DNA precedent (data-model §12: "DNA round-trip... no stable
   standard... deferred"). A `ResearchNote` is created/edited only via the CLI/GUI in v1.

## Rationale

- **One aggregate, not two.** Bundling the research-task/log thread into this aggregate (a `status` or
  `priority` field bolted onto `ResearchNote`) would mix an evidence-layer assertion with a mutable
  workflow field that is not itself a claim about the past — the one thing every other aggregate in
  this model avoids (data-model §4: projections are derived from immutable assertions). Keeping the
  task/log thread out entirely, rather than half-modelling it, keeps the new aggregate's shape as clean
  as the twelve it joins.
- **Pointing at the subject, not attaching from it, halves the blast radius.** Every other cross-
  aggregate reference pattern in this model already has a precedent for "the referencing side owns the
  link, the referenced side is read via reverse query" (the `EvidenceRef`/citation reverse index); reusing
  it here means zero changes to four already-shipped aggregates for a Phase 10 addition.
- **No new evidence channel, no new citation shape.** `EventContext.citations` already generalizes to
  cite a `DnaMatch` as well as a `Citation` (ADR 0023); a `ResearchNote`'s own assertions use exactly
  that mechanism, so the sources an argument reasons over are recorded identically to every other
  claim's sources.

## Consequences

### Positive

- Closes the data-model §17 item with a template-following aggregate; adding it costs one macro row
  per registry plus the new module, matching every prior aggregate addition (issue #38).
- No change to Person/Family/Event/Place's events, state, or projections.
- The GENTECH "assertions can be built on other assertions, forming an auditable chain of deductions"
  idea (data-model §2.4) is realized directly: a `ResearchNote`'s citations can themselves include
  other evidence, and the note is itself retractable/supersedable like any claim.

### Negative / costs

- No GEDCOM/Gramps export for `ResearchNote` content (matching the DNA precedent) — a proof argument
  written in this workspace is not portable to another program via round-trip.
- A UI screen (the ResearchNote/Argument mockup, listed in the Gate 2 delivery plan) is new surface
  area, not a reuse of an existing screen shape.
- Single-subject-per-note in v1 means a multi-conclusion analysis is recorded as several notes with
  duplicated narrative text until the multi-subject follow-up lands.

## Out of scope

- **The research-task/log workflow** (RootsMagic Tasks-style goal/status/priority/folders) — a
  distinct, future aggregate if built; not part of this ADR.
- **Multi-subject arguments** — additive follow-up; v1 is one `SubjectRef` per `ResearchNote`.
- **GEDCOM/Gramps round-trip** — no stable standard exists to map to (mirrors the DNA precedent).
- **A configurable/extensible `SubjectRef`** covering Source/Citation/Repository/Media/Note/Tag/DNA —
  not conclusions, so not subjects a proof argument concludes about in this model.

## References

- `docs/data-model.md` §4 (evidence/conclusion architecture), §7 (value objects — `RichText`,
  `EvidenceRef`), §9 (aggregate boundaries, the reverse-lookup precedent), §10.1 (the aggregate-tax
  error pattern), §17 (the deferred item this ADR closes).
- ADR 0004 §2 — `AssertionId`-addressed corrections, applied to `ResearchNote`'s own retract/supersede.
- ADR 0020 — evidence citations live in the envelope; the channel a `ResearchNote`'s sources use.
- ADR 0021 — uniform `Attributed<Asserted<T>>` projection shape, applied to `body`.
- ADR 0023 — `EvidenceRef` (citation/`DnaMatch` union) — the citation generalization a `ResearchNote`'s
  envelope reuses unchanged.
- `docs/research/proof-argument-modelling.md` — the GEDCOM X / GPS / RootsMagic / Evidentia findings
  this scoping decision rests on.
- Issue #38 / the x-macro registry pattern (`genealogy-app/src/aggregates.rs`,
  `genealogy-db/src/registry.rs`) — the mechanical wiring path a 13th aggregate follows.
