## The warning a command prints on stderr when the workspace holds events it cannot read (ADR 0051)
workspace-unreadable-events = advarsel: { $count ->
        [one] én hendelse i dette arbeidsområdet kunne ikke leses, så posten viser bare det som kom før den.
       *[other] { $count } hendelser i dette arbeidsområdet kunne ikke leses, så postene viser bare det som kom før dem.
    } Hvis arbeidsområdet sist ble brukt med en nyere versjon av vitni, oppdater vitni; ellers gjenopprett en tidligere sikkerhetskopi.
# One unreadable event under that warning: its aggregate kind and id, sequence, stored type and the decoder's reason
workspace-unreadable-event = { $kind }-hendelse { $id } nr. { $sequence } ({ $event }): { $reason }
