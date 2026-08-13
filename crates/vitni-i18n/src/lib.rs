//! Shared Fluent localization plumbing for the Vitni frontends (ADR 0003).
//!
//! Every frontend (CLI, the framework-neutral presentation layer, each renderer) embeds its own
//! `.ftl` catalogue as a baseline and overrides it at runtime, highest priority first, by
//! per-directory `.ftl` files: the open **workspace** dir, the **shared application** dir, then the
//! **embedded** baseline (which always ships the complete fallback language, so the UI is never left
//! unlocalized). The locale request is expanded into a fallback chain before loading, so a
//! `nb-NO`/`nn-NO` request resolves to the generic `no` catalogue and finally the `en` baseline.
//!
//! That negotiation, asset layering, and load are identical across frontends, so they live here.
//! What stays per-crate is everything frontend-specific: the `.ftl` message vocabularies, the
//! `fluent_language_loader!()` invocation (it reads each crate's own `i18n.toml` for the domain and
//! fallback language), the embedded baseline, and the typed wrappers that resolve message keys.
//!
//! This crate has no dependency on any other Vitni crate (the shared-override directory is
//! passed in by the caller), so it sits cleanly under the `app → ui → ui-framework` direction.

use std::path::Path;

use i18n_embed::fluent::FluentLanguageLoader;
use i18n_embed::{AssetsMultiplexor, FileSystemAssets, I18nAssets, LanguageLoader};
use tracing::warn;
use unic_langid::LanguageIdentifier;

/// Negotiates `requested` against the catalogue and loads it into `loader`.
///
/// Composes the override layers (workspace, shared) over `embedded`, expands `requested` into a
/// fallback chain, filters it to the languages that actually ship a catalogue, and loads them.
/// `workspace_dir` is a workspace root (its `i18n/` subdirectory is layered); `shared_dir` already
/// points at an `i18n/` directory. Bidi isolation marks are disabled afterwards, since both terminal
/// and HTML output render the loaded strings inline.
pub fn init(
    loader: &FluentLanguageLoader,
    workspace_dir: Option<&Path>,
    shared_dir: Option<&Path>,
    requested: &[LanguageIdentifier],
    embedded: Box<dyn I18nAssets + Send + Sync>,
) {
    let assets = layered_assets(workspace_dir, shared_dir, embedded);
    let available = loader.available_languages(&assets).unwrap_or_default();
    let chain: Vec<LanguageIdentifier> = fallback_chain(requested, loader.fallback_language())
        .into_iter()
        .filter(|lang| available.contains(lang))
        .collect();
    // `chain` always ends with the fallback language, which ships a catalogue, so it is non-empty;
    // lookups fall through it in order.
    if let Err(error) = loader.load_languages(&assets, &chain) {
        warn!(%error, "failed to load localization; falling back to message keys");
    }
    loader.set_use_isolating(false);
}

/// Composes the asset layers, highest priority first: workspace override, shared app override,
/// embedded baseline. `workspace_dir` is a workspace root (its `i18n/` subdirectory is used);
/// `shared_dir` already points at an `i18n/` directory.
#[must_use]
pub fn layered_assets(
    workspace_dir: Option<&Path>,
    shared_dir: Option<&Path>,
    embedded: Box<dyn I18nAssets + Send + Sync>,
) -> AssetsMultiplexor {
    let mut sources: Vec<Box<dyn I18nAssets + Send + Sync>> = Vec::new();
    if let Some(dir) = workspace_dir {
        push_filesystem(&mut sources, &dir.join("i18n"));
    }
    if let Some(dir) = shared_dir {
        push_filesystem(&mut sources, dir);
    }
    sources.push(embedded);
    AssetsMultiplexor::new(sources)
}

/// Expands requested languages into an ordered, deduplicated load/fallback chain, ending with the
/// `fallback` language. Each request contributes its region form, its language-only form, and — for
/// Norwegian Bokmål/Nynorsk (`nb`/`nn`) — the generic `no` macrolanguage, so `nb-NO`/`nn-NO` resolve
/// to the `no` catalogue. Languages without a catalogue are simply skipped at load time.
#[must_use]
pub fn fallback_chain(requested: &[LanguageIdentifier], fallback: &LanguageIdentifier) -> Vec<LanguageIdentifier> {
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

/// Appends `lang` to `chain` if not already present, preserving fallback order.
fn push_unique(chain: &mut Vec<LanguageIdentifier>, lang: LanguageIdentifier) {
    if !chain.contains(&lang) {
        chain.push(lang);
    }
}

/// Adds a filesystem override layer if the directory exists (overrides are optional).
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
    use super::fallback_chain;
    use unic_langid::LanguageIdentifier;

    fn lang(tag: &str) -> LanguageIdentifier {
        tag.parse().expect("valid language tag")
    }

    fn tags(chain: &[LanguageIdentifier]) -> Vec<String> {
        chain.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn norwegian_region_expands_to_base_then_macrolanguage_then_fallback() {
        let chain = fallback_chain(&[lang("nb-NO")], &lang("en"));
        assert_eq!(tags(&chain), ["nb-NO", "nb", "no", "en"].map(String::from));
    }

    #[test]
    fn nynorsk_also_reaches_the_generic_no_catalogue() {
        let chain = fallback_chain(&[lang("nn-NO")], &lang("en"));
        assert_eq!(tags(&chain), ["nn-NO", "nn", "no", "en"].map(String::from));
    }

    #[test]
    fn duplicates_are_collapsed_and_fallback_is_appended_once() {
        let chain = fallback_chain(&[lang("en"), lang("en")], &lang("en"));
        assert_eq!(tags(&chain), ["en"].map(String::from));
    }

    #[test]
    fn an_empty_request_yields_just_the_fallback() {
        let chain = fallback_chain(&[], &lang("en"));
        assert_eq!(tags(&chain), ["en"].map(String::from));
    }
}
