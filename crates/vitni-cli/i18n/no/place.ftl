## Place output
place-list-empty = Ingen steder ennå.
place-summary = { $id }  { $name }  type: { $place_type }  kode: { $code }  koord: { $coords }{ $restrictions }

## Place-type labels
place-type-country = land
place-type-state = delstat
place-type-county = grevskap
place-type-city = by
place-type-parish = sogn
place-type-locality = lokalitet
place-type-street = gate
place-type-province = provins
place-type-region = region
place-type-department = departement
place-type-neighborhood = nabolag
place-type-district = distrikt
place-type-borough = bydel
place-type-municipality = kommune
place-type-town = tettsted
place-type-village = landsby
place-type-hamlet = grend
place-type-farm = gård
place-type-building = bygning
place-type-number = nummer
place-type-none = ingen type
place-type-kind-key = { $kind } · { $key }

## Place-kind labels
place-kind-administrative = administrativ
place-kind-ecclesiastical = kirkelig
place-kind-judicial = rettslig
place-kind-registration = registrering
place-kind-fiscal = skatt
place-kind-military = militær
place-kind-manorial = gods
place-kind-cadastral = matrikkel
place-kind-statistical = statistisk
place-kind-settlement = bosetning
place-kind-building = bygning
place-kind-natural = natur

## Place-level labels
place-level-country = land
place-level-adm1 = første inndelingsnivå
place-level-adm2 = andre inndelingsnivå
place-level-adm3 = tredje inndelingsnivå
place-level-adm4 = fjerde inndelingsnivå
place-level-adm5 = femte inndelingsnivå
place-level-settlement = bosetning
place-level-sub-settlement = delområde av bosetning

## AppError
err-place-not-found = ingen sted med human_id "{ $id }"

## PlaceError (wrapped via AppError::PlaceDomain)
err-place-not-exist = sted { $id } finnes ikke
err-place-exists = sted { $id } finnes allerede
err-place-empty-name = et stedsnavn kan ikke være tomt
err-place-empty-code = en stedskode kan ikke være tom
err-place-unknown = sted viser til ukjent sted { $id }
err-place-invalid-geometry = en stedsgeometris polygonring må ha minst 3 punkter
err-place-empty-succession-endpoints = en stedsetterfølgelse må navngi minst ett "fra"-sted og ett "til"-sted
err-place-succession-anchor-mismatch = sted { $id } må være ett av etterfølgelsens "fra"-steder
err-place-merge-conflict = stedene { $surviving } og { $merged } kan ikke slås sammen: { $reason }
err-place-distinct-from-itself = sted { $id } kan ikke skilles fra seg selv
err-place-identity-decided = stedene { $place } og { $other } har allerede en gjeldende identitetsavgjørelse; angre den først
err-place-empty-type = en stedstype må oppgi en art, et nivå eller en nøkkel
err-place-text-only-type-date = datoen til en stedstype må være en kalenderdato, ikke fritekst
err-place-type-key-invalid = «{ $key }» er ikke en stedstypenøkkel: skriv <navnerom>:<navn>, for eksempel std:farm, eller custom:<tekst>
err-place-type-date-invalid = «{ $date }» er ikke en dato: skriv ÅÅÅÅ, ÅÅÅÅ-MM eller ÅÅÅÅ-MM-DD
