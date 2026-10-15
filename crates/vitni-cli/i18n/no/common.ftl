# Norwegian catalogue for the vitni CLI (ADR 0003), keyed `no` (generic Norwegian).
# Requests for nb-NO/nb and nn-NO/nn resolve here via the macrolanguage fallback chain
# (nb-NO -> nb -> no -> en, nn-NO -> nn -> no -> en); add `nb`/`nn` catalogues later to specialize.
# This catalogue is kept complete against the `en` baseline, enforced by `cargo xtask i18n-check`.

## Command output
created = Opprettet { $id }
updated = Oppdatert { $id }
# A `list --offset` page that starts past the last record.
list-page-empty = Ingenting å vise etter de første { $offset }.
init-success = Initialiserte arbeidsområde "{ $name }" i { $path }
config-line = Konfigurasjon: { $path }
rebuild-success = Bygde alle projeksjoner på nytt fra hendelsesloggen.
import-success = Importerte { $count } post(er) med { $plugin }.
# Hvorfor en import ikke erstattet noen verdi (ADR 0029 §3): filen oppga en eksportdato som ikke kan leses,
# vist ordrett som $date. Skrives ut etter import-success.
import-date-unread = { $source } oppgir en eksportdato som ikke kan leses ({ $date }), så denne importen la bare til i postene og erstattet ingen verdi.
export-success = Eksporterte { $count } post(er) til { $path }.
import-confirm = Arbeidsområdet "{ $name }" inneholder allerede { $count } person(er). Importere likevel? [j/N]
import-cancelled = Import avbrutt.
import-progress-writing = skriver poster
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
