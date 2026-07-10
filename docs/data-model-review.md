# Data-model design review

- **Status:** Review findings — implementation planned in
  [data-model-review-plan.md](data-model-review-plan.md) (ADRs 0019–0021)
- **Date:** 2026-07-10
- **Scope:** the model as implemented in `genealogy-core` (see
  [data-model-diagram.md](data-model-diagram.md)), reviewed against GEDCOM 7, GEDCOM X, Gramps
  (current master), GENTECH GDM 1.0, and Evidence Explained / GPS practice. Primary sources were
  re-checked, not taken from data-model.md §2–§3; citations at the end.

## Verdict

The core architecture is sound and better-grounded than most shipping products: the event log as
assertion layer with a mandatory provenance envelope is exactly GEDCOM X `Attribution` +
GENTECH's assertion, made structural. Three things need attention while the log format is still
cheap to change: **participation ownership** (the dual person↔event assertion is the known-bad
shape in prior art), **one rule for where evidence citations live** (today there are two competing
channels with undefined precedence), and **per-participant substance on events** (age, citations —
every surveyed standard has it; we only carry a role).

Findings ranked by severity × cost-to-change-later. "Now" = do while pre-1.0 (disposable
workspaces, no upcasters); "Defer" = safe to postpone.

---

## 1. Participation is asserted twice — finish moving to one owner (Now)

**Status:** implemented in PR [#112](https://github.com/magne/genealogy/pull/112) (ADR 0019).

**Problem.** A person's participation in an event is independently asserted in two aggregates:
`PersonState.participations` (`Participation { event_id, role }`,
`crates/genealogy-core/src/person/state.rs:99`) and `EventState.participants`
(`EventParticipant { participant_id, role }`, `crates/genealogy-core/src/event/state.rs:47`).
The UI read-merges both sides with an origin tag; writes are person-canonical and legacy event
rows are retract-only — i.e. the migration to one owner is already half-taken.

**Evidence from prior art.** Every surveyed model picks exactly one owner: GEDCOM X hangs
`EventRole` off the Event; Gramps hangs `EventRef` off the Person/Family; GEDCOM 7 embeds the
event in its INDI/FAM record. No surveyed model dual-asserts. Documented failures of dual-write +
reconcile designs (Obsidian charted-roots bug tracker, 2025–26) fall into exactly two shapes —
data loss when a reconciler misreads a format change on one side as a deletion and cascades it,
and silent divergence when the two write paths cover different field sets — and the maintainer's
own audit names the root cause: no single source of truth for whether the link is still asserted.
Our variant is safer (append-only log, no delete cascades), but the divergence class and the
correction ambiguity are real today: the "same" participation can exist as two live assertions
with two `AssertionId`s, and retracting one leaves the other asserting the link.

**Options.**
- *(a) Person owns participation; Event.participants becomes a pure cross-aggregate projection.*
  Matches the write path already shipped; person history stays self-contained (good for
  persona/merge flows and GEDCOM INDI-centric export). Event screens read a projection, not
  event-aggregate state. Requires: stop accepting `AddParticipantRole` on Event, drop
  `participants` from `EventState`, project the event's participant list from person events.
- *(b) Event owns participation.* Matches GEDCOM X/TMG conceptually ("an event shared by
  participants"), and per-participant substance (finding 3) sits naturally under the shared thing.
  But it reverses the shipped write path and makes person history incomplete without a join.
- *(c) Keep the bridge permanently.* Rejected by the evidence above: permanent read-merge means
  permanent origin-tag special cases in every consumer (export, narrative, dedup).

**Recommendation.** (a) — complete the person-canonical migration and record it in a new ADR
("Participation is person-owned; event participant lists are projections"). The bridge was the
right transitional move; it should have an end date. Retraction semantics become unambiguous: the
person-side `AssertionId` is the only handle.

## 2. Two citation channels with undefined precedence (Now)

**Status:** implemented in PR [#110](https://github.com/magne/genealogy/pull/110) (ADR 0020).

**Problem.** A claim's backing citations can live in two places with no stated rule:
the envelope (`EventContext.citations`, `crates/genealogy-core/src/provenance.rs:150`) and the
payload of some value objects (`Fact.citations`, `fact.rs:26`; `Attribute.citations`,
`text.rs:64`; `MediaRef.citations`, `text.rs:93`). The projections then denormalize
inconsistently: `AssertedName.citations` is copied from the envelope
(`person/state.rs:42`), while `AssertedFact` copies only confidence because `Fact` carries its own
payload list (`person/state.rs:62`). A fact asserted with citations in the envelope and different
citations in the payload is representable today, and nothing says which an exporter or UI should
trust.

**Evidence.** GEDCOM X deliberately single-sources this: `sources` lives once, on `Conclusion`
(which Fact/Name/EventRole extend); `SourceReference` carries no confidence/quality of its own.
GENTECH is one source per assertion at the leaf. Nobody has two channels for the same meaning.

**Options.**
- *(a) Envelope-only for evidence.* Drop `Fact.citations` and `Attribute.citations`; the
  envelope's `citations` is the one evidence link for the claim being asserted. Keep
  `MediaRef.citations`, whose meaning is genuinely different (citations for *using* the media at
  this attachment point, not for a claim). Payload shrinks; the §8 story ("the envelope is the
  evidence link") becomes true without exceptions.
- *(b) Payload-only for claim-shaped VOs.* Define envelope citations as fallback. Keeps GEDCOM
  import mapping slightly more literal (INDI attribute SOUR → Fact.citations) but contradicts §8
  and leaves Name/Sex (no payload slot) on the envelope path anyway — the inconsistency survives.

**Recommendation.** (a), now — this is an event-payload change that gets expensive the moment
workspaces stop being disposable. Fold-time denormalization then makes `AssertedFact` identical in
shape to `AssertedName`, which finding 6 wants anyway. Needs an ADR (it narrows ADR 0004 §1's
"citations may also appear in payloads" reading).

## 3. Event participation carries a role and nothing else (Now)

**Status:** fully closed. Payload half in PR [#111](https://github.com/magne/genealogy/pull/111)
(ADR 0019); the plugin-boundary + AGE/ASSO round-trip in PR [#113](https://github.com/magne/genealogy/pull/113) (`host-api@0.13.0`).

**Problem.** `EventParticipant`/`Participation` is `{ person_id/event_id, role }`. No
age-at-event, no participant-scoped notes, no participant attributes. data-model §17 already
flags event-level witnesses as a round-trip gap.

**Evidence.** Every surveyed standard has per-participant substance: GEDCOM 7 gives family events
per-role `AGE` (HUSB/WIFE blocks) and gives `ASSO` inside any event its own `ROLE` + 0..M
`SOURCE_CITATION` + 0..M `NOTE_STRUCTURE`; GEDCOM X `EventRole` extends `Conclusion` — every
role-assignment carries its own sources, confidence, notes, attribution, and a free-text
`details`; Gramps `EventRef` carries an attribute list (age, witness-specific data) with
per-attribute citations; TMG's per-role model is what drives narrative output. Our per-assertion
envelope already gives a participation its own confidence + citations (the `Asserted` wrapper on
the event side), so the gap is the *payload*: age and participant-scoped detail have nowhere to
go.

**Recommendation.** Extend the participation payload (on whichever side finding 1 keeps) with
optional fields: `age: Option<GenealogicalDate>` (or a dedicated `Age` VO mirroring GEDCOM's
`AGE` grammar), `attributes: Vec<Attribute>`, `notes: Vec<NoteId>`. Additive event change — cheap
now, and it directly closes the §17 witness round-trip gap. Do it together with finding 1's ADR so
the payload only changes once.

## 4. Child–parent relationship rows cannot be corrected independently (Now)

**Problem.** `ChildEntry.relationships: Vec<(PersonId, ChildParentRelationship)>`
(`family/state.rs:30`) packs all per-parent relationships into the single child assertion. One
`AssertionId` covers "child of this family" *and* "birth child of P1" *and* "step child of P2".
You cannot retract or re-cite the adoption link alone without retracting the child's membership.

**Evidence.** Gramps `ChildRef` (current master) is `SecondaryObject + PrivacyBase +
CitationBase + NoteBase + RefBase` with `frel`/`mrel` — per-child, per-parent-link **direct
citations**. GEDCOM X models child-and-parents as its own relationship subject. Polygenea's
"principle of sensible disbelief" states the design rule exactly: it should be sensible to
disbelieve a node without disbelieving nodes it references — here, disbelieving "adopted by P2"
should not force disbelieving "child of the family".

**Recommendation.** Split the assertion granularity: keep `ChildAdded` for membership, add a
per-(child, partner) relationship assertion (`ChildParentRelationshipAsserted { child_id,
parent_id, relationship }`) with its own envelope. The tuple list in `ChildEntry` becomes a fold
of those assertions. Payload change → do now, own ADR or fold into finding 1/3's family-side ADR.

## 5. Fact vs Event overlap has no promotion rule (Now — rule only, no schema change)

**Problem.** `FactType` and `EventType` both carry Birth, Death, Baptism, Burial (plus
Residence). The same real-world birth is representable as a single-person `Fact` or as an `Event`
with one Primary participant, and nothing states when each is right or how one becomes the other
(a birth gains a witness → now what? assert an Event and retract the Fact? keep both?). Importers
must pick silently; dedup and narrative must handle both shapes forever.

**Evidence.** GEDCOM X keeps `Fact` and `Event` separate and explicitly provides **no**
structural link between them (§2.5.2 "Events Versus Facts") — and that ambiguity is a known
complaint against the GEDCOM X model. Gramps avoids it by having only events.

**Recommendation.** Keep both types (the §7 rationale — a Fact is cheap, an Event is shared) but
write the rule into data-model §7: *vital and shared-capable types (birth/death/baptism/burial)
are asserted as Events with a Primary participant; `Fact` is reserved for attribute-shaped claims
(occupation, residence, religion, description, …)* — or, if the cheap path must stay, define the
promotion: asserting an Event of the same type+date supersedes the Fact, by `AssertionId`, in one
command. Either way the rule must exist before the GEDCOM importer's choice fossilizes into user
data. Consider trimming the four overlapping variants from `FactType` if the first rule is chosen.

## 6. Projection provenance is denormalized inconsistently (Now, cheap)

**Problem.** Whether a row surfaces confidence + citations depends on which aggregate you're in,
not on the domain:
- Event/Place/Source/DnaTest fields use the generic `Asserted<T>`; Person/Family use bespoke
  `AssertedName`/`AssertedFact`/`AssertedPartner`/… (the `assertions.rs:38` doc comment already
  calls the generic the replacement for these).
- `Person.sex` is plain `Attributed<Sex>` — a sex assertion's confidence/citations are invisible
  in the projection while a name's are surfaced (`person/state.rs:91`).
- `Person.participations` is plain `Attributed<Participation>` while the *same link* on the event
  side is `Asserted<EventParticipant>` (`event/state.rs:47`) — the person side hides the
  provenance the event side shows.
- `Citation.page/date`, `Source.title/author/…` are plain `Attributed<String>` — defensible
  (bibliographic bookkeeping, the citation *is* the evidence), but nowhere stated.
- `EventState` has no `restrictions_assertion`, so an event's `SetRestrictions` cannot be
  retracted the way a person's can (`person/state.rs:113` vs `event/state.rs`).

**Recommendation.** Converge on `Asserted<T>` everywhere a claim is domain-meaningful (sex,
participations included), delete the bespoke structs, add `restrictions_assertion` to the
aggregates that lack it, and state the exception rule (bibliographic setters stay bare) in
data-model §8. Fold-time only — no event-payload change, no upcasting; snapshots are disposable
here. Cheap, and it removes a whole class of "why does the UI show confidence here but not there"
bugs before the UI multiplies.

## 7. `Citation.created_by`/`created_at` shadows the envelope, stringly (Now, cheap)

**Problem.** `CitationState` carries `created_by: Option<String>`, `created_at:
Option<Timestamp>` — an untyped copy of what the creation event's `EventContext` already records
as `operator: Agent` + `occurred_at`. It can diverge from the envelope, is not an `Agent` (loses
the Human/Software/AiModel distinction §13 exists for), and no other aggregate has it.

**Recommendation.** Drop the fields; fold `operator`/`occurred_at` from the creation event's
context into the projection if a read model needs a creation stamp (same denormalize-at-fold
pattern as `Asserted`). Payload + projection cleanup while cheap.

## 8. Mandatory `Confidence` on every assertion is noise for mechanical ops (Now, trivial)

**Problem.** `EventContext.confidence` is non-optional (`provenance.rs:147`), so `Tagged`,
`RestrictionsChanged`, `TagColorSet`, path/checksum setters all carry a surety judgment nobody
made. Meaningless Normals pollute the very signal the field exists to carry.

**Evidence.** GEDCOM X: `confidence` on `Conclusion` is optional. GPS practice qualifies
*conclusions*, not every editorial act.

**Recommendation.** `confidence: Option<Confidence>` with `#[serde(default)]` — additive,
decodes all stored events, UI renders absence as "no judgment recorded". Small ADR note, not a
full ADR.

## 9. Two confidence layers exist — legitimate, but undocumented (Defer, doc-only)

**Problem.** Confidence lives on the assertion (`EventContext.confidence`) *and* as data on the
Citation aggregate (`citation/state.rs` `confidence`, `evidence_analysis`) — the Gramps
`QUAY`-shaped, per-citation quality. They can disagree; nothing says which a consumer shows where.

**Evidence.** This is *not* the STEMMA anti-pattern (numerically combined double surety, which its
own author flags as controversial) — the two layers have different subjects: the operator's surety
in *a claim* vs the recorded quality of *a citation*. GEDCOM X keeps only the claim layer; Gramps
keeps only the citation layer; we keep both because we round-trip Gramps/GEDCOM (`QUAY`) *and*
follow GENTECH ("surety is an attribute of the assertion, not the source" — §2.4). Both layers
are justified; the missing piece is the sentence saying so.

**Recommendation.** Document in data-model §8: claim surfaces show assertion confidence; citation
screens show citation confidence; never combine arithmetically. No structural change.

## 10. Family-linked events vs participant assertions — state the semantics (Defer, doc-only)

**Problem.** A marriage can be referenced three ways: `Family.linked_events`
(`AssertedFamilyEvent`), each spouse's participation, and the Event's own participant list. All
three are independently assertable; none is defined as authoritative for "who was married here".

**Recommendation.** After finding 1 lands, define: participations (person-owned) are the
authority on who took part; `Family.linked_events` is a categorization link ("this event belongs
to this family's story", GEDCOM `FAM.MARR` mapping), not a participation claim. One paragraph in
§9/§10; revisit only if narrative generation finds the link redundant enough to derive.

---

## Positive findings (no change)

- **Envelope-in-payload is validated by the standards.** GEDCOM X separates `Attribution`
  (editorial who/when/why) from `sources` (evidence) exactly as `EventContext.operator/rationale`
  vs `citations` do. The §8 design is the GEDCOM X model with the optionality removed.
- **`Asserted<T>` denormalization is sound** — it is a fold-time copy of envelope data, not a
  second source of truth. (Finding 6 is about applying it uniformly, not about its existence.)
- **Citation as its own aggregate earns its keep** — reusable per-fact citation is the surveyed
  best practice (Legacy per-detail sourcing, TMG), and Gramps' `Citation.confidence` maps onto it
  for round-trip.
- **Tag without an assertion chain is fine** — tag definitions are workspace vocabulary, not
  genealogical claims; last-writer-wins matches intent.
- **`ChildEntry` per-partner relationship** (`_FREL`/`_MREL` shape) is the right vocabulary —
  finding 4 is about assertion granularity, not the relationship model.
- **Fixed-decimal `Centimorgans`/`PercentShared`** avoid float equality trouble the sketch's
  `f64` would have had.
- **`Restriction::Privacy`**: GEDCOM 7 itself flags `PRIVACY` as ambiguous ("use … is not
  recommended"). Carrying it first-class for round-trip is still right; treat it as an import
  artifact, not a recommended user choice — worth a sentence in §16 eventually.

## Scale notes (watch, don't act)

- Aggregate replay for hot persons (hundreds of assertions) is bounded by cqrs-es snapshotting,
  which ADR 0004 defers until measured — the right call; the fold is linear and allocation-light.
- `live_assertions: BTreeSet<AssertionId>` grows monotonically per aggregate; harmless at
  genealogy scale, only matters if snapshots are serialized hot.
- Cross-aggregate checks against lagging projections (`UnknownPlace`) remain the accepted
  aggregate tax; nothing found that worsens it at 100k persons.

## Suggested follow-up ADRs

1. **ADR: Participation ownership** — person-canonical, event participant list becomes a
   projection; participation payload gains age/attributes/notes (findings 1 + 3, one payload
   change).
2. **ADR: Evidence citations live in the envelope** — drop `Fact.citations` /
   `Attribute.citations`; `MediaRef.citations` retained as per-use context (finding 2).
3. **ADR (small): assertion-granularity + envelope cleanups** — per-(child,parent) relationship
   assertions (4), optional confidence (8), drop citation creation stamp (7), uniform
   `Asserted<T>` projections + `restrictions_assertion` everywhere (6), Fact/Event promotion rule
   (5 — data-model §7 text).

## Sources consulted

- GEDCOM 7 spec — <https://gedcom.io/specifications/FamilySearchGEDCOMv7.html>
  (ASSOCIATION_STRUCTURE, INDIVIDUAL/FAMILY_EVENT_DETAIL `AGE`, `RESN` + enumset, SOURCE_RECORD).
- GEDCOM X conceptual model —
  <https://github.com/FamilySearch/gedcomx/blob/master/specifications/conceptual-model-specification.md>
  (§2.5 Event/EventRole, §2.5.2 Events-vs-Facts, §3.2 Attribution, §3.6 SourceReference,
  §3.10 Conclusion).
- Gramps master source — `gramps/gen/lib/{eventref,childref,attribute,citation}.py`
  (<https://github.com/gramps-project/gramps>): EventRef = IndirectCitationBase (citations only
  via attributes); ChildRef = direct CitationBase + frel/mrel; confidence only on Citation.
- GENTECH GDM 1.0 diagram —
  <https://www.ngsgenealogy.org/wp-content/uploads/NGS-History/Diagram_GENTECH_Data_Model_1.0.pdf>
  and primer <https://genealogy.sourceforge.net/GENTECH_Primer.html> (one source + one surety per
  assertion; ASSERTIONASSERTION join; Disproved flag).
- Evidence Explained / GPS — Mills §1.6 qualifier vocabulary (per-conclusion confidence);
  <https://genealogyauthority.com/genealogical-proof-standard> (source/information/evidence as
  analysis inputs, not stored fields).
- STEMMA probabilities (double-layer surety prior art, author-flagged controversial) —
  <https://parallaxview.co/stemma/home/document-structure/narrative-structure/probabilities/content.html>.
- Polygenea (append-only claims, "principle of sensible disbelief") —
  <https://github.com/rootsdev/polygenea>.
- charted-roots bidirectional-sync failure taxonomy —
  <https://github.com/banisterious/obsidian-charted-roots> issues #410, #420, #423, #534 and the
  bidirectional-sync audit commit (data-loss-shaped vs inconsistency-shaped failures; "no central
  registry" root cause).
