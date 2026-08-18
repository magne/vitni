# Ramme-strenger for Dioxus-GUI-en (ADR 0003, ADR 0008 §3). Data-strenger kommer fra vitni-ui.

app-title = Vitni
nav-people = Personer
back = Tilbake
loading = Laster…
not-found = { $id } finnes ikke
run-plugin = Kjør tillegg
plugin-error = tilleggsfeil: { $detail }

select-prompt = Velg en person for å se detaljene.
citation-select-prompt = Velg en kildehenvisning for å se detaljene.
family-select-prompt = Velg en familie for å se detaljene.
event-select-prompt = Velg en hendelse for å se detaljene.
place-select-prompt = Velg et sted for å se detaljene.
source-select-prompt = Velg en kilde for å se detaljene.
repository-select-prompt = Velg et oppbevaringssted for å se detaljene.
record-select-prompt = Velg en post for å se detaljene.

# Master-detalj-listerammeverk (PR3)
list-filter = Filtrer { $entity }…
list-new = Ny
tab-empty = Ingenting her ennå.

# Verktøylinje for listesortering (PR39)
sort-order-title = Endre sorteringsrekkefølge
sort-id-asc = Sortering: ID ↑
sort-id-desc = Sortering: ID ↓
sort-name-asc = Sortering: Navn ↑
sort-name-desc = Sortering: Navn ↓

# Komponentramme (tilgjengelige navn for ikonknapper, hopp-lenke)
skip-to-content = Hopp til innhold
close = Lukk
dismiss = Lukk

# Applikasjonsskall (PR2): rail, topplinje, fanerad, statuslinje, overlegg
brand-title = Vitni
nav-group-entities = Entiteter
nav-group-tools = Verktøy
nav-item-count = { $label }, { $count } oppføringer

nav-dashboard = Oversikt
nav-families = Familier
nav-events = Hendelser
nav-places = Steder
nav-sources = Kilder
nav-citations = Sitater
nav-repositories = Arkiv
nav-media = Medier
nav-notes = Notater
nav-research-notes = Forskningsnotater
nav-tags = Merker
nav-dna-tests = DNA-tester
nav-dna-matches = DNA-treff
nav-pedigree = Anetavle
nav-merge = Sammenlign / slå sammen
nav-import = Importer
nav-export = Eksport
nav-geography = Geografi
nav-plugins = Tillegg
nav-preferences = Innstillinger

aria-primary-nav = Hovednavigasjon
aria-breadcrumb = Brødsmulesti
aria-theme-cycle = Tema: { $mode } (klikk for å endre)
theme-mode-system = System
theme-mode-light = Lyst
theme-mode-dark = Mørkt
aria-help = Hurtigtaster
aria-open-records = Åpne poster
search-label = Søk
search-placeholder = Søk i personer, steder, kilder…
search-clear = Tøm søk
status-theme-system = system ({ $resolved })
status-theme-light = lyst
status-theme-dark = mørkt
new-tab-label = Opprett en ny post
new-record-picker-title = Ny post
draft-tab-label = Ny { $entity }
# Et andre (tredje, …) fortsatt navnløst utkast i samme kategori, slik at to nye poster aldri deler
# tilgjengelig navn. Det første nummereres ikke.
draft-tab-label-nth = Ny { $entity } ({ $ordinal })
close-tab-label = Lukk post
close-tab-named = Lukk { $name }
# En postfane med ulagret arbeid får en prikk-glyf; dette er dens tilgjengelige navn, slik at
# tilstanden også nås av en skjermleser og ikke bare ligger i markørens farge (WCAG 1.4.1).
tab-unsaved-named = { $name } – ulagrede endringer
table-actions = Handlinger
tab-back = Tilbake
tab-forward = Fremover
coming-soon = { $screen } kommer snart.

palette-title = Kommandopalett
palette-placeholder = Skriv en kommando eller søk…
palette-combobox-label = Søk i personer, steder, kilder, eller kjør en kommando
palette-results-label = Søkeresultater og kommandoer
palette-group-commands = Kommandoer
palette-group-recent = Nylig
palette-kind-command = Kommando
palette-kind-recent = Nylig
palette-cmd-create = Opprett { $entity }…
palette-cmd-find-duplicates = Finn duplikater
palette-cmd-open = Åpne { $target }
palette-hint-navigate = naviger
palette-hint-open = åpne
palette-hint-anywhere = { $chord } fra hvor som helst

# Keyboard notices (the shell toast bound to a shortcut outcome)
kbd-nothing-to-undo = Ingenting å angre
kbd-redo-unavailable = Gjenta er ikke tilgjengelig — loggen er kun tilføyelig; utfør endringen på nytt fra Historikk-fanen.
kbd-nothing-to-save = Ingenting å lagre
save-run-incomplete = Noen poster er fortsatt ulagret — programmet forble åpent.
notice-dismiss = Lukk

help-title = Hurtigtaster
help-col-global = Globalt
help-col-goto = Gå til
help-col-within = I skjermbildet

# Beskrivelser av hurtigtaster (radene i ?-overlegget)
sc-command-palette = Kommandopalett
sc-new-record = Ny (kontekstbasert)
sc-save-record = Lagre post
sc-find = Finn / filtrer
sc-undo = Angre
sc-redo = Gjør om
sc-switch-tab = Bytt postfane
sc-dock-tab = Fest postfane
sc-help = Hurtigtasthjelp
sc-close = Lukk / tøm
sc-move-up = Flytt valg opp
sc-move-down = Flytt valg ned
sc-open = Åpne post
sc-prev-record = Forrige post
sc-next-record = Neste post
sc-prev-tab = Forrige fane
sc-next-tab = Neste fane
sc-first-tab = Første fane
sc-last-tab = Siste fane
sc-add-source = Legg til kilde
sc-edit = Rediger
sc-quit = Avslutt
sc-close-tab = Lukk fane

# Bekreftelsesdialoger for lukk-fane / avslutt (⌘W, ⌘Q over ulagret arbeid). Hver har to
# brødtekster: `-body` for et ulagret utkast (ingenting er lagret ennå) og `-body-edits` for en
# påbegynt endring av en post som allerede er lagret — «er ikke lagret ennå» ville vært usant der.
# Begge dialogene tilbyr tre valg: behold arbeidet (lagre), mist det (forkast) eller gå tilbake.
close-tab-confirm-title = Lukk fane?
close-tab-confirm-body = «{ $label }» er ikke lagret ennå. Hvis du lukker denne fanen, forkastes den.
close-tab-confirm-body-edits = «{ $label }» har ulagrede endringer. Hvis du lukker denne fanen, forkastes de.
close-tab-confirm-save = Lagre
close-tab-confirm-discard = Forkast endringer
close-tab-confirm-discard-draft = Forkast utkast
close-tab-confirm-cancel = Avbryt
quit-confirm-title = Avslutte?
quit-confirm-body = En eller flere åpne faner er ikke lagret ennå. Hvis du avslutter, forkastes de.
quit-confirm-body-edits = En eller flere åpne poster har ulagrede endringer. Hvis du avslutter, forkastes de.
quit-confirm-list-intro = Poster med ulagret arbeid:
quit-confirm-save-all = Lagre alle
quit-confirm-discard-all = Forkast alle
quit-confirm-cancel = Avbryt
# Avslutt-dialogens delvis-lagring: «Lagre alle» kjører så snart *én* av postene kan lagres, så de som
# ikke kan lagres merkes i listen og notatet sier at de blir stående åpne i stedet for å gå tapt.
quit-confirm-item-blocked = { $label } — kan ikke lagres ennå
quit-confirm-leaves-open = Lagre alle lagrer postene den kan; resten blir stående åpne og programmet fortsetter.
# Hvorfor lagring ikke er tilgjengelig — vises i brødteksten istedenfor en død knapp.
close-confirm-cannot-save = «{ $label }» mangler påkrevde felter og kan ikke lagres ennå.
close-confirm-nothing-to-save = Ingenting er fylt ut for «{ $label }» ennå, så det er ingenting å lagre.
# Avfest-knappen i topplinjen på den festede ruten (aria-label)
undock-label = Løsne post
media-select-prompt = Velg et medieobjekt for å se detaljene.
note-select-prompt = Velg et notat for å se detaljene.
tag-select-prompt = Velg en etikett for å se detaljene.
dna-test-select-prompt = Velg en DNA-test for å se detaljene.
dna-match-select-prompt = Velg et DNA-treff for å se detaljene.
new-tag-name = Ny etikett

# Innebygd hjelp – rammestrenger (fase 5). Selve artikkelinnholdet ligger i
# vitni-ui-katalogen (løst via Localizer) — dette er kun indeks/liste-etiketter.
nav-help = Hjelp
help-index-label = Hjelpeemner
help-filter = Filtrer hjelp…
help-empty = Ingen hjelpeemner samsvarer.

# Slektstavle-verktøy – rammestrenger (PR 18). Tavlens plassholderhint og slektskaps-
# kalkulatorens resultatsetning ligger i vitni-ui-katalogen (data, løst via Localizer) —
# dette er kun visningsvelgeren og skjemarammen.
pedigree-view-list = Liste
pedigree-view-pedigree = Slektstavle
pedigree-view-descendants = Etterkommere
pedigree-view-relationships = Slektskap
pedigree-view-switcher-label = Slektstavlevisning
pedigree-focus-label = Fokusperson
pedigree-generations-label = Generasjoner
pedigree-show = Vis
pedigree-person-a-label = Person A
pedigree-person-b-label = Person B
pedigree-compute = Beregn slektskap
pedigree-empty-focus = Åpne en person, eller skriv inn personens id nedenfor, for å se slektstavlen.
pedigree-empty-relationship = Skriv inn begge personenes id for å beregne slektskapet.
pedigree-ancestor-tree-label = Anetavle
pedigree-descendant-tree-label = Etterkommertavle
pedigree-unknown-label = Ukjent

# Geografi-verktøy – rammestrenger (ADR 0025): kartflaten, tegneverktøy, tidslinje og leverandørvalg.
geography-provider-label = Leverandør
geography-provider-osm = OpenStreetMap
geography-provider-maplibre = MapLibre-stil
geography-provider-google = Google (API-nøkkel)
geography-provider-switch-error = Kunne ikke bytte kartleverandør: { $detail }
geography-tool-pan = Panorer
geography-tool-point = Slipp / flytt et punkt
geography-tool-polygon = Tegn polygon
geography-finish-polygon = Fullfør polygon
geography-clear-draft = Tøm
geography-drawing-on = Tegner på { $place }
geography-draw-target-none = Ikke noe sted valgt
geography-draw-target-required = Velg et sted i listen først — en tegnet form knyttes til det valgte stedet.
place-map-confirm-point = Bruk dette punktet
geography-tool-fit = Tilpass
geography-zoom-readout = z{ $level }
geography-zoom-aria = Zoomnivå { $level }
geography-zoom-in = Zoom inn
geography-zoom-out = Zoom ut
geography-scale-meters = m
geography-scale-kilometers = km
place-map-fit-title = Zoom til dette stedets geometri
place-map-open-in-geography = Åpne i Geografi ↗
place-map-open-in-geography-title = Åpne dette stedet i Geografi-verktøyet
geography-time-slider-label = Kart per
geography-time-caption = Viser kartet slik det var i { $year }.
geography-unplotted-note = { $count ->
    [one] { $count } sted har ingen geometri per { $year }.
   *[other] { $count } steder har ingen geometri per { $year }.
}
geography-empty-heading = Ingen steder å vise ennå
geography-empty-help = Steder trenger en koordinat eller grense før de vises her — angi én fra stedets Oversikt-fane, eller tegn én direkte på kartet.
geography-create-here = Nytt sted her
geography-edit-geometry = Rediger geometri
geography-map-aria = Kart med { $markers } stedsmarkører og { $events } hendelsesmarkører
geography-rail-label = Steder
geography-rail-note = Alle steder · 📍 avmerket per { $year }
geography-row-no-geometry-as-of = ingen geometri per { $year }
geography-row-no-geometry = ingen geometri
geography-screen-label = Geografi

# Innstillinger-verktøy – rammestrenger (PR 20). Alternativ-etiketter for rene
# konfigurasjons-enumer (temaet har egne etiketter over) ligger her, ikke i vitni-ui-
# datakatalogen — de navngir UI-innstillinger, ikke domenedata (ADR 0003).
prefs-nav-label = Innstillingsseksjoner
prefs-section-identity = Operatøridentitet
prefs-section-appearance = Utseende
prefs-section-locale = Språk og lokalitet
prefs-section-formats = Dato og tall
prefs-section-surety = Sikkerhetsskala
prefs-section-shortcuts = Hurtigtaster
prefs-section-defaults = Arbeidsområdestandarder

prefs-identity-title = Hvem som gjør endringer
prefs-display-name-label = Visningsnavn
prefs-email-label = E-post
prefs-agent-kind-label = Aktørtype
prefs-agent-kind-person = Person
prefs-agent-kind-software = Programvare (kun tillegg)
prefs-operator-id-label = Operatør-id
prefs-software-agent-note = Programvareaktører (import-/eksporttillegg) stemples automatisk — du kan ikke late som du er en.

prefs-theme-title = Tema
prefs-theme-radiogroup-label = Tema
prefs-theme-system-note = «System» følger lys/mørk-innstillingen til operativsystemet. Temaknappen i toppfeltet er en rask overstyring for denne sesjonen.

prefs-locale-title = Grensesnitt og data
prefs-ui-language-label = Grensesnittspråk
prefs-data-locale-label = Datalokalitet
prefs-data-locale-hint = sortering, navnevisning
prefs-follow-system = Systemstandard ({ $tag })
prefs-fallback-chain-label = Løst reservekjede
prefs-fallback-chain-note = En manglende streng faller videre til neste lokalitet, aldri til tomt.
prefs-locale-note = Grensesnittspråket (ADR 0003 / Fluent) og språket en post er skrevet i, er forskjellige — dette angir grensesnittet.

prefs-formats-title = Visningsformater
prefs-date-format-label = Datoformat
prefs-date-format-long = Lang — { $example }
prefs-date-format-medium = Middels — { $example }
prefs-date-format-numeric = Numerisk — { $example }
prefs-date-format-locale-default = Lokalitetsstandard
prefs-number-format-label = Tall / desimal
prefs-number-format-space-comma = { $example } (mellomrom · komma)
prefs-number-format-comma-point = { $example } (komma · punktum)
prefs-number-format-locale-default = Lokalitetsstandard
prefs-live-example-label = Levende eksempel
prefs-formats-note = Slektsforskningsdatoer (ca., før, anslått, intervaller) formateres med samme lokalitet.

prefs-surety-title = Ordlyd for sikkerhetsnivåer
prefs-surety-intro = Gi de fem sikkerhetsnivåene som brukes på hver påstand din egen ordlyd. Selve de fem nivåene er faste (ADR 0027) — dette endrer bare ordene som vises, aldri hvordan en påstands bevis lagres eller hvor mange nivåer det finnes.
prefs-surety-scope-label = Lagre denne ordlyden i
prefs-surety-scope-workspace = Dette arbeidsområdet
prefs-surety-scope-shared = Delt standard (alle arbeidsområder uten egen)
prefs-surety-field-very-low = Svært lav
prefs-surety-field-low = Lav
prefs-surety-field-normal = Normal
prefs-surety-field-high = Høy
prefs-surety-field-very-high = Svært høy
prefs-surety-hint = Tomme felt beholder den innebygde ordlyden. Et utfylt felt vises ordrett i alle lokaliteter — det oversettes ikke, siden det er din egen ordlyd. Å bytte omfang viser ordlyden som er lagret der.
prefs-surety-layers-label = Hvor ordlyden for hvert nivå kommer fra

prefs-shortcuts-title = Tilordne globale hurtigtaster på nytt
prefs-shortcuts-intro = Bare globale hurtigtaster (aktive overalt) kan tilordnes på nytt. Taster innenfor et skjermbilde og g-prefiks-navigasjonstaster er faste. Skriv en tastekombinasjon som mod+shift+alt+tast — mod er ⌘ på macOS, Ctrl ellers.
prefs-shortcuts-default-hint = Standard: { $chord }
prefs-shortcuts-general-errors = Noen overstyringer kunne ikke brukes:

prefs-defaults-title = Hvor en innstillings verdi kommer fra
prefs-defaults-intro = Hver innstilling løses gjennom tre lag. Det første laget som gir en verdi vinner; resten er reserven.
prefs-defaults-worked-example = Eksempel vist: standardtemaet og personens id-format.
prefs-person-id-format-label = Personens id-format
prefs-layer-wins = vinner
prefs-layer-fallback = reserve
prefs-layer-workspace = Arbeidsområde — { $path }
prefs-layer-shared = Delt app — { $path }
prefs-layer-embedded = Innebygd — grunnlinje
prefs-defaults-footnote = App-nivå-standarder fryses ved bruk; arbeidsområdestandarder forblir levende som reserve (ADR 0005/0006).

prefs-workspaces-title = Registrerte arbeidsområder
prefs-workspace-active = Aktivt
prefs-workspace-default = Standard
prefs-workspace-col-name = Navn
prefs-workspace-col-path = Sti
prefs-workspace-col-engine = Motor
prefs-open-workspace = Åpne
prefs-open-workspace-label = Åpne arbeidsområdet { $name }
prefs-make-default = Gjør til standard
prefs-make-default-label = Gjør { $name } til standard arbeidsområde
prefs-workspaces-note = Arbeidsområder refereres til med navn; standarden åpnes når ingen --workspace er oppgitt (ADR 0005). Åpne bytter økten uten å endre standarden; Gjør til standard lagrer uten omstart.
prefs-register-workspace = + Registrer arbeidsområde…
prefs-register-name-label = Navn
prefs-register-path-label = Mappe
prefs-register-path-hint = Valgfritt — bruker app-datamappen som standard.
prefs-register-database-url-label = Database-URL
prefs-register-database-url-hint = Valgfritt — en Postgres-tilkoblingsstreng (f.eks. postgres://host/db). La stå tomt for standard SQLite-motor.
prefs-register-submit = Registrer
prefs-register-cancel = Avbryt
prefs-register-name-required = Et arbeidsområdenavn er påkrevd.

prefs-maintenance-title = Vedlikehold
prefs-maintenance-intro = Bygg alle projeksjoner på nytt fra hendelsesloggen. Projeksjoner er avledet tilstand, ikke selve sannheten, så dette taper aldri data — bruk det etter en endring i vitni-db-skjemaet, eller hvis en projeksjon ser ut til å være ute av synk med loggen.
prefs-rebuild-projections = Bygg projeksjoner på nytt
prefs-rebuild-busy = Bygger på nytt…
prefs-rebuild-confirm-title = Bygg projeksjoner på nytt?
prefs-rebuild-confirm-body = Dette spiller av hele hendelsesloggen på nytt og avleder alle projeksjoner på nytt. Det kan ta en stund på et stort arbeidsområde, og arbeidsområdet er utilgjengelig for andre handlinger til det er ferdig.
prefs-rebuild-confirm-confirm = Bygg på nytt
prefs-rebuild-confirm-cancel = Avbryt
prefs-rebuild-success = Projeksjoner bygget på nytt.

prefs-reset = Tilbakestill til standard
prefs-save = Lagre innstillinger
prefs-saved = Innstillinger lagret.
prefs-save-error = Kunne ikke lagre: { $detail }

# Sammenligne / slå sammen-verktøy – rammestrenger (PR 19). Dublett-begrunnelser, feltetiketter
# og resultatoppsummeringen ligger i vitni-ui-katalogen (data, løst via Localizer) — dette er
# kun tabell-/veiviserrammen.
merge-duplicates-heading = Mulige dubletter
merge-duplicates-count = { $count } kandidatpar
merge-col-record-a = Post A
merge-col-record-b = Post B
merge-col-why = Hvorfor
merge-col-score = Treffscore
merge-score-tooltip = Dublettdetektorens treffscore — ikke den 5-nivåers påstands-sikkerheten
merge-compare = Sammenlign
merge-empty-duplicates = Ingen mulige dubletter funnet.
merge-wizard-heading = Sammenlign & slå sammen — { $a } ⟷ { $b }
merge-survivor-label = overlevende · behold id
merge-persona-label = blir en persona
merge-keep-label = behold
merge-radio-group-label = Hvilken post har for øyeblikket denne verdien
merge-cancel = Avbryt
merge-submit = Slå sammen (reversibelt)
merge-back = Tilbake til dubletter
merge-reason-label = Årsak til sammenslåing
merge-reason-hint = (registreres på sammenslåingshendelsen)

# Tilleggshåndtering – rammestrenger (PR21). Kapabilitets-/rolle-/tillitsetiketter leses fra selve
# komponenten av vitni-plugin-host::discover — dette er kun visningsnavnene.
plugin-manager-title = Installerte tillegg
plugin-manager-note = Tillegg er sandkassede WebAssembly-moduler med tilgang avslått som standard. Et tillegg kan bare gjøre det det har deklarert, og du har godkjent.
plugin-manager-empty = Fant ingen tilleggskomponenter. Kjør `cargo xtask build-plugins`.
plugin-reload = Last inn på nytt fra disk
plugin-col-name = Tillegg
plugin-col-enabled = Aktivert
plugin-col-capabilities = Deklarerte kapabiliteter
plugin-col-trust = Tillit
plugin-enabled-switch = { $plugin } aktivert
plugin-state-on = På
plugin-state-off = Av
plugin-role-bulk-import = Masseimport
plugin-role-bulk-export = Masseeksport
plugin-role-ui-panel = Brukergrensesnittpanel
plugin-role-assisted-import = Assistert import
plugin-role-test-fixture = Testoppsett
plugin-role-unknown = Ukjent rolle
plugin-cap-log = logg
plugin-cap-query = les
plugin-cap-commands = kommandoer
plugin-cap-progress = fremdrift
plugin-cap-import-source = importkilde
plugin-cap-export-sink = eksportmål
plugin-cap-net = nett
plugin-cap-media-store = medielager
plugin-cap-ai = ki
plugin-cap-present = presentasjon
plugin-trust-unsigned = usignert
plugin-trust-note = Fulle tillitsnivåer og signaturverifisering kommer i fase 8; alle tillegg vises kun som usignerte til da.
plugin-host-api-version = host-api { $version }

# Assistert import-veiviser (ADR 0017). Felt- og handlingsetiketter i bekreftelsessteget slås opp i
# programtilleggets egen katalog; dette er veiviserens egen tekst.
import-heading = Assistert import
import-stage-source = Kilde
import-stage-records = Poster
import-stage-confirm = Bekreft
import-stage-save = Lagre skann
import-stage-summary = Oppsummering
import-source-label = Kilde
import-url-label = Post- eller søke-URL
import-url-placeholder = https://www.digitalarkivet.no/…
import-fetch = Hent
import-no-plugins = Ingen tillegg for assistert import er installert. Kjør `cargo xtask build-plugins`.
import-running = Importerer…
import-back = Tilbake
import-start-over = Start på nytt
import-scan-url-label = URL til skannet side
import-scan-url-placeholder = Lim inn Digitalarkivet-skannets URL for å arkivere det
import-records-heading = Poster funnet
import-col-name = Navn
import-col-detail = Detaljer
import-col-status = Status
import-status-pending = Venter
import-status-imported = Importert
import-status-skipped = Hoppet over
import-review = Gjennomgå
import-finish = Fullfør
import-confirm-heading = Bekreft post
import-provenance-heading = Dette blir registrert
import-prov-operator = Operatør
import-prov-source = Kilde
import-prov-repository = Arkiv
import-prov-citation = Sitering
import-prov-external-id = Ekstern id
import-prov-confidence = Sikkerhet
import-software-agent = programvareagent
import-summary-heading = Oppsummering
import-summary-imported = { $count } importert
import-summary-skipped = { $count } hoppet over
import-another = Importer en til
import-cancel = Avbryt
import-save-title = Lagre skann i mediebiblioteket
import-save-choose-category = Velg en kategori
import-save-category = Kategori
import-save-subfolder = Undermappe
import-save-filename = Filnavn
import-save-path-preview = Forhåndsvisning av sti

# Masseeksport-veiviseren (ADR 0013): GUI-motstykket til `vitni export`.
export-heading = Masseeksport
export-stage-destination = Mål
export-stage-running = Kjører
export-stage-summary = Oppsummering
export-destination-heading = Velg hva som skal skrives, og hvor
export-plugin-label = Eksportformat
export-no-plugins = Ingen tillegg for masseeksport er installert. Kjør `cargo xtask build-plugins`.
export-destination-label = Mål
export-destination-placeholder = La stå tom for å skrive til arbeidsområdets exports-mappe
export-destination-preview = Skriver til
export-destination-dir-hint = under tilleggets filnavn
export-run = Eksporter
export-running-heading = Eksporterer…
export-progress-starting = Starter…
export-progress-count = { $processed } av { $total }
export-progress-processed = { $processed } skrevet
export-cancel = Avbryt
export-summary-heading = Eksport fullført
export-summary-records = { $count } oppføringer skrevet
export-summary-destination = Skrevet til
export-another = Eksporter på nytt
export-error-heading = Eksporten mislyktes
export-failed-unknown = Eksporten avsluttet uten å rapportere et resultat.
export-cancelled-heading = Eksporten ble avbrutt
export-cancelled-message = Eksporten stoppet før den var ferdig; en delvis skrevet fil kan ligge igjen.

# Import-verktøyets modusvelger (sak #191): filimport i bulk mot den assisterte nettveiviseren over.
import-tool-heading = Importer
import-mode-label = Hvordan vil du importere?
import-mode-bulk = Filimport i bulk
import-mode-assisted = Assistert nettimport

# Masseimport-veiviseren (sak #191): GUI-motstykket til `vitni import`.
bulk-import-heading = Masseimport
bulk-import-stage-source = Kilde
bulk-import-stage-running = Kjører
bulk-import-stage-summary = Oppsummering
bulk-import-source-heading = Velg hva som skal leses, og hvor det skal importeres
bulk-import-plugin-label = Importformat
bulk-import-no-plugins = Ingen tillegg for masseimport er installert. Kjør `cargo xtask build-plugins`.
bulk-import-source-label = Kildefil
bulk-import-source-placeholder = Sti til filen som skal importeres
bulk-import-source-preview = Leser fra
bulk-import-source-directory-hint = Den stien er en mappe — velg en fil å importere.
bulk-import-target-label = Importer til
bulk-import-target-existing = Et eksisterende arbeidsområde
bulk-import-target-new = Et nytt arbeidsområde
bulk-import-target-workspace-label = Arbeidsområde
bulk-import-run = Importer
bulk-import-running-heading = Importerer…
bulk-import-progress-starting = Starter…
bulk-import-progress-count = { $processed } av { $total }
bulk-import-progress-processed = { $processed } importert
bulk-import-cancel = Avbryt
bulk-import-summary-heading = Import fullført
bulk-import-summary-records = { $count } oppføringer importert
bulk-import-summary-source = Importert fra
bulk-import-another = Importer en til
bulk-import-error-heading = Importen mislyktes
bulk-import-failed-unknown = Importen avsluttet uten å rapportere et resultat.
bulk-import-cancelled-heading = Importen ble avbrutt
bulk-import-cancelled-message = Importen stoppet før den var ferdig.
bulk-import-confirm-title = Importere til { $workspace }?
bulk-import-confirm-body = { $workspace } har allerede { $count ->
    [one] 1 person
   *[other] { $count } personer
} registrert. Import kan legge til duplikater.
bulk-import-confirm-cancel = Avbryt
bulk-import-confirm-run = Importer likevel
bulk-import-target-name-required = Skriv inn et navn for det nye arbeidsområdet.
bulk-import-target-name-taken = Et arbeidsområde med det navnet er allerede registrert.
