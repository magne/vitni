# 41. Workspace backup and restore, with a versioned format

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

A workspace has no backup. `workspace.rs` creates a `backups/` directory and nothing ever writes to
it. The only copy of a researcher's work is the live database. Three things now need a backup:

- **Internal provenance must survive.** ADR 0037's record origins, and ADR 0039's identity decisions,
  are deliberately excluded from GEDCOM and Gramps exports. An export is therefore *not* a backup: a
  round trip through it loses which record every claim came from, every link between personas, and
  every "not the same" decision. The user was explicit that these ids are internal, not exported, and
  must be kept by backups.
- **Upgrades need a path.** The project's stance so far is **no backwards compatibility** (ADR 0018
  §3): workspaces are disposable, event schemas change freely, and old workspaces are recreated rather
  than migrated. That holds while every workspace is test data. It stops holding once a user has
  months of research in one. Recreating a workspace from a GEDCOM export would lose exactly the
  provenance above.
- **The user set the compatibility rule:**
  - until 1.0, a backup must restore across one or a couple of format changes
  - the upgrade code is temporary, and is removed when 1.0 freezes the format
  - after 1.0, every older backup must remain readable

The event log is the complete state: projections are derived and rebuildable (ADR 0009 §5,
ADR 0010 §5). So a backup is the log.

## Decision

1. **A backup is a single archive of the event log.** `vitni backup create [--with-media] <path>`
   writes one `.vitni-backup` file, a zip, containing:
   - `manifest.json`: `format_version`, the writing app version, the workspace name, the event count,
     the creation time, and a checksum per member
   - `events.jsonl`: every stored event row, in store order. Each row keeps the aggregate type and id,
     sequence, event type and version, payload, and metadata, so origins, identity decisions and
     import runs are all included.
   - `workspace.toml`: the manifest, without machine-local paths
   - `media.json`: a media manifest of path, checksum and size. With `--with-media`, the files
     themselves go under `media/`.

   The GUI offers the same through a *Back up…* / *Restore…* entry.

2. **Restore inserts rows and rebuilds.**
   - `vitni backup restore <archive> --new NAME PATH` creates a workspace, inserts every event row
     as stored (no command is re-decided), then rebuilds every projection by replay (ADR 0010 §5).
   - It is engine-neutral, so a SQLite backup restores into Postgres and vice versa.
   - Media is restored from the archive when present, and otherwise verified against `media.json`
     with missing files reported.
   - A restored workspace is identical to the original. The test for it compares projections row for
     row.

3. **The format is versioned, and restore upgrades.**
   - `format_version` is `0.N` before 1.0 and an integer from `1` onward. It is bumped whenever an
     event encoding changes in a way an older archive would not decode, or whenever the archive
     layout changes.
   - Restore runs a chain of pure upgraders, `vN → vN+1`, over the manifest and the JSONL rows
     (`serde_json::Value` in, `Value` out), then inserts the upgraded rows.
   - An upgrader is the ADR 0010 upcaster idea moved to the backup boundary. It runs **once, at
     restore**, and the live store never holds or needs an old shape.
   - **Upgrading the app is therefore backup → install → restore.** Live workspaces stay disposable
     between versions, as ADR 0018 §3 intended. No in-place database migration is introduced.

4. **The compatibility window:**
   - **Before 1.0:**
     - Restore reads the current format and **the two previous format versions**.
     - An older archive is refused with an actionable message that names its format version and the
       last app version able to read it.
     - Pre-1.0 upgraders live in one module, `backup::upgrade::pre_release`, whose header marks it
       temporary.
   - **At 1.0:**
     - The pre-release format in force at the release, `0.K`, is frozen as **v1**, the permanent
       baseline. v1 has the same layout and encoding as `0.K`. The only difference is the manifest's
       version number, so 1.0 reads a `0.K` archive as v1 through a permanent alias. That is a
       version mapping, not an upgrader.
     - The pre-release upgraders (`0.K−2 → 0.K−1 → 0.K`) are deleted, together with their fixtures.
     - An archive in an older pre-release format is refused by 1.0, with a message naming the last
       0.x release. The documented one-time path is to restore it on that release and back up again,
       which yields `0.K` = v1.
     - This is deliberate: the readable-forever promise begins at v1, and pre-1.0 archives get only
       the two-version window.
     - `cargo xtask check` fails if the workspace version is ≥ 1.0.0 while
       `backup::upgrade::pre_release` still exists, so the deletion cannot be forgotten.
     - `docs/release.md` gains the matching checklist line.
   - **After 1.0:** every format from v1 onward, including the `0.K` alias, stays readable forever,
     and every upgrader is permanent.

5. **Golden fixtures are the compatibility guard.**
   - One committed backup archive per supported format version, of invented data, lives under
     `crates/vitni-app/tests/fixtures/backup/v<version>/`, each with its expected projection digest.
     The `v0.K` fixture is kept after 1.0 as the witness for the v1 alias.
   - Every fixture must restore and reach its expected state.
   - The fixture for the current version must contain **every event variant of every aggregate**,
     enforced by a coverage test driven off `for_each_aggregate!`.
   - The effect on changes:
     - An additive `#[serde(default)]` change keeps every fixture green and needs no bump.
     - A breaking change turns a fixture red, which forces a format bump, an upgrader and a new
       fixture.
     - Adding a variant fails the coverage test until the current fixture includes it.
   - A schema snapshot would flag every harmless change. The fixtures flag only what an old archive
     would actually fail on.

6. **This narrows ADR 0018 §3 for one surface.** Workspaces, projections and the WIT contract stay
   unversioned and disposable. **The backup format is the project's single compatibility surface.**
   ADR 0018 §3 is not edited. This ADR supersedes it only as far as backups are concerned.

## Rationale

- **The log is the backup,** because it is the state. Anything else — a database file copy, a GEDCOM
  export — is either engine-specific or lossy. Rows are inserted as stored, not re-decided, so a
  restore cannot change history even where a `decide` rule has since tightened.
- **Upgrading at the boundary keeps the live system simple.** The runtime keeps its "no upcasters"
  freedom, and compatibility cost is paid in one pure, testable place, only when an archive is
  restored.
- **A two-version window before 1.0** matches the user's rule, and keeps the upgrader chain from
  growing while the format is still moving. Deleting it at 1.0, enforced by a check, gives the
  permanent format a clean baseline.
- **Fixtures test the real promise.** The promise is "an archive written by an older version
  restores". The only honest test is restoring such an archive, and requiring every variant in the
  current fixture makes a new event impossible to forget.

## Consequences

### Positive

- A researcher's work, including every internal provenance id, survives upgrades, machine moves and
  engine changes.
- The app has an upgrade path that does not depend on lossy interchange formats.
- Compatibility breaks are caught by CI at the change that causes them, not by a user after release.

### Negative / costs

- Every event-encoding change that is not additive now costs an upgrader and a fixture. This is new
  friction, deliberately.
- Fixtures must be regenerated with invented data only, and kept small. A dedicated fixture builder
  in `xtask` keeps them reproducible.
- A zip dependency for the archive, justified in the implementing change.
- Large workspaces produce large archives. Streaming the JSONL keeps memory flat, but a backup with
  media can be big.

## Out of scope

- **Scheduled or automatic backups,** and backup rotation in `backups/`.
- **Incremental backups.**
- **Encryption** of archives.
- **Partial restore** (one aggregate, one date range).

## References

- ADR 0001, 0004 — the event log as the source of truth.
- ADR 0009 §5, ADR 0010 — derived projections, rebuild by replay, and upcasting (relocated here to
  the backup boundary).
- ADR 0018 §3 — no backwards compatibility, narrowed here for backups.
- ADR 0037, 0039 — the internal provenance and identity decisions a backup must keep.
- `docs/release.md` — the 1.0 checklist line.
