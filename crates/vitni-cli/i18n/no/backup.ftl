# Sikkerhetskopi og gjenoppretting (ADR 0041)

err-backup-destination-exists = { $path } finnes allerede; velg et nytt filnavn
err-backup-archive = feil med sikkerhetskopifilen: { $detail }
err-backup-not-a-backup = ikke en vitni-sikkerhetskopi: { $detail }
err-backup-missing-member = sikkerhetskopien mangler { $member }
err-backup-unexpected-member = sikkerhetskopien inneholder { $member }, som manifestet ikke lister opp
err-backup-checksum = { $member } i sikkerhetskopien er skadet: sjekksummen stemmer ikke
err-backup-too-old = sikkerhetskopiformat { $found } er for gammelt: denne versjonen gjenoppretter { $oldest } og nyere. Gjenopprett den med en vitni-versjon eldre enn { $before }, og ta så en ny sikkerhetskopi.
err-backup-too-new = sikkerhetskopiformat { $found } er nyere enn denne versjonens { $current }; oppgrader vitni for å gjenopprette den
err-backup-unknown-format = sikkerhetskopiformat { $found } er ikke skrevet av noen vitni-versjon
err-backup-invalid-event = hendelsen på linje { $line } i sikkerhetskopien er ugyldig: { $detail }
err-backup-event-count = manifestet i sikkerhetskopien oppgir { $expected } hendelser, men den inneholder { $found }
err-backup-target-not-empty = { $path } er ikke tom; gjenopprett til en ny eller tom mappe
err-backup-database-not-empty = måldatabasen har allerede hendelser; gjenopprett til en tom database
