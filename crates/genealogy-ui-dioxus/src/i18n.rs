//! Chrome localization for the Dioxus renderer (ADR 0003, ADR 0008 §3).
//!
//! The renderer owns its own catalogue (window/navigation labels and renderer-level errors), layered
//! over runtime overrides exactly like the other frontends. Data strings (names, sex, field labels,
//! application errors) come from [`genealogy_ui::Localizer`]; this catalogue is only the GUI's chrome.

use std::path::Path;

use genealogy_app::config;
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

    /// The "Plugin form" navigation label.
    #[must_use]
    pub fn nav_plugin(&self) -> String {
        fl!(self.loader, "nav-plugin")
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

    /// A renderer-level plugin failure (technical detail passed through).
    #[must_use]
    pub fn plugin_error(&self, detail: &str) -> String {
        fl!(self.loader, "plugin-error", detail = detail)
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
