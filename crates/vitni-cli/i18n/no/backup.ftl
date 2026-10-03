# Sikkerhetskopi og gjenoppretting (ADR 0041)

## Kommandoutdata
backup-created = Sikkerhetskopierte { $events } hendelser til { $path }.
backup-media-files = Tok med { $count } mediefil(er).
backup-media-missing = Fant ikke mediefilen, så den ble ikke sikkerhetskopiert: { $path }
restore-success = Gjenopprettet { $events } hendelser i arbeidsområdet "{ $name }" i { $path } (sikkerhetskopiformat { $format }).
restore-media-restored = Gjenopprettet { $count } mediefil(er).
restore-media-missing = Mediefilen mangler etter gjenopprettingen: { $path }
restore-media-mismatched = Mediefilen er ulik den som ble sikkerhetskopiert: { $path }
replace-success = Erstattet arbeidsområdet "{ $name }" med { $events } hendelser fra sikkerhetskopien (sikkerhetskopiformat { $format }).
replace-pre-restore = Tilstanden før ble sikkerhetskopiert først til { $path }.
replace-no-pre-restore = Tilstanden før ble ikke sikkerhetskopiert: dette arbeidsområdet har slått av sikkerhetskopien før gjenoppretting.
replace-needs-yes = Å erstatte arbeidsområdet "{ $name }" forkaster de { $events } hendelsene i det og alle oppføringer bygd på dem. Kjør igjen med --yes for å erstatte det.
replace-needs-yes-backup = Tilstanden nå sikkerhetskopieres først til mappen backups i arbeidsområdet.
replace-needs-yes-no-backup = Tilstanden nå sikkerhetskopieres ikke først: dette arbeidsområdet har slått av sikkerhetskopien før gjenoppretting.

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
err-backup-pre-restore = sikkerhetskopien av dette arbeidsområdet til { $path } feilet, så ingenting ble erstattet: { $detail }
