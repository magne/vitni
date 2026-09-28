# Backup and restore (ADR 0041)

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
