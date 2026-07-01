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
provenance-title-claim = Hvorfor vi tror: { $claim }
provenance-asserted-by = hevdet av { $who } · { $when }
provenance-asserted-by-undated = hevdet av { $who }

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

# Innebygde hjelpeartikler (fase 5). Mellomrom ved grensene mellom innebygde
# tekstbiter bruker { " " }-literalen så Fluents trimming beholder dem.
help-section-overview = Oversikt
help-section-use-case = Veiledninger
help-section-reference = Referanse
help-topic-why-this-app = Hvorfor denne appen
help-label-most = De fleste verktøy
help-label-ours = Denne appen

# Hjelp · «Hvorfor denne appen»-artikkelen (speiler docs/phase5/strengths.html)
help-why-lede-1 = De fleste slektsprogrammer lagrer{ " " }
help-why-lede-conclusions = konklusjoner
help-why-lede-2 = { " " }og overskriver dem stille. Dette programmet lagrer{ " " }
help-why-lede-evidence = bevisene og resonnementet
help-why-lede-3 = { " " }som en kun-tilføy-hendelseslogg, og utleder slektstreet fra den. Resultatet: du kan alltid se hvem som hevdet hva, på hvilket grunnlag, og angre hva som helst. Nedenfor vises hver forskjell med den faktiske komponenten du møter i appen.

help-why-h-audit = Fullstendig revisjonsspor
help-why-audit-most = «Sist endret»-dato, kanskje. Hvem og hvorfor er tapt.
help-why-audit-ours-1 = Hver endring er en uforanderlig hendelse:{ " " }
help-why-audit-ours-bold = hvem · når · hvorfor
help-why-audit-ours-2 = , og reversibel.
help-why-spec-timeline = Tidslinje for historikk — rett fra hendelsesloggen

help-why-h-evidence = Bevis først — sikkerhet overalt
help-why-evidence-most = Et faktum er bare en verdi i en boks. Med eller uten kilde ser det likt ut.
help-why-evidence-ours-1 = Hvert faktum har et sikkerhetsnivå og flagges når det har{ " " }
help-why-evidence-ours-bold = ingen kilde
help-why-evidence-ours-2 = .
help-why-spec-facts = Vitale fakta — sikkerhet + kilde på hver rad

help-why-h-citations = Kildehenvisninger av forskningskvalitet
help-why-citations-most = Et fritekst-«kilde»-felt. Ingen analyse av hvor god den er.
help-why-citations-ours-1 = Hver kildehenvisning vurderes etter de tre{ " " }
help-why-citations-ours-italic = Evidence Explained
help-why-citations-ours-2 = { " " }-aksene.
help-why-spec-evidence = Bevisanalyse-akser — kilde · informasjon · bevis

help-why-h-merge = Ikke-destruktiv, reversibel sammenslåing
help-why-merge-most = Sammenslåing sletter én post. En feil betyr å skrive inn på nytt fra hukommelsen.
help-why-merge-ours = Sammenslåing er en hendelse — felt for felt, med en overlevende, og kan angres.

help-why-h-provenance = Opphav — «hvorfor vi tror dette»
help-why-provenance-most = En dato er bare der. Du kan ikke se hva den hviler på.
help-why-provenance-ours = Ett klikk viser hvert utsagn bak en konklusjon, vurdert og tilskrevet.

help-why-h-plugins = Sandkasse-tillegg og full lokalisering
help-why-plugins-most = Tillegg kjører med full tilgang; grensesnittet er engelsk-først, tillegg oversettes sjelden.
help-why-plugins-ours-1 = Tillegg er nekt-som-standard WASM;{ " " }
help-why-plugins-ours-bold = alt
help-why-plugins-ours-2 = { " " }grensesnitt — inkludert tilleggs-grensesnitt — er lokalisert.

help-why-h-glance = Med ett blikk
help-why-tbl-h-capability = Funksjon
help-why-tbl-h-this = Denne appen
help-why-tbl-h-typical = Typisk verktøy
help-why-tbl-r1-cap = Hvem/når/hvorfor på hver endring
help-why-tbl-r1-this = Innebygd, uforanderlig
help-why-tbl-r1-typ = Sist endret-dato, om noen
help-why-tbl-r2-cap = Sikkerhet per faktum
help-why-tbl-r2-this = 5-nivås sikkerhet
help-why-tbl-r2-typ = Ikke modellert
help-why-tbl-r3-cap = Kildeanalyse
help-why-tbl-r3-this = 3 bevisakser
help-why-tbl-r3-typ = Fritekstfelt
help-why-tbl-r4-cap = Reversibel sammenslåing
help-why-tbl-r4-this = Hendelsesbasert, kan angres
help-why-tbl-r4-typ = Destruktiv
help-why-tbl-r5-cap = Opphavsvisning
help-why-tbl-r5-this = Per konklusjon
help-why-tbl-r5-typ = Ingen
help-why-tbl-r6-cap = Tillegg-sandkasse
help-why-tbl-r6-this = WASM, nekt-som-standard
help-why-tbl-r6-typ = Full tilgang / ingen
help-why-tbl-r7-cap = Lokalisering av tilleggs-grensesnitt
help-why-tbl-r7-this = Samme Fluent-pipeline
help-why-tbl-r7-typ = Kun engelsk

# Hjelp · titler for registreringsveiledningene og ordlisten
help-topic-recording = Registrer forskningen din
help-topic-record-person = Registrere en person
help-topic-record-family = Registrere en familie
help-topic-record-census = Registrere en folketelling
help-topic-record-burial = Registrere en gravlegging
help-topic-personal-knowledge = Kildebelegge egen kunnskap
help-topic-glossary = Ordliste

# Hjelp · innholdsside «Registrer forskningen din»
help-rec-lede = Denne appen registrerer forskning, ikke bare svar. Du fører inn det en kilde sier som et kildebelagt utsagn; slektstreet utledes fra utsagnene. Disse veiledningene går gjennom de vanlige tilfellene — de beskriver postene du oppretter og hvordan hvert utsagn er underbygd av bevis, ikke hvilke knapper du trykker på.
help-rec-h-guides = Veiledninger
help-rec-link-person = Registrere en person
help-rec-desc-person = { " " }— ett individ: navn og vitale datoer, hver underbygd av en kildehenvisning.
help-rec-link-family = Registrere en familie
help-rec-desc-family = { " " }— partnere, en vigselshendelse og barn.
help-rec-link-census = Registrere en folketelling
help-rec-desc-census = { " " }— en husstand på en gård på en dato, fra en skannet og transkribert kilde.
help-rec-link-burial = Registrere en gravlegging
help-rec-desc-burial = { " " }— én linje i en kirkebok som registrerer et dødsfall og en gravlegging.
help-rec-link-personal = Kildebelegge egen kunnskap
help-rec-desc-personal = { " " }— å kildebelegge deg selv for det du vet førstehånds, og hvordan det skiller seg fra et vitne.
help-rec-h-reference = Referanse
help-rec-link-glossary = Ordliste
help-rec-desc-glossary = { " " }— hva utsagn, faktum, hendelse, kildehenvisning, tilknytning og familie betyr her.

# Hjelp · «Registrere en person»
help-person-lede = Begynn med personen, og legg så til det du vet som separate, kildebelagte utsagn — et navn, en fødsel, og (når det gjelder) et dødsfall.
help-person-h-record = Opprett personen og navnet
help-person-p-record = Legg til en ny Person-post, og fastsett så et navn (fornavn og etternavn) på den. Navnet er selv et utsagn, så det har en sikkerhet og kan kildebelegges — nyttig når skrivemåter varierer mellom kilder.
help-person-h-vitals = Fødsel og død
help-person-p-vitals = Registrer fødsel som en Hendelse med dato og Sted, og knytt personen til den; gjør det samme for død når du kjenner den. Datoer og steder er egenskaper ved hendelsen, delt av alle som deltok — ikke fritekstfelt på personen.
help-person-h-source = Underbygg hvert utsagn
help-person-p-source = Hvert utsagn underbygges av en Kildehenvisning til en Kilde. For noe du vet førstehånds er kilden din egen kunnskap — registrer den som det, en original/primær kilde, og sett sikkerheten deretter. Et utsagn uten kilde flagges i stedet for å skjules, så hull forblir synlige.
help-person-spec = Vitale fakta — sikkerhet og kilde på hver rad

# Hjelp · «Registrere en familie»
help-family-lede = En familie knytter partnere sammen og til barna sine. Vigselen og hver relasjon er utsagn du registrerer og kildebelegger.
help-family-h-record = Opprett familien og partnerne
help-family-p-record = Legg til en ny Familie-post og legg til partnerne ved henvisning til Person-postene deres. Partnerroller er nøytrale — familien er foreningen, ikke et fast mann/kone-par.
help-family-h-marriage = Vigselshendelsen
help-family-p-marriage = Registrer vigselen som en Hendelse med dato og Sted, og knytt den til familien. Datoen og stedet bor på hendelsen; familien peker på den.
help-family-h-children = Barn
help-family-p-children = Legg til hvert barn i familien ved henvisning til Person-posten deres. Hver barnekobling er et utsagn i seg selv, så den kan ha en sikkerhet og kildebelegges fra kilden som fastslår relasjonen.
help-family-p-source = Som for en person underbygges hver kobling og dato av en Kildehenvisning, så hele familien kan spores til bevisene bak den.

# Hjelp · «Registrere en folketelling»
help-census-lede = En folketellingspost plasserer en husstand på en gård på en dato og lister hvem som var der, relasjonene deres, alder og yrker. Arkivet gir deg to sider av én kilde — et skannet original og en transkripsjon som ikke alltid er korrekt.
help-census-h-source = Én kilde, to sider
help-census-p-source = Opprett én Kilde for folketellingsposten og en Kildehenvisning som peker på den nøyaktige oppføringen (rull, side, husstand). Skannet er originalen; transkripsjonen er en avledning av den. Kildebelegg det du faktisk leste, og noter hvor transkripsjonen og skannet er uenige.
help-census-h-place = Gården og hendelsen
help-census-p-place = Registrer gården som et Sted og tellingen som en folketellings-Hendelse på det stedet og den datoen. Alle oppførte deltar i samme hendelse.
help-census-h-people = Husstanden
help-census-p-people = Legg til en Person for hvert individ (eller knytt til en eksisterende), med rollen i husstanden. En oppgitt alder gir et utledet fødselsdato-utsagn — indirekte bevis — så registrer det med lavere sikkerhet enn en dato kilden oppgir direkte.
help-census-h-evidence = Å lese bevisene
help-census-p-evidence = Vurder hver kildehenvisning på de tre Evidence Explained-aksene: original (skannet) mot avledet (transkripsjonen); primær mot sekundær informasjon; og direkte mot indirekte bevis — en alder svarer på en fødselsdato bare ved slutning.
help-census-spec = Bevisanalyse-akser — kilde · informasjon · bevis

# Hjelp · «Registrere en gravlegging»
help-burial-lede = En gravleggingsoppføring er én linje i en kirkebok. Den oppgir et dødsfall og en gravlegging, og noen ganger en fødselsdato — hver et utsagn du registrerer fra den ene linjen.
help-burial-h-source = Kirkeboken og linjen
help-burial-p-source = Opprett en Kilde for kirkeboken og en Kildehenvisning som fester den nøyaktige siden og linjen. Den presise henvisningen er det som lar hvem som helst vende tilbake til samme oppføring og kontrollere lesingen din.
help-burial-h-event = Døds- og gravleggingshendelser
help-burial-p-event = Registrer dødsfallet som en Hendelse og gravleggingen som en Hendelse med dato og Sted, og knytt personen til begge. Gravstedet er en egenskap ved gravleggingshendelsen.
help-burial-h-evidence = Hva oppføringen beviser
help-burial-p-evidence = Oppføringen er original, primær og direkte bevis for dødsfallet og gravleggingen den ble skrevet for å registrere. For en fødselsdato er den avledet og sekundær — sett det utsagnet med lavere sikkerhet og let etter en dåps- eller fødselskilde for å bekrefte den.
help-burial-spec = Hvorfor vi tror dette — hvert utsagn vurdert og tilskrevet

# Hjelp · «Kildebelegge egen kunnskap»
help-pk-lede = Noe av det du vet har ingen kilde bak seg — du var der, eller det er din egen familie. Du kan likevel registrere det ordentlig. «Du» kommer inn i posten på to måter: som operatøren som gjorde utsagnet, og som kilden utsagnet hviler på.
help-pk-h-operator = Du er alltid operatøren
help-pk-p-operator = Hvert utsagn du gjør stemples med deg som operatør — hvem som hevdet det, når, og hvorfor — automatisk, fra din konfigurerte identitet. Det revisjonssporet er atskilt fra kilden, og du oppretter det aldri for hånd.
help-pk-h-source = Kildebelegg deg selv
help-pk-p-source = For å underbygge et utsagn med din egen kunnskap, opprett en Kilde som «Egen kunnskap hos <navnet ditt>» med deg selv som forfatter, og deretter en Kildehenvisning inn i den. Vurder kildehenvisningen: en original kilde (din egen kunnskap, ikke en kopi), og direkte bevis (det svarer på spørsmålet direkte). Ingen arkivinstans — det er ikke et arkivert dokument.
help-pk-spec = Bevisanalyse-akser — vurder egen kunnskap som enhver kilde
help-pk-h-firsthand = Førstehånds eller annenhånds?
help-pk-p-firsthand = Hvis du var vitne til det eller det gjelder deg, er informasjonen primær — sett høy sikkerhet. Hvis familien fortalte deg det, er den sekundær (annenhånds eller tradisjon) — sett lavere sikkerhet og registrer hvem som fortalte deg det i kilden. Uansett er utsagnet synlig og vurderbart, ikke en umerket gjetning.
help-pk-h-witness = Et vitne er noe annet
help-pk-p-witness = Å være en kilde er ikke det samme som å være et vitne. Vitne er en rolle du tar på en Hendelse du var til stede ved — legg deg selv til som deltaker med vitnerollen (eller som en vitne-tilknytning mellom personer). Bruk egen-kunnskap-kilden for det du vet; bruk vitnerollen bare når du faktisk var der.

# Hjelp · Ordliste
help-gloss-lede = Noen få ord betyr noe bestemt her. Kjerneidéen: hendelsesloggen er bevislaget (utsagn), og postene du blar i er konklusjonslaget (den nåværende beste syntesen), utledet fra utsagnene.
help-gloss-h-layers = Utsagn og konklusjoner
help-gloss-p-layers = Du redigerer aldri en post direkte. Du gjør utsagn; appen utleder posten. En korreksjon er et nytt utsagn som erstatter et gammelt, så resonnementet beholdes, ikke overskrives.
help-gloss-tbl-h-term = Begrep
help-gloss-tbl-h-meaning = Hva det betyr her
help-gloss-assertion-term = Utsagn
help-gloss-assertion-def = Ett kildebelagt utsagn gjort av en operatør — en oppføring i hendelsesloggen, bevislaget. Bærer en sikkerhet og som regel kildehenvisninger.
help-gloss-fact-term = Faktum
help-gloss-fact-def = En egenskap vist på en post (en fødselsdato, et yrke) — en konklusjon appen utleder fra ett eller flere utsagn.
help-gloss-event-term = Hendelse
help-gloss-event-def = Noe som skjedde med dato og sted, som personer deltar i — fødsel, vigsel, folketelling, gravlegging. En post i seg selv, ikke et felt på en person.
help-gloss-citation-term = Kildehenvisning
help-gloss-citation-def = En henvisning inn i en Kilde (med side eller henvisning, sikkerhet og bevisanalyse). Den er det som underbygger et utsagn.
help-gloss-h-relations = Hendelser, tilknytninger og familier
help-gloss-p-relations = Tre måter personer kobles på. Bruk den som passer typen kobling du registrerer.
help-gloss-rel-event-term = Hendelse
help-gloss-rel-event-def = Knytter personer til et tidspunkt og sted — en delt hendelse de hver deltok i.
help-gloss-association-term = Tilknytning
help-gloss-association-def = En direkte person-til-person-relasjon som ikke er et familiebånd — fadder, vitne, nabo — med en rolle.
help-gloss-family-term = Familie
help-gloss-family-def = Posten som knytter partnere og barna deres. Bruk den for husstanden; bruk en tilknytning for andre person-til-person-koblinger.

# Slektstavle-verktøy (PR 18) — ane-/etterkommertavler + slektskapskalkulatoren
pedigree-unknown-father-of = far til { $name }
pedigree-unknown-mother-of = mor til { $name }
pedigree-father-unresearched = far (linje ikke undersøkt)
pedigree-mother-unresearched = mor (linje ikke undersøkt)
pedigree-focus = Fokus: { $name } · { $generations } generasjoner
kinship-not-found = Ingen kjent relasjon funnet innen de gjennomsøkte generasjonene.
kinship-same = { $a } — samme person.
kinship-a-is-b-term = { $a } er { $b } sin { $term }.
kinship-a-and-b-are = { $a } og { $b } er { $term }.
kinship-full-sibling = helsøsken
kinship-half-sibling = halvsøsken
kinship-parent = forelder
kinship-grandparent = bestemor/bestefar
kinship-great-grandparent = tippbestemor/tippbestefar
kinship-great-n-grandparent = { $n }× tippbestemor/tippbestefar
kinship-child = barn
kinship-grandchild = barnebarn
kinship-great-grandchild = tippbarnebarn
kinship-great-n-grandchild = { $n }× tippbarnebarn
kinship-aunt-uncle = tante/onkel
kinship-great-aunt-uncle = tantes/onkels forelder (tante/onkel én gang forskjøvet)
kinship-great-n-aunt-uncle = { $n }× tante/onkel i oppadstigende linje
kinship-niece-nephew = niese/nevø
kinship-great-niece-nephew = niese/nevø én gang forskjøvet
kinship-great-n-niece-nephew = { $n }× niese/nevø i nedadstigende linje
kinship-cousins-removed = { $cousins }, { $removed }
cousin-first = søskenbarn
cousin-second = tremenninger
cousin-third = firmenninger
cousin-nth = { $n }.-menninger
removed-once = ett ledd forskjøvet
removed-twice = to ledd forskjøvet
removed-n-times = { $n }× forskjøvet
