//! Localization for the presentation layer (ADR 0003).
//!
//! A framework-neutral [`Localizer`] generalized from the CLI's: a Fluent catalogue is embedded as
//! the baseline and overridden at runtime, highest priority first, by per-directory `.ftl` files —
//! the open **workspace** dir, the **shared application** dir, then the **embedded** baseline (which
//! always carries the complete fallback language, so the UI is never left unlocalized). The system
//! locale is negotiated against the available languages, expanding a `nb-NO`/`nn-NO` request to the
//! generic `no` catalogue and finally the `en` baseline.
//!
//! A framework renderer owns its own chrome catalogue (ADR 0008 §3); this crate owns the strings the
//! view-models need — value labels, field labels, and the localized [`AppError`] surface — keeping
//! `genealogy-app`/`genealogy-core` free of UI text.

use std::path::Path;

use genealogy_app::{AppError, DbError, Sex, config};
use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::{AssetsMultiplexor, DesktopLanguageRequester, FileSystemAssets, I18nAssets, LanguageLoader};
use i18n_embed_fl::fl;
use rust_embed::RustEmbed;
use tracing::warn;
use unic_langid::LanguageIdentifier;

/// The embedded baseline catalogue (compiled into the crate; complete fallback language).
#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Embedded;

/// The loaded message catalogue: resolves every user-facing string the view-models emit.
pub struct Localizer {
    loader: FluentLanguageLoader,
}

impl Localizer {
    /// Builds a localizer over the baseline layers (shared app dir over the embedded baseline),
    /// negotiating the system locale.
    #[must_use]
    pub fn baseline() -> Self {
        Self::build(None)
    }

    /// Builds a localizer that layers the open workspace's `i18n/` override at top priority.
    #[must_use]
    pub fn for_workspace(workspace_dir: &Path) -> Self {
        Self::build(Some(workspace_dir))
    }

    fn build(workspace_dir: Option<&Path>) -> Self {
        Self::with_languages(workspace_dir, &DesktopLanguageRequester::requested_languages())
    }

    /// Builds a localizer for an explicit set of requested languages, expanded into a fallback chain
    /// (region → language → macrolanguage → `en`) before loading. Separated from [`Self::build`] so a
    /// renderer (or a test) can request languages deterministically instead of host-locale dependent.
    #[must_use]
    pub fn with_languages(workspace_dir: Option<&Path>, requested: &[LanguageIdentifier]) -> Self {
        let loader = fluent_language_loader!();
        let assets = build_assets(workspace_dir);
        let available = loader.available_languages(&assets).unwrap_or_default();
        let chain: Vec<LanguageIdentifier> = fallback_chain(requested, loader.fallback_language())
            .into_iter()
            .filter(|lang| available.contains(lang))
            .collect();
        // `chain` always ends with the fallback language, which ships a catalogue, so it is
        // non-empty; lookups fall through it in order.
        if let Err(error) = loader.load_languages(&assets, &chain) {
            warn!(%error, "failed to load localization; falling back to message keys");
        }
        loader.set_use_isolating(false);
        Self { loader }
    }

    /// The negotiated UI language as a BCP-47 tag (e.g. `en`, `no`). Frontends pass this to plugins
    /// so plugin-supplied UI text can be localized (ADR 0012 §5).
    #[must_use]
    pub fn language_tag(&self) -> String {
        self.loader.current_language().to_string()
    }

    /// The display name, or the localized "no name" placeholder when absent.
    #[must_use]
    pub fn display_name(&self, name: Option<&str>) -> String {
        match name {
            Some(name) => name.to_owned(),
            None => fl!(self.loader, "no-name"),
        }
    }

    /// The localized sex label; [`Sex::Other`] renders verbatim and `None` is the "no value"
    /// placeholder.
    #[must_use]
    pub fn sex_label(&self, sex: Option<&Sex>) -> String {
        match sex {
            Some(Sex::Male) => fl!(self.loader, "sex-male"),
            Some(Sex::Female) => fl!(self.loader, "sex-female"),
            Some(Sex::Unknown) => fl!(self.loader, "sex-unknown"),
            Some(Sex::Other(value)) => value.clone(),
            None => fl!(self.loader, "no-value"),
        }
    }

    /// `No persons yet.`
    #[must_use]
    pub fn list_empty(&self) -> String {
        fl!(self.loader, "list-empty")
    }

    /// The `(private)` tag.
    #[must_use]
    pub fn private_tag(&self) -> String {
        fl!(self.loader, "private-tag")
    }

    /// The "ID" field label.
    #[must_use]
    pub fn label_id(&self) -> String {
        fl!(self.loader, "field-id")
    }

    /// The "Name" field label.
    #[must_use]
    pub fn label_name(&self) -> String {
        fl!(self.loader, "field-name")
    }

    /// The "Given name" field label.
    #[must_use]
    pub fn label_given(&self) -> String {
        fl!(self.loader, "field-given")
    }

    /// The "Surname" field label.
    #[must_use]
    pub fn label_surname(&self) -> String {
        fl!(self.loader, "field-surname")
    }

    /// The "Sex" field label.
    #[must_use]
    pub fn label_sex(&self) -> String {
        fl!(self.loader, "field-sex")
    }

    /// The "Private" field label.
    #[must_use]
    pub fn label_private(&self) -> String {
        fl!(self.loader, "field-private")
    }

    /// The full error line, e.g. `error: I9999 not found`.
    #[must_use]
    pub fn error(&self, error: &AppError) -> String {
        let message = self.error_message(error);
        fl!(self.loader, "error-prefix", message = message)
    }

    fn error_message(&self, error: &AppError) -> String {
        match error {
            AppError::Config(detail) => fl!(self.loader, "err-config", detail = detail.clone()),
            AppError::Workspace(detail) => fl!(self.loader, "err-workspace", detail = detail.clone()),
            AppError::HumanIdTaken(id)
            | AppError::PersonNotFound(id)
            | AppError::FamilyNotFound(id)
            | AppError::PlaceNotFound(id)
            | AppError::SourceNotFound(id)
            | AppError::CitationNotFound(id)
            | AppError::EventNotFound(id) => fl!(self.loader, "err-not-found", id = id.clone()),
            AppError::Domain(_)
            | AppError::FamilyDomain(_)
            | AppError::PlaceDomain(_)
            | AppError::SourceDomain(_)
            | AppError::CitationDomain(_)
            | AppError::EventDomain(_) => fl!(self.loader, "err-domain"),
            AppError::Db(db) => self.db_error(db),
        }
    }

    fn db_error(&self, error: &DbError) -> String {
        match error {
            DbError::Unsupported(detail) => fl!(self.loader, "err-db-unsupported", detail = detail.clone()),
            DbError::Backend(detail) => fl!(self.loader, "err-db-backend", detail = detail.clone()),
            DbError::Malformed(detail) => fl!(self.loader, "err-db-malformed", detail = detail.clone()),
        }
    }

    /// Builds a localizer for one language tag, for deterministic tests.
    #[cfg(test)]
    pub(crate) fn for_test(tag: &str) -> Self {
        let lang: LanguageIdentifier = tag.parse().expect("valid language tag");
        Self::with_languages(None, &[lang])
    }
}

/// Expands requested languages into an ordered, deduplicated load/fallback chain, ending with the
/// `fallback` language. Each request contributes its region form, its language-only form, and — for
/// Norwegian Bokmål/Nynorsk (`nb`/`nn`) — the generic `no` macrolanguage, so `nb-NO`/`nn-NO` resolve
/// to the `no` catalogue. Languages without a catalogue are skipped at load time.
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

/// Appends `lang` to `chain` if not already present, preserving fallback order.
fn push_unique(chain: &mut Vec<LanguageIdentifier>, lang: LanguageIdentifier) {
    if !chain.contains(&lang) {
        chain.push(lang);
    }
}

/// Composes the asset layers, highest priority first: workspace override, shared app override,
/// embedded baseline.
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
    use super::Localizer;
    use genealogy_app::{AppError, DbError, Sex};

    #[test]
    fn selects_the_requested_language() {
        assert_eq!(Localizer::for_test("en").list_empty(), "No persons yet.");
        assert_eq!(Localizer::for_test("no").list_empty(), "Ingen personer ennå.");
    }

    #[test]
    fn norwegian_variants_resolve_to_the_generic_catalogue() {
        assert_eq!(Localizer::for_test("nb-NO").sex_label(Some(&Sex::Female)), "kvinne");
        assert_eq!(Localizer::for_test("nn-NO").sex_label(Some(&Sex::Female)), "kvinne");
    }

    #[test]
    fn empty_request_falls_back_to_english() {
        assert_eq!(Localizer::with_languages(None, &[]).list_empty(), "No persons yet.");
    }

    #[test]
    fn sex_other_renders_verbatim() {
        let loc = Localizer::for_test("en");
        assert_eq!(loc.sex_label(Some(&Sex::Other("intersex".to_owned()))), "intersex");
        assert_eq!(loc.sex_label(None), "-");
    }

    #[test]
    fn errors_are_mapped_through_the_catalogue() {
        let loc = Localizer::for_test("en");
        assert_eq!(
            loc.error(&AppError::PersonNotFound("I9999".to_owned())),
            "error: I9999 not found"
        );
        assert_eq!(
            loc.error(&AppError::Db(DbError::Unsupported("postgres".to_owned()))),
            "error: unsupported: postgres"
        );
    }

    #[test]
    fn a_workspace_override_wins_over_the_embedded_baseline() {
        let dir = tempfile::tempdir().expect("tempdir");
        let en_dir = dir.path().join("i18n").join("en");
        std::fs::create_dir_all(&en_dir).expect("create override dir");
        std::fs::write(en_dir.join("genealogy-ui.ftl"), "list-empty = OVERRIDDEN\n").expect("write override");

        let overridden = Localizer::with_languages(Some(dir.path()), &["en".parse().expect("tag")]);
        assert_eq!(overridden.list_empty(), "OVERRIDDEN");
        assert_eq!(Localizer::for_test("en").list_empty(), "No persons yet.");
    }
}
