# Norwegian catalogue for the genealogy CLI (ADR 0003), keyed `no` (generic Norwegian).
# Requests for nb-NO/nb and nn-NO/nn resolve here via the macrolanguage fallback chain
# (nb-NO -> nb -> no -> en, nn-NO -> nn -> no -> en); add `nb`/`nn` catalogues later to specialize.
# This catalogue is kept complete against the `en` baseline, enforced by `cargo xtask i18n-check`.

## Command output
created = Opprettet { $id }
updated = Oppdatert { $id }
init-success = Initialiserte arbeidsområde "{ $name }" i { $path }
config-line = Konfigurasjon: { $path }
list-empty = Ingen personer ennå.
summary = { $id }  { $name }  kjønn: { $sex }{ $private }
no-name = (uten navn)
no-value = -
private-tag = { " " }[privat]
error-prefix = feil: { $message }

## Family output
family-list-empty = Ingen familier ennå.
family-summary = { $id }  partnere: { $partners }  barn: { $children }{ $private }
family-none = (ingen)

## Place output
place-list-empty = Ingen steder ennå.
place-summary = { $id }  { $name }  type: { $place_type }  kode: { $code }  koord: { $coords }

## Source output
source-list-empty = Ingen kilder ennå.
source-summary = { $id }  { $title }

## Citation output
citation-list-empty = Ingen sitater ennå.
citation-summary = { $id }  kilde: { $source }  side: { $page }

## Event output
event-list-empty = Ingen hendelser ennå.
event-summary = { $id }  type: { $event_type }  dato: { $date }  sted: { $place }

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

## Sex labels
sex-male = mann
sex-female = kvinne
sex-unknown = ukjent

## Place-type labels
place-type-country = land
place-type-county = fylke
place-type-municipality = kommune
place-type-parish = prestegjeld
place-type-city = by
place-type-town = tettsted
place-type-village = landsby
place-type-farm = gård
place-type-building = bygning

## Event-type labels
event-type-birth = fødsel
event-type-death = død
event-type-marriage = ekteskap
event-type-baptism = dåp
event-type-burial = begravelse
event-type-census = folketelling
event-type-residence = bosted
event-type-immigration = innvandring
event-type-emigration = utvandring

## AppError
err-config = konfigurasjonsfeil: { $detail }
err-workspace = arbeidsområdefeil: { $detail }
err-human-id-taken = human_id "{ $id }" er allerede i bruk
err-person-not-found = ingen person med human_id "{ $id }"
err-place-not-found = ingen sted med human_id "{ $id }"
err-source-not-found = ingen kilde med human_id "{ $id }"
err-citation-not-found = ingen sitat med human_id "{ $id }"
err-event-not-found = ingen hendelse med human_id "{ $id }"

## PersonError (wrapped via AppError::Domain)
err-person-not-exist = person { $id } finnes ikke
err-person-exists = person { $id } finnes allerede
err-empty-name = et navn må ha et fornavn eller et etternavn
err-missing-assertion = påstand { $id } finnes ikke eller er allerede trukket tilbake
err-invalid-date = ugyldig dato: { $detail }
err-merge-conflict = personer { $surviving } og { $merged } kan ikke slås sammen: { $reason }
err-self-association = person { $id } kan ikke knyttes til seg selv

## FamilyError (wrapped via AppError::FamilyDomain)
err-family-not-found = ingen familie med human_id "{ $id }"
err-family-not-exist = familie { $id } finnes ikke
err-family-exists = familie { $id } finnes allerede
err-partner-present = person { $id } er allerede en partner i denne familien
err-partner-absent = person { $id } er ikke en partner i denne familien
err-child-present = person { $id } er allerede et barn i denne familien
err-child-absent = person { $id } er ikke et barn i denne familien

## PlaceError (wrapped via AppError::PlaceDomain)
err-place-not-exist = sted { $id } finnes ikke
err-place-exists = sted { $id } finnes allerede
err-place-empty-name = et stedsnavn kan ikke være tomt
err-place-empty-code = en stedskode kan ikke være tom
err-place-unknown-enclosing = sted viser til ukjent omsluttende sted { $id }

## SourceError (wrapped via AppError::SourceDomain)
err-source-not-exist = kilde { $id } finnes ikke
err-source-exists = kilde { $id } finnes allerede

## CitationError (wrapped via AppError::CitationDomain)
err-citation-not-exist = sitat { $id } finnes ikke
err-citation-exists = sitat { $id } finnes allerede
err-unknown-source = sitat viser til ukjent kilde { $id }

## EventError (wrapped via AppError::EventDomain)
err-event-not-exist = hendelse { $id } finnes ikke
err-event-exists = hendelse { $id } finnes allerede
err-unknown-place = hendelse viser til ukjent sted { $id }

## DbError
err-db-unsupported = ikke støttet: { $detail }
err-db-backend = lagringsfeil: { $detail }
err-db-malformed = ugyldige inndata: { $detail }
