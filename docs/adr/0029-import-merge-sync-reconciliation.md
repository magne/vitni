# 29. Import merge/sync: timestamp-gated reconciliation

- **Status:** Proposed
- **Date:** 2026-07-23

## Context

Re-import is additive-only today (`docs/archive/phase-4-followups.md` "Future — merge/sync"): an
identical value is a no-op, a genuinely new value is added, but a *conflicting* single-valued fact — the
file disagrees with what the workspace already has — is left untouched. This under-serves the common
workflow of treating another program as the file of record and periodically re-exporting/re-importing:
any correction made there after the first import is silently dropped on every subsequent re-import.

Research (`docs/research/merge-sync-conflict-resolution.md`) surveyed how other tools resolve this:

- **Gramps' Import Merge Tool** only works between two Gramps files sharing internal `handle`s, and is
  explicit that a foreign-format round-trip (GEDCOM, or Gramps XML re-imported elsewhere) loses that
  shared identity — "this tool won't be any easier than a manual merge... you'd still need to hand
  compare every record." Our equivalent stable identity across a foreign-format round-trip is
  `ExternalId` (data-model §11), already resolve-or-create (ADR 0013 §6).
- **FamilySearch Family Tree**'s GEDCOM-copy flow lets a file overwrite stored data only per-field,
  opt-in, and only with a mandatory typed reason — exactly the shape `EventContext.rationale`
  (data-model §8) already gives every assertion.
- **Apex Family Tree** names three record-level resolutions (Skip/Overwrite/Merge); **GenConverse**'s
  retrospective on desktop↔online sync frames unpredictable, vendor-specific conflict policy as the
  failure mode to avoid by fixing one explicit, documented rule instead.

Our own model already has every piece a reconciliation rule needs, without inventing anything new:
every single-valued claim modelled as `Attributed<Asserted<T>>` (or the bibliographic exception's bare
`Attributed<T>` — `Source.title`/`author`/`pub_info`/`abbrev`, `Citation.page`/`date`, ADR 0021 §3)
carries the introducing `AssertionId` and is correctable via the universal `AssertionSuperseded` event
(data-model §10); `EventContext.occurred_at` timestamps every assertion (ADR 0004 §1); and the
resolve-or-create importer already runs at the `genealogy-app` use-case layer (data-model §11, ADR 0013
§6). The one missing input is the **file's own asserted-as-of date** — GEDCOM `HEAD.1 DATE` — which
`genealogy-gedcom` already parses and today discards.

## Decision

1. **The reconciliation rule is a timestamp comparison, reusing existing correction machinery — no new
   event, no new aggregate.** For a re-imported single-valued field whose incoming value differs from
   the workspace's current *live* assertion:
   - If the live assertion's `EventContext.occurred_at` is **after** the file's export date, the file
     is stale — leave the workspace's value untouched (today's behaviour, unchanged).
   - If the live assertion's `occurred_at` is **at or before** the file's export date, the file's claim
     is at least as current — the importer calls the aggregate's existing supersede path with the
     incoming value, attributed to the `Software` agent already used for every import claim (ADR 0007
     §7), with a `rationale` naming the source file and its export date (the reason is generated, not
     typed, because "the file is more recent" is itself mechanical and auditable — mirroring
     FamilySearch's mandatory-reason precedent without requiring user input).

2. **The file's export date is one new input to the bulk-import use-case, resolved once per document,
   not inferred per record.** `genealogy-gedcom` already parses `HEAD.1 DATE`; the Gramps XML
   equivalent is its `<header><created date="…">`. The importer resolves a single
   `file_asserted_at: Option<Timestamp>` before importing any record and threads it through the host
   `commands` capability as one new parameter set once per import session. This is a `host-api@0.19.0`
   → `0.20.0` bump — a documentation label per the established no-upcaster convention (ADR 0018 §3),
   edited in lockstep across `host.wit` and the first-party GEDCOM/Gramps plugins' `with` keys, not a
   compatibility gate.

3. **A missing or unparseable export date is the conservative default: additive-only, unchanged.** If
   `file_asserted_at` cannot be resolved, the importer cannot honestly judge staleness, so it keeps
   today's behaviour rather than guessing — the same "honest about carrying no structure" stance
   `GenealogicalDate::TextOnly` already takes for unparseable dates.

4. **Minimal field coverage for the gating PR: `Person.sex` and the `Source` bibliographic fields
   (`title`/`author`/`pub_info`/`abbrev`).** Both are genuinely singular, last-write fields already
   modelled with an existing supersede path and no list semantics to reason about (data-model §8's own
   "bibliographic exception," ADR 0021 §3's `Person.sex`) — unlike `PersonName` (additive/multi-valued)
   or `Fact` (where "the same fact updated" versus "a new fact of the same type" is itself an open
   matching question the importer does not resolve today). Widening to further single-valued fields
   (`Citation.page`/`date`, further `Fact`s once fact-matching is decided) is additive, mechanical
   follow-up once these two prove the rule end-to-end — not blocked by this ADR, just not built in the
   gating PR (YAGNI).

5. **No interactive conflict UI in this slice.** Reconciliation under rule 1 is fully automatic; the
   audit trail — the new `AssertionSuperseded` event, its Software operator, `occurred_at` = import
   time, and its generated rationale — is the record of *why* a value changed, inspectable exactly like
   every other correction (the person/source history tab). An interactive "which side wins" picker
   (Apex FT's Skip/Overwrite/Merge) is named as a documented follow-up (§Out of scope), built only if
   real re-import usage shows the automatic rule needs an override.

## Rationale

- **Reuses machinery that already exists in full.** `AssertionSuperseded`, `EventContext.occurred_at`,
  and the Software-agent import attribution are all shipped; this ADR adds one comparison and one new
  threaded timestamp, not a new correction concept.
- **Avoids the two failure modes research surfaced.** It does not depend on a shared internal handle
  across a foreign-format round-trip (Gramps' own dead end); it does not leave the "which side wins"
  question to unpredictable, vendor-specific heuristics (GenConverse's retrospective) — the rule is
  one explicit, documented, auditable comparison.
- **The generated rationale is honest, not a shortcut.** "The file's export date is at least as recent
  as what we had" is a true, mechanical reason for every field the rule touches; it is not standing in
  for a human's judgment the way a blank or fabricated rationale would.
- **Scoping the first PR to two field groups keeps the mechanism provable without conflating it with
  the harder, separate question of Fact-identity matching** (is an imported Fact of the same type an
  update to an existing one, or a second, distinct claim?) — a question this ADR deliberately leaves
  open rather than answering under pressure to widen coverage.

## Consequences

### Positive

- A workspace kept in sync with an external, actively-edited file no longer silently loses corrections
  made there after the first import — the common "my other program is the file of record" workflow is
  finally served.
- Every reconciliation is an audited, retractable `AssertionSuperseded` — never a silent overwrite — so
  the correction is visible and reversible exactly like a human-made correction.
- No schema churn: no new event variant, no new aggregate, no change to the twelve aggregates' `decide`
  signatures beyond the app-layer policy that chooses supersede over no-op.

### Negative / costs

- A single per-document `file_asserted_at` is coarser than a record-level change date (a GEDCOM record
  can itself carry a more specific `CHAN` date the importer does not yet consult) — a documented
  follow-up, not solved here.
- The host-api bump touches the shared `genealogy-plugin-api` import plumbing (ADR 0013 §5) and both
  first-party GEDCOM and Gramps plugins in lockstep — small but mandatory mechanical work.
- Two field groups is a narrow first slice; the value of "true merge" is not fully realized until more
  single-valued fields are widened into the rule.

## Out of scope

- **Interactive per-field conflict UI** (Apex FT's Skip/Overwrite/Merge picker) — automatic-only in
  this slice.
- **Per-record change dates** (GEDCOM `CHAN`) — the rule uses one per-document date only.
- **Widening beyond `Person.sex`/`Source` bibliographic fields** to the rest of the single-valued
  catalogue — additive, mechanical follow-up.
- **Multi-valued field merge** (names, facts-as-a-list) — stays additive-only, unchanged by this ADR.
- **Two-way sync** (writing local changes back out automatically) — this ADR is import-direction only;
  export is unaffected.

## References

- ADR 0004 §1 — `EventContext.occurred_at`, the timestamp this rule compares against.
- ADR 0007 §7 — Software-agent attribution, reused for the generated supersede.
- ADR 0013 §5–6 — the `genealogy-plugin-api` shared import plumbing this ADR extends; the resolve-or-
  create/`ExternalId` mechanism this reconciliation rule builds on.
- ADR 0018 §3 — no backwards compatibility (disposable workspaces, first-party plugins): the stance
  that makes the `0.20.0` bump a label, not a compatibility gate.
- ADR 0021 §3 — the uniform `Attributed<Asserted<T>>` shape (and the bibliographic exception) that
  supplies the `AssertionId` a supersede targets.
- `docs/data-model.md` §8 (`EventContext`), §10 (`AssertionSuperseded`), §11 (`ExternalId`, re-import).
- `docs/archive/phase-4-followups.md` — "Future — merge/sync," the deferred item this ADR closes.
- `docs/research/merge-sync-conflict-resolution.md` — the Gramps/FamilySearch/Apex FT/GenConverse
  findings this decision rests on.
