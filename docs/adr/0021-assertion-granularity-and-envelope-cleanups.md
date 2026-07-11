# 21. Assertion granularity and envelope cleanups

- **Status:** Accepted
- **Date:** 2026-07-11

## Context

The data-model review (`docs/data-model-review.md`) grouped five smaller findings (4–8) under one
"assertion-granularity + envelope cleanups" heading. They share a theme — the *shape* of an
assertion or its projection is finer-grained or more uniform than the code currently makes it — and
none is large enough to warrant its own ADR. This ADR records the decision for all five so the
vocabulary changes only once; each is implemented in its own PR (below).

The finding this ADR's first PR implements is **finding 4 — child–parent relationship rows cannot be
corrected independently**. `ChildEntry.relationships: Vec<(PersonId, ChildParentRelationship)>`
(`family/state.rs`) packed every per-parent link into the single `ChildAdded` assertion. One
`AssertionId` covered "child of this family" *and* "birth child of P1" *and* "step child of P2", so
an adoption link could not be retracted or re-cited without retracting the child's membership.

Prior art is unanimous that a per-link claim is its own subject: Gramps `ChildRef` (current master)
is a `CitationBase + NoteBase` object carrying `frel`/`mrel` with direct per-link citations; GEDCOM X
models child-and-parents as its own relationship subject; and Polygenea's *principle of sensible
disbelief* states the rule exactly — it should be sensible to disbelieve a node without disbelieving
the nodes it references, so disbelieving "adopted by P2" must not force disbelieving "child of the
family".

The evidence model is already settled by ADR 0020 (the envelope's `EventContext.citations` is the
sole channel for a claim's backing citations) and by ADR 0004 (provenance travels in the payload,
corrections target an `AssertionId`, events are self-contained and per-variant versioned). This ADR
respects both.

## Decision

1. **Child membership and each child–parent relationship are separate assertions (finding 4 —
   implemented by PR-C1).** `ChildAdded { family_id, child_id }` asserts membership only. A new
   `ChildRelationshipAsserted { family_id, child_id, parent_id, relationship }` asserts one
   child-to-partner link (GEDCOM `_FREL`/`_MREL`), each with its own envelope and `AssertionId`. The
   per-partner tuple list a read model exposes (`FamilyView::children()`) is reconstructed by folding
   the relationship assertions against the membership rows, rather than stored on the membership
   assertion.

   The decision carries these consequences, all in PR-C1:
   - **Guards.** `AssertChildRelationship` requires the child to be a current member
     (`ChildNotPresent`), the parent to be a current partner (new `FamilyError::ParentNotPartner`),
     and the live `(child, parent)` pair to be unique (new
     `FamilyError::ChildRelationshipAlreadyPresent`); editing a link goes through supersede.
   - **Cascade.** Removing a child (`ChildRemoved`) and retracting a child's membership assertion both
     drop that child's relationship rows; retracting one relationship never touches membership or the
     other links. `RemovePartner` does not cascade relationship rows (parity with today's behaviour —
     a removed partner's links remain until explicitly retracted).
   - **Naming.** The review sketched the event as `ChildParentRelationshipAsserted`; the shipped name
     is the shorter `ChildRelationshipAsserted`. Same subject.
   - **Version label.** `ChildAdded` becomes `"2.0"` (its payload lost `relationships`);
     `ChildRelationshipAsserted` starts at `"1.0"`. No upcaster (ADR 0018 §3): the labels are
     documentation, and `ChildAdded` events written in the old fat shape do not decode.

2. **Fact vs Event promotion rule (finding 5 — PR-C4, data-model §7 text only).** Vital,
   shared-capable types (Birth, Death, Baptism, Burial) are asserted as **Events with a Primary
   participant**; `Fact` is reserved for attribute-shaped claims (occupation, residence, religion,
   description, …). The four overlapping variants are trimmed from `FactType`; `Residence` stays a
   Fact. This removes the "same birth as a Fact or an Event, no rule" ambiguity GEDCOM X leaves open.

3. **Uniform `Asserted<T>` projections (finding 6 — PR-C3).** The bespoke `AssertedName` /
   `AssertedFact` / `AssertedAssociation` / `AssertedPartner` / `AssertedChild` / `AssertedFamilyEvent`
   structs are replaced by the generic `Asserted<T>` (the `assertions.rs` doc comment already promises
   this), `Person.sex` and person participations become `Attributed<Asserted<…>>`, and
   `restrictions_assertion` is added to the aggregates that lack it so `SetRestrictions` is retractable
   everywhere. Fold-time only — no event-payload change. Bibliographic setters (`Source.title/…`,
   `Citation.page/date`) stay bare `Attributed`, and that exception is stated in data-model §8.

4. **Typed citation creation stamp (finding 7 — PR-C3).** `CitationState.created_by: Option<String>`
   / `created_at: Option<Timestamp>` — an untyped shadow of the creation event's
   `EventContext.operator`/`occurred_at` — are replaced by a typed fold of that context (an `Agent` +
   `Timestamp`), preserving the Human/Software/AiModel distinction. Already fold-derived today, so this
   is a typing and naming change, not a payload change.

5. **Optional confidence on the envelope (finding 8 — PR-C2).** `EventContext.confidence` becomes
   `Option<Confidence>` with `#[serde(default)]`, so mechanical acts (`Tagged`, `RestrictionsChanged`,
   colour/path/checksum setters) carry no surety judgment nobody made. Additive — every stored event
   still decodes; the UI renders absence as "no judgment recorded".

## Rationale

- **Sensible disbelief needs per-link identity.** Only a per-`(child, parent)` `AssertionId` lets an
  operator retract "adopted by P2" while "child of this family" and "birth child of P1" stand — the
  exact case the review's finding 4 called out. Reconstructing the tuple list at fold time keeps the
  read model's shape unchanged for consumers (`FamilyForPerson`, export) while making each link
  independently correctable.
- **Matches the surveyed models.** Gramps and GEDCOM X both make the child–parent link its own
  citeable subject; the split brings us in line rather than inventing a shape.
- **One payload change per finding.** Grouping 4–8 in one ADR but implementing each in its own PR
  keeps every PR buildable and green while the log format is still cheap to change (pre-1.0,
  disposable workspaces).
- **The envelope stays the evidence channel (ADR 0020).** A relationship assertion's citations ride
  its envelope like every other claim; the split adds a new *claim*, not a new evidence channel.

## Consequences

- `FamilyState` gains `child_relationships: Vec<Attributed<Asserted<ChildRelationship>>>` (a new
  `ChildRelationship { child_id, parent_id, relationship }` value object) alongside the slimmed
  `children: Vec<Attributed<AssertedChild>>` (now `{ child_id, confidence, citations }`). `ChildEntry`
  survives as the read-model reconstruction type `FamilyView::children()` returns.
- The app `add_child` use-case keeps its signature but sequences a membership `AddChild` followed by
  one `AssertChildRelationship` per partner; because cqrs-es commits per aggregate, a failure after
  membership can leave an orphaned membership row (no relationships) — recoverable, not corruption,
  and re-running `add_child` is additive. A new `assert_child_relationship` use-case is the per-link
  edit path; `import_add_child` swallows both `ChildAlreadyPresent` and
  `ChildRelationshipAlreadyPresent` per piece so re-import stays additive.
- The child read DTO gains a per-link `ChildRelationshipRef { partner_human_id, relationship,
  confidence, source_count, assertion_id }`; the UI child row exposes an edit per partner link
  (supersede the link's id) and a per-link clear (retract the link's id), while a row Remove still
  retracts the membership assertion.
- The WIT `add-child` signature and the `family-dto.children` shape are unchanged — the host fans the
  incoming per-partner list out to the new commands, so no plugin, WIT world, or host-api version
  changes.
- `ChildAdded` events written in the pre-split fat shape do not decode against the new payload;
  accepted under ADR 0018 §3 (no upcaster).
- Findings 5/6/7/8 land in PR-C2/C3/C4 as described above; this ADR is their recorded decision.

### Rejected alternatives

- **Packed tuple with partial supersede.** Keeping `ChildAdded.relationships` and inventing a
  field-level supersede (retract one tuple inside the assertion) was rejected: it reintroduces the
  "one assertion, many claims" shape the correction model (ADR 0004 §2) exists to avoid, and no
  surveyed model works that way.
- **A relationship-only aggregate.** Modelling child–parent as its own aggregate (à la GEDCOM X's
  relationship subject) was rejected as over-heavy: the link has no lifecycle independent of the
  family, and a new aggregate would add registry rows, a resolver, and a projection for a claim that
  folds cleanly onto the family it already belongs to.
- **Upcasting the old fat `ChildAdded`.** Rejected by ADR 0018 §3 — workspaces are disposable and
  plugins are first-party, so the version bump is a label, not an upcaster gate.

## References

- ADR 0004 — event-sourcing implementation contract (provenance in the payload, corrections by
  `AssertionId`, self-contained per-variant-versioned events).
- ADR 0018 §3 — no backwards compatibility (disposable workspaces, first-party plugins): the stance
  that makes the version bumps labels rather than upcaster gates.
- ADR 0019 — participation ownership and payload: the sibling granularity/payload decision this ADR
  follows in house style.
- ADR 0020 — evidence citations live in the envelope: the evidence rule a relationship assertion
  respects.
- `docs/data-model.md` §7 (Fact/Event rule, `FactType` set), §8 (uniform projections, optional
  confidence, typed stamp), §10 (family command/event catalogue) — the vocabulary updated alongside.
- `docs/data-model-review.md` — findings 4–8, the review items this ADR decides.
