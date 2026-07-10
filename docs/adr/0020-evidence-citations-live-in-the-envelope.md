# 20. Evidence citations live in the envelope

- **Status:** Accepted
- **Date:** 2026-07-10

## Context

A claim's backing citations could be recorded in two places with no precedence rule between them:

1. **The event envelope.** `EventContext.citations` (ADR 0004 §1) travels on every assertion event
   as part of the provenance the operator stamped on the claim.
2. **Payload value objects.** `Fact.citations` and `Attribute.citations` (`Vec<CitationRef>`) carried
   their own citation lists inside the asserted value.

The data-model review (`docs/data-model-review.md`, finding 2) flagged this: two channels, no rule
for which wins, and projections that denormalize inconsistently. `AssertedName` and
`AssertedAssociation` copy the envelope's citation ids at fold time (`person/decide.rs`), while
`AssertedFact` copied only the confidence and dropped the citations — so a fact's sources were
unreachable from the read model even when the operator supplied them. In practice the payload lists
were written empty (`citations: Vec::new()`) everywhere they were constructed; the envelope was the
channel actually carrying evidence.

`MediaRef.citations` is a different concern: it records why *this particular use* of a media file at
one attachment point is warranted, not the evidence for a claim. It is per-use context, not claim
evidence, and is out of scope here.

## Decision

1. **The event envelope is the sole evidence channel for a claim.** `EventContext.citations` is where
   the citations backing an assertion live. Payload value objects carry no citation lists.

2. **Drop `Fact.citations` and `Attribute.citations`.** `AssertedFact` denormalizes
   `citations: Vec<CitationId>` from the envelope at fold time, exactly as `AssertedName` and
   `AssertedAssociation` already do, so a read model surfaces a fact's source count per row without
   re-reading the log.

3. **`MediaRef.citations` is retained.** It is per-use context for a media attachment, not evidence
   for a claim, so it is unaffected by this decision.

4. **This narrows ADR 0004 §1.** ADR 0004 §1 placed the `EventContext` (including its `citations`) in
   the payload as the provenance envelope; it did not forbid additional citation lists on payload
   value objects. This ADR supersedes that latitude: the envelope's `citations` is now the *only*
   place a claim's evidence lives. ADRs are immutable, so this is recorded as a narrowing here rather
   than an edit to ADR 0004.

5. **`FactAsserted` becomes version `"2.0"`, with no upcaster.** Removing `Fact.citations` is an
   incompatible payload change. Workspaces are disposable and every plugin is first-party (ADR 0018
   §3), so the version string is a documentation label — no upcasting tooling is written, and the
   bump is per-variant (`FactAsserted => "2.0"`, all other Person events stay `"1.0"`).

## Rationale

- **One channel, one denormalization pattern.** With the envelope as the only source, every asserted
  value object folds citations the same way; the `AssertedFact` inconsistency the review found
  disappears because there is nothing else to reconcile against.
- **No information lost.** The payload lists were written empty in every constructor, so dropping
  them removes dead structure, not data. Evidence that operators actually supplied travelled on the
  envelope and continues to.
- **Denormalize, don't reference-hunt.** Copying the envelope's citation ids onto `AssertedFact` at
  fold time matches ADR 0004 §1's confidence denormalization and ADR 0009's projection shape, so a
  read model answers "what backs this fact?" from projected state.

## Consequences

- `AssertedFact` gains `citations: Vec<CitationId>`; a fact's sources are now reachable from the
  Person projection, and the reverse citation index (source → citing records) covers facts.
- The Source-attribute read model loses its always-empty source-count column (it was fed by the
  removed `Attribute.citations`), aligning the Source attributes table with Media and Citation
  attribute tables.
- `FactAsserted` events written before this change do not decode against the new payload; this is
  accepted under the disposable-workspace stance (ADR 0018 §3) — no upcaster bridges the versions.
- The WIT/plugin boundary already omitted fact citations, so no plugin or host contract changes.

## References

- ADR 0004 — event-sourcing implementation contract; §1 (provenance in the payload) is the decision
  this narrows, not supersedes wholesale.
- ADR 0009 — read-model/projection schema; the denormalization shape this fold follows.
- ADR 0018 §3 — no backwards compatibility (disposable workspaces, first-party plugins): the stance
  that makes the `2.0` bump a label rather than an upcaster gate.
- `docs/data-model.md` §7 (Fact/Attribute), §8 (`EventContext`) — the vocabulary updated alongside.
- `docs/data-model-review.md` — finding 2, the review item this closes.
