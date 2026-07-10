# 19. Participation ownership and payload substance

- **Status:** Accepted
- **Date:** 2026-07-10

## Context

A person's participation in a shared event can be claimed from two aggregate sides. The Person
aggregate emits `ParticipationAsserted` (person → event, with a role); the Event aggregate emits
its own participant-role assertions (event → person). Both sides project into the person and event
read models, which read-merge them so a link is visible regardless of where it was asserted
(data-model §6, §10).

The data-model review (`docs/data-model-review.md`) flagged two problems:

- **Finding 1 — no single owner.** Two writable sides with a permanent read-merge bridge mean a
  `(person, event, role)` link has no canonical home. Corrections, export, and de-duplication all
  have to reason about both sides forever.
- **Finding 3 — a thin payload.** `ParticipationAsserted` carried only `event_id` and `role`. A
  participation in genealogical sources routinely records the participant's **age** at the event
  (GEDCOM `INDI.*.AGE`), participant-scoped **attributes** (e.g. a witness's occupation as recorded
  on that record), and **notes** about the participation. None of these had anywhere to live, so
  importers dropped them and the round-trip lost data.

The evidence model is already settled: ADR 0020 makes the event envelope (`EventContext.citations`)
the sole channel for a claim's backing citations, and payload value objects carry no citation lists.
Any new participation payload must respect that.

## Decision

1. **The Person aggregate owns participation.** `ParticipationAsserted` on the Person aggregate is
   the canonical assertion of a `(person, event, role)` link (review finding 1, option a). The
   event-side participant lists become **projections** that read-merge the canonical person-side rows;
   the event aggregate stops being a writable second source of truth for participation. Retiring the
   event-side write path is executed in PR-A2 — this ADR fixes the ownership decision and the payload;
   the event side keeps working as a read-merged legacy source until then.

2. **The participation payload gains substance (review finding 3).** `ParticipationAsserted` and its
   `Participation` projection value gain three optional fields:
   - `age: Option<Age>` — the participant's age at the event.
   - `attributes: Vec<Attribute>` — participant-scoped typed key/value attributes.
   - `notes: Vec<NoteId>` — notes about this participation.

3. **`Age` is a new value object.** Age is a *duration* (how old the participant was), not a calendar
   point, so it is modelled as its own value object rather than reusing `GenealogicalDate`:

   ```
   Age { bound: Option<AgeBound>, years: Option<u16>, months: Option<u16>, days: Option<u16>, phrase: Option<String> }
   AgeBound = LessThan | GreaterThan
   ```

   `bound` captures GEDCOM's `<` / `>` age qualifiers (e.g. "less than 18"). `phrase` carries a
   free-text age that does not decompose into parts (GEDCOM `AGE` phrase). `Age::is_empty()` reports
   whether every field is absent, so an all-`None` age is normalized to `None` at the boundary rather
   than stored as an empty value object. **Weeks are rejected:** GEDCOM's `w` age unit is normalized
   to days at import time, keeping the value object to the three units genealogical consumers query.
   **`GenealogicalDate`-as-age is rejected:** an age is a span, not a date, and overloading the date
   type would misrepresent the data and complicate every consumer.

4. **Evidence stays on the envelope (ADR 0020).** Neither the participant-scoped `Attribute`s nor the
   `notes` carry their own citation lists. A participation's backing citations live on the assertion
   envelope (`EventContext.citations`), exactly as for every other Person claim. This ADR adds no new
   evidence channel.

5. **`ParticipationAsserted` becomes version `"2.0"`, with no upcaster.** Widening the payload is a
   schema change. Workspaces are disposable and every plugin is first-party (ADR 0018 §3), so the
   version string is a documentation label — no upcasting tooling is written, and the bump is
   per-variant (`ParticipationAsserted => "2.0"`, all other Person events keep their version).

## Rationale

- **One owner ends the reconciliation.** With the Person side canonical, corrections, GEDCOM export,
  and de-duplication reason about a single write path; the read-merge bridge becomes a
  read-only compatibility shim until the event-side write path is retired (PR-A2).
- **The payload matches the sources.** Age, participant-scoped attributes, and notes are routinely
  present in the records importers read; giving them a home closes the round-trip data loss finding 3
  identified.
- **Age as its own type keeps consumers honest.** A duration value object cannot be mistaken for a
  calendar date, and `is_empty()` keeps the "no age recorded" case as `None` rather than an empty
  struct.
- **No new evidence channel.** Reusing the envelope (ADR 0020) keeps every Person claim's evidence in
  one place; attributes and notes describe the participation, they do not re-cite it.

## Consequences

- `Participation` (projection) and `ParticipationAsserted` (event) gain `age`, `attributes`, and
  `notes`; the Person `participations` state field becomes `Vec<Attributed<Asserted<Participation>>>`
  so a folded row carries the envelope confidence and citation ids like every other asserted value.
- `AssertParticipation` (command), the app `assert_participation` use-case, the UI intent, and the
  CLI `person add-participation` all gain the age/attribute/note inputs; the participation read DTO
  surfaces them plus the confidence and source count.
- `ParticipationAsserted` events written before this change do not decode against the new payload;
  this is accepted under the disposable-workspace stance (ADR 0018 §3) — no upcaster bridges the
  versions.
- Event-side (legacy) participant rows merged into a person's participations carry no age/attributes/
  notes and no per-participation confidence; those fields are empty for `origin: Event` rows until the
  event-side path is retired (PR-A2).
- The GEDCOM round-trip that carries age/attributes/notes end-to-end is completed in PR-A3; this ADR
  and its PR add the payload half.

### Rejected alternatives

- **Event-owned participation.** Making the Event aggregate the canonical owner was rejected: the
  Person aggregate is where a genealogist reasons about an individual's life events, and the existing
  read-merge already favours the person side as the export source.
- **A permanent dual-side bridge.** Keeping both sides writable forever was rejected as the status quo
  finding 1 called out — it leaves the link with no owner and forces every consumer to reconcile two
  sources indefinitely.

## References

- ADR 0004 — event-sourcing implementation contract (pure decision core, provenance in the payload,
  per-variant versioning).
- ADR 0018 §3 — no backwards compatibility (disposable workspaces, first-party plugins): the stance
  that makes the `2.0` bump a label rather than an upcaster gate.
- ADR 0020 — evidence citations live in the envelope: the decision this payload respects (no citation
  lists on attributes or notes).
- `docs/data-model.md` §7 (value objects — `Attribute`, the new `Age`), §10 (the Person
  command/event catalogue) — the vocabulary updated alongside.
- `docs/data-model-review.md` — findings 1 (participation ownership) and 3 (participation payload
  substance), the review items this begins to close.
