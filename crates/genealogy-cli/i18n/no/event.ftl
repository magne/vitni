## Event output
event-list-empty = Ingen hendelser ennå.
event-summary = { $id }  type: { $event_type }  dato: { $date }  sted: { $place }  beskr: { $description }  deltakere: { $participants }

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
err-event-not-found = ingen hendelse med human_id "{ $id }"

## EventError (wrapped via AppError::EventDomain)
err-event-not-exist = hendelse { $id } finnes ikke
err-event-exists = hendelse { $id } finnes allerede
err-unknown-place = hendelse viser til ukjent sted { $id }
