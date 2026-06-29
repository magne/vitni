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
field-year = År
field-month = Måned
field-day = Dag
field-code = Kode
field-web-path = Nettadresse
field-coordinates = Koordinater
field-latitude = Breddegrad
field-longitude = Lengdegrad
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
history-fact-asserted-kind = { $fact } fastslått
history-citation-created = Kildehenvisning opprettet
history-page-set = Side satt
history-date-asserted = Dato fastslått
history-confidence-set = Sikkerhet satt
history-evidence-analysis-set = Kildeanalyse satt
history-attribute-added = Attributt lagt til
history-dna-match-observed = DNA-treff registrert
history-segment-added = Segment lagt til
history-shared-ancestor-asserted = Felles ane fastslått
history-match-confirmed = Treff bekreftet
history-match-rejected = Treff avvist
history-dna-test-created = DNA-test opprettet
history-provider-set = Leverandør satt
history-kit-id-set = Kit-ID satt
history-test-type-set = Testtype satt
history-genome-build-set = Genomversjon satt
history-haplogroup-asserted = Haplogruppe fastslått
history-family-created = Familie opprettet
history-partner-added = Partner lagt til
history-partner-removed = Partner fjernet
history-child-added = Barn lagt til
history-child-removed = Barn fjernet
history-family-event-linked = Familiehendelse koblet
history-media-created = Media opprettet
history-path-set = Filbane satt
history-checksum-set = Kontrollsum satt
history-mime-set = MIME-type satt
history-note-created = Notat opprettet
history-note-type-set = Notattype satt
history-rich-text-set = Tekst satt
history-place-created = Sted opprettet
history-place-type-set = Stedstype satt
history-enclosed-by-asserted = Overordnet sted fastslått
history-coordinates-asserted = Koordinater fastslått
history-code-set = Kode satt
history-repository-created = Arkiv opprettet
history-repository-type-set = Arkivtype satt
history-name-set = Navn satt
history-address-added = Adresse lagt til
history-url-added = URL lagt til
history-source-created = Kilde opprettet
history-title-set = Tittel satt
history-author-set = Forfatter satt
history-pub-info-set = Publiseringsinfo satt
history-abbrev-set = Forkortelse satt
history-repository-linked = Arkiv koblet
history-tag-created = Etikett opprettet
history-tag-renamed = Etikett omdøpt
history-tag-color-set = Etikettfarge satt
history-tag-priority-set = Etikettprioritet satt
history-event-created = Hendelse opprettet
history-event-type-set = Hendelsestype satt
history-description-set = Beskrivelse satt
history-place-linked = Sted koblet
history-participant-role-added = Deltakerrolle lagt til
history-participant-role-removed = Deltakerrolle fjernet
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

# Source · Repository slices (Phase 5 PR9)
source-list-empty = Ingen kilder ennå.
source-overview-note = En kilde er hovedposten; enkeltsiteringer peker inn i den med side og en bevisanalyse. Postene som bruker den, gjør proveniens sporbar i begge retninger.
source-citations-note = Siteringer som bruker denne kilden — følg hver inn i posten den underbygger.
repository-list-empty = Ingen oppbevaringssteder ennå.
repository-overview-note = Et oppbevaringssted er det fysiske eller virtuelle stedet som rommer kilder. Kilder her lenker tilbake til sitt opprinnelige arkiv — proveniens du kan følge fra et faktum helt til hyllen.

# Source · Repository detail tabs
tab-repositories = Oppbevaringssteder
tab-sources = Kilder
tab-addresses = Adresser
tab-urls = Nettadresser

# Source · Repository overview sections
section-bibliographic = Bibliografisk
section-reliability = Pålitelighet
section-repository = Oppbevaringssted
section-contact = Hovedkontakt

# Source · Repository field labels
field-title = Tittel
field-author = Forfatter
field-publication = Publikasjon
field-abbreviation = Forkortelse
field-call-number = Hyllesignatur
field-media-type = Medietype
field-used-by = Brukt av
field-typical-surety = Typisk sikkerhet
field-type = Type
field-street = Gate
field-locality = Sted
field-region = Region
field-postal-code = Postnummer
field-country = Land
field-phone = Telefon
field-email = E-post
field-url = Lenke
field-description = Beskrivelse
field-backs-record = Underbygger post
field-sources = Kilder
field-citations = Siteringer

# Source · Repository actions
action-link-repository = Lenk oppbevaringssted
action-add-address = Legg til adresse
action-add-url = Legg til nettadresse
action-link-source = Lenk kilde

# Repository types
repository-type-library = Bibliotek
repository-type-archive = Arkiv
repository-type-church = Kirke
repository-type-cemetery = Gravlund
repository-type-museum = Museum
repository-type-website = Nettsted
repository-type-collection = Samling

# Source media types (GEDCOM MEDI)
media-type-book = Bok
media-type-card = Kort
media-type-electronic = Elektronisk
media-type-fiche = Mikrofilmkort
media-type-film = Film
media-type-magazine = Magasin
media-type-manuscript = Manuskript
media-type-map = Kart
media-type-newspaper = Avis
media-type-photo = Foto
media-type-tombstone = Gravstein
media-type-video = Video
media-type-audio = Lyd

# Backs-record sub-context (Source citations tab)
citing-name = Navn
citing-partner = Partner
citing-child = Barn
citing-family-event = Familiehendelse
citing-place-type = Stedstype

# Media · Note slices (PR 10)
tab-content = Innhold
tab-language = Språk
tab-references = Referanser
note-type-general = Generelt
note-type-research = Forskning
note-type-transcript = Avskrift
note-type-citation = Sitat
using-kind-person = Person
using-kind-family = Familie
using-kind-event = Hendelse
using-kind-place = Sted
using-kind-source = Kilde
using-kind-citation = Sitering
using-kind-repository = Oppbevaringssted
reference-count = { $count } referanser
field-file-path = Filsti
field-mime = MIME
field-checksum = Kontrollsum
field-translator = Oversetter
field-translation = Oversatt tekst
field-object = Objekt
action-add-translation = Legg til oversettelse
section-file = Fil
section-content = Innhold
section-primary-language = Primærspråk
section-related-media = Relaterte medier
media-preview = Forhåndsvisning
media-list-empty = Ingen medier ennå.
note-list-empty = Ingen notater ennå.
media-used-by-note = Hver post som bruker dette medieobjektet er listet her — bakreferansene den hendelsesbaserte kjernen holder gratis.
note-references-note = Hva som refererer til dette notatet. Notater deles — ett forskningsnotat kan informere en person, en familie og en hendelse samtidig.
note-content-note = Notater har en type og rik tekst — arbeidsloggen bak en konklusjon, som revisjonssporet holder knyttet til faktaene den informerte.

# Tag · DnaTest · DnaMatch slices (PR 11)
using-kind-media = Medie
using-kind-note = Notat
using-kind-dna-test = DNA-test
using-kind-dna-match = DNA-treff

# Detail tabs (PR 11)
tab-usage = Bruk
tab-haplogroups = Haplogrupper
tab-matches = Treff
tab-segments = Segmenter
tab-ancestors = Felles aner

# List empty states
tag-list-empty = Ingen etiketter ennå.
dna-test-list-empty = Ingen DNA-tester ennå.
dna-match-list-empty = Ingen DNA-treff ennå.

# Section notes
tag-overview-note = Etiketter er tverrgående merkelapper med en farge og en prioritet. Prioritet ordner hvordan etiketter stables på en post; fargen styrer prikken som vises overalt hvor etiketten brukes.
tag-usage-note = Alt som bærer denne etiketten, gruppert etter objekttype. Antallene kommer rett fra projeksjonen.
dna-test-overview-note = Den reviderbare DNA-testposten — kitt-metadata, haplogrupper og treffene den produserer. Rike DNA-visualiseringer kommer i en senere fase; her holder vi bevisene.
dna-test-ethnicity-note = Opphavs-/etnisitetsprosenter er en visualisering i en senere fase. Det underliggende estimatet lagres som en sitert påstand så det kan revideres og erstattes.
dna-match-overview-note = De delte DNA-tallene er en rå observasjon rapportert av leverandøren. Den utledede relasjonen er en separat, sitert påstand med egen sikkerhet som kan erstattes uten å røre observasjonen — bevis/konklusjon-modellen anvendt på DNA.
dna-match-segments-note = Matchende segmenter som rapportert. Mors-/farssiden fases der et foreldrekitt er tilgjengelig.
dna-match-ancestors-note = Felles aner utledet fra de koblede trærne til begge testere. Disse er konklusjoner, hver uavhengig sitert.

# Overview section headings (PR 11)
section-tag = Etikett
section-color = Farge
section-kit = Kitt-detaljer
section-tested-person = Testet person
section-ethnicity = Etnisitetsestimat
section-compared-tests = Sammenlignede tester
section-shared-dna = Delt DNA
section-inferred-relationship = Utledet relasjon

# Field labels (PR 11)
field-priority = Prioritet
field-color = Farge
field-provider = Leverandør
field-test-type = Testtype
field-kit-id = Kitt-id
field-genome-build = Genombygg
field-person = Person
field-haplogroup = Haplogruppe
field-lineage = Linje
field-terminal-snp = Terminal-SNP
field-shared-cm = Delte cM
field-percent-shared = Prosent delt
field-largest-segment = Største segment
field-segment-count = Antall segmenter
field-predicted = Forutsagt
field-status = Status
field-compared-test = Sammenlignet test
field-test-a = Test A
field-test-b = Test B
field-ancestor = Ane
field-chromosome = Kr
field-start = Start (bp)
field-end = Slutt (bp)
field-centimorgans = cM
field-snps = SNP-er
field-side = Side
field-object-type = Objekttype
field-count = Antall
field-examples = Eksempler

# Actions (PR 11)
action-add-haplogroup = Legg til haplogruppe
action-set-name = Sett navn
action-set-priority = Sett prioritet
action-set-color = Sett farge
action-confirm = Bekreft
action-reject = Avvis

# DNA providers (data-model §7, §12)
dna-provider-ancestry = AncestryDNA
dna-provider-23andme = 23andMe
dna-provider-myheritage = MyHeritage
dna-provider-ftdna = FamilyTreeDNA
dna-provider-gedmatch = GEDmatch
dna-provider-livingdna = Living DNA

# DNA test types
dna-test-type-autosomal = Autosomal
dna-test-type-ydna = Y-DNA
dna-test-type-mtdna = mtDNA
dna-test-type-xdna = X-DNA

# DNA genome builds
dna-genome-build-37 = GRCh37
dna-genome-build-38 = GRCh38

# Chromosome side (segment phasing)
chromosome-side-maternal = mors
chromosome-side-paternal = fars
chromosome-side-unknown = ufaset

# DNA match status
match-status-confirmed = Bekreftet
match-status-rejected = Avvist
match-status-undecided = Uavklart
