# 39. Identity decisions: persona clusters, merge and distinct

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

When the matching engine (ADR 0038) proposes that two records describe one entity, the user decides.
That decision needs a durable, audited, reversible representation, and today only part of one exists:

- **`PersonsMerged { surviving, merged }`** is non-destructive and survivor-side, and both streams are
  kept (data-model §9). But:
  - nothing downstream honours it: the merged person still appears in every list and picker, and a
    family that references it still shows it (`vitni-app/src/merge_usage.rs` only counts those
    references)
  - the surviving person's view shows none of the merged person's claims
  - `merge_persons` hardcodes `Confidence::Normal` and a default rationale, so the decision records
    neither how sure the user was nor what evidence they were shown
- **There is no "not the same" decision.** A pair the user rejected is proposed again on the next
  duplicate scan, forever.
- **No other kind can be merged.** Two Place, Source, Event or Family records describing one thing
  have no identity link at all.
- **Data-model §11.3 envisions stored suggestions.** It says a machine match "enters as a
  low-confidence assertion in the evidence layer attributed to a `Software` agent", and that the
  user's confirm or reject is itself audited. It does not say how a suggestion stays true while the
  data under it changes.

The evidence/conclusion model (data-model §4, §9) already names the shape the user asked for. A record
imported from one source is a **persona**. A researcher's conclusion that several personas are one
individual is a **link**, not a rewrite. What is missing is the read side that makes a linked cluster
behave as one person, plus the equivalent for the other kinds.

## Decision

1. **Identity decisions are assertions on the survivor, one pair of event variants per kind.**
   - **Person** keeps `PersonsMerged { surviving, merged }` and gains `PersonsDistinguished { person,
     other }`.
   - **Family, Event, Place, Source, Citation, Repository, Note and Media** gain the same pair:
     `<Kind>sMerged { surviving, merged }` and `<Kind>sDistinguished { <kind>, other }`, each emitted
     on the aggregate the user was reviewing.
   - **DnaTest, DnaMatch, ResearchNote and ImportRun get neither** (ADR 0038 §2).
   - **Tag gets neither either.**
     - Tag has no assertion chain (data-model §8), so a tag merge could not be undone by retraction.
     - A tag *is* its name, so tags resolve deterministically by case-folded name (ADR 0038 §6) and
       are never proposed as a pair.
     - Two differently named tags meaning one thing are consolidated by re-tagging, as today.

   Both variants are ordinary assertions. They carry the `EventContext` operator, `confidence` and
   `rationale` the user supplied, and are undone by retracting their `AssertionId` (data-model §10).
   `merge_persons` stops hardcoding confidence and rationale.

2. **Each decision carries the evidence it was made on.**
   - Merge and distinguish variants gain an additive `assessment: Option<MatchEvidence>`: a snapshot
     of the ADR 0038 `MatchAssessment` the user was shown. It holds the score, band, engine version,
     cultures and the feature list with weights.
   - `MatchEvidence` is defined in `vitni_core::matching` next to `MatchAssessment`, and
     `MatchAssessment::evidence()` produces it. It carries the same fields, with the score and weights
     in fixed point (basis points).
   - It is a separate type because event payloads derive `Eq` and must encode stably (ADR 0004 §4),
     which an `f64` score cannot.
   - The history then reads "matched at 87% by vitni-matching 1 (names agree after normalization,
     birth within census tolerance); confirmed by *operator*, confidence High". A later engine
     version cannot rewrite why the decision was taken.
   - It is `None` for a merge made without the engine, such as a direct CLI merge.

3. **Suggestions are computed, not stored.**
   - A proposed match is a function of the current data. It changes when either record gains a claim,
     when a pack or weight changes, or when the engine improves. Storing it as an assertion would
     freeze a stale judgement into the log and then need its own retraction machinery.
   - So the **review queue** is computed on demand, like the `checks.rs` findings: `find_similar`
     over the workspace (or over one import run's records), minus every pair already decided either
     way.
   - Only the user's *decision* is an assertion.
   - This amends data-model §11.3. A machine match is not an assertion. The confirm or reject is, and
     it records the machine's assessment as its evidence (§2).

4. **Linked records form clusters, resolved by one index.**
   - A projection, `identity_links(kind, member, root)`, holds the transitive closure of live merge
     decisions per kind:
     - every merged member points at its cluster's root, the survivor that is not itself merged
     - retracting a merge splits the cluster again
   - Resolution rules:
     - **Cycles and double membership are refused.** Merging a record already merged elsewhere
       targets that cluster's root instead, and the app layer checks this against the projection
       (ADR 0002 lagging-projection rule).
     - **A live distinct decision blocks a merge** between the two clusters. The compare view shows
       the earlier decision and offers *Undo "not the same" and merge*, a sequenced retract-then-merge.
     - Distinctness is judged **between clusters**: if any member of cluster X was distinguished from
       any member of cluster Y, X and Y are not proposed to each other.

5. **A cluster reads as one record; its members stay separate streams.** The composition happens in
   `vitni-app` at read time, over `identity_links`, not in a cross-aggregate fold.
   - **Detail:** a root's detail DTO is the union of its own claims and every member's.
     - Each row stays attributed to the aggregate and assertion it came from, so provenance,
       *Why we believe*, and retraction all keep working per row.
     - An edit to a member-owned row is routed to that member's aggregate.
     - A new claim goes on the root.
   - **Lists and pickers** hide members. **References** to a member (a family partner, an event
     participant, an event's place) resolve to the root. This is the same redirect for every kind.
   - **Person clusters:**
     - An import creates a `Persona` for every record's person. Confirming a match links the persona
       to the existing person (ADR 0040), which is the persona→conclusion join of data-model §9.
     - The conclusion person's screen gains a *Linked records* view: each persona with its origin
       (ADR 0037), source and an *Unlink* action.
     - A root may itself be a persona, when two imported personas are linked before any conclusion
       exists. Its evidence level is shown honestly.
   - **Exports fold clusters.** The WIT read path hands exporters the composed record under the
     root's id, and references are remapped to roots. A GEDCOM or Gramps export therefore contains one
     person per cluster, which is what a user sharing a tree means.

6. **Identity is distinct from succession.** ADR 0026's place succession (a parish merged into
   another in 1964) is a *historical* claim about the world, and both places stay real. A
   `PlacesMerged` identity decision says two records describe *one* place. They are different
   events, and the UI names them differently.

## Rationale

- **Link, don't rewrite, is the model's own rule.** The persona/conclusion split exists so an evidence
  record is never destroyed by a conclusion. The cluster read model is the missing half that makes
  that rule usable: without it, a non-destructive merge leaves two visible people.
- **Survivor-side for every kind** follows the shape `PersonsMerged` already has (ADR 0019 keeps
  person history self-contained). One shape for every matchable kind keeps the composition code and the UI
  generic.
- **Distinct decisions are what make a review queue finite.** Without them, every rejected pair
  returns on every scan. That is the difference between a queue the user can empty and one they learn
  to ignore.
- **Computed suggestions stay correct by construction.** Nothing has to invalidate a stale suggestion,
  because none is stored. The cost is recomputation, which the blocking index (ADR 0038 §7) keeps
  cheap.
- **The assessment snapshot makes decisions auditable over time.** It records what the user saw when
  they decided, independent of how the engine scores the pair today.

## Consequences

### Positive

- Confirming a match makes two records behave as one everywhere: lists, pickers, references,
  exports. Every original record is kept, and the link undoes with one retraction.
- Rejected pairs never come back. Decided pairs of either kind are excluded from every consumer.
- Every identity decision records who made it, how sure they were, and the evidence behind it.
- Places, sources, events and families can finally be deduplicated, not only people.

### Negative / costs

- **Sixteen new event variants** (eight kinds × two), plus `PersonsDistinguished`, each with decide
  rules, i18n and history rendering.
- **Read-time composition** costs extra reads per detail view, one per cluster member. Lists pay a
  join against `identity_links`.
- **Every reference-rendering path must resolve through the index.** A path that forgets shows a
  hidden member. A test sweep over the reference-bearing DTOs guards this.
- **Edit routing** on a composed record is new UI logic: a row edit goes to the row's owner.
- **Exporters must fold clusters,** which is new WIT read-path behaviour.
- Data-model §9 and §11.3 are updated in the implementing change to describe the computed queue and
  the cluster read model.

## Out of scope

- **Field-level reconciliation on merge** (choosing which name is primary). The union view shows
  every claim with its evidence, and the user retracts what they reject.
- **Automatic merging on a score**, which ADR 0038 §6 forbids.
- **Splitting a single record into two** (the inverse of a merge on one stream). That is a correction
  of claims, done with retractions.

## References

- ADR 0002 — cross-aggregate checks against lagging projections.
- ADR 0019 — person-owned participation; the survivor-side `PersonsMerged` shape.
- ADR 0021 — uniform attributed claims, the unit the union view keeps attributed.
- ADR 0026 — place succession, which is not identity.
- ADR 0028 — a research note can hold the argument behind a difficult identity decision.
- ADR 0037 — the record origin shown in *Linked records*.
- ADR 0038 — the assessment snapshot and the deterministic band.
- ADR 0040 — the import flow that creates personas and links them.
- `docs/data-model.md` §4, §9, §10, §11.3.
