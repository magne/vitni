# Data-model review — implementation plan

- **Status:** Implemented — all nine PRs merged (PRs #110–#118 + PR-D1)
- **Date:** 2026-07-10
- **Implements:** all 10 findings of [data-model-review.md](data-model-review.md)
- **Docs kept in sync per PR:** [data-model.md](../data-model.md),
  [data-model-diagram.md](../data-model-diagram.md)

Nine PRs in four phases, seeded by the review's suggested ADR grouping and scoped from a
traced blast radius (below). Three new ADRs: **0019** (participation), **0020** (evidence
citations), **0021** (assertion granularity + envelope cleanups). The roadmap owns overall phase
tracking; this plan tracks only its own PRs.

## Ground rules (apply to every PR)

- **ADRs are immutable** — new decisions get new ADRs (0019–0021, next free numbers after 0018);
  ADR text lands in the first PR of its group, like ADR 0018 did. Never edit ADR 0004; ADR 0020
  narrows its §1 payload-citations reading by superseding statement.
- **No backwards compatibility** (disposable workspaces, first-party plugins): payload changes are
  breaking-OK, no upcasters, no dual formats, delete legacy decode paths. WIT/host-api version
  bumps are labels.
- **Every PR leaves the workspace green:** `cargo build --workspace` ·
  `cargo nextest run --workspace --all-features --all-targets` ·
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` ·
  `cargo xtask i18n-check` · `cargo xtask build-plugins` · `prek run`.
- **Feature branches, `--no-ff` merges, never direct to main.** PRs are independent unless the
  graph below says otherwise; avoid stacking (merging a stacked base with `--delete-branch`
  closes the dependent PR — retarget first if unavoidable).
- **Docs in the same PR as the code:** the affected `data-model.md` sections, the affected
  `data-model-diagram.md` blocks (regenerate from `state.rs`/`view.rs`; keep conventions —
  `Attributed` elided, `Asserted~T~` shown, plain-text `«enumeration»`, never `<<…>>`; re-validate
  with mermaid-cli using a `--no-sandbox` puppeteer config), and a
  `Status: implemented in PR #NNN` line on the closed finding in `data-model-review.md`.
- **Registry note:** participation and child links ride their owning aggregate's rows in the three
  `for_each_*!` registries (`vitni-app/src/aggregates.rs`, `vitni-db/src/registry.rs`,
  `vitni-cli/src/main.rs`). No PR here adds an aggregate, so no registry rows are added;
  only PR-A2 touches registry-adjacent wiring (the event resolver).

## Blast radius (traced, not guessed)

Findings that turned out **smaller** than the review assumed:

- **Importers already write person-side participation.** `plugin-host/src/state.rs:347
  add_event_participant()` calls `vitni_app::assert_participation` (person aggregate), and
  both import plugins go through it. No importer change for finding 1.
- **Exporters already walk `person.participations`** to reconstruct INDI/FAM events
  (`plugins/gedcom-export/src/lib.rs:47–56, 157`; `plugins/gramps-export/src/lib.rs:48–52`).
  Event-side removal does not touch export logic.
- **`Attribute.citations` is a dead field** — constructed empty at all four production sites
  (`vitni-app/src/{citation.rs:364, source.rs:343, media.rs:290}`, one core test), read
  nowhere, absent from WIT. Removal is trivial.
- **`Citation.created_by`/`created_at` are already fold-derived from the envelope**
  (`citation/decide.rs:171–172` read `event.context`), so they cannot diverge; finding 7 reduces
  to a typing/naming cleanup, not a payload change.

Key consumer sets per finding (full traces in the PR items):

| Finding | Hot spots |
| --- | --- |
| 1 (dual participation) | core `event/{command,events,decide,state,view}.rs`, `db/src/resolver.rs:171`, app `event.rs:379 set_participant_role`, the merge pair `person.rs:1131 merged_participations` / `event.rs:901 merged_participants` + `dto.rs:29 ParticipationOrigin`, dioxus `screens/event.rs:1009` origin routing, CLI `set-participant-role` |
| 3 (payload substance) | person `command/event/state/decide/view`, app `person.rs:415 assert_participation` + `ParticipationRef`, ui `intent.rs:772` + `ParticipationForm` (dioxus person.rs:1907), WIT `host.wit:124 record participation` |
| 2 (citation channels) | `fact.rs:26`, lone payload-fold asymmetry `person/decide.rs:226` (names/associations read the envelope at :214/:245), readers `app/person.rs:1027`, `app/citation_usage.rs:79`, `ui/view_model/person.rs:99`, `ui/view_model/dashboard.rs:39`; `Fact` never crosses WIT (host `state.rs:1357 from_fact` drops citations) |
| 4 (child granularity) | family `command.rs:39 AddChild.relationships`, `event.rs:46`, `decide.rs:88/206`, app `family.rs:245 add_child` + `import.rs:121`, ui `navigation.rs:959` + dioxus `family.rs:1175 FamilyAddChildForm`, WIT `host.wit:547 add-child` + `:406 child-parent-rel`, plugins `_FREL`/`_MREL` mapping |
| 6/7 (projection uniformity, stamp) | bespoke `Asserted*` structs in `person/state.rs`/`family/state.rs`, `person.sex` (`state.rs:91`), `citation/{state.rs:32–34, view.rs:52}`, DTO `dto.rs:322 asserted_by/at`, ui `view_model/citation.rs`, cli `i18n/citation.rs` |
| 8 (optional confidence) | `provenance.rs:148`, sole builder `app/session.rs:76` + `use_case.rs:23`, `assertions.rs:44 Asserted.confidence` + every fold, app summaries (non-Option `confidence` fields), ui `view_model/provenance.rs:19` + dioxus provenance block, CLI label `i18n/mod.rs:142`, ~12 dioxus test files. WIT unaffected (per-assertion confidence never crossed the boundary; WIT `confidence` is Citation QUAY only) |

---

## Phase A — Participation (ADR 0019, findings 1 + 3)

### PR-A1 — ADR 0019 + participation payload substance (person side)

Findings: 3 (and the payload half of 1). Crates: `vitni-core`, `vitni-app`,
`vitni-ui`, `vitni-ui-dioxus`, `vitni-cli`. Additive — event side untouched.

- [x] Write `docs/adr/0019-participation-ownership-and-payload.md`: person-owned participation
      (review option a); event participant lists become projections; payload gains substance.
      Decide the age representation — **default: a dedicated `Age` value object mirroring GEDCOM's
      `AGE` grammar (`{ years?, months?, days?, phrase? }`)**; `GenealogicalDate` rejected in the
      ADR (an age is a duration, not a calendar point). Record rejected alternatives (event-owned,
      permanent bridge) in the ADR, not here.
- [x] Core: add `Age` VO; extend `Participation` and `PersonEventBody::ParticipationAsserted`
      with `age: Option<Age>`, `attributes: Vec<Attribute>`, `notes: Vec<NoteId>`; bump the
      variant's payload version label; fold participations as provenance-carrying rows
      (`Asserted`-shaped: confidence + citations denormalized from the envelope — person side
      reaches parity with the event side it will replace).
- [x] App: extend `assert_participation` (`person.rs:415`) and `ParticipationRef`; re-export any
      new types from `vitni-app/src/lib.rs` (the pub-use surface).
- [x] UI: extend `PersonEdit::AssertParticipation` intent, `participation_vm`, and the dioxus
      `ParticipationForm` (age/notes/attributes inputs); new Fluent keys in the per-module
      fragments (never the generated `vitni-cli.ftl`).
- [x] CLI: extend `person assert-participation` args.
- [x] Tests: decide/fold round-trip for the new payload; form dispatch; old-shape events are gone
      (no legacy decode path — prove new events decode, delete fixtures using the old shape).
- [x] Docs: data-model §7 (`Age` VO), §10 (`ParticipationAsserted` payload); diagram Person &
      Family block + overview edge label; review finding 3 status.

### PR-A2 — retire the event-side participation and the read-merge bridge

Finding: 1. Depends on: PR-A1 (parity of the person-side rows). Crates: `vitni-core`,
`vitni-db`, `vitni-app`, `vitni-ui`, `vitni-ui-dioxus`, `vitni-cli`.

- [x] Core: remove `AddParticipantRole`/`RemoveParticipantRole` commands,
      `ParticipantRoleAdded/Removed` events, `EventState.participants`, `EventParticipant`, and
      the view accessors; `NoParticipants` error semantics move to the app layer if still wanted.
- [x] Db: drop the resolver skip rows (`resolver.rs:171–172`).
- [x] App: delete `set_participant_role` (`event.rs:379`); `merged_participants` (`event.rs:901`)
      becomes a pure person-side projection over the existing `EventLookups.person_participations`
      (already loaded — the lookup infrastructure stays, the event-side half and the dedup go);
      `merged_participations` (`person.rs:1131`) loses its event-side fold; delete
      `ParticipationOrigin` (`dto.rs:29`) and the `origin` fields.
- [x] UI: drop origin-based row routing (dioxus `event.rs:1009–1030`); event-screen "add
      participant" keeps writing the person aggregate (already does); remove retract-only legacy
      paths and their Fluent keys.
- [x] CLI: remove `event set-participant-role` (person `assert-participation` is the write path).
- [x] Tests: event projection shows person-asserted participants; retraction via the person-side
      `AssertionId` removes the row from both screens; delete bridge/origin tests.
- [x] Docs: data-model §9/§10 (event participant list is a projection; remove the event-side
      verbs), diagram overview (drop the `Event --> Person : participants` edge + the dual-write
      note) and Event & Place block (drop `participants`/`EventParticipant`); review finding 1
      status. Supersedes the participation-bridge read-merge described in PR #106/#107.

### PR-A3 — participation over the plugin boundary + AGE/ASSO round-trip

Finding: 3 (round-trip half; closes the data-model §17 witness gap). Depends on: PR-A1.
Crates: `vitni-plugin-host`, `plugins/*`, `vitni-gedcom`.

- [x] WIT: extend `record participation` (`host.wit:124`) with age/attributes/notes; bump the
      host-api version label (`@0.13.0`); host mapping in `state.rs` (`add_event_participant` keeps
      writing the person aggregate — now takes a `participation-input` record). `Age` lives in
      `vitni-interchange` with the shared AGE grammar (`parse_age`/`age_value`).
- [x] Import: GEDCOM `AGE` (individual events) and `HUSB`/`WIFE` `AGE` (family events) → the
      participation `age`; event-level `ASSO` (`ROLE` + citations→envelope + notes) → a participation
      with role and note/citation payload — the §17 "event-level witnesses" gap. Gramps eventref
      `role`/`Age`/attributes/noteref/citationref → the participation payload; a `(person, event)`
      seen-set keeps a payload-carrying partner single-asserted.
- [x] Export: emit `2 ASSO`/`3 ROLE`/`3 NOTE` under events for non-primary participants and `AGE`
      (INDI `2 AGE`, family `HUSB`/`WIFE` `3 AGE`) where the model has it; Gramps person-side
      `<eventref role=…>` with `Age`/attributes/noteref for payload-carrying participations.
- [x] Tests: gedcom/gramps round-trip fixtures with witnesses and ages (fixtures under
      `vitni-import` are verbatim captures — added new fixtures, never reformatted existing).
- [x] Docs: data-model §17 (moved witnesses/AGE to the round-trip list; documented the new
      GEDCOM-lossy gaps — attributes, primary-participation notes, import-only witness citations);
      review finding 3 status (fully closed).

## Phase B — Evidence citations (ADR 0020, finding 2)

### PR-B1 — ADR 0020 + envelope-only evidence citations

Finding: 2 (+ the dead-field removal). Independent of Phase A. Crates: `vitni-core`,
`vitni-app`, `vitni-ui`.

- [x] Write `docs/adr/0020-evidence-citations-live-in-the-envelope.md`: `EventContext.citations`
      is the sole evidence channel for a claim; `MediaRef.citations` retained (per-use context,
      different meaning); narrows ADR 0004 §1's payload-citations reading.
- [x] Core: drop `Fact.citations` (`fact.rs:26`) and the dead `Attribute.citations`
      (`text.rs:64`); `AssertedFact` gains `citations` denormalized from the envelope — the fold
      at `person/decide.rs:226` joins names/associations in reading `event.context.citations`;
      bump the `FactAsserted` variant version label.
- [x] App: `assert_fact` (`person.rs:462`) stops copying `meta.citations` into the payload;
      readers move to `AssertedFact.citations` (`person.rs:1027` source_count,
      `citation_usage.rs:79` reverse index); the three `add_attribute` use-cases drop the empty
      vec.
- [x] UI: `view_model/person.rs:99` and `view_model/dashboard.rs:39` read the denormalized list
      (no visible behavior change — counts now come from the envelope).
- [x] Tests: a fact asserted with envelope citations shows them on the projection; the reverse
      citation-usage index still finds facts; no payload slot remains to disagree with the
      envelope.
- [x] Docs: data-model §7 (`Fact`, `Attribute` shapes), §8 (envelope-is-the-evidence-link now
      exception-free); diagram Person & Family (Fact) and Evidence (Attribute) blocks; review
      finding 2 status.

## Phase C — Granularity + envelope cleanups (ADR 0021, findings 4, 5, 6, 7, 8)

### PR-C1 — ADR 0021 + per-(child, parent) relationship assertions

Finding: 4. Independent. Crates: `vitni-core`, `vitni-app`, `vitni-ui`,
`vitni-ui-dioxus`, `vitni-plugin-host`, `plugins/*`.

- [x] Write `docs/adr/0021-assertion-granularity-and-envelope-cleanups.md` covering findings
      4/5/6/7/8 (one small ADR, per the review's grouping); this PR implements the finding-4 part.
- [x] Core: `AddChild`/`ChildAdded` slims to membership (`child_id` only); new
      `AssertChildRelationship`/`ChildRelationshipAsserted { child_id, parent_id, relationship }`
      with its own envelope; `ChildEntry.relationships` becomes a fold of those assertions
      (per-row `AssertionId`, so one adoption link retracts alone).
- [x] App: `add_child` (`family.rs:245`) sequences membership + per-parent relationship commands
      (validate-first, like the person change-set — cqrs-es commits per aggregate, orphans
      possible but not corruption); `import.rs:121 import_add_child` follows.
- [x] UI: child form dispatches per-parent relationship intents; per-row edit/retract for a single
      parent link.
- [x] Plugin host: WIT `add-child` keeps its `list<child-parent-rel>` signature — the host fans
      out to the new commands (imports unchanged); `family-dto.children` unchanged shape.
- [x] Tests: retracting one parent-relationship leaves membership and the other parent's link
      live; `_FREL`/`_MREL` round-trip unchanged.
- [x] Docs: data-model §10 (family verbs); diagram Person & Family block (ChildEntry note);
      review finding 4 status.

### PR-C2 — optional confidence on the envelope

Finding: 8. Independent (do before PR-C3 so the `Asserted` shape settles once). Crates:
`vitni-core`, `vitni-app`, `vitni-ui`, `vitni-ui-dioxus`, `vitni-cli`,
`vitni-db` (test fixtures).

- [x] Core: `EventContext.confidence: Option<Confidence>` with `#[serde(default)]`
      (`provenance.rs:148`); `Asserted<T>.confidence` (and the bespoke structs until PR-C3)
      follow; `from_context` passes the Option through.
- [x] App: `session.rs:76 new_meta` + `use_case.rs Provenance` stop defaulting to `Normal` —
      absence means "no judgment recorded"; summary DTOs carry `Option<Confidence>`.
- [x] UI/CLI: provenance block renders absence (new Fluent key), confidence select gains an
      unset state; CLI `--confidence` becomes optional (the CLI shows per-assertion confidence
      only for citations, which stay required); update the dioxus SSR tests + fixture builders.
- [x] Tests: an event stored without confidence decodes (serde default) and renders as
      "no judgment"; mechanical ops (`Tagged`, `RestrictionsChanged`) emit no confidence.
- [x] Docs: data-model §8 (confidence optional, what absence means); diagram provenance-substrate
      block (`confidence Confidence?`); review finding 8 status.

### PR-C3 — uniform `Asserted<T>` projections + typed creation stamp

Findings: 6, 7. Depends on: PR-A1/A2 (participation rows final), PR-B1 (`AssertedFact` shape),
PR-C2 (Option confidence). Crates: `vitni-core`, `vitni-app`, `vitni-ui` (+ dioxus).

- [x] Core: replace bespoke `AssertedName`/`AssertedFact`/`AssertedAssociation`/`AssertedPartner`/
      `AssertedChild`/`AssertedFamilyEvent` with generic `Asserted<T>` (the `assertions.rs:38` doc
      comment already promises this); `person.sex` and person participations become
      `Attributed<Asserted<…>>`; add `restrictions_assertion` to the 10 aggregates missing it
      (Person already has it; **Tag is excepted** — it has no assertion chain) so `SetRestrictions`
      is retractable everywhere except Tag; fold-time only — no event payload changes.
- [x] Core (finding 7): replace `CitationState.created_by: Option<String>`/`created_at` with a
      typed fold of the creation event's context (operator `Agent` + `occurred_at`) — already
      fold-derived today (`citation/decide.rs:171`), so this is typing + naming (`asserted_by`
      DTO fields at `dto.rs:322` align), preserving the Human/Software/AiModel distinction.
- [x] App/UI: accessor and DTO adjustments (`CitationRef.asserted_by_kind`, `citation_ref_from_ref`);
      state the §8 exception rule: bibliographic setters (`Source.title/…`, `Citation.page/date`)
      stay bare `Attributed`.
- [x] Tests: sex retraction restores prior state with visible provenance; restrictions retract on
      every aggregate; citation stamp shows agent kind.
- [x] Docs: data-model §8 (uniformity + exception rule, typed stamp); diagram provenance-substrate
      note (bespoke structs gone), Person & Family + Evidence blocks; review findings 6 + 7
      status.

### PR-C4 — Fact/Event rule + `FactType` trim

Finding: 5. Depends on: PR-B1 (Fact shape settled). Crates: `vitni-core`,
`vitni-app`, `plugins/*` (import mapping), `vitni-ui` (fact-type choices).

- [x] Adopt the ADR 0021 rule (default per review): vital/shared-capable types — Birth, Death,
      Baptism, Burial — are asserted as **Events with a Primary participant**; `Fact` is reserved
      for attribute-shaped claims.
- [x] Core: remove the four overlapping `FactType` variants (`enums.rs`); keep `Residence` as a
      Fact (GEDCOM `RESI` attribute) — the Event `Residence` variant remains for imported
      residence *events*; state this split in the ADR.
- [x] Importers: GEDCOM `BIRT`/`DEAT`/`BAPM`/`BURI` already arrive as event structures → confirm
      they emit Events (they do); remove any fact-side fallback for those types.
- [x] UI/CLI: fact-type selects lose the vital variants; person screens surface vital events from
      participations (already do via the events tab).
- [x] Tests: importer emits no vital Facts; a vital `FactType` no longer compiles/parses.
- [x] Docs: data-model §7 (the rule, `FactType` set); diagram enum table; review finding 5 status.

## Phase D — Documentation tail

### PR-D1 — doc-only findings + review closure

Findings: 9, 10. Depends on: PR-A2 (finding 10's semantics presuppose person-owned
participation). Files: `docs/data-model.md`, `docs/data-model-review.md`,
`docs/data-model-diagram.md`.

- [x] data-model §8: the two confidence layers (assertion surety vs citation quality — never
      combined arithmetically; claim surfaces show assertion confidence, citation screens show
      citation confidence) — finding 9.
- [x] data-model §9/§10: `Family.linked_events` is a categorization link (GEDCOM `FAM.MARR`
      mapping); person-owned participations are the authority on who took part — finding 10.
- [x] §16 sentence: `Restriction::Privacy` is a GEDCOM-flagged-ambiguous import artifact, not a
      recommended user choice (review "positive findings" tail).
- [x] Final sweep: every finding 1–10 has a status line; diagram regenerated against post-C3
      state; plan file statuses checked off.

---

## Dependency graph and suggested order

```text
PR-A1 ──> PR-A2 ──> PR-D1
   └────> PR-A3
PR-B1 ──> PR-C4
PR-A1/A2 + PR-B1 + PR-C2 ──> PR-C3
PR-C1, PR-C2: independent
```

Suggested sequence (independent PRs may proceed in parallel):
**B1 → A1 → A2 → A3 → C1 → C2 → C3 → C4 → D1.**
B1 first: smallest sharp payload change, settles `AssertedFact` before the participation series
builds on the same fold pattern.

## Finding → PR matrix

| Finding | PR(s) | ADR |
| --- | --- | --- |
| 1 dual participation | A2 (payload prep in A1) | 0019 |
| 2 citation channels | B1 | 0020 |
| 3 participation substance | A1 + A3 | 0019 |
| 4 child-link granularity | C1 | 0021 |
| 5 Fact/Event rule | C4 | 0021 |
| 6 projection uniformity | C3 | 0021 |
| 7 citation stamp | C3 | 0021 |
| 8 optional confidence | C2 | 0021 |
| 9 two confidence layers (doc) | D1 | — |
| 10 family-event semantics (doc) | D1 | — |

## Verification (plan-level)

- Per PR: the green-gate command list above, plus the PR's own test items.
- After A2: create person → event → participation in the app; retract the participation by its
  (person-side) `AssertionId`; confirm both person and event screens drop the row — the review's
  correction-ambiguity scenario is now unrepresentable.
- After A3: import a GEDCOM fixture with an event-level `ASSO` witness and family-event ages;
  export; re-import; diff — witnesses and ages survive.
- After C2: assert a fact with no confidence; UI shows "no judgment recorded", not Normal.
- After D1: `prek run --files docs/…` clean; all review findings carry a status; diagrams match
  `state.rs`/`view.rs` on main.
