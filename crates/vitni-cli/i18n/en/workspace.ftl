## The warning a command prints on stderr when the workspace holds events it cannot read (ADR 0051)
workspace-unreadable-events = warning: { $count ->
        [one] one event in this workspace could not be read, so its record shows only what came before it.
       *[other] { $count } events in this workspace could not be read, so their records show only what came before them.
    } If the workspace was last used with a newer version of vitni, update vitni; otherwise restore an earlier backup.
# One unreadable event under that warning: its aggregate kind and id, sequence, stored type and the decoder's reason
workspace-unreadable-event = { $kind } event { $id } #{ $sequence } ({ $event }): { $reason }
