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

# Sammenligne / slå sammen-verktøy – rammestrenger (PR 19). Dublett-begrunnelser, feltetiketter
# og resultatoppsummeringen ligger i genealogy-ui-katalogen (data, løst via Localizer) — dette er
# kun tabell-/veiviserrammen.
merge-duplicates-heading = Mulige dubletter
merge-duplicates-count = { $count } kandidatpar
merge-col-record-a = Post A
merge-col-record-b = Post B
merge-col-why = Hvorfor
merge-col-confidence = Sikkerhet
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
