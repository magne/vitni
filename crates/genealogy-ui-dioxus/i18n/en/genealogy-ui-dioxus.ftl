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

# List toolbar sort control (PR39)
sort-order-title = Change sort order
sort-id-asc = Sort: ID ↑
sort-id-desc = Sort: ID ↓
sort-name-asc = Sort: Name ↑
sort-name-desc = Sort: Name ↓

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
nav-research-notes = Research notes
nav-tags = Tags
nav-dna-tests = DNA tests
nav-dna-matches = DNA matches
nav-pedigree = Pedigree
nav-merge = Compare / merge
nav-import = Import
nav-export = Export
nav-geography = Geography
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
draft-tab-label = New { $entity }
close-tab-label = Close record
close-tab-named = Close { $name }
table-actions = Actions
tab-back = Back
tab-forward = Forward
coming-soon = { $screen } is coming soon.

palette-title = Command palette
palette-placeholder = Type a command or search…
palette-combobox-label = Search people, places, sources, or run a command
palette-results-label = Search results and commands
palette-group-commands = Commands
palette-group-recent = Recent
palette-kind-command = Command
palette-kind-recent = Recent
palette-cmd-create = Create { $entity }…
palette-cmd-find-duplicates = Find duplicates
palette-cmd-open = Open { $target }
palette-hint-navigate = navigate
palette-hint-open = open
palette-hint-anywhere = { $chord } from anywhere

# Keyboard notices (the shell toast bound to a shortcut outcome)
kbd-nothing-to-undo = Nothing to undo
kbd-redo-unavailable = Redo isn't available — the log is append-only; re-apply the change from the History tab.
notice-dismiss = Dismiss

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
sc-dock-tab = Dock record tab
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
sc-quit = Quit
sc-close-tab = Close tab

# Close-tab / quit confirm dialogs (⌘W, ⌘Q on a draft tab)
close-tab-confirm-title = Close tab?
close-tab-confirm-body = "{ $label }" hasn't been saved yet. Closing this tab discards it.
close-tab-confirm-confirm = Close tab
close-tab-confirm-cancel = Cancel
quit-confirm-title = Quit?
quit-confirm-body = One or more open tabs haven't been saved yet. Quitting discards them.
quit-confirm-confirm = Quit
quit-confirm-cancel = Cancel
# The docked-pane header's undock button (aria-label)
undock-label = Undock record
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

# Geography tool chrome (ADR 0025): the map surface, draw tools, time slider, and provider select.
geography-provider-label = Provider
geography-provider-osm = OpenStreetMap
geography-provider-maplibre = MapLibre style
geography-provider-google = Google (API key)
geography-tool-pan = Pan
geography-tool-point = Drop / move a point
geography-tool-polygon = Draw polygon
geography-finish-polygon = Finish polygon
geography-clear-draft = Clear
place-map-confirm-point = Use this point
geography-tool-fit = Fit
place-map-fit-title = Zoom to this place's geometry
place-map-open-in-geography = Open in Geography ↗
place-map-open-in-geography-title = Open this place in the Geography atlas
geography-time-slider-label = Map as of
geography-time-caption = Showing the map as of { $year }.
geography-empty-heading = No places to plot yet
geography-empty-help = Places need a coordinate or boundary before they show up here — set one from a place's Overview tab, or draw one directly on this map.
geography-create-here = New place here
geography-edit-geometry = Edit geometry
geography-map-aria = Map with { $markers } place markers and { $events } event pins
geography-rail-label = Places with a location
geography-screen-label = Geography

# Preferences tool chrome (PR 20). Option labels for config-only enums (theme already has its own
# labels above) live here, not in the genealogy-ui data catalogue — they name UI settings, not
# domain data (ADR 0003 "presentation vs data" split).
prefs-nav-label = Preference sections
prefs-section-identity = Operator identity
prefs-section-appearance = Appearance
prefs-section-locale = Language & locale
prefs-section-formats = Date & number
prefs-section-surety = Surety scheme
prefs-section-shortcuts = Keyboard shortcuts
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

prefs-surety-title = Confidence-level wording
prefs-surety-intro = Relabel the five surety levels asserted on every claim with your own wording. The five levels themselves are fixed (ADR 0027) — this changes only the words shown, never how a claim's evidence is stored or how many levels there are.
prefs-surety-field-very-low = Very low
prefs-surety-field-low = Low
prefs-surety-field-normal = Normal
prefs-surety-field-high = High
prefs-surety-field-very-high = Very high
prefs-surety-hint = Blank fields keep the built-in wording. A filled-in label is shown verbatim in every locale — it is not translated, since it is your own wording.

prefs-shortcuts-title = Rebind global shortcuts
prefs-shortcuts-intro = Only global shortcuts (active anywhere) can be rebound. Within-screen keys and the g-prefix navigation keys are fixed. Type a chord as mod+shift+alt+key — mod is ⌘ on macOS, Ctrl elsewhere.
prefs-shortcuts-default-hint = Default: { $chord }
prefs-shortcuts-general-errors = Some overrides could not be applied:

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
prefs-workspace-default = Default
prefs-workspace-col-name = Name
prefs-workspace-col-path = Path
prefs-workspace-col-engine = Engine
prefs-open-workspace = Open
prefs-open-workspace-label = Open workspace { $name }
prefs-make-default = Make default
prefs-make-default-label = Make { $name } the default workspace
prefs-workspaces-note = Workspaces are referenced by name; the default opens when no --workspace is given (ADR 0005). Open switches the session without changing the default; Make default persists without restarting.
prefs-register-workspace = + Register workspace…
prefs-register-name-label = Name
prefs-register-path-label = Directory
prefs-register-path-hint = Optional — defaults to the app data directory.
prefs-register-database-url-label = Database URL
prefs-register-database-url-hint = Optional — a Postgres connection string (e.g. postgres://host/db). Leave blank for the default SQLite engine.
prefs-register-submit = Register
prefs-register-cancel = Cancel
prefs-register-name-required = A workspace name is required.

prefs-reset = Reset to defaults
prefs-save = Save preferences
prefs-saved = Preferences saved.
prefs-save-error = Could not save: { $detail }

# Compare / merge tool chrome (PR 19). The duplicate-match reasons, field labels, and the outcome
# summary live in the genealogy-ui catalogue (data, resolved via Localizer) — these are the
# renderer's table/wizard chrome only.
merge-duplicates-heading = Possible duplicates
merge-duplicates-count = { $count } candidate pairs
merge-col-record-a = Record A
merge-col-record-b = Record B
merge-col-why = Why
merge-col-score = Match score
merge-score-tooltip = Duplicate-detector match score — not the 5-level assertion Confidence
merge-compare = Compare
merge-empty-duplicates = No possible duplicates found.
merge-wizard-heading = Compare & merge — { $a } ⟷ { $b }
merge-survivor-label = survivor · keeps id
merge-persona-label = becomes a persona
merge-keep-label = keep
merge-radio-group-label = Which record currently holds this value
merge-cancel = Cancel
merge-submit = Merge (reversible)
merge-back = Back to duplicates
merge-reason-label = Reason for merge
merge-reason-hint = (recorded on the merge event)

# Plugin manager chrome (PR21). Capability/role/trust labels are read off the component itself by
# genealogy-plugin-host::discover — these are only their display names.
plugin-manager-title = Installed plugins
plugin-manager-note = Plugins are sandboxed WebAssembly with deny-by-default access. A plugin can only touch what it has declared and you have granted.
plugin-manager-empty = No plugin components found. Run `cargo xtask build-plugins`.
plugin-reload = Reload from disk
plugin-col-name = Plugin
plugin-col-enabled = Enabled
plugin-col-capabilities = Declared capabilities
plugin-col-trust = Trust
plugin-enabled-switch = { $plugin } enabled
plugin-state-on = On
plugin-state-off = Off
plugin-role-bulk-import = Bulk import
plugin-role-bulk-export = Bulk export
plugin-role-ui-panel = UI panel
plugin-role-assisted-import = Assisted import
plugin-role-test-fixture = Test fixture
plugin-role-unknown = Unknown role
plugin-cap-log = log
plugin-cap-query = query
plugin-cap-commands = commands
plugin-cap-progress = progress
plugin-cap-import-source = import-source
plugin-cap-export-sink = export-sink
plugin-cap-net = net
plugin-cap-media-store = media-store
plugin-cap-ai = ai
plugin-cap-present = present
plugin-trust-unsigned = unsigned
plugin-trust-note = Full trust tiers and signature verification land in Phase 8; every plugin is shown read-only as unsigned until then.
plugin-host-api-version = host-api { $version }

# Assisted import wizard (ADR 0017). The confirm-stage field/action labels are resolved against the
# plugin's own catalogue; these are the wizard's own chrome.
import-heading = Assisted import
import-stage-source = Source
import-stage-records = Records
import-stage-confirm = Confirm
import-stage-save = Save scan
import-stage-summary = Summary
import-source-label = Source
import-url-label = Record or search URL
import-url-placeholder = https://www.digitalarkivet.no/…
import-fetch = Fetch
import-no-plugins = No assisted-import plugins are installed. Run `cargo xtask build-plugins`.
import-running = Importing…
import-back = Back
import-start-over = Start over
import-scan-url-label = Scanned page URL
import-scan-url-placeholder = Paste the Digitalarkivet scan URL to file it
import-records-heading = Records found
import-col-name = Name
import-col-detail = Details
import-col-status = Status
import-status-pending = Pending
import-status-imported = Imported
import-status-skipped = Skipped
import-review = Review
import-finish = Finish
import-confirm-heading = Confirm record
import-provenance-heading = What will be recorded
import-prov-operator = Operator
import-prov-source = Source
import-prov-repository = Repository
import-prov-citation = Citation
import-prov-external-id = External id
import-prov-confidence = Confidence
import-software-agent = software agent
import-summary-heading = Summary
import-summary-imported = { $count } imported
import-summary-skipped = { $count } skipped
import-another = Import another
import-cancel = Cancel
import-save-title = Save scan to media library
import-save-choose-category = Choose a category
import-save-category = Category
import-save-subfolder = Subfolder
import-save-filename = Filename
import-save-path-preview = Path preview

# Bulk-export wizard (ADR 0013): the GUI counterpart of `genealogy export`.
export-heading = Bulk export
export-stage-destination = Destination
export-stage-running = Running
export-stage-summary = Summary
export-destination-heading = Choose what to write, and where
export-plugin-label = Export format
export-no-plugins = No bulk-export plugins are installed. Run `cargo xtask build-plugins`.
export-destination-label = Destination
export-destination-placeholder = Leave empty to write into the workspace exports folder
export-destination-preview = Writes to
export-destination-dir-hint = under the plugin's file name
export-run = Export
export-running-heading = Exporting…
export-progress-starting = Starting…
export-progress-count = { $processed } of { $total }
export-progress-processed = { $processed } written
export-cancel = Cancel
export-summary-heading = Export complete
export-summary-records = { $count } records written
export-summary-destination = Written to
export-another = Export again
export-error-heading = Export failed
export-failed-unknown = The export ended without reporting a result.
export-cancelled-heading = Export cancelled
export-cancelled-message = The export stopped before it finished; a partly written file may be left behind.

# `Tool::Import`'s mode chooser (issue #191): Bulk file import vs the assisted online wizard above.
import-tool-heading = Import
import-mode-label = How do you want to import?
import-mode-bulk = Bulk file import
import-mode-assisted = Assisted online import

# Bulk-import wizard (issue #191): the GUI counterpart of `genealogy import`.
bulk-import-heading = Bulk import
bulk-import-stage-source = Source
bulk-import-stage-running = Running
bulk-import-stage-summary = Summary
bulk-import-source-heading = Choose what to read, and where to import it
bulk-import-plugin-label = Import format
bulk-import-no-plugins = No bulk-import plugins are installed. Run `cargo xtask build-plugins`.
bulk-import-source-label = Source file
bulk-import-source-placeholder = Path to the file to import
bulk-import-source-preview = Reads from
bulk-import-source-directory-hint = That path names a directory — pick a file to import.
bulk-import-target-label = Import into
bulk-import-target-existing = An existing workspace
bulk-import-target-new = A new workspace
bulk-import-target-workspace-label = Workspace
bulk-import-run = Import
bulk-import-running-heading = Importing…
bulk-import-progress-starting = Starting…
bulk-import-progress-count = { $processed } of { $total }
bulk-import-progress-processed = { $processed } imported
bulk-import-cancel = Cancel
bulk-import-summary-heading = Import complete
bulk-import-summary-records = { $count } records imported
bulk-import-summary-source = Imported from
bulk-import-another = Import another
bulk-import-error-heading = Import failed
bulk-import-failed-unknown = The import ended without reporting a result.
bulk-import-cancelled-heading = Import cancelled
bulk-import-cancelled-message = The import stopped before it finished.
bulk-import-confirm-title = Import into { $workspace }?
bulk-import-confirm-body = { $workspace } already has { $count ->
    [one] 1 person
   *[other] { $count } persons
} recorded. Importing may add duplicates.
bulk-import-confirm-cancel = Cancel
bulk-import-confirm-run = Import anyway
bulk-import-target-name-required = Enter a name for the new workspace.
bulk-import-target-name-taken = A workspace with that name is already registered.
