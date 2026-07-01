# Chrome strings for the Dioxus GUI (ADR 0003, ADR 0008 §3). Data strings come from genealogy-ui.

app-title = Genealogy
nav-people = People
back = Back
loading = Loading…
not-found = { $id } not found
run-plugin = Run plugin
plugin-error = plugin error: { $detail }

select-prompt = Select a person to view their details.
citation-select-prompt = Select a citation to view its details.
family-select-prompt = Select a family to view its details.
event-select-prompt = Select an event to view its details.
place-select-prompt = Select a place to view its details.
source-select-prompt = Select a source to view its details.
repository-select-prompt = Select a repository to view its details.
record-select-prompt = Select a record to view its details.

# Master-detail list framework (PR3)
list-filter = Filter { $entity }…
list-new = New
tab-empty = Nothing here yet.

# Component chrome (icon-button accessible names, skip link)
skip-to-content = Skip to content
close = Close
dismiss = Dismiss

# App shell (PR2): rail, top bar, tabstrip, status bar, overlays
brand-title = Genealogy
nav-group-entities = Entities
nav-group-tools = Tools
nav-item-count = { $label }, { $count } records

nav-dashboard = Dashboard
nav-families = Families
nav-events = Events
nav-places = Places
nav-sources = Sources
nav-citations = Citations
nav-repositories = Repositories
nav-media = Media
nav-notes = Notes
nav-tags = Tags
nav-dna-tests = DNA tests
nav-dna-matches = DNA matches
nav-pedigree = Pedigree
nav-merge = Compare / merge
nav-plugins = Plugins
nav-preferences = Preferences

aria-primary-nav = Primary
aria-breadcrumb = Breadcrumb
aria-theme-cycle = Theme: { $mode } (click to change)
theme-mode-system = System
theme-mode-light = Light
theme-mode-dark = Dark
aria-help = Keyboard shortcuts
aria-open-records = Open records
search-label = Search
search-placeholder = Search people, places, sources…
search-clear = Clear search
status-theme-system = system ({ $resolved })
status-theme-light = light
status-theme-dark = dark
new-tab-label = Create a new record
close-tab-label = Close record
tab-back = Back
tab-forward = Forward
coming-soon = { $screen } is coming soon.

palette-title = Command palette
palette-placeholder = Type a command or search…
palette-hint = Search is coming soon.

help-title = Keyboard shortcuts
help-col-global = Global
help-col-goto = Go to
help-col-within = Within a screen

# Shortcut descriptions (the ? overlay rows)
sc-command-palette = Command palette
sc-new-record = New (context-aware)
sc-find = Find / filter
sc-undo = Undo
sc-redo = Redo
sc-switch-tab = Switch record tab
sc-help = Shortcut help
sc-close = Close / clear
sc-move-up = Move selection up
sc-move-down = Move selection down
sc-open = Open record
sc-prev-record = Previous record
sc-next-record = Next record
sc-prev-tab = Previous tab
sc-next-tab = Next tab
sc-first-tab = First tab
sc-last-tab = Last tab
sc-add-source = Add source
sc-edit = Edit
media-select-prompt = Select a media object to view its details.
note-select-prompt = Select a note to view its details.
tag-select-prompt = Select a tag to view its details.
dna-test-select-prompt = Select a DNA test to view its details.
dna-match-select-prompt = Select a DNA match to view its details.
new-tag-name = New tag

# In-app help browser chrome (Phase 5). The article *content* lives in the
# genealogy-ui catalogue (resolved via Localizer) — these are the renderer's
# index/list labels only.
nav-help = Help
help-index-label = Help topics
help-filter = Filter help…
help-empty = No matching help topics.

# Pedigree tool chrome (PR 18). The chart's placeholder hints and the kinship-calculator's result
# sentence live in the genealogy-ui catalogue (data, resolved via Localizer) — these are the
# renderer's view-switcher and form chrome only.
pedigree-view-list = List
pedigree-view-pedigree = Pedigree
pedigree-view-descendants = Descendants
pedigree-view-relationships = Relationships
pedigree-view-switcher-label = Pedigree view
pedigree-focus-label = Focus person
pedigree-generations-label = Generations
pedigree-show = Show
pedigree-person-a-label = Person A
pedigree-person-b-label = Person B
pedigree-compute = Compute relationship
pedigree-empty-focus = Open a person, or enter their id below, to see their pedigree.
pedigree-empty-relationship = Enter both people's ids to compute their relationship.
pedigree-ancestor-tree-label = Ancestor chart
pedigree-descendant-tree-label = Descendant chart
pedigree-unknown-label = Unknown

# Preferences tool chrome (PR 20). Option labels for config-only enums (theme already has its own
# labels above) live here, not in the genealogy-ui data catalogue — they name UI settings, not
# domain data (ADR 0003 "presentation vs data" split).
prefs-nav-label = Preference sections
prefs-section-identity = Operator identity
prefs-section-appearance = Appearance
prefs-section-locale = Language & locale
prefs-section-formats = Date & number
prefs-section-defaults = Workspace defaults

prefs-identity-title = Who is making changes
prefs-display-name-label = Display name
prefs-email-label = Email
prefs-agent-kind-label = Agent kind
prefs-agent-kind-person = Person
prefs-agent-kind-software = Software (plugins only)
prefs-operator-id-label = Operator id
prefs-software-agent-note = Software agents (import/export plugins) are stamped automatically — you cannot pose as one.

prefs-theme-title = Theme
prefs-theme-radiogroup-label = Theme
prefs-theme-system-note = "System" follows your OS light/dark setting. The top-bar theme toggle is a quick override for this session.

prefs-locale-title = Interface & data
prefs-ui-language-label = UI language
prefs-data-locale-label = Data locale
prefs-data-locale-hint = sort, name display
prefs-follow-system = System default ({ $tag })
prefs-fallback-chain-label = Resolved fallback chain
prefs-fallback-chain-note = A missing string falls through to the next locale, never to a blank.
prefs-locale-note = UI chrome (ADR 0003 / Fluent) and the language a record is written in are distinct — this sets the chrome.

prefs-formats-title = Display formats
prefs-date-format-label = Date format
prefs-date-format-long = Long — { $example }
prefs-date-format-medium = Medium — { $example }
prefs-date-format-numeric = Numeric — { $example }
prefs-date-format-locale-default = Locale default
prefs-number-format-label = Number / decimal
prefs-number-format-space-comma = { $example } (space · comma)
prefs-number-format-comma-point = { $example } (comma · point)
prefs-number-format-locale-default = Locale default
prefs-live-example-label = Live example
prefs-formats-note = Genealogical date qualifiers (abt, bef, est, ranges) are formatted with the same locale.

prefs-defaults-title = Where a setting's value comes from
prefs-defaults-intro = Each setting resolves through three layers. The first layer that supplies a value wins; the rest are the fallback.
prefs-defaults-worked-example = Example shown: the default theme and the Person id format.
prefs-person-id-format-label = Person id format
prefs-layer-wins = wins
prefs-layer-fallback = fallback
prefs-layer-workspace = Workspace — { $path }
prefs-layer-shared = Shared app — { $path }
prefs-layer-embedded = Embedded — built-in baseline
prefs-defaults-footnote = App-level defaults are frozen at the moment of use; workspace-defaults stay live as a fallback (ADR 0005/0006).

prefs-workspaces-title = Registered workspaces
prefs-workspace-active = Active
prefs-switch-to = Switch to { $name }
prefs-switch-error = Could not switch workspace: { $detail }

prefs-reset = Reset to defaults
prefs-save = Save preferences
prefs-saved = Preferences saved.
prefs-save-error = Could not save: { $detail }
