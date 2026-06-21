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
use i18n_embed::{DesktopLanguageRequester, FileSystemAssets, LanguageLoader};
use i18n_embed_fl::fl;
use rust_embed::RustEmbed;
use tracing::warn;
use unic_langid::LanguageIdentifier;

use crate::vocabulary::{Field, Form, SelectOption};

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
        let shared = config::shared_i18n_dir().ok();
        genealogy_i18n::init(&loader, workspace_dir, shared.as_deref(), requested, Box::new(Embedded));
        Self { loader }
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

/// Resolves a plugin form's label IDs to display text (ADR 0012 §5, ADR 0003).
///
/// A plugin returns Fluent **message IDs**, not display strings; this looks each up in the plugin's
/// own catalogue (the file `<domain>.ftl` under `catalogue_dir/<locale>/`), negotiating `requested`
/// against the same nb/nn→no→en fallback the app uses. A missing id — or an absent catalogue —
/// resolves to the id itself, so an unlocalized plugin still renders.
#[must_use]
pub fn resolve_form(form: &Form, catalogue_dir: &Path, domain: &str, requested: &[LanguageIdentifier]) -> Form {
    let fallback: LanguageIdentifier = "en".parse().unwrap_or_default();
    let loader = FluentLanguageLoader::new(domain, fallback.clone());
    // `FileSystemAssets::available_languages` only reports embedded locales, so detect the plugin's
    // shipped catalogues by probing `<catalogue_dir>/<locale>/<domain>.ftl` directly and load only
    // those — loading a locale with no file would panic inside `load_languages`.
    let chain: Vec<LanguageIdentifier> = genealogy_i18n::fallback_chain(requested, &fallback)
        .into_iter()
        .filter(|lang| {
            catalogue_dir
                .join(lang.to_string())
                .join(format!("{domain}.ftl"))
                .is_file()
        })
        .collect();
    if chain.is_empty() {
        // No catalogue shipped for any negotiated locale — render the ids unchanged.
        return form.clone();
    }
    match FileSystemAssets::try_new(catalogue_dir) {
        Ok(assets) => {
            if let Err(error) = loader.load_languages(&assets, &chain) {
                warn!(%error, "failed to load plugin catalogue; rendering message ids");
                return form.clone();
            }
            loader.set_use_isolating(false);
        }
        Err(error) => {
            warn!(%error, "unreadable plugin catalogue; rendering message ids");
            return form.clone();
        }
    }
    Form {
        title: loader.get(&form.title),
        submit: loader.get(&form.submit),
        fields: form.fields.iter().map(|field| resolve_field(field, &loader)).collect(),
    }
}

/// Resolves one field's label-id(s) to display text.
fn resolve_field(field: &Field, loader: &FluentLanguageLoader) -> Field {
    match field {
        Field::Text {
            label,
            name,
            placeholder,
        } => Field::Text {
            label: loader.get(label),
            name: name.clone(),
            placeholder: placeholder.as_deref().map(|id| loader.get(id)),
        },
        Field::Number { label, name } => Field::Number {
            label: loader.get(label),
            name: name.clone(),
        },
        Field::Checkbox { label, name } => Field::Checkbox {
            label: loader.get(label),
            name: name.clone(),
        },
        Field::Select { label, name, options } => Field::Select {
            label: loader.get(label),
            name: name.clone(),
            options: options
                .iter()
                .map(|option| SelectOption {
                    label: loader.get(&option.label),
                    value: option.value.clone(),
                })
                .collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{Localizer, resolve_form};
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
    fn resolve_form_looks_up_label_ids_in_the_plugin_catalogue() {
        use crate::vocabulary::{Field, Form};

        let dir = tempfile::tempdir().expect("tempdir");
        for (locale, title, year) in [("en", "Add note", "Year"), ("no", "Legg til notat", "År")] {
            let locale_dir = dir.path().join(locale);
            std::fs::create_dir_all(&locale_dir).expect("create locale dir");
            std::fs::write(
                locale_dir.join("demo.ftl"),
                format!("form-title = {title}\nform-submit = Save\nf-year = {year}\n"),
            )
            .expect("write catalogue");
        }
        let form = Form {
            title: "form-title".to_owned(),
            submit: "form-submit".to_owned(),
            fields: vec![Field::Number {
                label: "f-year".to_owned(),
                name: "year".to_owned(),
            }],
        };

        let english = resolve_form(&form, dir.path(), "demo", &["en".parse().expect("tag")]);
        assert_eq!(english.title, "Add note");
        assert_eq!(
            english.fields[0],
            Field::Number {
                label: "Year".to_owned(),
                name: "year".to_owned()
            }
        );

        // nb-NO negotiates to the `no` catalogue (ADR 0003 fallback).
        let norwegian = resolve_form(&form, dir.path(), "demo", &["nb-NO".parse().expect("tag")]);
        assert_eq!(norwegian.title, "Legg til notat");
        assert_eq!(
            norwegian.fields[0],
            Field::Number {
                label: "År".to_owned(),
                name: "year".to_owned()
            }
        );

        // A missing catalogue leaves the ids untouched (still renders).
        let raw = resolve_form(&form, &dir.path().join("absent"), "demo", &["en".parse().expect("tag")]);
        assert_eq!(raw.title, "form-title");
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
