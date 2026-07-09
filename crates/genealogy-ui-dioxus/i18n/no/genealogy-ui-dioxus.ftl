# Ramme-strenger for Dioxus-GUI-en (ADR 0003, ADR 0008 §3). Data-strenger kommer fra genealogy-ui.

app-title = Slektsforskning
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

# Komponentramme (tilgjengelige navn for ikonknapper, hopp-lenke)
skip-to-content = Hopp til innhold
close = Lukk
dismiss = Lukk

# Applikasjonsskall (PR2): rail, topplinje, fanerad, statuslinje, overlegg
brand-title = Slektsforskning
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
nav-tags = Merker
nav-dna-tests = DNA-tester
nav-dna-matches = DNA-treff
nav-pedigree = Anetavle
nav-merge = Sammenlign / slå sammen
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
close-tab-label = Lukk post
tab-back = Tilbake
tab-forward = Fremover
coming-soon = { $screen } kommer snart.

palette-title = Kommandopalett
palette-placeholder = Skriv en kommando eller søk…
palette-hint = Søk kommer snart.

help-title = Hurtigtaster
help-col-global = Globalt
help-col-goto = Gå til
help-col-within = I skjermbildet

# Beskrivelser av hurtigtaster (radene i ?-overlegget)
sc-command-palette = Kommandopalett
sc-new-record = Ny (kontekstbasert)
sc-find = Finn / filtrer
sc-undo = Angre
sc-redo = Gjør om
sc-switch-tab = Bytt postfane
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
media-select-prompt = Velg et medieobjekt for å se detaljene.
note-select-prompt = Velg et notat for å se detaljene.
tag-select-prompt = Velg en etikett for å se detaljene.
dna-test-select-prompt = Velg en DNA-test for å se detaljene.
dna-match-select-prompt = Velg et DNA-treff for å se detaljene.
new-tag-name = Ny etikett

# Innebygd hjelp – rammestrenger (fase 5). Selve artikkelinnholdet ligger i
# genealogy-ui-katalogen (løst via Localizer) — dette er kun indeks/liste-etiketter.
nav-help = Hjelp
help-index-label = Hjelpeemner
help-filter = Filtrer hjelp…
help-empty = Ingen hjelpeemner samsvarer.

# Slektstavle-verktøy – rammestrenger (PR 18). Tavlens plassholderhint og slektskaps-
# kalkulatorens resultatsetning ligger i genealogy-ui-katalogen (data, løst via Localizer) —
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

# Innstillinger-verktøy – rammestrenger (PR 20). Alternativ-etiketter for rene
# konfigurasjons-enumer (temaet har egne etiketter over) ligger her, ikke i genealogy-ui-
# datakatalogen — de navngir UI-innstillinger, ikke domenedata (ADR 0003).
prefs-nav-label = Innstillingsseksjoner
prefs-section-identity = Operatøridentitet
prefs-section-appearance = Utseende
prefs-section-locale = Språk og lokalitet
prefs-section-formats = Dato og tall
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
prefs-switch-to = Bytt til { $name }
prefs-switch-error = Kunne ikke bytte arbeidsområde: { $detail }

prefs-reset = Tilbakestill til standard
prefs-save = Lagre innstillinger
prefs-saved = Innstillinger lagret.
prefs-save-error = Kunne ikke lagre: { $detail }

# Sammenligne / slå sammen-verktøy – rammestrenger (PR 19). Dublett-begrunnelser, feltetiketter
# og resultatoppsummeringen ligger i genealogy-ui-katalogen (data, løst via Localizer) — dette er
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
# komponenten av genealogy-plugin-host::discover — dette er kun visningsnavnene.
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
plugin-role-test-fixture = Testoppsett
plugin-role-unknown = Ukjent rolle
plugin-cap-log = logg
plugin-cap-query = les
plugin-cap-commands = kommandoer
plugin-cap-progress = fremdrift
plugin-cap-import-source = importkilde
plugin-cap-export-sink = eksportmål
plugin-trust-unsigned = usignert
plugin-trust-note = Fulle tillitsnivåer og signaturverifisering kommer i fase 8; alle tillegg vises kun som usignerte til da.
plugin-host-api-version = host-api { $version }
