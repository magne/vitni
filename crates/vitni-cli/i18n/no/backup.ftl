# Sikkerhetskopi og gjenoppretting (ADR 0041)

## Kommandoutdata
backup-created = Sikkerhetskopierte { $events } hendelser til { $path }.
backup-media-files = Tok med { $count } mediefil(er).
backup-media-missing = Fant ikke mediefilen, så den ble ikke sikkerhetskopiert: { $path }
restore-success = Gjenopprettet { $events } hendelser i arbeidsområdet "{ $name }" i { $path } (sikkerhetskopiformat { $format }).
restore-media-restored = Gjenopprettet { $count } mediefil(er).
restore-media-missing = Mediefilen mangler etter gjenopprettingen: { $path }
restore-media-mismatched = Mediefilen er ulik den som ble sikkerhetskopiert: { $path }

## Feil
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
