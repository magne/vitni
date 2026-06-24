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
tab-history = Historikk
tab-empty = Ingenting her ennå.
history-placeholder = Endringsloggen kommer i en senere milepæl.

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
action-add-association = Legg til forbindelse
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
