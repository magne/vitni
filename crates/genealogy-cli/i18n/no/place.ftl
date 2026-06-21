## Place output
place-list-empty = Ingen steder ennå.
place-summary = { $id }  { $name }  type: { $place_type }  kode: { $code }  koord: { $coords }

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

## AppError
err-place-not-found = ingen sted med human_id "{ $id }"

## PlaceError (wrapped via AppError::PlaceDomain)
err-place-not-exist = sted { $id } finnes ikke
err-place-exists = sted { $id } finnes allerede
err-place-empty-name = et stedsnavn kan ikke være tomt
err-place-empty-code = en stedskode kan ikke være tom
err-place-unknown-enclosing = sted viser til ukjent omsluttende sted { $id }
