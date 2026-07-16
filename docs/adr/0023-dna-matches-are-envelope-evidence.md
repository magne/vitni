# 23. DNA matches are envelope evidence

- **Status:** Accepted
- **Date:** 2026-07-16

## Context

Data-model §12 models a DNA relationship inference as *"a normal assertion event on a Person or
Family — a `FactAsserted` / `AssociationAsserted` that **cites the `DnaMatch`** via the
`EventContext.citations` link"*. The raw match is high-surety observed data; the relationship it
implies is a lower-confidence conclusion that lives in the evidence layer as an ordinary,
retractable assertion.

The vocabulary promised that citing shape, but no verb existed for it. `EventContext.citations`
(ADR 0004 §1) was `Vec<CitationRef>`, and a `CitationRef` targets a `Citation` aggregate only
(`{ citation_id: CitationId }`). A `DnaMatch` is a separate aggregate (§9), not a `Citation`, so
there was no way to record that a person's relationship fact rests on a DNA match rather than on a
documentary source.

ADR 0020 established that the event envelope is the *sole* evidence channel for a claim: a claim's
backing evidence lives in `EventContext.citations` and nowhere else. That decision fixed *where*
evidence lives; it left the evidence *target* mono-typed (a citation).

## Decision

1. **The envelope's evidence link is polymorphic.** `CitationRef` (the envelope's evidence-target
   value object) becomes an enum:

   ```rust
   pub enum EvidenceRef {
       Citation(CitationId),
       DnaMatch(DnaMatchId),
   }
   ```

   `EventContext.citations` becomes `Vec<EvidenceRef>` (the field name is unchanged — it is still
   "the evidence backing this claim"). A Person/Family `FactAsserted` / `AssociationAsserted` cites
   a DNA match by carrying an `EvidenceRef::DnaMatch(id)` in that same channel, exactly as it cites
   a source by carrying an `EvidenceRef::Citation(id)`.

2. **This narrows ADR 0020, it does not reverse it.** The envelope is still the *only* place a
   claim's evidence lives; there is still one channel and one denormalization pattern. What changes
   is that the channel's target is now a union of the two evidence-bearing aggregates rather than a
   single citation. `EventContext` gains `citation_ids()` / `dna_match_ids()` accessors so a read
   model that only wants one variant filters without re-implementing the match.

3. **The denormalized projection field follows the envelope.** `Asserted<T>.citations` (the
   fold-time copy of the envelope's evidence — ADR 0004 §1, ADR 0021 §3) widens from
   `Vec<CitationId>` to `Vec<EvidenceRef>`. Read models that counted or listed a row's citations
   filter the `Citation` variant through the accessor; the new DNA reverse index filters the
   `DnaMatch` variant.

4. **`MediaRef.citations` moves to the same vocabulary.** `MediaRef` (a media object's per-use
   context — ADR 0020 §3) carried `Vec<CitationRef>`; it becomes `Vec<EvidenceRef>` because it
   shares the one evidence-reference type. In practice a media use is only ever backed by citations;
   nothing in the app or UI writes an `EvidenceRef::DnaMatch` there.

5. **Version bumps are documentation labels.** Widening `CitationRef` to `EvidenceRef` is an
   incompatible payload change for every event variant that embeds the envelope's evidence or a
   `MediaRef`. Workspaces are disposable and every plugin is first-party (ADR 0018 §3), so the
   touched variants' `version()` strings advance per the per-variant rule with **no upcaster** —
   the bump records the change, it does not gate a compatibility layer.

6. **The WIT/plugin boundary is unaffected.** `EventContext.citations` does not cross the host/plugin
   boundary (the plugin evidence path attaches citations by verb, not by envelope), so there is no
   `host-api` change.

## Rationale

- **The model already prescribed this link.** Data-model §12 committed to a `DnaMatch` being cited
  through `EventContext.citations`; making the target polymorphic is the minimal change that
  delivers the promised shape rather than inventing a parallel reference field on the payload
  (which ADR 0020 exists to prevent).
- **One channel stays one channel.** A dedicated `dna_match_id` field on `Fact`/`Association` would
  reintroduce exactly the two-channels-no-precedence problem ADR 0020 closed. A union on the single
  channel keeps evidence in one place.
- **Symmetry with citations for free.** Because the DNA link travels the same channel as citations,
  corrections (`AssertionRetracted` / `AssertionSuperseded` by `AssertionId`), confidence
  denormalization, and the reverse index all work for DNA-backed inferences without new machinery —
  they reuse the citation substrate, filtered by variant.

## Consequences

- A Person/Family relationship inference can be recorded as DNA-backed: the assertion carries the
  match id in its envelope and its own (typically lower) confidence, and a revised inference is a
  superseding assertion. The DNA-match screen shows the inferences citing it (a reverse index over
  the evidence-bearing projections, filtered to the `DnaMatch` variant), each with a back-link to
  the citing Person/Family record.
- `Vec<CitationId>` read-model call-sites (citation reverse index, source-count columns) filter the
  `Citation` variant; their counts are unchanged because a DNA-backed inference contributes no
  citation.
- Events written before this change do not decode against the widened payload; accepted under the
  disposable-workspace stance (ADR 0018 §3) — no upcaster bridges the versions.

## References

- ADR 0004 — event-sourcing implementation contract; §1 (provenance/evidence in the payload), §2
  (corrections by `AssertionId`), §4 (self-contained, versioned events).
- ADR 0020 — evidence citations live in the envelope; the sole-channel decision this narrows (target
  now polymorphic), not reverses.
- ADR 0018 §3 — no backwards compatibility (disposable workspaces, first-party plugins): the stance
  that makes the version bumps labels rather than upcaster gates.
- ADR 0021 §3 — the uniform `Asserted<T>` projection shape whose `citations` field widens here.
- `docs/data-model.md` §12 (DNA as evidence) — the vocabulary that prescribed this citing shape.
