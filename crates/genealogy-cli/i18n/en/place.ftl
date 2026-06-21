## Place output
place-list-empty = No places yet.
place-summary = { $id }  { $name }  type: { $place_type }  code: { $code }  coords: { $coords }

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

## AppError
err-place-not-found = no place with human_id "{ $id }"

## PlaceError (wrapped via AppError::PlaceDomain)
err-place-not-exist = place { $id } does not exist
err-place-exists = place { $id } already exists
err-place-empty-name = a place name must not be empty
err-place-empty-code = a place code must not be empty
err-place-unknown-enclosing = place references unknown enclosing place { $id }
