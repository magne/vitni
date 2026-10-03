# Backup and restore (ADR 0041)

## Command output
backup-created = Backed up { $events } events to { $path }.
backup-media-files = Included { $count } media file(s).
backup-media-missing = Media file not found, so not backed up: { $path }
restore-success = Restored { $events } events into workspace "{ $name }" at { $path } (backup format { $format }).
restore-media-restored = Restored { $count } media file(s).
restore-media-missing = Media file missing after the restore: { $path }
restore-media-mismatched = Media file differs from the one backed up: { $path }
replace-success = Replaced workspace "{ $name }" with { $events } events from the backup (backup format { $format }).
replace-pre-restore = The previous state was backed up first to { $path }.
replace-no-pre-restore = No backup of the previous state was taken: this workspace has the pre-restore backup switched off.
replace-needs-yes = Replacing workspace "{ $name }" discards its { $events } events and every record built from them. Run again with --yes to replace it.
replace-needs-yes-backup = Its current state is backed up into its backups folder first.
replace-needs-yes-no-backup = Its current state is not backed up first: this workspace has the pre-restore backup switched off.

## Errors
err-backup-destination-exists = { $path } already exists; choose a new file name
err-backup-archive = backup file error: { $detail }
err-backup-not-a-backup = not a vitni backup: { $detail }
err-backup-missing-member = the backup is missing { $member }
err-backup-unexpected-member = the backup holds { $member }, which its manifest does not list
err-backup-checksum = { $member } in the backup is damaged: its checksum does not match
err-backup-too-old = backup format { $found } is too old: this version restores { $oldest } and later. Restore it with a vitni release older than { $before }, then back up again.
err-backup-too-new = backup format { $found } is newer than this version's { $current }; upgrade vitni to restore it
err-backup-unknown-format = backup format { $found } was not written by any vitni release
err-backup-invalid-event = the event on line { $line } of the backup is invalid: { $detail }
err-backup-event-count = the backup's manifest records { $expected } events, but it holds { $found }
err-backup-target-not-empty = { $path } is not empty; restore into a new or empty folder
err-backup-database-not-empty = the target database already holds events; restore into an empty database
err-backup-pre-restore = the backup of this workspace to { $path } failed, so nothing was replaced: { $detail }
