# Research — import merge/sync conflict resolution (Phase 10, gates ADR 0029)

- **Status:** Findings informing ADR 0029 (timestamp-gated reconciliation on re-import).
- **Date:** 2026-07-23

## Question

Today's re-import is additive-only (`docs/archive/phase-4-followups.md`): an identical value is a
no-op, a genuinely new value is added, but a *conflicting* single-valued fact — the file disagrees with
what is stored — is left untouched. This asks how other tools resolve that conflict, and what rule
this workspace's own event-sourced model can support without inventing new machinery.

## Same-vendor "compare and merge": needs a shared identity, not a foreign format

**Gramps' Import Merge Tool** is the closest first-party precedent to a real reconciliation UI, and its
own documentation is explicit about the one thing it depends on:

> "It depends on the two Trees sharing common internal handles (not just the user-editable IDs) to
> simplify ignoring unchanged records... say the other user doesn't use Gramps, so you exported with
> GEDCOM (or another format) and got it back? Sorry... this tool won't be any easier than a manual
> merge after a standard import. You'd still need to hand compare every record."

Gramps' `handle` is an internal, never-exported primary key — round-tripping through *any* foreign
format (GEDCOM, Gramps XML re-imported into a different tree) loses it, and the tool degrades to
"select Merge Original and the mergeable parts merge, the rest stays as your tree already has it" —
i.e. per-field, not per-record, and only ever additive/non-destructive on conflict ("the portions of
the data that cannot be merged will remain as it is in your current tree data"). Gramps never silently
overwrites a conflicting field even with a shared handle.

We have no shared internal handle across formats either, but we do have an equivalent stable key that
survives export/import: `ExternalId` (data-model §11, already resolve-or-create per ADR 0013 §6). This
is the identity axis Gramps' tool needs and a foreign-format round-trip cannot supply on its own.

## Explicit, reasoned overwrite: FamilySearch Family Tree

FamilySearch's "copy a GEDCOM into Family Tree" flow is the one surveyed system that lets a foreign
file's data *overwrite* already-stored conclusion data, and it gates every overwrite behind an explicit
per-decision human action:

> "For each Replace tag... If [it] indicates information that [is more accurate], click Replace... If
> you moved at least one Replace tag... enter a reason statement. Explain why the information from your
> GEDCOM file is more accurate than the information in Family Tree."

Two properties matter: (1) overwrite is **opt-in per field**, never automatic; (2) a **reason** is
mandatory, exactly the shape our own `EventContext.rationale` (data-model §8) already provides for
every assertion. FamilySearch's own guidance to users is conservative — "consider transferring the
information manually, entering only the dates and facts you know are correct" — reflecting that even
with a reason field, an automatic bulk overwrite is treated as risky.

## Simpler desktop tools: named strategies, chosen once or per-record

**Apex Family Tree**'s GEDCOM conflict UI (a modern, actively-documented desktop tool) names three
resolutions applied either globally or per-record: **Skip** (keep existing), **Overwrite** (replace with
incoming), **Merge** (combine, keeping data from both "where possible"). This is a coarser, record-level
choice (not our field-level granularity) but confirms the three-way shape — keep / replace / combine —
recurs across tools independent of data model.

## What "unpredictable, vendor-specific" looks like when there is no fixed rule

A 2025 retrospective on desktop↔online tree sync (GenConverse, a genealogy-software commentary site)
frames the failure mode a fixed rule should avoid:

> "When the same tree is edited in two places... the software must merge changes intelligently. Should
> it prioritize the desktop version? The online version?... Different software makes different
> choices, often with unpredictable results."

The retrospective's own conclusion is that **GEDCOM is insurance, not sync** — a manual export/import
step, never a live merge — which matches this workspace's own posture (import is a deliberate,
user-triggered action, not a background sync daemon). The lesson taken is not "build real-time sync"
but "make the one-shot re-import rule explicit and documented," so re-running an import never surprises
the researcher the way ad hoc vendor sync does.

## The rule this model can support without new machinery

Every single-valued claim in this workspace already carries an `EventContext.occurred_at` (data-model
§8) — the moment a human or a prior import *asserted* it — and is correctable via the universal
`AssertionSuperseded` event (data-model §10) referencing the claim's `AssertionId` (ADR 0004 §2). A
GEDCOM file's `HEAD.1 DATE` (already parsed by `genealogy-gedcom`, currently discarded) is the moment
the *file's* claim was asserted as of. Comparing the two timestamps gives an auditable, mechanical
answer to "which is more current" without a UI decision per field:

- If the workspace's live assertion is *newer* than the file's export date, something (a human, or a
  later import) already superseded what the file knows — leave it alone (today's behaviour).
- If the workspace's live assertion is *no newer* than the file's export date, the file is at least as
  current — supersede automatically, attributed to the Software agent that already attributes every
  import claim, with a rationale naming the source file and its export date (mirroring FamilySearch's
  mandatory reason, generated rather than typed, because the reason — the file is more recent — is
  itself mechanical and auditable).

This is deliberately narrower than any tool surveyed: no interactive per-field prompt (FamilySearch,
Apex FT), no record-level three-way choice (Apex FT), and no dependency on a shared internal handle
(Gramps) — it reuses the timestamp and correction machinery the event-sourced model already has,
scoped to the fields where "conflicting single-valued fact" is unambiguous. An interactive override is
named as future work if the automatic rule proves too coarse in practice.

## References

- Gramps Import Merge Tool — <https://www.gramps-project.org/wiki/index.php/Import_Merge_Tool>.
- FamilySearch, "Copy a GEDCOM file into Family Tree" —
  <https://www.familysearch.org/en/help/helpcenter/article/how-do-i-copy-information-from-my-gedcom-file-into-family-tree>.
- Apex Family Tree, GEDCOM Import/Export conflict resolution —
  <https://github.com/tokendad/Apex-Family-Tree/blob/main/Docs/Guides/GEDCOM-Import-Export.md>.
- GenConverse, "Tree Synchronization: A 13-Year Journey" —
  <https://genconverse.com/blog/family-tree-sync-challenges/>.
- `docs/data-model.md` §8 (`EventContext.occurred_at`/`rationale`), §10 (`AssertionSuperseded`), §11
  (`ExternalId`, re-import); `docs/archive/phase-4-followups.md` ("Future — merge/sync").
- `docs/adr/0013-import-export-contract.md` §6, `docs/adr/0018-round-trip-owner-links-and-host-api-0.8.md`
  — the resolve-or-create/owner-gated import mechanism this reconciliation rule extends.
