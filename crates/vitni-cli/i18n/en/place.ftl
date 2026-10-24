## Place output
place-list-empty = No places yet.
place-summary = { $id }  { $name }  type: { $place_type }  code: { $code }  coords: { $coords }{ $restrictions }

## Place-type labels
place-type-country = country
place-type-state = state
place-type-county = county
place-type-city = city
place-type-parish = parish
place-type-locality = locality
place-type-street = street
place-type-province = province
place-type-region = region
place-type-department = department
place-type-neighborhood = neighborhood
place-type-district = district
place-type-borough = borough
place-type-municipality = municipality
place-type-town = town
place-type-village = village
place-type-hamlet = hamlet
place-type-farm = farm
place-type-building = building
place-type-number = number
place-type-none = no type
place-type-kind-key = { $kind } · { $key }

## Place-kind labels
place-kind-administrative = administrative
place-kind-ecclesiastical = ecclesiastical
place-kind-judicial = judicial
place-kind-registration = registration
place-kind-fiscal = fiscal
place-kind-military = military
place-kind-manorial = manorial
place-kind-cadastral = cadastral
place-kind-statistical = statistical
place-kind-settlement = settlement
place-kind-building = building
place-kind-natural = natural

## Place-level labels
place-level-country = country
place-level-adm1 = first-level division
place-level-adm2 = second-level division
place-level-adm3 = third-level division
place-level-adm4 = fourth-level division
place-level-adm5 = fifth-level division
place-level-settlement = settlement
place-level-sub-settlement = sub-settlement

## AppError
err-place-not-found = no place with human_id "{ $id }"

## PlaceError (wrapped via AppError::PlaceDomain)
err-place-not-exist = place { $id } does not exist
err-place-exists = place { $id } already exists
err-place-empty-name = a place name must not be empty
err-place-empty-code = a place code must not be empty
err-place-unknown = place references unknown place { $id }
err-place-invalid-geometry = a place geometry polygon ring must have at least 3 points
err-place-empty-succession-endpoints = a place succession must name at least one "from" and one "to" place
err-place-succession-anchor-mismatch = place { $id } must be one of the succession's "from" places
err-place-merge-conflict = places { $surviving } and { $merged } cannot be merged: { $reason }
err-place-distinct-from-itself = place { $id } cannot be distinguished from itself
err-place-identity-decided = places { $place } and { $other } already have a live identity decision; undo it first
err-place-empty-type = a place type must state a kind, a level or a key
err-place-text-only-type-date = a place type's date must be a calendar date, not free text
err-place-type-key-invalid = "{ $key }" is not a place type key: write <namespace>:<name>, such as std:farm, or custom:<text>
