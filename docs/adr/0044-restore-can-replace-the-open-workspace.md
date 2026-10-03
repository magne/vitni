# 44. A restore can replace the open workspace

- **Status:** Accepted
- **Date:** 2026-10-03

## Context

ADR 0041 §2 restores a backup only into a *new* workspace. That keeps a restore from ever destroying
work, but it leaves two common needs awkward:

- **Going back.** An import or a bulk edit went wrong, and the operator wants the workspace as it was
  at the last backup. Today that means restoring into a second workspace, registering it, switching to
  it, and abandoning the first. The registry name, the directory, and every path that points at it
  change.
- **A server database.** A Postgres workspace's database is provisioned outside vitni. A restore into
  a new workspace needs a second empty database, when the operator only has the one.

Replacing the open workspace discards its event log. The log is the evidence layer (ADR 0001), so
this is the one operation in vitni that destroys history. It needs a guard proportionate to that, and
a way back.

## Decision

1. **Restore gains a replace mode.**
   - `vitni backup restore <archive> --replace --yes` replaces the open workspace. `--replace` and
     `--new NAME PATH` are exclusive, and one of them is required. Without `--yes`, `--replace` is
     refused with a message naming the workspace and how many events it would discard.
   - The Preferences *Backup & restore* card offers *Replace this workspace…*. It opens a danger
     modal that names what is lost and asks the operator to type the workspace's name. The confirm
     button stays disabled until the name matches.

2. **A replace is one transaction.**
   - The archive is checked first, exactly as for a new restore: members, checksums, format window,
     upgrades, and a decode of every row. Nothing is touched until the whole archive is known to be
     restorable.
   - Then the old log is deleted, the archive's rows are inserted as stored (no command is
     re-decided), and every projection is rebuilt by replay. All three run in **one transaction on
     both engines**. A failure at any point leaves the workspace's log and projections as they were.
   - The rebuild goes through repositories bound to a connection pool, not to a transaction. The
     replace therefore runs on a dedicated pool of a single connection that holds the open
     transaction. Nested transactions inside the rebuild become savepoints on it.
   - With one connection, a replay cannot stream: its reader would hold the connection while the
     projection writes wait for it. Each replay therefore reads its aggregate type's whole log before
     writing, and only during a replace.
   - The caller ensures no command runs during the replace, as for a rebuild (ADR 0010 §5).

3. **The safety net is a pre-restore backup, configured per workspace.**
   - Before replacing, vitni writes a backup of the open workspace into its `backups/` directory, named
     `<workspace>-pre-restore-<UTC timestamp>.vitni-backup`. If that backup fails, the replace is
     aborted and nothing has changed.
   - The backup carries the media manifest but not the media files, because a replace never deletes
     or overwrites a media file (§5).
   - `workspace.toml` gains `[backup] pre_restore = false` to switch this off. It is for disposable
     development workspaces, which then keep only the typed confirmation. The default is on, and an
     absent key means on.
   - Restoring the pre-restore backup, by either mode, brings the previous state back.

4. **The settings follow the archive, except what belongs to this copy.** A replace adopts the
   archive's portable manifest the way a new restore does (ADR 0041 §1), including its workspace id
   (ADR 0043 §3). It keeps the open workspace's database URL, window geometry, and `[backup]` setting.
   Whether a workspace is disposable is a property of this copy, not of the tree in the archive.
   Operators are merged, as for a new restore.

5. **Media is added, never removed.** Archived media files are extracted only where no file exists at
   their path. Existing files are left alone, and the usual check against `media.json` reports any
   that are missing or differ.

6. **This supersedes ADR 0041 §2 only in its restriction to a new workspace**, and narrows ADR 0041's
   *Out of scope* item on automatic backups: the pre-restore backup is the one backup vitni writes
   unasked. Scheduled backups and rotation in `backups/` stay out of scope.

## Rationale

- **Typing the name** is the established guard for an irreversible action on a named thing. It cannot
  be confirmed by reflex, and it shows which workspace is about to go. The CLI equivalent is an
  explicit flag, because a scripted restore must not block on a prompt.
- **A pre-restore backup** makes the destructive operation reversible with the tool the operator
  already has. Aborting when it fails keeps that promise unconditional. The per-workspace switch
  keeps disposable workspaces fast without weakening the default.
- **One transaction** means a failed replace is not a half-replaced workspace. The pre-restore backup
  is then a way back from a *successful* replace the operator regrets, not a repair tool. Using one
  mechanism on both engines keeps a single code path and a single test.

## Consequences

- The event log can now lose history, by an explicit, confirmed, and by default backed-up operator
  action. That is the only such path.
- Each replace with the default setting leaves one archive in `backups/`. Nothing rotates them.
- A replace holds one aggregate type's events in memory at a time while it rebuilds. A backup, a
  restore into a new workspace, and an ordinary rebuild still stream.
- A replace holds one write transaction for the whole rebuild. On a large workspace, other
  connections' writes wait for it. The GUI runs the replace as a modal operation and reloads the
  workspace afterwards.

## References

- ADR 0001 (the event log as evidence), ADR 0010 §5 (rebuild by replay), ADR 0041 (backup and
  restore), ADR 0043 (workspace id).
- Issue #426.
