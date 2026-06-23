# Norwegian catalogue for the genealogy CLI (ADR 0003), keyed `no` (generic Norwegian).
# Requests for nb-NO/nb and nn-NO/nn resolve here via the macrolanguage fallback chain
# (nb-NO -> nb -> no -> en, nn-NO -> nn -> no -> en); add `nb`/`nn` catalogues later to specialize.
# This catalogue is kept complete against the `en` baseline, enforced by `cargo xtask i18n-check`.

## Command output
created = Opprettet { $id }
updated = Oppdatert { $id }
init-success = Initialiserte arbeidsområde "{ $name }" i { $path }
config-line = Konfigurasjon: { $path }
rebuild-success = Bygde alle projeksjoner på nytt fra hendelsesloggen.
import-success = Importerte { $count } post(er) med { $plugin }.
export-success = Eksporterte { $count } post(er) til { $path }.
import-confirm = Arbeidsområdet "{ $name }" inneholder allerede { $count } person(er). Importere likevel? [j/N]
import-cancelled = Import avbrutt.
error-prefix = feil: { $message }

## Date qualifiers (selve datoen formateres av ICU4X; disse omslutter den — data-model §7.1)
date-before = før { $date }
date-after = etter { $date }
date-about = omkring { $date }
date-from = fra { $date }
date-to = til { $date }
date-range = mellom { $start } og { $end }
date-span = { $start } til { $end }
date-estimated = antatt { $date }
date-calculated = beregnet { $date }

## Privacy restrictions (GEDCOM v7 RESN — data-model §6)
restrictions-tag = { " " }[{ $value }]
restriction-confidential = konfidensiell
restriction-locked = låst
restriction-privacy = personvern

## Confidence labels (data-model §8)
confidence-very-low = svært lav
confidence-low = lav
confidence-normal = normal
confidence-high = høy
confidence-very-high = svært høy

## AppError
err-config = konfigurasjonsfeil: { $detail }
err-workspace = arbeidsområdefeil: { $detail }
err-human-id-taken = human_id "{ $id }" er allerede i bruk
err-plugin = programtilleggsfeil: { $detail }

## DbError
err-db-unsupported = ikke støttet: { $detail }
err-db-backend = lagringsfeil: { $detail }
err-db-malformed = ugyldige inndata: { $detail }
