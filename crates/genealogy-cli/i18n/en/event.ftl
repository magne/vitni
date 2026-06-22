## Event output
event-list-empty = No events yet.
event-summary = { $id }  type: { $event_type }  date: { $date }  place: { $place }  desc: { $description }  participants: { $participants }

## Event-type labels
event-type-birth = birth
event-type-death = death
event-type-marriage = marriage
event-type-baptism = baptism
event-type-christening = christening
event-type-burial = burial
event-type-cremation = cremation
event-type-census = census
event-type-residence = residence
event-type-immigration = immigration
event-type-emigration = emigration
event-type-adoption = adoption
event-type-confirmation = confirmation
event-type-bar-mitzvah = bar mitzvah
event-type-bas-mitzvah = bas mitzvah
event-type-first-communion = first communion
event-type-graduation = graduation
event-type-naturalization = naturalization
event-type-ordination = ordination
event-type-probate = probate
event-type-retirement = retirement
event-type-will = will
event-type-engagement = engagement
event-type-annulment = annulment
event-type-divorce = divorce
event-type-divorce-filed = divorce filed
event-type-marriage-banns = marriage banns
event-type-marriage-contract = marriage contract
event-type-marriage-license = marriage license
event-type-marriage-settlement = marriage settlement

## AppError
err-event-not-found = no event with human_id "{ $id }"

## EventError (wrapped via AppError::EventDomain)
err-event-not-exist = event { $id } does not exist
err-event-exists = event { $id } already exists
err-unknown-place = event references unknown place { $id }
