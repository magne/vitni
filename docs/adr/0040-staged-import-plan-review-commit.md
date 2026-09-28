# 40. Staged import: plan, review, commit

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

An importer today is an **imperative command stream**. The guest calls `create-person`,
`create-place`, `assert-fact` … one host call at a time (`host.wit`, `commands`). The host turns each
call into one use-case call that commits immediately (ADR 0013 §2, ADR 0017 §5). This has four
consequences that block the matching work:

- **Nothing can be reviewed before it is written.** By the time the host sees a person, it has
  already created it. A "these 40 people may already exist — review them" step has nothing to act on.
- **Each plugin has to deduplicate for itself.** The GEDCOM, Gramps and Digitalarkivet importers each
  keep their own maps and heuristics: owner-gated creation, title and path lookups through `query`.
  None of them can reach the matching engine, which is AGPL core code a permissive plugin cannot link
  (ADR 0034).
- **A cancelled or failed run leaves a partial import.** There is no plan to resume from.
- **Resolution depends on a plugin's choices.** Whether a re-imported record updates, duplicates or
  is skipped depends on code in three plugins, not on one rule in the host.

The user's requirements are:

- a re-import is a no-op
- a certain match needs no question
- an uncertain match is shown side by side for a decision
- a bulk import can be previewed and uncertain matches deferred
- the whole thing stays unobtrusive when there is nothing to ask

All of that needs the host to see the whole incoming record, or the whole file, *before* committing.

## Decision

1. **Importers submit declarative record graphs.** `host-api@0.24.0` adds a `staging` interface to
   the `bulk-import` and `assisted-import` worlds:
   - `begin-run(dataset-hint, source-label, file-asserted-at) -> run`: opens an `ImportRun`
     (ADR 0037 §5).
   - `submit(graph: record-graph) -> submit-outcome`, where a graph is one source record:
     - `record-graph { record, entities: list<staged-entity>, links: list<staged-link> }`
     - each entity has a local id, a kind, an item key (ADR 0037 §1) and its incoming fields, as
       typed variants per kind covering the fields the `commands` verbs cover today
     - each link joins two local ids: participation (role, age), partner, child, event place,
       enclosure, citation-of, media-of, note-of
   - The guest computes no digest and resolves nothing. The host derives the origin, including the
     digest, from the graph.

   The imperative `create-*` / `assert-*` verbs are removed from the import worlds. They remain in no
   importer's contract, so plugins become **pure parsers**: record in, graph out. Following ADR 0018
   §3 this is a lockstep bump across `host.wit` and the three first-party importers, not a
   compatibility gate.

2. **The host plans each entity before anything is written.** `vitni-app` builds an `ImportPlan` from
   the submitted graphs. Each entity's disposition is one of:

   | Disposition | When | Written at commit |
   | --- | --- | --- |
   | `Unchanged` | same origin, same digest (ADR 0037 §4) | nothing |
   | `Update(fields)` | same origin, changed digest | per-field ADR 0029 reconciliation, honouring tombstones |
   | `Link(target)` | deterministic identity (ADR 0038 §6) | as for a decided `Same` |
   | `Candidates(assessments)` | one or more candidates at `Possible` or above | per the user's decision |
   | `New` | no candidate | a new record |

   Resolution follows the graph:
   - When a person resolves, its relatives in the same graph are reassessed with that as evidence.
     A father confirmed as an existing Ole Hansen raises the child's match to Ole's known son.
   - Two items of the same graph are never matched to each other (ADR 0038 §4, same-record
     conflict).

3. **A decision means different things per kind, following the evidence/conclusion split.**
   - **Person, `Same`:**
     - a new `Persona` is created from the record, carrying the record's claims
     - it is linked to the chosen person with `PersonsMerged` (ADR 0039)
     - the record's evidence stays a separate, unlinkable unit
   - **Other kinds, `Same`:**
     - the existing record is reused and nothing new is created (a second copy of a place or source
       carries no evidence of its own)
     - the resolution is recorded as `ImportRun.ItemResolved` (ADR 0037 §5), so the next run
       resolves deterministically
     - incoming fields that the existing record lacks are added additively, with the origin
   - **`Not the same`:** the entity is created as `New`, and a `<Kind>sDistinguished` decision is
     recorded, so the pair is never proposed again.
   - **`Decide later`:** the entity is created as `New`. The pair then appears in the computed review
     queue (ADR 0039 §3), filtered to this run.
   - **Bulk actions are explicit:** *Treat all 312 Probable place matches as the same*, or *Decide
     the rest later*. They are still the user's decision, recorded per pair, with the assessment each
     was shown.

4. **The flow per world:**
   - **Bulk** (GEDCOM, Gramps): the guest submits every graph and returns, and the host holds the
     plan.
     - The wizard gains a **Plan** stage: counts of new, unchanged, updated and possible matches, by
       kind.
     - It then shows a **Review** stage (the shared compare view) only if there are candidates, and
       finally commits.
     - The CLI gets:
       - `vitni import --plan`: prints the plan (text or `--json`) and writes nothing
       - `--defer-matches`: commits with every candidate as `Decide later`, which is the
         non-interactive default
       - interactive review on a TTY
   - **Assisted** (Digitalarkivet): each `submit` plans one record.
     - If the record has candidates, the host itself inserts a **Match** stage into the wizard.
       This is a host-owned `present` stage the plugin neither sees nor drives, reusing the ADR 0017
       §5 channel.
     - `submit` then returns the committed outcome, or skipped or cancelled, to the guest.
     - With no candidates there is no extra stage at all.

5. **Commit is sequenced, idempotent and resumable.**
   - Commit runs the existing use-cases in dependency order (places, sources and repositories, then
     citations, persons, families, events, links), stamping every assertion with its origin and run.
   - **Creation is atomic per aggregate.** A new aggregate's create command carries its external
     ids, and the origin in its meta, so one `decide` emits them together and cqrs-es commits them in
     one transaction. The create-then-add-key window disappears.
   - **Across aggregates the commit stays sequenced, not atomic** (the existing constraint of
     `person_change_set.rs`). Because every write is keyed by origin, re-running an interrupted
     import resolves everything already written as `Unchanged` and finishes the rest.
   - `ImportRunAbandoned` records an interruption, and History offers *Resume*.

6. **Plans are held in memory for one run.** A bulk plan for a large file is proportional to its
   size. Spilling plans to disk, and resuming a *review* across app restarts, are out of scope. An
   interrupted *commit* is resumable by re-running (§5).

## Rationale

- **The host is the only place that can apply one rule.** Matching, origins, tombstones and the
  deterministic-only rule are host concerns that must be identical for every importer. Moving
  resolution out of the plugins is what makes "re-import is a no-op" a property of the system rather
  than of three plugins.
- **A declarative graph is what makes preview possible.** A plan needs the whole record, relatives
  included, before any write. A command stream cannot provide that without every plugin buffering its
  own output.
- **Personas for people, reuse for everything else,** because only people carry per-record evidence
  worth keeping as a unit. A census person *is* a persona with its own age, role and household. A
  second copy of a parish place is only noise, and the reference to it (the event's place) already
  carries the origin.
- **The host owns the match stage in the assisted wizard.** A plugin that knew about matching would
  need a matching capability, and each would render the decision differently. With the host owning
  it, Digitalarkivet and any future assisted source get the same stage for free.
- **Unobtrusive by construction.** Every extra stage appears only when it has something to ask.

## Consequences

### Positive

- A re-import of an unchanged file or record writes nothing, for every kind.
- Uncertain matches are decided side by side before import, or deferred, and never guessed.
- Importers shrink to parsers and are trivially unit-testable: bytes in, graph out.
- An interrupted import finishes on re-run instead of leaving a partial one.
- `vitni import --plan` gives agents and scripts a dry run with the same resolution the GUI uses.

### Negative / costs

- **A breaking WIT change** and a rewrite of the three importers' output side.
- **A new `ImportPlan` layer** in `vitni-app`, with graph-aware resolution.
- **Memory for bulk plans** scales with the file (§6).
- **Two new wizard stages** (Plan, Review) and a host-owned assisted stage, each with mockups, SSR
  tests and gui-pass scenarios.
- The `present` channel gains host-originated stages, a documented extension of the ADR 0017 §5
  contract.

## Out of scope

- **Persisting a plan across restarts,** or reviewing a bulk import in several sessions. Deferral to
  the review queue covers the need.
- **Export-side staging.** Exports are unchanged apart from cluster folding (ADR 0039 §5).
- **Plugin-supplied match hints.** A plugin reports the data. The host judges identity.

## References

- ADR 0007 §7, ADR 0011 — the plugin host, worlds and Software-agent attribution.
- ADR 0013 — the bulk import/export worlds this changes.
- ADR 0017 §5 — the assisted `present` channel the host-owned match stage reuses.
- ADR 0018 §3 — lockstep WIT bumps, no compatibility gate.
- ADR 0029 — per-field reconciliation on `Update`.
- ADR 0034 — why matching must run host-side.
- ADR 0037 — origins, digests, import runs, `ItemResolved`.
- ADR 0038 — the assessment and the deterministic band.
- ADR 0039 — personas, links, distinct decisions and the review queue.
