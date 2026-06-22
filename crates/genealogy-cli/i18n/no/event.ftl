## Event output
event-list-empty = Ingen hendelser ennå.
event-summary = { $id }  type: { $event_type }  dato: { $date }  sted: { $place }  beskr: { $description }  deltakere: { $participants }

## Event-type labels
event-type-birth = fødsel
event-type-death = død
event-type-marriage = ekteskap
event-type-baptism = dåp
event-type-christening = navngiving
event-type-burial = begravelse
event-type-cremation = kremasjon
event-type-census = folketelling
event-type-residence = bosted
event-type-immigration = innvandring
event-type-emigration = utvandring
event-type-adoption = adopsjon
event-type-confirmation = konfirmasjon
event-type-bar-mitzvah = bar mitsva
event-type-bas-mitzvah = bat mitsva
event-type-first-communion = første kommunion
event-type-graduation = eksamen
event-type-naturalization = naturalisering
event-type-ordination = ordinasjon
event-type-probate = skifte
event-type-retirement = pensjonering
event-type-will = testamente
event-type-engagement = forlovelse
event-type-annulment = annullering
event-type-divorce = skilsmisse
event-type-divorce-filed = skilsmisse begjært
event-type-marriage-banns = lysing
event-type-marriage-contract = ektepakt
event-type-marriage-license = vigselslisens
event-type-marriage-settlement = ekteskapsavtale

## AppError
err-event-not-found = ingen hendelse med human_id "{ $id }"

## EventError (wrapped via AppError::EventDomain)
err-event-not-exist = hendelse { $id } finnes ikke
err-event-exists = hendelse { $id } finnes allerede
err-unknown-place = hendelse viser til ukjent sted { $id }
