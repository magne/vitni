# Presentasjonslag-strenger (ADR 0003). Renderer-laget eier sin egen ramme-katalog; denne katalogen
# holder verdietiketter, feltetiketter og feiloverflaten visningsmodellene trenger.

# Verdi-plassholdere
no-name = (uten navn)
no-value = -

# Personvernrestriksjoner (GEDCOM v7 RESN — data-model §6)
restriction-confidential = Konfidensiell
restriction-locked = Låst
restriction-privacy = Personvern

# Kjønnsetiketter
sex-male = mann
sex-female = kvinne
sex-unknown = ukjent
sex-intersex = interkjønn

# Feltetiketter
field-id = ID
field-name = Navn
field-given = Fornavn
field-surname = Etternavn
field-sex = Kjønn
field-private = Privat

# Personliste
list-empty = Ingen personer ennå.

# Detaljfaner
tab-overview = Oversikt
tab-names = Navn
tab-facts = Fakta
tab-events = Hendelser
tab-associations = Forbindelser
tab-families = Familier
tab-citations = Kilder
tab-media = Media
tab-notes = Notater
tab-tags = Etiketter
tab-attributes = Attributter
tab-history = Historikk
tab-empty = Ingenting her ennå.
history-placeholder = Endringsloggen kommer i en senere milepæl.
history-empty = Ingen endringer registrert ennå.
history-note = Hver endring er en uforanderlig hendelse som registrerer hvem, når og hvorfor — et revisjonsspor som følger gratis fra den hendelseskildede kjernen. Enhver oppføring kan angres.

# Endringslogg-sammendrag (Historikk-fanen + aktivitetsstrøm) — én frase per hendelsestype
history-person-created = Person opprettet
history-name-asserted = Navn fastslått
history-sex-asserted = Kjønn fastslått
history-fact-asserted = Faktum fastslått
history-participation-asserted = Lagt til i en hendelse
history-association-asserted = Forbindelse fastslått
history-media-attached = Media knyttet
history-note-attached = Notat knyttet
history-citation-added = Kilde knyttet
history-external-id-added = Ekstern ID lagt til
history-tagged = Etikett satt
history-untagged = Etikett fjernet
history-restrictions-changed = Personvernrestriksjoner endret
history-assertion-retracted = Påstand trukket tilbake
history-assertion-superseded = Påstand erstattet
history-persons-merged = Persona slått sammen
history-generic = Registrerte en endring

# Endringslogg-operatørlinje
history-operator-human = { $name } · { $confidence }
history-operator-agent = { $name } ({ $kind })
history-operator-software = programvareagent
history-operator-ai = KI-modell
history-operator-unknown = ukjent operatør
history-undo = Angre: { $what }
history-undo-short = Angre

# Redigeringsetiketter
field-nickname = Kallenavn
field-prefix = Tittel
field-suffix = Etterstavelse
field-name-type = Navnetype
field-fact-type = Faktatype
field-value = Verdi
field-date = Dato
field-place = Sted
field-confidence = Sikkerhet
field-citation = Kilde
field-media = Media
field-note = Notat
field-tag = Etikett
field-association = Person
field-role = Rolle
field-language = Språk
field-source = Kilde
field-surety = Sikkerhet
field-relationship = Forhold
field-page = Side
field-attribute-type = Type
field-evidence = Bevis

# Handlinger
action-save = Lagre
action-cancel = Avbryt
action-saved = Lagret
action-dismiss = Lukk
action-edit = Rediger
action-add-name = Legg til navn
action-add-fact = Legg til faktum
action-add-source = Legg til kilde
action-attach-citation = Knytt kilde
action-attach-media = Knytt media
action-attach-note = Knytt notat
action-add-tag = Legg til etikett
action-remove-tag = Fjern etikett
action-add-association = Legg til forbindelse
action-set-page = Angi side
action-set-date = Angi dato
action-set-confidence = Angi sikkerhet
action-set-evidence = Angi bevisanalyse
action-add-attribute = Legg til attributt
action-compare = Sammenlign

# Vital-sammendrag (detaljhode)
vital-born = f. { $date }
vital-died = d. { $date }

# Oversikt-seksjonsoverskrifter
section-vitals = Vitale fakta
section-family = Nærmeste familie
overview-note = Hvert faktum viser sin sikkerhet og om en kilde støtter det. Fakta uten kilde flagges — bevis-først-signalet som følger hver skjerm.
family-children = Barn

# Bevis-signaler (farge er aldri eneste signal)
no-source = Ingen kilde
source-count = { $count } kilder
provenance-title = Hvorfor vi tror dette

# Bevisnivå — persona-merket (datamodell §7)
evidence-level-persona = Persona
evidence-level-conclusion = Konklusjon

# Kildehenvisninger
citation-list-empty = Ingen kildehenvisninger ennå.

# Bevisanalyse-akser (Evidence Explained — datamodell §7)
evidence-original = Original
evidence-derivative = Avledet
evidence-primary = Førstehånds
evidence-secondary = Annenhånds
evidence-direct = Direkte
evidence-indirect = Indirekte
evidence-negative = Negativt

# Sikkerhetsnivåer (data-model §8)
confidence-very-low = Svært lav
confidence-low = Lav
confidence-normal = Normal
confidence-high = Høy
confidence-very-high = Svært høy

# Faktatyper (INDI-attributter — data-model §7)
fact-birth = Fødsel
fact-death = Død
fact-baptism = Dåp
fact-burial = Begravelse
fact-occupation = Yrke
fact-residence = Bosted
fact-religion = Religion
fact-caste = Kaste
fact-physical-description = Fysisk beskrivelse
fact-education = Utdanning
fact-ethnicity = Etnisitet
fact-national-id = Nasjonalt ID-nummer
fact-nationality = Nasjonalitet
fact-number-of-children = Antall barn
fact-number-of-marriages = Antall ekteskap
fact-property = Eiendom
fact-social-security-number = Personnummer
fact-nobility-title = Adelstittel

# Navnetyper
name-type-birth = Fødenavn
name-type-married = Gift navn
name-type-maiden = Pikenavn
name-type-immigrant = Innvandrernavn
name-type-professional = Profesjonsnavn
name-type-aka = Også kjent som
name-type-religious = Religiøst navn

# Roller (delt av hendelsesdeltakelse og personforbindelser)
role-primary = Hovedperson
role-witness = Vitne
role-officiator = Forrettende
role-clergy = Geistlig
role-father = Far
role-mother = Mor
role-parent = Forelder
role-child = Barn
role-husband = Ektemann
role-wife = Hustru
role-spouse = Ektefelle
role-godparent = Fadder
role-friend = Venn
role-neighbour = Nabo
role-multiple = Flere
role-bride = Brud
role-groom = Brudgom

# Barn–forelder-forhold (data-model §6)
rel-birth = Biologisk
rel-adopted = Adoptert
rel-foster = Fosterbarn
rel-step = Stebarn
rel-sealed = Beseglet
rel-unknown = Ukjent

# Datokvalifikatorer (numerisk gjengivelse; kvalifikatorer lokalisert)
date-before = før { $date }
date-after = etter { $date }
date-about = omkring { $date }
date-from = fra { $date }
date-to = til { $date }
date-range = mellom { $start } og { $end }
date-span = { $start } til { $end }
date-estimated = anslått { $date }
date-calculated = beregnet { $date }

# Instrumentbord
dashboard-title = Arbeidsområde i et øyekast
dashboard-stat-people = Personer
dashboard-people-caption = { $families } familier · { $events } hendelser
dashboard-stat-evidence = Bevishelse
dashboard-stat-evidence-caption = fakta med minst én kilde
dashboard-stat-attention = Trenger oppmerksomhet
dashboard-recent-activity = Nylig aktivitet — hvem endret hva
dashboard-import-batch = { $count } poster importert
dashboard-jump-back = Hopp tilbake
dashboard-data-quality = Datakvalitet
dashboard-no-source-facts = Fakta uten kilde
dashboard-later-milestone = Kommer i en senere milepæl
dashboard-activity-empty = Ingen aktivitet ennå.

# Familie-snitt
tab-children = Barn
family-list-empty = Ingen familier ennå.
family-overview-note = Partnere registreres med nøytrale roller — ingen kjønnet «ektemann/hustru»-antakelse. Hvert familiefaktum viser sikkerheten og om en kilde støtter det.
section-partners = Partnere
section-marriage = Ekteskap
family-children-count = { $count } barn
field-born = Født
field-partner = Partner
field-child = Barn
action-add-partner = Legg til partner
action-add-child = Legg til barn
action-link-event = Knytt familiehendelse

# Hendelsestyper (datamodell §7) — delt av familiehendelser og hendelses-snittet
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

# Hendelse · Sted (PR8)
tab-participants = Deltakere
tab-hierarchy = Hierarki
event-list-empty = Ingen hendelser ennå.
event-overview-note = Datoer er strukturerte, ikke fritekst — modellen beholder presisjon og kalender slik at datoer forblir maskinsammenlignbare. Hver opplysning viser sin sikkerhet og kilde.
place-list-empty = Ingen steder ennå.
place-overview-note = Et sted beholder navnehistorikk og jurisdiksjonskjede over tid, slik at en post løses til det riktige historiske navnet. Opplysninger viser sikkerhet og kilde.
place-names-note = Navn er datert og språkmerket, slik at stedsregisteret gjenspeiler hva et sted het på et gitt tidspunkt.
place-hierarchy-note = Hver omsluttet-av-kobling kan dateres — jurisdiksjoner endres, så kjeden gjelder for et tidsrom, ikke for alltid.
action-add-participant = Legg til deltaker
action-add-enclosing = Legg til omsluttende sted
place-type-country = Land
place-type-county = Fylke
place-type-municipality = Kommune
place-type-parish = Sogn
place-type-city = By
place-type-town = Tettsted
place-type-village = Landsby
place-type-farm = Gård
place-type-building = Bygning

# Feil
error-prefix = feil: { $message }
err-config = konfigurasjonsfeil: { $detail }
err-workspace = arbeidsområdefeil: { $detail }
err-not-found = { $id } finnes ikke
err-domain = ugyldig operasjon
err-plugin = programtilleggsfeil: { $detail }
err-db-unsupported = ikke støttet: { $detail }
err-db-backend = databasefeil: { $detail }
err-db-malformed = ødelagte data: { $detail }
