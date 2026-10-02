# 43. A workspace has an id, and its exports name it

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

ADR 0037 §3 proposes a dataset for a file by its header fingerprint plus shared record keys. In GEDCOM
the fingerprint is `HEAD.SOUR` together with `HEAD.FILE`, and a header without `HEAD.FILE` declares
none. A product name alone is shared by unrelated files, and so are their xrefs (`I1`).

Vitni's own GEDCOM exporter wrote `1 SOUR vitni` and no `HEAD.FILE`. Re-importing a later export of the
same workspace was therefore never proposed the dataset its first export went into, and the operator
had to pick it by hand (#463).

The exporter needs a value that stays the same across every export of one workspace and differs between
workspaces. Nothing in a workspace provided one:

- **The registry name** (`config.toml`) is a local alias. It can be renamed, and two machines can hold
  unrelated trees under the same name.
- **The directory name** can be moved, and every test and many users have a `gen` or `slekt`.
- **`workspace.toml`** recorded settings and operators, but no identity.

## Decision

1. **A workspace has an id.** `workspace.toml` carries a top-level `id`, a UUID v7. `Workspace::init`
   mints it. `Workspace::open` mints and records one for a manifest written before ids existed, the same
   way it records a new operator. Nothing changes it afterwards.

2. **An import never adopts another workspace's id.** The id names the workspace that writes an export,
   not the tree's content. If importing A's export gave B A's id, B's own later exports would carry A's
   fingerprint for a tree that has since diverged. A third workspace importing both would then be
   proposed one dataset for two trees.

3. **A restore keeps the id.** The backup's portable manifest (ADR 0041) carries it, so the restored
   workspace continues the same tree and its exports keep being proposed the same dataset. An archive
   taken before ids existed restores with an id of its own.

4. **Exporters read it through the export sink.** host-api 0.26.0 adds `export-sink.workspace-id`,
   gated by the `export-sink` grant like the rest of that interface. The GEDCOM exporter writes it as
   `HEAD.FILE`, so every vitni export declares the fingerprint `vitni|<id>`.

## Rationale

- **Opaque but stable beats readable but mutable.** The fingerprint only has to match itself. The
  operator never sees it: the import dialogs show the file's own name, which the host records as the
  run's source label.
- **The proposal stays a proposal.** A shared fingerprint still needs shared record keys (ADR 0037
  §3), and the operator confirms. A restored copy that diverges from its original can be told apart at
  that confirm.
- **The export sink already owns "what this export is".** It names the file and holds the destination,
  so the identity of the tree being written belongs beside it, without a new capability.

## Consequences

- Every existing workspace gains an `id` line in `workspace.toml` the next time it is opened.
- Two workspaces restored from one backup share an id. Their exports are proposed each other's dataset
  when the record keys overlap, and the operator decides.
- The Gramps exporter does not use the id yet. Its fingerprint is the researcher (ADR 0037 §3), which
  it does not write, so a vitni Gramps export still declares no fingerprint. `workspace-id` is available
  to it when that is taken up.

## References

- ADR 0005 (the workspace manifest), ADR 0013 (the export sink), ADR 0037 §3 (dataset proposal),
  ADR 0041 (backup and restore).
- Issue #463.
