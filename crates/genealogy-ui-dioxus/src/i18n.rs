//! Chrome localization for the Dioxus renderer (ADR 0003, ADR 0008 §3).
//!
//! The renderer owns its own catalogue (window/navigation labels and renderer-level errors), layered
//! over runtime overrides exactly like the other frontends. Data strings (names, sex, field labels,
//! application errors) come from [`genealogy_ui::Localizer`]; this catalogue is only the GUI's chrome.

use std::path::Path;

use genealogy_app::config;
use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::{AssetsMultiplexor, DesktopLanguageRequester, FileSystemAssets, I18nAssets, LanguageLoader};
use i18n_embed_fl::fl;
use rust_embed::RustEmbed;
use tracing::warn;
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
        let assets = build_assets(workspace_dir);
        let available = loader.available_languages(&assets).unwrap_or_default();
        let chain: Vec<LanguageIdentifier> = fallback_chain(requested, loader.fallback_language())
            .into_iter()
            .filter(|lang| available.contains(lang))
            .collect();
        if let Err(error) = loader.load_languages(&assets, &chain) {
            warn!(%error, "failed to load chrome localization; falling back to message keys");
        }
        loader.set_use_isolating(false);
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

    /// A renderer-level plugin failure (technical detail passed through).
    #[must_use]
    pub fn plugin_error(&self, detail: &str) -> String {
        fl!(self.loader, "plugin-error", detail = detail)
    }
}

fn fallback_chain(requested: &[LanguageIdentifier], fallback: &LanguageIdentifier) -> Vec<LanguageIdentifier> {
    let mut chain: Vec<LanguageIdentifier> = Vec::new();
    for lang in requested {
        push_unique(&mut chain, lang.clone());
        if let Ok(base) = lang.language.as_str().parse::<LanguageIdentifier>() {
            push_unique(&mut chain, base);
        }
        let language = lang.language.as_str();
        if (language == "nb" || language == "nn")
            && let Ok(generic) = "no".parse::<LanguageIdentifier>()
        {
            push_unique(&mut chain, generic);
        }
    }
    push_unique(&mut chain, fallback.clone());
    chain
}

fn push_unique(chain: &mut Vec<LanguageIdentifier>, lang: LanguageIdentifier) {
    if !chain.contains(&lang) {
        chain.push(lang);
    }
}

fn build_assets(workspace_dir: Option<&Path>) -> AssetsMultiplexor {
    let mut sources: Vec<Box<dyn I18nAssets + Send + Sync>> = Vec::new();
    if let Some(dir) = workspace_dir {
        push_filesystem(&mut sources, &dir.join("i18n"));
    }
    if let Ok(shared) = config::shared_i18n_dir() {
        push_filesystem(&mut sources, &shared);
    }
    sources.push(Box::new(Embedded));
    AssetsMultiplexor::new(sources)
}

fn push_filesystem(sources: &mut Vec<Box<dyn I18nAssets + Send + Sync>>, dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    match FileSystemAssets::try_new(dir) {
        Ok(assets) => sources.push(Box::new(assets)),
        Err(error) => warn!(dir = %dir.display(), %error, "skipping unreadable i18n override directory"),
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
