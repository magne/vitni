# 18. Round-trip owner-links and host-api 0.8.0

- **Status:** Accepted
- **Date:** 2026-06-23

## Context

Phase 4 group G (`docs/archive/phase-4-followups.md`) adds Gramps XML import/export and, with it, the goal
that the records GEDCOM/Gramps carry under a person, family, or event — citations, media, notes —
**round-trip out**, alongside repositories, tags, place hierarchy, citation confidence, and source
author/publication. Two gaps blocked this:

1. **No owner-link projection.** The attachment events (`MediaAttached`, `NoteAttached`, `Tagged`,
   and the new `CitationAdded`) existed on the aggregates, but their values were tracked only in
   `live_assertions`, not projected — a deliberate YAGNI deferral noted in `genealogy-core`'s state
   docs as *"ADR 0009 §4: not projected until a query needs one."* Export is that query: an exporter
   cannot re-attach a citation/media/note it cannot read back from the owner.
2. **The host contract (`host-api@0.7.0`) exposed only a subset.** No verbs created repositories or
   tags, set citation confidence, source author/pub-info, or place hierarchy; no query returned
   citations/media/notes/repositories/tags/places or the attachment id-lists on person/family/event.

ADR 0011/0013 make the host WIT contract an ADR-governed surface, so growing it is recorded here.

## Decision

1. **Project owner-link attachments (applies ADR 0009 §4's escape hatch).** `PersonView`,
   `FamilyView`, and `EventView` now fold their attached `citations`, `media`, `notes`, and `tags`
   into projected state, following the `Attributed<T>` + `live_assertions` correction substrate
   (a retraction removes the matching attachment). Person and Family gain an `AddCitation`
   command/event (mirroring `Place::AddCitation`); Family gains `AttachMedia`/`AttachNote`
   (Person and Event already had them). This is the "when a real query needs it" case ADR 0009 §4
   reserved — a local, additive, rebuildable change — **not** a reversal of ADR 0009.

2. **`host-api@0.8.0`.** The WIT contract grows (a breaking change to `commands`, `query`, `types`):
   - `commands`: `attach-{person,family,event}-{citation,media,note}`, `apply-{person,family,event}-tag`,
     `create-repository`, `create-tag`, `set-source-author`/`-pub-info`, `link-source-repository`,
     `set-citation-confidence`, `set-place-type`, `set-place-enclosed-by`.
   - `query`: `list-citations`/`-media`/`-notes`/`-repositories`/`-tags`/`-places`; `person-dto`/
     `family-dto`/`event-dto` gain attached `citations`/`media`/`notes`/`tags` id-lists; `source-dto`
     gains `author`/`pub-info`/`repositories`. New `citation-dto`/`media-dto`/`note-dto`/
     `repository-dto`/`tag-dto`/`place-dto` records and the `confidence`/`place-type` enums.
   - New verbs reuse the existing `Commands`/`Query` capabilities (no new capability).

3. **No backwards compatibility.** Workspaces are disposable and every plugin is first-party
   (project constraint), so: **no event-version upcasters** for the new/changed events, and the
   `0.8.0` bump is a documentation label edited in lockstep across `host.wit` and the first-party
   plugins' `with` keys — not a dual-version compatibility gate.

## Rationale

- **Projection over a side index.** Folding attachments into the same `evolve` the aggregate already
  runs keeps corrections correct for free and matches the Person/Family/Event view shape (ADR 0009).
- **One contract version, edited in lockstep.** wit-bindgen validates the `with`-key version against
  the package version at compile time, so a missed string fails the build — enough to keep the
  first-party plugins honest without a compatibility layer we do not need.
- **Owner-gated creation keeps re-import idempotent.** An importer creates a person/family's owned
  records only when that owner is newly created (resolved by external id otherwise), so re-importing
  an unchanged document emits no new events — verified for both GEDCOM and Gramps.

## Consequences

- Citations, media, notes, repositories, place hierarchy, and citation confidence round-trip out for
  both GEDCOM and Gramps XML; the Gramps round-trip is proven by
  `crates/genealogy-plugin-host/tests/gramps_round_trip.rs`.
- The host `commands`/`query` surface roughly doubles; each verb is a thin delegation to a
  `genealogy-app` use-case, so the growth is mechanical.
- Tags are not yet carried on the Gramps person/family record (no `<tagref>` mapping yet); place
  coordinates and `MAP` are deferred. These are catalogued in `docs/data-model.md` §17.

## References

- ADR 0009 — read-model/projection schema; §4 (no projected field until a query needs one) is the
  decision this applies, not supersedes.
- ADR 0011, ADR 0013 — the host WIT world/capabilities and the import/export contract this versions.
- `docs/data-model.md` §17 — the GEDCOM/Gramps round-trip strategy and remaining gaps.
- `docs/archive/phase-4-followups.md` — Phase 4 group G.
