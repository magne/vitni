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
citation-summary = { $id }  kilde: { $source }  side: { $page }  dato: { $date }  sikkerhet: { $confidence }

## Note output
note-list-empty = Ingen notater ennå.
note-summary = { $id }  type: { $note_type }  { $text }

## Media output
media-list-empty = Ingen medier ennå.
media-summary = { $id }  sti: { $path }  sjekksum: { $checksum }  attributter: { $attributes }

## Note-type labels
note-type-general = generelt
note-type-research = forskning
note-type-transcript = avskrift
note-type-citation = sitat

## Tag output
tag-list-empty = Ingen merkelapper ennå.
tag-summary = { $id }  { $name }  farge: { $color }  prioritet: { $priority }

## DNA-test output
dna-test-list-empty = Ingen DNA-tester ennå.
dna-test-summary = { $id }  person: { $person }  leverandør: { $provider }  type: { $test_type }  haplogrupper: { $haplogroups }

## DNA-provider labels
dna-provider-ancestry = AncestryDNA
dna-provider-23andme = 23andMe
dna-provider-myheritage = MyHeritage
dna-provider-ftdna = FamilyTreeDNA
dna-provider-gedmatch = GEDmatch
dna-provider-livingdna = Living DNA

## DNA-test-type labels
dna-test-type-autosomal = autosomal
dna-test-type-ydna = Y-DNA
dna-test-type-mtdna = mtDNA
dna-test-type-xdna = X-DNA

## Event output
event-list-empty = Ingen hendelser ennå.
event-summary = { $id }  type: { $event_type }  dato: { $date }  sted: { $place }  beskr: { $description }  deltakere: { $participants }

## Repository output
repository-list-empty = Ingen oppbevaringssteder ennå.
repository-summary = { $id }  { $name }  type: { $repository_type }  adresser: { $addresses }  nettadresser: { $urls }

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

## Confidence labels (data-model §8)
confidence-very-low = svært lav
confidence-low = lav
confidence-normal = normal
confidence-high = høy
confidence-very-high = svært høy

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

## Repository-type labels
repository-type-library = bibliotek
repository-type-archive = arkiv
repository-type-church = kirke
repository-type-cemetery = gravlund
repository-type-museum = museum
repository-type-website = nettsted
repository-type-collection = samling

## AppError
err-config = konfigurasjonsfeil: { $detail }
err-workspace = arbeidsområdefeil: { $detail }
err-human-id-taken = human_id "{ $id }" er allerede i bruk
err-person-not-found = ingen person med human_id "{ $id }"
err-place-not-found = ingen sted med human_id "{ $id }"
err-source-not-found = ingen kilde med human_id "{ $id }"
err-citation-not-found = ingen sitat med human_id "{ $id }"
err-event-not-found = ingen hendelse med human_id "{ $id }"
err-dna-test-not-found = ingen DNA-test med human_id "{ $id }"
err-repository-not-found = ingen oppbevaringssted med human_id "{ $id }"
err-note-not-found = ingen notat med human_id "{ $id }"
err-media-not-found = ingen medium med human_id "{ $id }"
err-tag-not-found = ingen merkelapp med id "{ $id }"

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

## DnaTestError (wrapped via AppError::DnaTestDomain)
err-dna-test-not-exist = DNA-test { $id } finnes ikke
err-dna-test-exists = DNA-test { $id } finnes allerede
err-dna-test-unknown-person = DNA-test viser til ukjent person { $id }

## RepositoryError (wrapped via AppError::RepositoryDomain)
err-repository-not-exist = oppbevaringssted { $id } finnes ikke
err-repository-exists = oppbevaringssted { $id } finnes allerede
err-repository-empty-name = et oppbevaringssteds navn kan ikke være tomt

## NoteError (wrapped via AppError::NoteDomain)
err-note-not-exist = notat { $id } finnes ikke
err-note-exists = notat { $id } finnes allerede

## MediaError (wrapped via AppError::MediaDomain)
err-media-not-exist = medium { $id } finnes ikke
err-media-exists = medium { $id } finnes allerede

## TagError (wrapped via AppError::TagDomain)
err-tag-not-exist = merkelapp { $id } finnes ikke
err-tag-exists = merkelapp { $id } finnes allerede
err-tag-empty-name = en merkelapp kan ikke ha tomt navn

## DbError
err-db-unsupported = ikke støttet: { $detail }
err-db-backend = lagringsfeil: { $detail }
err-db-malformed = ugyldige inndata: { $detail }
