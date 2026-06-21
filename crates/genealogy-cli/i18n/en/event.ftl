## Event output
event-list-empty = No events yet.
event-summary = { $id }  type: { $event_type }  date: { $date }  place: { $place }  desc: { $description }  participants: { $participants }

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
err-event-not-found = no event with human_id "{ $id }"

## EventError (wrapped via AppError::EventDomain)
err-event-not-exist = event { $id } does not exist
err-event-exists = event { $id } already exists
err-unknown-place = event references unknown place { $id }
