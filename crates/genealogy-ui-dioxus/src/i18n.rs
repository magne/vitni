//! Chrome localization for the Dioxus renderer (ADR 0003, ADR 0008 §3).
//!
//! The renderer owns its own catalogue (window/navigation labels and renderer-level errors), layered
//! over runtime overrides exactly like the other frontends. Data strings (names, sex, field labels,
//! application errors) come from [`genealogy_ui::Localizer`]; this catalogue is only the GUI's chrome.

use std::path::Path;

use genealogy_app::{DateFormat, NumberFormat, config};
use genealogy_ui::ShortcutGroup;
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
    /// Builds the chrome localizer, layering the open workspace's `i18n/` override at top priority.
    #[must_use]
    pub fn for_workspace(workspace_dir: &Path) -> Self {
        Self::with_languages(Some(workspace_dir), &DesktopLanguageRequester::requested_languages())
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

    /// The command-palette stub hint shown until live search lands.
    #[must_use]
    pub fn palette_hint(&self) -> String {
        fl!(self.loader, "palette-hint")
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

    /// The accessible name for the Preferences settings sub-nav.
    #[must_use]
    pub fn prefs_nav_label(&self) -> String {
        fl!(self.loader, "prefs-nav-label")
    }

    /// The localized heading for a Preferences section id (`identity`/`appearance`/`locale`/
    /// `formats`/`defaults`); an unknown id falls back to the identity section's heading.
    #[must_use]
    pub fn prefs_section_label(&self, id: &str) -> String {
        match id {
            "appearance" => fl!(self.loader, "prefs-section-appearance"),
            "locale" => fl!(self.loader, "prefs-section-locale"),
            "formats" => fl!(self.loader, "prefs-section-formats"),
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

    /// The badge naming the currently-active workspace.
    #[must_use]
    pub fn prefs_workspace_active(&self) -> String {
        fl!(self.loader, "prefs-workspace-active")
    }

    /// The accessible name for a non-active workspace's "switch to" button.
    #[must_use]
    pub fn prefs_switch_to(&self, name: &str) -> String {
        fl!(self.loader, "prefs-switch-to", name = name)
    }

    /// The "Could not switch workspace: {detail}" error message.
    #[must_use]
    pub fn prefs_switch_error(&self, detail: &str) -> String {
        fl!(self.loader, "prefs-switch-error", detail = detail)
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

    /// The duplicates table's "Confidence" column header.
    #[must_use]
    pub fn merge_col_confidence(&self) -> String {
        fl!(self.loader, "merge-col-confidence")
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
            other => self.rail_label(other),
        }
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
