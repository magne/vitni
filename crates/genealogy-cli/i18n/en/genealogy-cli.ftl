# Fallback (English) catalogue for the genealogy CLI (ADR 0003).
# Every key must exist here — `fl!()` checks calls against this file at compile time.

## Command output
created = Created { $id }
updated = Updated { $id }
init-success = Initialized workspace "{ $name }" at { $path }
config-line = Config: { $path }
list-empty = No persons yet.
summary = { $id }  { $name }  sex: { $sex }{ $private }
no-name = (no name)
no-value = -
private-tag = { " " }[private]
error-prefix = error: { $message }

## Family output
family-list-empty = No families yet.
family-summary = { $id }  partners: { $partners }  children: { $children }{ $private }
family-none = (none)

## Place output
place-list-empty = No places yet.
place-summary = { $id }  { $name }  type: { $place_type }  code: { $code }  coords: { $coords }

## Source output
source-list-empty = No sources yet.
source-summary = { $id }  { $title }

## Citation output
citation-list-empty = No citations yet.
citation-summary = { $id }  source: { $source }  page: { $page }

## Note output
note-list-empty = No notes yet.
note-summary = { $id }  type: { $note_type }  { $text }

## Media output
media-list-empty = No media yet.
media-summary = { $id }  path: { $path }  checksum: { $checksum }  attributes: { $attributes }

## Note-type labels
note-type-general = general
note-type-research = research
note-type-transcript = transcript
note-type-citation = citation

## Event output
event-list-empty = No events yet.
event-summary = { $id }  type: { $event_type }  date: { $date }  place: { $place }

## Date qualifiers (the calendar date itself is formatted by ICU4X; these wrap it — data-model §7.1)
date-before = before { $date }
date-after = after { $date }
date-about = about { $date }
date-from = from { $date }
date-to = to { $date }
date-range = between { $start } and { $end }
date-span = { $start } to { $end }
date-estimated = estimated { $date }
date-calculated = calculated { $date }

## Sex labels
sex-male = male
sex-female = female
sex-unknown = unknown

## Place-type labels
place-type-country = country
place-type-county = county
place-type-municipality = municipality
place-type-parish = parish
place-type-city = city
place-type-town = town
place-type-village = village
place-type-farm = farm
place-type-building = building

## Event-type labels
event-type-birth = birth
event-type-death = death
event-type-marriage = marriage
event-type-baptism = baptism
event-type-burial = burial
event-type-census = census
event-type-residence = residence
event-type-immigration = immigration
event-type-emigration = emigration

## AppError
err-config = configuration error: { $detail }
err-workspace = workspace error: { $detail }
err-human-id-taken = human_id "{ $id }" is already taken
err-person-not-found = no person with human_id "{ $id }"
err-place-not-found = no place with human_id "{ $id }"
err-source-not-found = no source with human_id "{ $id }"
err-citation-not-found = no citation with human_id "{ $id }"
err-event-not-found = no event with human_id "{ $id }"
err-note-not-found = no note with human_id "{ $id }"
err-media-not-found = no media with human_id "{ $id }"

## PersonError (wrapped via AppError::Domain)
err-person-not-exist = person { $id } does not exist
err-person-exists = person { $id } already exists
err-empty-name = a name must have a given name or a surname
err-missing-assertion = assertion { $id } is not present or already retracted
err-invalid-date = invalid date: { $detail }
err-merge-conflict = persons { $surviving } and { $merged } cannot be merged: { $reason }
err-self-association = person { $id } cannot be associated with itself

## FamilyError (wrapped via AppError::FamilyDomain)
err-family-not-found = no family with human_id "{ $id }"
err-family-not-exist = family { $id } does not exist
err-family-exists = family { $id } already exists
err-partner-present = person { $id } is already a partner of this family
err-partner-absent = person { $id } is not a partner of this family
err-child-present = person { $id } is already a child of this family
err-child-absent = person { $id } is not a child of this family

## PlaceError (wrapped via AppError::PlaceDomain)
err-place-not-exist = place { $id } does not exist
err-place-exists = place { $id } already exists
err-place-empty-name = a place name must not be empty
err-place-empty-code = a place code must not be empty
err-place-unknown-enclosing = place references unknown enclosing place { $id }

## SourceError (wrapped via AppError::SourceDomain)
err-source-not-exist = source { $id } does not exist
err-source-exists = source { $id } already exists

## CitationError (wrapped via AppError::CitationDomain)
err-citation-not-exist = citation { $id } does not exist
err-citation-exists = citation { $id } already exists
err-unknown-source = citation references unknown source { $id }

## EventError (wrapped via AppError::EventDomain)
err-event-not-exist = event { $id } does not exist
err-event-exists = event { $id } already exists
err-unknown-place = event references unknown place { $id }

## NoteError (wrapped via AppError::NoteDomain)
err-note-not-exist = note { $id } does not exist
err-note-exists = note { $id } already exists

## MediaError (wrapped via AppError::MediaDomain)
err-media-not-exist = media { $id } does not exist
err-media-exists = media { $id } already exists

## DbError
err-db-unsupported = unsupported: { $detail }
err-db-backend = storage backend error: { $detail }
err-db-malformed = malformed input: { $detail }
