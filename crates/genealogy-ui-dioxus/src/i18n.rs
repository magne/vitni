//! Chrome localization for the Dioxus renderer (ADR 0003, ADR 0008 §3).
//!
//! The renderer owns its own catalogue (window/navigation labels and renderer-level errors), layered
//! over runtime overrides exactly like the other frontends. Data strings (names, sex, field labels,
//! application errors) come from [`genealogy_ui::Localizer`]; this catalogue is only the GUI's chrome.

use std::path::Path;

use genealogy_app::{DateFormat, NumberFormat, config};
use genealogy_ui::{RowSort, ShortcutGroup};
use i18n_embed::DesktopLanguageRequester;
use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed_fl::fl;
use rust_embed::RustEmbed;
use unic_langid::LanguageIdentifier;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Embedded;

/// The renderer's chrome catalogue.
pub struct Chrome {
    loader: FluentLanguageLoader,
}

impl Chrome {
    /// Builds the chrome localizer, layering the open workspace's `i18n/` override at top priority,
    /// with the workspace's configured `ui_language` outranking the ambient env request (ADR 0015 §4).
    /// `DesktopLanguageRequester` stays here so the app layer is `i18n_embed`-free.
    #[must_use]
    pub fn for_workspace(workspace_dir: &Path, config_ui_language: Option<&LanguageIdentifier>) -> Self {
        let requested = genealogy_app::requested_languages_for(
            config_ui_language,
            &DesktopLanguageRequester::requested_languages(),
        );
        Self::with_languages(Some(workspace_dir), &requested)
    }

    /// Builds a chrome localizer for explicit languages (deterministic for tests).
    #[must_use]
    pub fn with_languages(workspace_dir: Option<&Path>, requested: &[LanguageIdentifier]) -> Self {
        let loader = fluent_language_loader!();
        let shared = config::shared_i18n_dir().ok();
        genealogy_i18n::init(&loader, workspace_dir, shared.as_deref(), requested, Box::new(Embedded));
        Self { loader }
    }

    /// The window/application title.
    #[must_use]
    pub fn app_title(&self) -> String {
        fl!(self.loader, "app-title")
    }

    /// The "People" navigation label.
    #[must_use]
    pub fn nav_people(&self) -> String {
        fl!(self.loader, "nav-people")
    }

    /// The "Back" button label.
    #[must_use]
    pub fn back(&self) -> String {
        fl!(self.loader, "back")
    }

    /// The "Loading…" placeholder.
    #[must_use]
    pub fn loading(&self) -> String {
        fl!(self.loader, "loading")
    }

    /// The "{id} not found" message.
    #[must_use]
    pub fn not_found(&self, id: &str) -> String {
        fl!(self.loader, "not-found", id = id)
    }

    /// The "Run plugin" button label.
    #[must_use]
    pub fn run_plugin(&self) -> String {
        fl!(self.loader, "run-plugin")
    }

    /// The "select a person" placeholder shown when no person is selected.
    #[must_use]
    pub fn select_prompt(&self) -> String {
        fl!(self.loader, "select-prompt")
    }

    /// The "select a citation" placeholder shown when no citation is selected.
    #[must_use]
    pub fn citation_select_prompt(&self) -> String {
        fl!(self.loader, "citation-select-prompt")
    }

    /// The "select a family" placeholder shown when no family is selected.
    #[must_use]
    pub fn family_select_prompt(&self) -> String {
        fl!(self.loader, "family-select-prompt")
    }

    /// The "select an event" placeholder shown when no event is selected.
    #[must_use]
    pub fn event_select_prompt(&self) -> String {
        fl!(self.loader, "event-select-prompt")
    }

    /// The "select a place" placeholder shown when no place is selected.
    #[must_use]
    pub fn place_select_prompt(&self) -> String {
        fl!(self.loader, "place-select-prompt")
    }

    /// The "select a source" placeholder shown when no source is selected.
    #[must_use]
    pub fn source_select_prompt(&self) -> String {
        fl!(self.loader, "source-select-prompt")
    }

    /// The "select a repository" placeholder shown when no repository is selected.
    #[must_use]
    pub fn repository_select_prompt(&self) -> String {
        fl!(self.loader, "repository-select-prompt")
    }

    /// The "select a media object" placeholder shown when no media object is selected.
    #[must_use]
    pub fn media_select_prompt(&self) -> String {
        fl!(self.loader, "media-select-prompt")
    }

    /// The "select a note" placeholder shown when no note is selected.
    #[must_use]
    pub fn note_select_prompt(&self) -> String {
        fl!(self.loader, "note-select-prompt")
    }

    /// The "select a tag" placeholder shown when no tag is selected.
    #[must_use]
    pub fn tag_select_prompt(&self) -> String {
        fl!(self.loader, "tag-select-prompt")
    }

    /// The "select a DNA test" placeholder shown when no DNA test is selected.
    #[must_use]
    pub fn dna_test_select_prompt(&self) -> String {
        fl!(self.loader, "dna-test-select-prompt")
    }

    /// The "select a DNA match" placeholder shown when no DNA match is selected.
    #[must_use]
    pub fn dna_match_select_prompt(&self) -> String {
        fl!(self.loader, "dna-match-select-prompt")
    }

    /// The generic "select a record" placeholder shown by [`crate::screens::RecordDetail`] when no
    /// record tab is open (or the active destination has no detail pane).
    #[must_use]
    pub fn record_select_prompt(&self) -> String {
        fl!(self.loader, "record-select-prompt")
    }

    /// The default name a newly-created tag gets, refined afterwards in its Overview.
    #[must_use]
    pub fn new_tag_name(&self) -> String {
        fl!(self.loader, "new-tag-name")
    }

    /// A renderer-level plugin failure (technical detail passed through).
    #[must_use]
    pub fn plugin_error(&self, detail: &str) -> String {
        fl!(self.loader, "plugin-error", detail = detail)
    }

    /// The list filter searchbox placeholder/accessible name, naming the entity (e.g. "Filter
    /// people…").
    #[must_use]
    pub fn list_filter(&self, entity: &str) -> String {
        fl!(self.loader, "list-filter", entity = entity)
    }

    /// The list toolbar sort-button `title` ("Change sort order").
    #[must_use]
    pub fn sort_order_title(&self) -> String {
        fl!(self.loader, "sort-order-title")
    }

    /// The list toolbar sort-button label for the current order (e.g. "Sort: Name ↑").
    #[must_use]
    pub fn sort_label(&self, sort: RowSort) -> String {
        match sort {
            RowSort::IdAsc => fl!(self.loader, "sort-id-asc"),
            RowSort::IdDesc => fl!(self.loader, "sort-id-desc"),
            RowSort::TitleAsc => fl!(self.loader, "sort-name-asc"),
            RowSort::TitleDesc => fl!(self.loader, "sort-name-desc"),
        }
    }

    /// The "New" button label for a list.
    #[must_use]
    pub fn list_new(&self) -> String {
        fl!(self.loader, "list-new")
    }

    /// The placeholder for an empty related-item detail tab.
    #[must_use]
    pub fn tab_empty(&self) -> String {
        fl!(self.loader, "tab-empty")
    }

    /// The "Skip to content" skip-link label.
    #[must_use]
    pub fn skip_to_content(&self) -> String {
        fl!(self.loader, "skip-to-content")
    }

    /// The accessible name for a close control.
    #[must_use]
    pub fn close(&self) -> String {
        fl!(self.loader, "close")
    }

    /// The accessible name for a dismiss control.
    #[must_use]
    pub fn dismiss(&self) -> String {
        fl!(self.loader, "dismiss")
    }

    /// The accessible name for the docked-pane's undock (`✕`) control.
    #[must_use]
    pub fn undock_label(&self) -> String {
        fl!(self.loader, "undock-label")
    }

    /// The brand title shown in the rail.
    #[must_use]
    pub fn brand_title(&self) -> String {
        fl!(self.loader, "brand-title")
    }

    /// The "Entities" rail-group heading.
    #[must_use]
    pub fn nav_group_entities(&self) -> String {
        fl!(self.loader, "nav-group-entities")
    }

    /// The accessible name for an entity rail item, folding in its record count.
    #[must_use]
    pub fn rail_item_count(&self, label: &str, count: u64) -> String {
        fl!(self.loader, "nav-item-count", label = label, count = count)
    }

    /// The "Tools" rail-group heading.
    #[must_use]
    pub fn nav_group_tools(&self) -> String {
        fl!(self.loader, "nav-group-tools")
    }

    /// The accessible name for the primary navigation rail.
    #[must_use]
    pub fn aria_primary_nav(&self) -> String {
        fl!(self.loader, "aria-primary-nav")
    }

    /// The accessible name for the breadcrumb.
    #[must_use]
    pub fn aria_breadcrumb(&self) -> String {
        fl!(self.loader, "aria-breadcrumb")
    }

    /// The visually-hidden label for the global search input.
    #[must_use]
    pub fn search_label(&self) -> String {
        fl!(self.loader, "search-label")
    }

    /// The global search input placeholder.
    #[must_use]
    pub fn search_placeholder(&self) -> String {
        fl!(self.loader, "search-placeholder")
    }

    /// The accessible name for the search clear button.
    #[must_use]
    pub fn search_clear(&self) -> String {
        fl!(self.loader, "search-clear")
    }

    /// The localized name of a theme mode (for the theme control's label).
    #[must_use]
    pub fn theme_mode_label(&self, mode: genealogy_app::ThemeMode) -> String {
        match mode {
            genealogy_app::ThemeMode::System => fl!(self.loader, "theme-mode-system"),
            genealogy_app::ThemeMode::Light => fl!(self.loader, "theme-mode-light"),
            genealogy_app::ThemeMode::Dark => fl!(self.loader, "theme-mode-dark"),
        }
    }

    /// The theme-mode summary shown in the status bar and the theme control's tooltip. `System`
    /// names the palette it resolves to (e.g. `system (dark)`); `Light`/`Dark` stand alone.
    #[must_use]
    pub fn theme_mode_status(&self, mode: genealogy_app::ThemeMode, resolved_dark: bool) -> String {
        match mode {
            genealogy_app::ThemeMode::System => {
                let resolved = if resolved_dark {
                    fl!(self.loader, "status-theme-dark")
                } else {
                    fl!(self.loader, "status-theme-light")
                };
                fl!(self.loader, "status-theme-system", resolved = resolved)
            }
            genealogy_app::ThemeMode::Light => fl!(self.loader, "status-theme-light"),
            genealogy_app::ThemeMode::Dark => fl!(self.loader, "status-theme-dark"),
        }
    }

    /// The accessible name for the theme-cycle control, naming the current mode.
    #[must_use]
    pub fn aria_theme_cycle(&self, mode: genealogy_app::ThemeMode) -> String {
        let mode = self.theme_mode_label(mode);
        fl!(self.loader, "aria-theme-cycle", mode = mode)
    }

    /// The accessible name for the keyboard-shortcuts control.
    #[must_use]
    pub fn aria_help(&self) -> String {
        fl!(self.loader, "aria-help")
    }

    /// The accessible name for the open-records tabstrip.
    #[must_use]
    pub fn aria_open_records(&self) -> String {
        fl!(self.loader, "aria-open-records")
    }

    /// The accessible name for the "create a new record" control (the tabstrip `+`).
    #[must_use]
    pub fn new_tab_label(&self) -> String {
        fl!(self.loader, "new-tab-label")
    }

    /// The label of an unsaved draft record tab (e.g. "New Person"); `entity` is the record's
    /// already-localized category name.
    #[must_use]
    pub fn draft_tab_label(&self, entity: &str) -> String {
        fl!(self.loader, "draft-tab-label", entity = entity)
    }

    /// The accessible name for the tabstrip's back-navigation control.
    #[must_use]
    pub fn tab_back(&self) -> String {
        fl!(self.loader, "tab-back")
    }

    /// The accessible name for the tabstrip's forward-navigation control.
    #[must_use]
    pub fn tab_forward(&self) -> String {
        fl!(self.loader, "tab-forward")
    }

    /// The accessible name for a record-tab close control.
    #[must_use]
    pub fn close_tab_label(&self) -> String {
        fl!(self.loader, "close-tab-label")
    }

    /// The row-scoped accessible name for a record tab's close control, naming the record.
    #[must_use]
    pub fn close_tab_named(&self, name: &str) -> String {
        fl!(self.loader, "close-tab-named", name = name)
    }

    /// The accessible name for a data table's row-actions column header (visually hidden).
    #[must_use]
    pub fn table_actions(&self) -> String {
        fl!(self.loader, "table-actions")
    }

    /// The command-palette dialog title.
    #[must_use]
    pub fn palette_title(&self) -> String {
        fl!(self.loader, "palette-title")
    }

    /// The command-palette input placeholder.
    #[must_use]
    pub fn palette_placeholder(&self) -> String {
        fl!(self.loader, "palette-placeholder")
    }

    /// The command-palette input's combobox accessible name.
    #[must_use]
    pub fn palette_combobox_label(&self) -> String {
        fl!(self.loader, "palette-combobox-label")
    }

    /// The command-palette listbox's accessible name.
    #[must_use]
    pub fn palette_results_label(&self) -> String {
        fl!(self.loader, "palette-results-label")
    }

    /// The "Commands" palette group heading.
    #[must_use]
    pub fn palette_group_commands(&self) -> String {
        fl!(self.loader, "palette-group-commands")
    }

    /// The "Recent" palette group heading.
    #[must_use]
    pub fn palette_group_recent(&self) -> String {
        fl!(self.loader, "palette-group-recent")
    }

    /// The "Command" kind badge on a command option.
    #[must_use]
    pub fn palette_kind_command(&self) -> String {
        fl!(self.loader, "palette-kind-command")
    }

    /// The "Recent" word in a recent option's kind badge (composed with the category label).
    #[must_use]
    pub fn palette_kind_recent(&self) -> String {
        fl!(self.loader, "palette-kind-recent")
    }

    /// The "Create {entity}…" command label.
    #[must_use]
    pub fn palette_cmd_create(&self, entity: &str) -> String {
        fl!(self.loader, "palette-cmd-create", entity = entity)
    }

    /// The "Find duplicates" command label.
    #[must_use]
    pub fn palette_cmd_find_duplicates(&self) -> String {
        fl!(self.loader, "palette-cmd-find-duplicates")
    }

    /// The "Open {target}" command label (a tool or the help browser).
    #[must_use]
    pub fn palette_cmd_open(&self, target: &str) -> String {
        fl!(self.loader, "palette-cmd-open", target = target)
    }

    /// The palette footer's "navigate" hint (beside ↑↓).
    #[must_use]
    pub fn palette_hint_navigate(&self) -> String {
        fl!(self.loader, "palette-hint-navigate")
    }

    /// The palette footer's "open" hint (beside ↵).
    #[must_use]
    pub fn palette_hint_open(&self) -> String {
        fl!(self.loader, "palette-hint-open")
    }

    /// The palette footer's "⌘K from anywhere" hint, taking the resolved command-palette chord as an
    /// argument (ADR 0030 §4) so it never goes stale under a rebind.
    #[must_use]
    pub fn palette_hint_anywhere(&self, chord: &str) -> String {
        fl!(self.loader, "palette-hint-anywhere", chord = chord)
    }

    /// The `⌘Z` "nothing to undo" notice (no record open / wrong screen / nothing undoable).
    #[must_use]
    pub fn kbd_nothing_to_undo(&self) -> String {
        fl!(self.loader, "kbd-nothing-to-undo")
    }

    /// The `⌘⇧Z` "redo isn't available" notice (the log is append-only).
    #[must_use]
    pub fn kbd_redo_unavailable(&self) -> String {
        fl!(self.loader, "kbd-redo-unavailable")
    }

    /// The accessible label for the shell notice's dismiss control.
    #[must_use]
    pub fn notice_dismiss(&self) -> String {
        fl!(self.loader, "notice-dismiss")
    }

    /// The keyboard-shortcuts help-sheet title.
    #[must_use]
    pub fn help_title(&self) -> String {
        fl!(self.loader, "help-title")
    }

    /// The "{screen} is coming soon" placeholder message for a not-yet-built destination.
    #[must_use]
    pub fn coming_soon(&self, screen: &str) -> String {
        fl!(self.loader, "coming-soon", screen = screen)
    }

    /// The accessible name for the help-index listbox.
    #[must_use]
    pub fn help_index_label(&self) -> String {
        fl!(self.loader, "help-index-label")
    }

    /// The help-index filter searchbox placeholder/accessible name.
    #[must_use]
    pub fn help_filter(&self) -> String {
        fl!(self.loader, "help-filter")
    }

    /// The empty-index message shown when the filter matches no topics.
    #[must_use]
    pub fn help_empty(&self) -> String {
        fl!(self.loader, "help-empty")
    }

    /// The heading for a help-overlay column.
    #[must_use]
    pub fn help_column(&self, group: ShortcutGroup) -> String {
        match group {
            ShortcutGroup::Global => fl!(self.loader, "help-col-global"),
            ShortcutGroup::Navigation => fl!(self.loader, "help-col-goto"),
            ShortcutGroup::WithinScreen => fl!(self.loader, "help-col-within"),
        }
    }

    /// Resolves the Pedigree tool's view-switcher label id (`list`/`pedigree`/`descendants`/
    /// `relationships`) to its display text; an unknown id falls back to the Pedigree label.
    #[must_use]
    pub fn pedigree_view_label(&self, id: &str) -> String {
        match id {
            "list" => fl!(self.loader, "pedigree-view-list"),
            "descendants" => fl!(self.loader, "pedigree-view-descendants"),
            "relationships" => fl!(self.loader, "pedigree-view-relationships"),
            _ => fl!(self.loader, "pedigree-view-pedigree"),
        }
    }

    /// The accessible name for the Pedigree tool's view-switcher tablist.
    #[must_use]
    pub fn pedigree_view_switcher_label(&self) -> String {
        fl!(self.loader, "pedigree-view-switcher-label")
    }

    /// The focus-person picker's field label.
    #[must_use]
    pub fn pedigree_focus_label(&self) -> String {
        fl!(self.loader, "pedigree-focus-label")
    }

    /// The generations-count field label.
    #[must_use]
    pub fn pedigree_generations_label(&self) -> String {
        fl!(self.loader, "pedigree-generations-label")
    }

    /// The focus-picker's submit button label.
    #[must_use]
    pub fn pedigree_show(&self) -> String {
        fl!(self.loader, "pedigree-show")
    }

    /// The kinship calculator's "Person A" field label.
    #[must_use]
    pub fn pedigree_person_a_label(&self) -> String {
        fl!(self.loader, "pedigree-person-a-label")
    }

    /// The kinship calculator's "Person B" field label.
    #[must_use]
    pub fn pedigree_person_b_label(&self) -> String {
        fl!(self.loader, "pedigree-person-b-label")
    }

    /// The kinship calculator's submit button label.
    #[must_use]
    pub fn pedigree_compute(&self) -> String {
        fl!(self.loader, "pedigree-compute")
    }

    /// The prompt shown before a focus person has been chosen.
    #[must_use]
    pub fn pedigree_empty_focus(&self) -> String {
        fl!(self.loader, "pedigree-empty-focus")
    }

    /// The prompt shown before both people are entered in the Relationships view.
    #[must_use]
    pub fn pedigree_empty_relationship(&self) -> String {
        fl!(self.loader, "pedigree-empty-relationship")
    }

    /// The accessible name for the ancestor chart's `role="tree"`.
    #[must_use]
    pub fn pedigree_ancestor_tree_label(&self) -> String {
        fl!(self.loader, "pedigree-ancestor-tree-label")
    }

    /// The accessible name for the descendant chart's `role="tree"`.
    #[must_use]
    pub fn pedigree_descendant_tree_label(&self) -> String {
        fl!(self.loader, "pedigree-descendant-tree-label")
    }

    /// The "Unknown" name shown on an unresearched ancestor slot's placeholder `treeitem`.
    #[must_use]
    pub fn pedigree_unknown_label(&self) -> String {
        fl!(self.loader, "pedigree-unknown-label")
    }

    /// The tile/style "Provider" select label.
    #[must_use]
    pub fn geography_provider_label(&self) -> String {
        fl!(self.loader, "geography-provider-label")
    }

    /// The localized label for a provider kind (`osm-raster`/`maplibre-style`/`google`); an unknown
    /// kind falls back to the OSM label.
    #[must_use]
    pub fn geography_provider_kind_label(&self, kind: &str) -> String {
        match kind {
            "maplibre-style" => fl!(self.loader, "geography-provider-maplibre"),
            "google" => fl!(self.loader, "geography-provider-google"),
            _ => fl!(self.loader, "geography-provider-osm"),
        }
    }

    /// The Pan draw-tool button's accessible title.
    #[must_use]
    pub fn geography_tool_pan(&self) -> String {
        fl!(self.loader, "geography-tool-pan")
    }

    /// The Point draw-tool button's accessible title (drop/move a point).
    #[must_use]
    pub fn geography_tool_point(&self) -> String {
        fl!(self.loader, "geography-tool-point")
    }

    /// The Polygon draw-tool button's accessible title (add a boundary vertex).
    #[must_use]
    pub fn geography_tool_polygon(&self) -> String {
        fl!(self.loader, "geography-tool-polygon")
    }

    /// The "Finish polygon" action, committing the drafted ring.
    #[must_use]
    pub fn geography_finish_polygon(&self) -> String {
        fl!(self.loader, "geography-finish-polygon")
    }

    /// The "Clear" action, discarding the in-progress draft.
    #[must_use]
    pub fn geography_clear_draft(&self) -> String {
        fl!(self.loader, "geography-clear-draft")
    }

    /// The Place Map tab's "Use this point" action, confirming a dropped point as the pending
    /// geometry to save (Phase 9's map editor).
    #[must_use]
    pub fn place_map_confirm_point(&self) -> String {
        fl!(self.loader, "place-map-confirm-point")
    }

    /// The "⤢ Fit" toolbar button's label, zooming/panning the map to fit the shown geometry (the
    /// Place Map tab's own shape, or the Geography atlas' every filtered marker).
    #[must_use]
    pub fn geography_tool_fit(&self) -> String {
        fl!(self.loader, "geography-tool-fit")
    }

    /// The Place Map tab's "Fit" button's accessible title.
    #[must_use]
    pub fn place_map_fit_title(&self) -> String {
        fl!(self.loader, "place-map-fit-title")
    }

    /// The Place Map tab's "Open in Geography ↗" action, navigating to the Geography tool with this
    /// place pre-selected in its rail.
    #[must_use]
    pub fn place_map_open_in_geography(&self) -> String {
        fl!(self.loader, "place-map-open-in-geography")
    }

    /// The "Open in Geography ↗" button's accessible title.
    #[must_use]
    pub fn place_map_open_in_geography_title(&self) -> String {
        fl!(self.loader, "place-map-open-in-geography-title")
    }

    /// The "Map as of" time-slider caption label.
    #[must_use]
    pub fn geography_time_slider_label(&self) -> String {
        fl!(self.loader, "geography-time-slider-label")
    }

    /// The "Showing the map as of {year}" caption under the time slider.
    #[must_use]
    pub fn geography_time_caption(&self, year: i32) -> String {
        fl!(self.loader, "geography-time-caption", year = year)
    }

    /// The empty-state heading when no place has a resolved geometry to plot.
    #[must_use]
    pub fn geography_empty_heading(&self) -> String {
        fl!(self.loader, "geography-empty-heading")
    }

    /// The empty-state helper text under [`Self::geography_empty_heading`].
    #[must_use]
    pub fn geography_empty_help(&self) -> String {
        fl!(self.loader, "geography-empty-help")
    }

    /// The "New place here" quick-create panel's title.
    #[must_use]
    pub fn geography_create_here(&self) -> String {
        fl!(self.loader, "geography-create-here")
    }

    /// The "Edit geometry" side-panel title for an existing place.
    #[must_use]
    pub fn geography_edit_geometry(&self) -> String {
        fl!(self.loader, "geography-edit-geometry")
    }

    /// The accessible name for the map surface, given how many markers/pins it holds.
    #[must_use]
    pub fn geography_map_aria(&self, markers: usize, events: usize) -> String {
        fl!(self.loader, "geography-map-aria", markers = markers, events = events)
    }

    /// The accessible name for the place rail list.
    #[must_use]
    pub fn geography_rail_label(&self) -> String {
        fl!(self.loader, "geography-rail-label")
    }

    /// The accessible name for the Geography tool overall.
    #[must_use]
    pub fn geography_screen_label(&self) -> String {
        fl!(self.loader, "geography-screen-label")
    }

    /// The accessible name for the Preferences settings sub-nav.
    #[must_use]
    pub fn prefs_nav_label(&self) -> String {
        fl!(self.loader, "prefs-nav-label")
    }

    /// The localized heading for a Preferences section id (`identity`/`appearance`/`locale`/
    /// `formats`/`surety`/`defaults`); an unknown id falls back to the identity section's heading.
    #[must_use]
    pub fn prefs_section_label(&self, id: &str) -> String {
        match id {
            "appearance" => fl!(self.loader, "prefs-section-appearance"),
            "locale" => fl!(self.loader, "prefs-section-locale"),
            "formats" => fl!(self.loader, "prefs-section-formats"),
            "surety" => fl!(self.loader, "prefs-section-surety"),
            "shortcuts" => fl!(self.loader, "prefs-section-shortcuts"),
            "defaults" => fl!(self.loader, "prefs-section-defaults"),
            _ => fl!(self.loader, "prefs-section-identity"),
        }
    }

    /// The "Who is making changes" card title.
    #[must_use]
    pub fn prefs_identity_title(&self) -> String {
        fl!(self.loader, "prefs-identity-title")
    }

    /// The display-name field label.
    #[must_use]
    pub fn prefs_display_name_label(&self) -> String {
        fl!(self.loader, "prefs-display-name-label")
    }

    /// The email field label.
    #[must_use]
    pub fn prefs_email_label(&self) -> String {
        fl!(self.loader, "prefs-email-label")
    }

    /// The agent-kind field label.
    #[must_use]
    pub fn prefs_agent_kind_label(&self) -> String {
        fl!(self.loader, "prefs-agent-kind-label")
    }

    /// The "Person" agent-kind option label.
    #[must_use]
    pub fn prefs_agent_kind_person(&self) -> String {
        fl!(self.loader, "prefs-agent-kind-person")
    }

    /// The disabled "Software (plugins only)" agent-kind option label.
    #[must_use]
    pub fn prefs_agent_kind_software(&self) -> String {
        fl!(self.loader, "prefs-agent-kind-software")
    }

    /// The read-only operator-id field label.
    #[must_use]
    pub fn prefs_operator_id_label(&self) -> String {
        fl!(self.loader, "prefs-operator-id-label")
    }

    /// The editable Person `HumanId` format field label (the "Workspace defaults" worked example).
    #[must_use]
    pub fn prefs_person_id_format_label(&self) -> String {
        fl!(self.loader, "prefs-person-id-format-label")
    }

    /// The note explaining software agents are stamped automatically.
    #[must_use]
    pub fn prefs_software_agent_note(&self) -> String {
        fl!(self.loader, "prefs-software-agent-note")
    }

    /// The "Theme" card title.
    #[must_use]
    pub fn prefs_theme_title(&self) -> String {
        fl!(self.loader, "prefs-theme-title")
    }

    /// The accessible name for the theme radiogroup.
    #[must_use]
    pub fn prefs_theme_radiogroup_label(&self) -> String {
        fl!(self.loader, "prefs-theme-radiogroup-label")
    }

    /// The note explaining "System" follows the OS setting.
    #[must_use]
    pub fn prefs_theme_system_note(&self) -> String {
        fl!(self.loader, "prefs-theme-system-note")
    }

    /// The "Interface & data" card title.
    #[must_use]
    pub fn prefs_locale_title(&self) -> String {
        fl!(self.loader, "prefs-locale-title")
    }

    /// The UI-language field label.
    #[must_use]
    pub fn prefs_ui_language_label(&self) -> String {
        fl!(self.loader, "prefs-ui-language-label")
    }

    /// The data-locale field label.
    #[must_use]
    pub fn prefs_data_locale_label(&self) -> String {
        fl!(self.loader, "prefs-data-locale-label")
    }

    /// The data-locale field's "— sort, name display" hint.
    #[must_use]
    pub fn prefs_data_locale_hint(&self) -> String {
        fl!(self.loader, "prefs-data-locale-hint")
    }

    /// The "System default (<tag>)" option label for a language/locale select.
    #[must_use]
    pub fn prefs_follow_system(&self, tag: &str) -> String {
        fl!(self.loader, "prefs-follow-system", tag = tag)
    }

    /// The "Resolved fallback chain" field label.
    #[must_use]
    pub fn prefs_fallback_chain_label(&self) -> String {
        fl!(self.loader, "prefs-fallback-chain-label")
    }

    /// The note explaining the fallback chain never leaves a blank.
    #[must_use]
    pub fn prefs_fallback_chain_note(&self) -> String {
        fl!(self.loader, "prefs-fallback-chain-note")
    }

    /// The note distinguishing UI chrome from a record's own language.
    #[must_use]
    pub fn prefs_locale_note(&self) -> String {
        fl!(self.loader, "prefs-locale-note")
    }

    /// The "Display formats" card title.
    #[must_use]
    pub fn prefs_formats_title(&self) -> String {
        fl!(self.loader, "prefs-formats-title")
    }

    /// The date-format field label.
    #[must_use]
    pub fn prefs_date_format_label(&self) -> String {
        fl!(self.loader, "prefs-date-format-label")
    }

    /// The option label for one [`DateFormat`] variant, showing `example` rendered in that style.
    #[must_use]
    pub fn prefs_date_format_option(&self, format: DateFormat, example: &str) -> String {
        match format {
            DateFormat::Long => fl!(self.loader, "prefs-date-format-long", example = example),
            DateFormat::Medium => fl!(self.loader, "prefs-date-format-medium", example = example),
            DateFormat::Numeric => fl!(self.loader, "prefs-date-format-numeric", example = example),
            DateFormat::LocaleDefault => fl!(self.loader, "prefs-date-format-locale-default"),
        }
    }

    /// The number-format field label.
    #[must_use]
    pub fn prefs_number_format_label(&self) -> String {
        fl!(self.loader, "prefs-number-format-label")
    }

    /// The option label for one [`NumberFormat`] variant, showing `example` rendered in that style.
    #[must_use]
    pub fn prefs_number_format_option(&self, format: NumberFormat, example: &str) -> String {
        match format {
            NumberFormat::SpaceComma => fl!(self.loader, "prefs-number-format-space-comma", example = example),
            NumberFormat::CommaPoint => fl!(self.loader, "prefs-number-format-comma-point", example = example),
            NumberFormat::LocaleDefault => fl!(self.loader, "prefs-number-format-locale-default"),
        }
    }

    /// The "Live example" field label.
    #[must_use]
    pub fn prefs_live_example_label(&self) -> String {
        fl!(self.loader, "prefs-live-example-label")
    }

    /// The note explaining genealogical date qualifiers share the same locale.
    #[must_use]
    pub fn prefs_formats_note(&self) -> String {
        fl!(self.loader, "prefs-formats-note")
    }

    /// The "Confidence-level wording" card title (ADR 0027).
    #[must_use]
    pub fn prefs_surety_title(&self) -> String {
        fl!(self.loader, "prefs-surety-title")
    }

    /// The paragraph explaining the surety scheme is relabel-only (fixed cardinality, ADR 0027).
    #[must_use]
    pub fn prefs_surety_intro(&self) -> String {
        fl!(self.loader, "prefs-surety-intro")
    }

    /// The field label (and empty-field placeholder) for one fixed `Confidence` ordinal
    /// (`very-low`/`low`/`normal`/`high`/`very-high`); an unknown ordinal falls back to `normal`'s
    /// label.
    #[must_use]
    pub fn prefs_surety_field_label(&self, ordinal: &str) -> String {
        match ordinal {
            "very-low" => fl!(self.loader, "prefs-surety-field-very-low"),
            "low" => fl!(self.loader, "prefs-surety-field-low"),
            "high" => fl!(self.loader, "prefs-surety-field-high"),
            "very-high" => fl!(self.loader, "prefs-surety-field-very-high"),
            _ => fl!(self.loader, "prefs-surety-field-normal"),
        }
    }

    /// The note explaining a blank field keeps the built-in Fluent-resolved wording.
    #[must_use]
    pub fn prefs_surety_hint(&self) -> String {
        fl!(self.loader, "prefs-surety-hint")
    }

    /// The "Rebind global shortcuts" card title (ADR 0030).
    #[must_use]
    pub fn prefs_shortcuts_title(&self) -> String {
        fl!(self.loader, "prefs-shortcuts-title")
    }

    /// The paragraph explaining only `Global` shortcuts are rebindable and the chord syntax.
    #[must_use]
    pub fn prefs_shortcuts_intro(&self) -> String {
        fl!(self.loader, "prefs-shortcuts-intro")
    }

    /// The "Default: { $chord }" hint shown under an empty/unmodified shortcut field.
    #[must_use]
    pub fn prefs_shortcuts_default_hint(&self, chord: &str) -> String {
        fl!(self.loader, "prefs-shortcuts-default-hint", chord = chord)
    }

    /// The lead-in for the card's general (not row-attachable) rejected-override list.
    #[must_use]
    pub fn prefs_shortcuts_general_errors(&self) -> String {
        fl!(self.loader, "prefs-shortcuts-general-errors")
    }

    /// The "Where a setting's value comes from" card title.
    #[must_use]
    pub fn prefs_defaults_title(&self) -> String {
        fl!(self.loader, "prefs-defaults-title")
    }

    /// The intro sentence explaining the three-layer override chain.
    #[must_use]
    pub fn prefs_defaults_intro(&self) -> String {
        fl!(self.loader, "prefs-defaults-intro")
    }

    /// The sentence naming the worked example (theme + Person id format).
    #[must_use]
    pub fn prefs_defaults_worked_example(&self) -> String {
        fl!(self.loader, "prefs-defaults-worked-example")
    }

    /// The "wins" badge text for the layer that supplied a resolved value.
    #[must_use]
    pub fn prefs_layer_wins(&self) -> String {
        fl!(self.loader, "prefs-layer-wins")
    }

    /// The "fallback" badge text for a layer that did not win.
    #[must_use]
    pub fn prefs_layer_fallback(&self) -> String {
        fl!(self.loader, "prefs-layer-fallback")
    }

    /// The "Workspace — {path}" row label.
    #[must_use]
    pub fn prefs_layer_workspace(&self, path: &str) -> String {
        fl!(self.loader, "prefs-layer-workspace", path = path)
    }

    /// The "Shared app — {path}" row label.
    #[must_use]
    pub fn prefs_layer_shared(&self, path: &str) -> String {
        fl!(self.loader, "prefs-layer-shared", path = path)
    }

    /// The "Embedded — built-in baseline" row label.
    #[must_use]
    pub fn prefs_layer_embedded(&self) -> String {
        fl!(self.loader, "prefs-layer-embedded")
    }

    /// The footnote distinguishing frozen app defaults from live workspace-defaults.
    #[must_use]
    pub fn prefs_defaults_footnote(&self) -> String {
        fl!(self.loader, "prefs-defaults-footnote")
    }

    /// The "Registered workspaces" card title.
    #[must_use]
    pub fn prefs_workspaces_title(&self) -> String {
        fl!(self.loader, "prefs-workspaces-title")
    }

    /// The badge naming the currently-active (open) workspace.
    #[must_use]
    pub fn prefs_workspace_active(&self) -> String {
        fl!(self.loader, "prefs-workspace-active")
    }

    /// The badge naming the configured default workspace.
    #[must_use]
    pub fn prefs_workspace_default(&self) -> String {
        fl!(self.loader, "prefs-workspace-default")
    }

    /// The workspaces table's "Name" column header.
    #[must_use]
    pub fn prefs_workspace_col_name(&self) -> String {
        fl!(self.loader, "prefs-workspace-col-name")
    }

    /// The workspaces table's "Path" column header.
    #[must_use]
    pub fn prefs_workspace_col_path(&self) -> String {
        fl!(self.loader, "prefs-workspace-col-path")
    }

    /// The workspaces table's "Engine" column header.
    #[must_use]
    pub fn prefs_workspace_col_engine(&self) -> String {
        fl!(self.loader, "prefs-workspace-col-engine")
    }

    /// The card footnote explaining name references and the Open vs Make-default distinction.
    #[must_use]
    pub fn prefs_workspaces_note(&self) -> String {
        fl!(self.loader, "prefs-workspaces-note")
    }

    /// The "Open" (switch this session) action label.
    #[must_use]
    pub fn prefs_open_workspace(&self) -> String {
        fl!(self.loader, "prefs-open-workspace")
    }

    /// The row-scoped accessible name for a workspace's "Open" action, naming the workspace.
    #[must_use]
    pub fn prefs_open_workspace_label(&self, name: &str) -> String {
        fl!(self.loader, "prefs-open-workspace-label", name = name)
    }

    /// The "Make default" (persist the default) action label.
    #[must_use]
    pub fn prefs_make_default(&self) -> String {
        fl!(self.loader, "prefs-make-default")
    }

    /// The row-scoped accessible name for a workspace's "Make default" action, naming the workspace.
    #[must_use]
    pub fn prefs_make_default_label(&self, name: &str) -> String {
        fl!(self.loader, "prefs-make-default-label", name = name)
    }

    /// The "+ Register workspace…" disclosure button label.
    #[must_use]
    pub fn prefs_register_workspace(&self) -> String {
        fl!(self.loader, "prefs-register-workspace")
    }

    /// The register form's "Name" field label.
    #[must_use]
    pub fn prefs_register_name_label(&self) -> String {
        fl!(self.loader, "prefs-register-name-label")
    }

    /// The register form's "Directory" field label.
    #[must_use]
    pub fn prefs_register_path_label(&self) -> String {
        fl!(self.loader, "prefs-register-path-label")
    }

    /// The register form's directory-field hint (empty ⇒ default data dir).
    #[must_use]
    pub fn prefs_register_path_hint(&self) -> String {
        fl!(self.loader, "prefs-register-path-hint")
    }

    /// The register form's optional "Database URL" field label (rendered only under the `postgres`
    /// feature; kept un-cfg'd here so `i18n-check` never sees an unused key in a default build).
    #[must_use]
    pub fn prefs_register_database_url_label(&self) -> String {
        fl!(self.loader, "prefs-register-database-url-label")
    }

    /// The register form's Database URL field hint (empty ⇒ default SQLite engine).
    #[must_use]
    pub fn prefs_register_database_url_hint(&self) -> String {
        fl!(self.loader, "prefs-register-database-url-hint")
    }

    /// The register form's submit button label.
    #[must_use]
    pub fn prefs_register_submit(&self) -> String {
        fl!(self.loader, "prefs-register-submit")
    }

    /// The register form's cancel button label.
    #[must_use]
    pub fn prefs_register_cancel(&self) -> String {
        fl!(self.loader, "prefs-register-cancel")
    }

    /// The validation message shown when the register form's name is empty.
    #[must_use]
    pub fn prefs_register_name_required(&self) -> String {
        fl!(self.loader, "prefs-register-name-required")
    }

    /// The "Reset to defaults" button label.
    #[must_use]
    pub fn prefs_reset(&self) -> String {
        fl!(self.loader, "prefs-reset")
    }

    /// The "Save preferences" button label.
    #[must_use]
    pub fn prefs_save(&self) -> String {
        fl!(self.loader, "prefs-save")
    }

    /// The `aria-live` success message after saving.
    #[must_use]
    pub fn prefs_saved(&self) -> String {
        fl!(self.loader, "prefs-saved")
    }

    /// The `aria-live` "Could not save: {detail}" error message.
    #[must_use]
    pub fn prefs_save_error(&self, detail: &str) -> String {
        fl!(self.loader, "prefs-save-error", detail = detail)
    }

    /// The Merge tool's duplicates table heading.
    #[must_use]
    pub fn merge_duplicates_heading(&self) -> String {
        fl!(self.loader, "merge-duplicates-heading")
    }

    /// The "{n} candidate pairs" count shown beside the duplicates heading.
    #[must_use]
    pub fn merge_duplicates_count(&self, count: usize) -> String {
        fl!(
            self.loader,
            "merge-duplicates-count",
            count = u64::try_from(count).unwrap_or(u64::MAX)
        )
    }

    /// The duplicates table's "Record A" column header.
    #[must_use]
    pub fn merge_col_record_a(&self) -> String {
        fl!(self.loader, "merge-col-record-a")
    }

    /// The duplicates table's "Record B" column header.
    #[must_use]
    pub fn merge_col_record_b(&self) -> String {
        fl!(self.loader, "merge-col-record-b")
    }

    /// The duplicates table's "Why" column header.
    #[must_use]
    pub fn merge_col_why(&self) -> String {
        fl!(self.loader, "merge-col-why")
    }

    /// The duplicates table's "Match score" column header.
    #[must_use]
    pub fn merge_col_score(&self) -> String {
        fl!(self.loader, "merge-col-score")
    }

    /// The tooltip on a match-score badge, distinguishing it from the 5-level assertion Confidence.
    #[must_use]
    pub fn merge_score_tooltip(&self) -> String {
        fl!(self.loader, "merge-score-tooltip")
    }

    /// The duplicates table's per-row "Compare" button label.
    #[must_use]
    pub fn merge_compare(&self) -> String {
        fl!(self.loader, "merge-compare")
    }

    /// The duplicates table's empty state.
    #[must_use]
    pub fn merge_empty_duplicates(&self) -> String {
        fl!(self.loader, "merge-empty-duplicates")
    }

    /// The compare/merge wizard's heading, naming both people.
    #[must_use]
    pub fn merge_wizard_heading(&self, a: &str, b: &str) -> String {
        fl!(self.loader, "merge-wizard-heading", a = a, b = b)
    }

    /// The survivor column's "survivor · keeps id" caption.
    #[must_use]
    pub fn merge_survivor_label(&self) -> String {
        fl!(self.loader, "merge-survivor-label")
    }

    /// The merged column's "becomes a persona" caption.
    #[must_use]
    pub fn merge_persona_label(&self) -> String {
        fl!(self.loader, "merge-persona-label")
    }

    /// The per-field radio column's "keep" caption.
    #[must_use]
    pub fn merge_keep_label(&self) -> String {
        fl!(self.loader, "merge-keep-label")
    }

    /// The accessible group name for a field row's read-only "which record holds this value" radios.
    #[must_use]
    pub fn merge_radio_group_label(&self) -> String {
        fl!(self.loader, "merge-radio-group-label")
    }

    /// The wizard's "Cancel" button label.
    #[must_use]
    pub fn merge_cancel(&self) -> String {
        fl!(self.loader, "merge-cancel")
    }

    /// The wizard's "Merge (reversible)" submit button label.
    #[must_use]
    pub fn merge_submit(&self) -> String {
        fl!(self.loader, "merge-submit")
    }

    /// The wizard's "Back to duplicates" button label.
    #[must_use]
    pub fn merge_back(&self) -> String {
        fl!(self.loader, "merge-back")
    }

    /// The "Reason for merge" field label in the wizard foot.
    #[must_use]
    pub fn merge_reason_label(&self) -> String {
        fl!(self.loader, "merge-reason-label")
    }

    /// The faint hint beside the reason label ("recorded on the merge event").
    #[must_use]
    pub fn merge_reason_hint(&self) -> String {
        fl!(self.loader, "merge-reason-hint")
    }

    /// The assisted-import wizard heading.
    #[must_use]
    pub fn import_heading(&self) -> String {
        fl!(self.loader, "import-heading")
    }

    /// The five wizard stage names, in order (Source, Records, Confirm, Save scan, Summary).
    #[must_use]
    pub fn import_stages(&self) -> [String; 5] {
        [
            fl!(self.loader, "import-stage-source"),
            fl!(self.loader, "import-stage-records"),
            fl!(self.loader, "import-stage-confirm"),
            fl!(self.loader, "import-stage-save"),
            fl!(self.loader, "import-stage-summary"),
        ]
    }

    /// The Source-stage plugin-selector label.
    #[must_use]
    pub fn import_source_label(&self) -> String {
        fl!(self.loader, "import-source-label")
    }

    /// The Source-stage URL field label.
    #[must_use]
    pub fn import_url_label(&self) -> String {
        fl!(self.loader, "import-url-label")
    }

    /// The Source-stage URL field placeholder.
    #[must_use]
    pub fn import_url_placeholder(&self) -> String {
        fl!(self.loader, "import-url-placeholder")
    }

    /// The Fetch button label.
    #[must_use]
    pub fn import_fetch(&self) -> String {
        fl!(self.loader, "import-fetch")
    }

    /// The "no assisted-import plugins installed" message.
    #[must_use]
    pub fn import_no_plugins(&self) -> String {
        fl!(self.loader, "import-no-plugins")
    }

    /// The "importing…" progress text shown while the session runs.
    #[must_use]
    pub fn import_running(&self) -> String {
        fl!(self.loader, "import-running")
    }

    /// The Back button label (step back one stage: confirm → records, save-scan → confirm).
    #[must_use]
    pub fn import_back(&self) -> String {
        fl!(self.loader, "import-back")
    }

    /// The "Start over" button label (abandon this page and return to the URL entry).
    #[must_use]
    pub fn import_start_over(&self) -> String {
        fl!(self.loader, "import-start-over")
    }

    /// The confirm-stage scan-URL field label.
    #[must_use]
    pub fn import_scan_url_label(&self) -> String {
        fl!(self.loader, "import-scan-url-label")
    }

    /// The confirm-stage scan-URL field placeholder.
    #[must_use]
    pub fn import_scan_url_placeholder(&self) -> String {
        fl!(self.loader, "import-scan-url-placeholder")
    }

    /// The records-table heading.
    #[must_use]
    pub fn import_records_heading(&self) -> String {
        fl!(self.loader, "import-records-heading")
    }

    /// The records-table column headers (name, details, status).
    #[must_use]
    pub fn import_records_headers(&self) -> Vec<String> {
        vec![
            fl!(self.loader, "import-col-name"),
            fl!(self.loader, "import-col-detail"),
            fl!(self.loader, "import-col-status"),
            String::new(),
        ]
    }

    /// A record's status chip label (pending / imported / skipped).
    #[must_use]
    pub fn import_status(&self, status: crate::screens::ImportRowStatus) -> String {
        match status {
            crate::screens::ImportRowStatus::Pending => fl!(self.loader, "import-status-pending"),
            crate::screens::ImportRowStatus::Imported => fl!(self.loader, "import-status-imported"),
            crate::screens::ImportRowStatus::Skipped => fl!(self.loader, "import-status-skipped"),
        }
    }

    /// The records-row "Review" action label.
    #[must_use]
    pub fn import_review(&self) -> String {
        fl!(self.loader, "import-review")
    }

    /// The records-stage "Finish" action label.
    #[must_use]
    pub fn import_finish(&self) -> String {
        fl!(self.loader, "import-finish")
    }

    /// The confirm-stage heading.
    #[must_use]
    pub fn import_confirm_heading(&self) -> String {
        fl!(self.loader, "import-confirm-heading")
    }

    /// The provenance-preview card heading.
    #[must_use]
    pub fn import_provenance_heading(&self) -> String {
        fl!(self.loader, "import-provenance-heading")
    }

    /// The provenance-preview row labels: operator, source, repository, citation, external id,
    /// confidence.
    #[must_use]
    pub fn import_prov_labels(&self) -> [String; 6] {
        [
            fl!(self.loader, "import-prov-operator"),
            fl!(self.loader, "import-prov-source"),
            fl!(self.loader, "import-prov-repository"),
            fl!(self.loader, "import-prov-citation"),
            fl!(self.loader, "import-prov-external-id"),
            fl!(self.loader, "import-prov-confidence"),
        ]
    }

    /// The "software agent" badge label in the provenance preview.
    #[must_use]
    pub fn import_software_agent(&self) -> String {
        fl!(self.loader, "import-software-agent")
    }

    /// The summary heading.
    #[must_use]
    pub fn import_summary_heading(&self) -> String {
        fl!(self.loader, "import-summary-heading")
    }

    /// The "{n} imported" summary count.
    #[must_use]
    pub fn import_summary_imported(&self, count: usize) -> String {
        fl!(self.loader, "import-summary-imported", count = count)
    }

    /// The "{n} skipped" summary count.
    #[must_use]
    pub fn import_summary_skipped(&self, count: u32) -> String {
        fl!(self.loader, "import-summary-skipped", count = count)
    }

    /// The "Import another" summary action label.
    #[must_use]
    pub fn import_another(&self) -> String {
        fl!(self.loader, "import-another")
    }

    /// The wizard Cancel action label.
    #[must_use]
    pub fn import_cancel(&self) -> String {
        fl!(self.loader, "import-cancel")
    }

    /// The save-scan dialog labels, for the shared [`MediaSaveDialog`](crate::components::MediaSaveDialog).
    #[must_use]
    pub fn import_save_labels(&self) -> crate::components::MediaSaveLabels {
        crate::components::MediaSaveLabels {
            title: fl!(self.loader, "import-save-title"),
            choose_category: fl!(self.loader, "import-save-choose-category"),
            category: fl!(self.loader, "import-save-category"),
            subfolder: fl!(self.loader, "import-save-subfolder"),
            filename: fl!(self.loader, "import-save-filename"),
            path_preview: fl!(self.loader, "import-save-path-preview"),
            save: fl!(self.loader, "import-stage-save"),
            cancel: fl!(self.loader, "import-cancel"),
        }
    }

    /// The bulk-export wizard heading.
    #[must_use]
    pub fn export_heading(&self) -> String {
        fl!(self.loader, "export-heading")
    }

    /// The three wizard stage names, in order (Destination, Running, Summary).
    #[must_use]
    pub fn export_stages(&self) -> [String; 3] {
        [
            fl!(self.loader, "export-stage-destination"),
            fl!(self.loader, "export-stage-running"),
            fl!(self.loader, "export-stage-summary"),
        ]
    }

    /// The Destination-stage heading.
    #[must_use]
    pub fn export_destination_heading(&self) -> String {
        fl!(self.loader, "export-destination-heading")
    }

    /// The Destination-stage plugin-selector label.
    #[must_use]
    pub fn export_plugin_label(&self) -> String {
        fl!(self.loader, "export-plugin-label")
    }

    /// The "no bulk-export plugins installed" message.
    #[must_use]
    pub fn export_no_plugins(&self) -> String {
        fl!(self.loader, "export-no-plugins")
    }

    /// The destination field label.
    #[must_use]
    pub fn export_destination_label(&self) -> String {
        fl!(self.loader, "export-destination-label")
    }

    /// The destination field placeholder.
    #[must_use]
    pub fn export_destination_placeholder(&self) -> String {
        fl!(self.loader, "export-destination-placeholder")
    }

    /// The live destination-preview label.
    #[must_use]
    pub fn export_destination_preview(&self) -> String {
        fl!(self.loader, "export-destination-preview")
    }

    /// The hint shown beside a directory destination (the plugin names the file).
    #[must_use]
    pub fn export_destination_dir_hint(&self) -> String {
        fl!(self.loader, "export-destination-dir-hint")
    }

    /// The Export action label.
    #[must_use]
    pub fn export_run(&self) -> String {
        fl!(self.loader, "export-run")
    }

    /// The Running-stage heading.
    #[must_use]
    pub fn export_running_heading(&self) -> String {
        fl!(self.loader, "export-running-heading")
    }

    /// The step name shown before the plugin reports its first step.
    #[must_use]
    pub fn export_progress_starting(&self) -> String {
        fl!(self.loader, "export-progress-starting")
    }

    /// The progress count: "{processed} of {total}", or just the processed count while the plugin
    /// does not yet know the total.
    #[must_use]
    pub fn export_progress_count(&self, processed: u32, total: Option<u32>) -> String {
        match total {
            Some(total) => fl!(
                self.loader,
                "export-progress-count",
                processed = processed,
                total = total
            ),
            None => fl!(self.loader, "export-progress-processed", processed = processed),
        }
    }

    /// The Cancel action label.
    #[must_use]
    pub fn export_cancel(&self) -> String {
        fl!(self.loader, "export-cancel")
    }

    /// The Summary-stage heading.
    #[must_use]
    pub fn export_summary_heading(&self) -> String {
        fl!(self.loader, "export-summary-heading")
    }

    /// The "{n} records written" summary count.
    #[must_use]
    pub fn export_summary_records(&self, count: u32) -> String {
        fl!(self.loader, "export-summary-records", count = count)
    }

    /// The summary's destination-row label.
    #[must_use]
    pub fn export_summary_destination(&self) -> String {
        fl!(self.loader, "export-summary-destination")
    }

    /// The "Export again" action label.
    #[must_use]
    pub fn export_another(&self) -> String {
        fl!(self.loader, "export-another")
    }

    /// The failed-export heading.
    #[must_use]
    pub fn export_error_heading(&self) -> String {
        fl!(self.loader, "export-error-heading")
    }

    /// The message shown when the run ends without an outcome (its channel was dropped).
    #[must_use]
    pub fn export_failed_unknown(&self) -> String {
        fl!(self.loader, "export-failed-unknown")
    }

    /// The cancelled-export heading.
    #[must_use]
    pub fn export_cancelled_heading(&self) -> String {
        fl!(self.loader, "export-cancelled-heading")
    }

    /// The cancelled-export explanation.
    #[must_use]
    pub fn export_cancelled_message(&self) -> String {
        fl!(self.loader, "export-cancelled-message")
    }

    /// Resolves a rail/navigation label id (`nav-*`) to its display text; unknown ids render as-is.
    #[must_use]
    pub fn rail_label(&self, id: &str) -> String {
        match id {
            "nav-dashboard" => fl!(self.loader, "nav-dashboard"),
            "nav-people" => fl!(self.loader, "nav-people"),
            "nav-families" => fl!(self.loader, "nav-families"),
            "nav-events" => fl!(self.loader, "nav-events"),
            "nav-places" => fl!(self.loader, "nav-places"),
            "nav-sources" => fl!(self.loader, "nav-sources"),
            "nav-citations" => fl!(self.loader, "nav-citations"),
            "nav-repositories" => fl!(self.loader, "nav-repositories"),
            "nav-media" => fl!(self.loader, "nav-media"),
            "nav-notes" => fl!(self.loader, "nav-notes"),
            "nav-tags" => fl!(self.loader, "nav-tags"),
            "nav-dna-tests" => fl!(self.loader, "nav-dna-tests"),
            "nav-dna-matches" => fl!(self.loader, "nav-dna-matches"),
            "nav-pedigree" => fl!(self.loader, "nav-pedigree"),
            "nav-merge" => fl!(self.loader, "nav-merge"),
            "nav-import" => fl!(self.loader, "nav-import"),
            "nav-export" => fl!(self.loader, "nav-export"),
            "nav-geography" => fl!(self.loader, "nav-geography"),
            "nav-plugins" => fl!(self.loader, "nav-plugins"),
            "nav-preferences" => fl!(self.loader, "nav-preferences"),
            "nav-help" => fl!(self.loader, "nav-help"),
            other => other.to_owned(),
        }
    }

    /// Resolves a shortcut description label id to its display text. `sc-*` ids resolve here;
    /// `g`-prefix navigation rows reuse the rail labels (`nav-*`), delegated to [`Self::rail_label`].
    #[must_use]
    pub fn shortcut_label(&self, id: &str) -> String {
        match id {
            "sc-command-palette" => fl!(self.loader, "sc-command-palette"),
            "sc-new-record" => fl!(self.loader, "sc-new-record"),
            "sc-find" => fl!(self.loader, "sc-find"),
            "sc-undo" => fl!(self.loader, "sc-undo"),
            "sc-redo" => fl!(self.loader, "sc-redo"),
            "sc-switch-tab" => fl!(self.loader, "sc-switch-tab"),
            "sc-dock-tab" => fl!(self.loader, "sc-dock-tab"),
            "sc-help" => fl!(self.loader, "sc-help"),
            "sc-close" => fl!(self.loader, "sc-close"),
            "sc-move-up" => fl!(self.loader, "sc-move-up"),
            "sc-move-down" => fl!(self.loader, "sc-move-down"),
            "sc-open" => fl!(self.loader, "sc-open"),
            "sc-prev-record" => fl!(self.loader, "sc-prev-record"),
            "sc-next-record" => fl!(self.loader, "sc-next-record"),
            "sc-prev-tab" => fl!(self.loader, "sc-prev-tab"),
            "sc-next-tab" => fl!(self.loader, "sc-next-tab"),
            "sc-first-tab" => fl!(self.loader, "sc-first-tab"),
            "sc-last-tab" => fl!(self.loader, "sc-last-tab"),
            "sc-add-source" => fl!(self.loader, "sc-add-source"),
            "sc-edit" => fl!(self.loader, "sc-edit"),
            "sc-quit" => fl!(self.loader, "sc-quit"),
            "sc-close-tab" => fl!(self.loader, "sc-close-tab"),
            other => self.rail_label(other),
        }
    }

    /// The close-current-tab confirm dialog's heading (`⌘W` on a draft tab).
    #[must_use]
    pub fn close_tab_confirm_title(&self) -> String {
        fl!(self.loader, "close-tab-confirm-title")
    }

    /// The close-current-tab confirm dialog's body, naming the draft tab that would be discarded.
    #[must_use]
    pub fn close_tab_confirm_body(&self, label: &str) -> String {
        fl!(self.loader, "close-tab-confirm-body", label = label)
    }

    /// The close-current-tab confirm dialog's confirm action label.
    #[must_use]
    pub fn close_tab_confirm_confirm(&self) -> String {
        fl!(self.loader, "close-tab-confirm-confirm")
    }

    /// The close-current-tab confirm dialog's cancel action label.
    #[must_use]
    pub fn close_tab_confirm_cancel(&self) -> String {
        fl!(self.loader, "close-tab-confirm-cancel")
    }

    /// The quit confirm dialog's heading (`⌘Q` with an unsaved draft open).
    #[must_use]
    pub fn quit_confirm_title(&self) -> String {
        fl!(self.loader, "quit-confirm-title")
    }

    /// The quit confirm dialog's body.
    #[must_use]
    pub fn quit_confirm_body(&self) -> String {
        fl!(self.loader, "quit-confirm-body")
    }

    /// The quit confirm dialog's confirm action label.
    #[must_use]
    pub fn quit_confirm_confirm(&self) -> String {
        fl!(self.loader, "quit-confirm-confirm")
    }

    /// The quit confirm dialog's cancel action label.
    #[must_use]
    pub fn quit_confirm_cancel(&self) -> String {
        fl!(self.loader, "quit-confirm-cancel")
    }

    /// The plugin manager's heading.
    #[must_use]
    pub fn plugin_manager_title(&self) -> String {
        fl!(self.loader, "plugin-manager-title")
    }

    /// The sandboxing explainer shown above the plugin table.
    #[must_use]
    pub fn plugin_manager_note(&self) -> String {
        fl!(self.loader, "plugin-manager-note")
    }

    /// The "Reload from disk" button label.
    #[must_use]
    pub fn plugin_reload(&self) -> String {
        fl!(self.loader, "plugin-reload")
    }

    /// The plugin table's column headers, in display order.
    #[must_use]
    pub fn plugin_table_headers(&self) -> Vec<String> {
        vec![
            fl!(self.loader, "plugin-col-name"),
            fl!(self.loader, "plugin-col-enabled"),
            fl!(self.loader, "plugin-col-capabilities"),
            fl!(self.loader, "plugin-col-trust"),
        ]
    }

    /// The accessible name for a plugin's enabled/disabled switch.
    #[must_use]
    pub fn plugin_enabled_switch_label(&self, plugin_id: &str) -> String {
        fl!(self.loader, "plugin-enabled-switch", plugin = plugin_id)
    }

    /// The "On"/"Off" text on an enabled/disabled switch (colour is never the only signal).
    #[must_use]
    pub fn plugin_enabled_state(&self, enabled: bool) -> String {
        if enabled {
            fl!(self.loader, "plugin-state-on")
        } else {
            fl!(self.loader, "plugin-state-off")
        }
    }

    /// A plugin's role, resolved from [`genealogy_plugin_host::PluginRole`] to display text.
    #[must_use]
    pub fn plugin_role_label(&self, role: genealogy_plugin_host::PluginRole) -> String {
        match role {
            genealogy_plugin_host::PluginRole::BulkImport => fl!(self.loader, "plugin-role-bulk-import"),
            genealogy_plugin_host::PluginRole::BulkExport => fl!(self.loader, "plugin-role-bulk-export"),
            genealogy_plugin_host::PluginRole::UiPanel => fl!(self.loader, "plugin-role-ui-panel"),
            genealogy_plugin_host::PluginRole::AssistedImport => fl!(self.loader, "plugin-role-assisted-import"),
            genealogy_plugin_host::PluginRole::TestFixture => fl!(self.loader, "plugin-role-test-fixture"),
            genealogy_plugin_host::PluginRole::Unknown => fl!(self.loader, "plugin-role-unknown"),
        }
    }

    /// A capability, resolved from [`genealogy_plugin_host::Capability`] to display text (the badge
    /// label that makes the badge's colour redundant).
    #[must_use]
    pub fn plugin_capability_label(&self, capability: genealogy_plugin_host::Capability) -> String {
        match capability {
            genealogy_plugin_host::Capability::Log => fl!(self.loader, "plugin-cap-log"),
            genealogy_plugin_host::Capability::Query => fl!(self.loader, "plugin-cap-query"),
            genealogy_plugin_host::Capability::Commands => fl!(self.loader, "plugin-cap-commands"),
            genealogy_plugin_host::Capability::Progress => fl!(self.loader, "plugin-cap-progress"),
            genealogy_plugin_host::Capability::ImportSource => fl!(self.loader, "plugin-cap-import-source"),
            genealogy_plugin_host::Capability::ExportSink => fl!(self.loader, "plugin-cap-export-sink"),
            genealogy_plugin_host::Capability::Net => fl!(self.loader, "plugin-cap-net"),
            genealogy_plugin_host::Capability::MediaStore => fl!(self.loader, "plugin-cap-media-store"),
            genealogy_plugin_host::Capability::Ai => fl!(self.loader, "plugin-cap-ai"),
            genealogy_plugin_host::Capability::Present => fl!(self.loader, "plugin-cap-present"),
        }
    }

    /// The read-only trust-tier label (ADR 0007 §9 signing/trust tiers are Phase 8; every plugin
    /// discovered today is unsigned, so this is the only tier the manager can honestly show).
    #[must_use]
    pub fn plugin_trust_unsigned(&self) -> String {
        fl!(self.loader, "plugin-trust-unsigned")
    }

    /// The note explaining that full trust tiers/signing land later.
    #[must_use]
    pub fn plugin_trust_note(&self) -> String {
        fl!(self.loader, "plugin-trust-note")
    }

    /// The host-api version caption under a plugin's name (e.g. "host-api 0.12.0").
    #[must_use]
    pub fn plugin_host_api_version(&self, version: &str) -> String {
        fl!(self.loader, "plugin-host-api-version", version = version)
    }

    /// The empty state shown when the plugins directory has no components.
    #[must_use]
    pub fn plugin_manager_empty(&self) -> String {
        fl!(self.loader, "plugin-manager-empty")
    }
}

#[cfg(test)]
mod tests {
    use super::Chrome;

    #[test]
    fn resolves_chrome_strings() {
        let en = Chrome::with_languages(None, &["en".parse().expect("tag")]);
        assert_eq!(en.nav_people(), "People");
        assert_eq!(en.theme_mode_label(genealogy_app::ThemeMode::System), "System");
        assert_eq!(en.search_clear(), "Clear search");
        assert_eq!(
            en.theme_mode_status(genealogy_app::ThemeMode::System, true),
            "system (dark)"
        );
        assert_eq!(en.theme_mode_status(genealogy_app::ThemeMode::Light, true), "light");
        assert_eq!(en.theme_mode_status(genealogy_app::ThemeMode::Dark, false), "dark");
        let no = Chrome::with_languages(None, &["no".parse().expect("tag")]);
        assert_eq!(no.nav_people(), "Personer");
        assert_eq!(no.theme_mode_label(genealogy_app::ThemeMode::Light), "Lyst");
        assert_eq!(
            no.theme_mode_status(genealogy_app::ThemeMode::System, false),
            "system (lyst)"
        );
    }
}
