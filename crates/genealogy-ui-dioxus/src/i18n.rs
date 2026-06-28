//! Chrome localization for the Dioxus renderer (ADR 0003, ADR 0008 §3).
//!
//! The renderer owns its own catalogue (window/navigation labels and renderer-level errors), layered
//! over runtime overrides exactly like the other frontends. Data strings (names, sex, field labels,
//! application errors) come from [`genealogy_ui::Localizer`]; this catalogue is only the GUI's chrome.

use std::path::Path;

use genealogy_app::config;
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

    /// The accessible name for a list's sort control.
    #[must_use]
    pub fn list_sort(&self) -> String {
        fl!(self.loader, "list-sort")
    }

    /// The placeholder for an empty related-item detail tab.
    #[must_use]
    pub fn tab_empty(&self) -> String {
        fl!(self.loader, "tab-empty")
    }

    /// The localized sort options for a list, in display order.
    #[must_use]
    pub fn sort_options(&self) -> Vec<(RowSort, String)> {
        vec![
            (RowSort::IdAsc, fl!(self.loader, "sort-id-asc")),
            (RowSort::IdDesc, fl!(self.loader, "sort-id-desc")),
            (RowSort::TitleAsc, fl!(self.loader, "sort-name-asc")),
            (RowSort::TitleDesc, fl!(self.loader, "sort-name-desc")),
        ]
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

    /// The accessible name for the theme toggle.
    #[must_use]
    pub fn aria_theme_toggle(&self) -> String {
        fl!(self.loader, "aria-theme-toggle")
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

    /// The accessible name for the "open another record" control.
    #[must_use]
    pub fn new_tab_label(&self) -> String {
        fl!(self.loader, "new-tab-label")
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

    /// The heading for a help-overlay column.
    #[must_use]
    pub fn help_column(&self, group: ShortcutGroup) -> String {
        match group {
            ShortcutGroup::Global => fl!(self.loader, "help-col-global"),
            ShortcutGroup::Navigation => fl!(self.loader, "help-col-goto"),
            ShortcutGroup::WithinScreen => fl!(self.loader, "help-col-within"),
        }
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
}

#[cfg(test)]
mod tests {
    use super::Chrome;

    #[test]
    fn resolves_chrome_strings() {
        let en = Chrome::with_languages(None, &["en".parse().expect("tag")]);
        assert_eq!(en.nav_people(), "People");
        let no = Chrome::with_languages(None, &["no".parse().expect("tag")]);
        assert_eq!(no.nav_people(), "Personer");
    }
}
