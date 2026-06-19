//! Localization for the `genealogy` CLI (ADR 0003).
//!
//! A Mozilla Fluent message catalogue is embedded as the baseline and overridden at runtime,
//! highest priority first, by per-directory `.ftl` files: the **open workspace** dir, then the
//! **shared application** dir, then the **embedded** baseline (which always carries the complete
//! fallback language, so the UI is never left unlocalized). The system locale is negotiated against
//! the available languages via [`i18n_embed::select`].
//!
//! [`Localizer`] owns the loaded catalogue and is the only place message keys are resolved; it maps
//! the structured [`AppError`] / [`PersonError`] / [`DbError`] surface and the [`Sex`] value to
//! localized strings, keeping `genealogy-app`/`genealogy-core`/`genealogy-db` free of UI text.

use std::path::Path;

use genealogy_app::config;
use genealogy_app::{
    AppError, CitationError, CitationSummary, DbError, EventError, EventSummary, EventType, FamilyError, FamilySummary,
    PersonError, PersonSummary, PlaceError, PlaceSummary, PlaceType, Sex, SourceError, SourceSummary,
};
use genealogy_core::date::{DateModifier, DatePoint, GenealogicalDate, GenealogicalDateBody};
use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::{AssetsMultiplexor, DesktopLanguageRequester, FileSystemAssets, I18nAssets, LanguageLoader};
use i18n_embed_fl::fl;
use rust_embed::RustEmbed;
use tracing::warn;
use unic_langid::LanguageIdentifier;

/// The embedded baseline catalogue (compiled into the binary; complete fallback language).
#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Embedded;

/// The loaded message catalogue: resolves every user-facing string the CLI emits.
pub struct Localizer {
    loader: FluentLanguageLoader,
}

impl Localizer {
    /// Builds a localizer over the baseline layers (shared app dir over the embedded baseline),
    /// negotiating the system locale. Used for `init` and any error before a workspace is open.
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

    /// Builds a localizer for an explicit set of requested languages. The request is expanded into a
    /// fallback chain (region → language → macrolanguage → `en`) before loading, so a `nb-NO` (or
    /// `nn-NO`) request resolves to the generic `no` catalogue and finally the `en` baseline.
    /// Separated from [`Self::build`] so tests are deterministic instead of host-locale dependent.
    fn with_languages(workspace_dir: Option<&Path>, requested: &[LanguageIdentifier]) -> Self {
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
        // Terminal output is not bidi-isolated, so drop Fluent's default U+2068/U+2069 marks.
        // Applied after loading so it reaches the bundles just loaded.
        loader.set_use_isolating(false);
        Self { loader }
    }

    /// `Created I0001`.
    #[must_use]
    pub fn created(&self, id: &str) -> String {
        fl!(self.loader, "created", id = id)
    }

    /// `Updated I0001`.
    #[must_use]
    pub fn updated(&self, id: &str) -> String {
        fl!(self.loader, "updated", id = id)
    }

    /// `Initialized workspace "gen" at /path`.
    #[must_use]
    pub fn init_success(&self, name: &str, path: &str) -> String {
        fl!(self.loader, "init-success", name = name, path = path)
    }

    /// `Config: /path`.
    #[must_use]
    pub fn config_line(&self, path: &str) -> String {
        fl!(self.loader, "config-line", path = path)
    }

    /// `No persons yet.`
    #[must_use]
    pub fn list_empty(&self) -> String {
        fl!(self.loader, "list-empty")
    }

    /// One person line: `I0001  Ada Lovelace  sex: female [private]`.
    #[must_use]
    pub fn summary_line(&self, summary: &PersonSummary) -> String {
        let name = match &summary.display_name {
            Some(name) => name.clone(),
            None => fl!(self.loader, "no-name"),
        };
        let sex = match &summary.sex {
            Some(sex) => self.sex(sex),
            None => fl!(self.loader, "no-value"),
        };
        let private = if summary.private {
            fl!(self.loader, "private-tag")
        } else {
            String::new()
        };
        fl!(
            self.loader,
            "summary",
            id = summary.human_id.clone(),
            name = name,
            sex = sex,
            private = private
        )
    }

    /// `No families yet.`
    #[must_use]
    pub fn family_list_empty(&self) -> String {
        fl!(self.loader, "family-list-empty")
    }

    /// One family line: `F0001  partners: I0001, I0002  children: I0003 [private]`.
    #[must_use]
    pub fn family_summary_line(&self, summary: &FamilySummary) -> String {
        let partners = self.members(&summary.partners);
        let children = self.members(&summary.children);
        let private = if summary.private {
            fl!(self.loader, "private-tag")
        } else {
            String::new()
        };
        fl!(
            self.loader,
            "family-summary",
            id = summary.human_id.clone(),
            partners = partners,
            children = children,
            private = private
        )
    }

    /// Renders a member id list, or the localized `(none)` placeholder when empty.
    fn members(&self, ids: &[String]) -> String {
        if ids.is_empty() {
            fl!(self.loader, "family-none")
        } else {
            ids.join(", ")
        }
    }

    /// `No places yet.`
    #[must_use]
    pub fn place_list_empty(&self) -> String {
        fl!(self.loader, "place-list-empty")
    }

    /// One place line: `P0001  Vågå (Vaage)  type: parish`.
    #[must_use]
    pub fn place_summary_line(&self, summary: &PlaceSummary) -> String {
        let name = if summary.names.is_empty() {
            fl!(self.loader, "no-name")
        } else {
            summary.names.join(" / ")
        };
        let place_type = match &summary.place_type {
            Some(place_type) => self.place_type(place_type),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "place-summary",
            id = summary.human_id.clone(),
            name = name,
            place_type = place_type
        )
    }

    /// The localized place-type label; a custom [`PlaceType::Custom`] value renders verbatim.
    #[must_use]
    fn place_type(&self, place_type: &PlaceType) -> String {
        match place_type {
            PlaceType::Country => fl!(self.loader, "place-type-country"),
            PlaceType::County => fl!(self.loader, "place-type-county"),
            PlaceType::Municipality => fl!(self.loader, "place-type-municipality"),
            PlaceType::Parish => fl!(self.loader, "place-type-parish"),
            PlaceType::City => fl!(self.loader, "place-type-city"),
            PlaceType::Town => fl!(self.loader, "place-type-town"),
            PlaceType::Village => fl!(self.loader, "place-type-village"),
            PlaceType::Farm => fl!(self.loader, "place-type-farm"),
            PlaceType::Building => fl!(self.loader, "place-type-building"),
            PlaceType::Custom(value) => value.clone(),
        }
    }

    /// `No sources yet.`
    #[must_use]
    pub fn source_list_empty(&self) -> String {
        fl!(self.loader, "source-list-empty")
    }

    /// One source line: `S0001  Folketelling 1801`.
    #[must_use]
    pub fn source_summary_line(&self, summary: &SourceSummary) -> String {
        let title = match &summary.title {
            Some(title) => title.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "source-summary",
            id = summary.human_id.clone(),
            title = title
        )
    }

    /// `No citations yet.`
    #[must_use]
    pub fn citation_list_empty(&self) -> String {
        fl!(self.loader, "citation-list-empty")
    }

    /// One citation line: `C0001  source: S0001  page: p. 42`.
    #[must_use]
    pub fn citation_summary_line(&self, summary: &CitationSummary) -> String {
        let source = match &summary.source {
            Some(source) => source.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let page = match &summary.page {
            Some(page) => page.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "citation-summary",
            id = summary.human_id.clone(),
            source = source,
            page = page
        )
    }

    /// `No events yet.`
    #[must_use]
    pub fn event_list_empty(&self) -> String {
        fl!(self.loader, "event-list-empty")
    }

    /// One event line: `E0001  type: birth  date: 1847-03-12  place: P0001`.
    #[must_use]
    pub fn event_summary_line(&self, summary: &EventSummary) -> String {
        let event_type = match &summary.event_type {
            Some(event_type) => self.event_type(event_type),
            None => fl!(self.loader, "no-value"),
        };
        let date = match &summary.date {
            Some(date) => render_date(date),
            None => fl!(self.loader, "no-value"),
        };
        let place = match &summary.place {
            Some(place) => place.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "event-summary",
            id = summary.human_id.clone(),
            event_type = event_type,
            date = date,
            place = place
        )
    }

    /// The localized event-type label; a custom [`EventType::Custom`] value renders verbatim.
    #[must_use]
    fn event_type(&self, event_type: &EventType) -> String {
        match event_type {
            EventType::Birth => fl!(self.loader, "event-type-birth"),
            EventType::Death => fl!(self.loader, "event-type-death"),
            EventType::Marriage => fl!(self.loader, "event-type-marriage"),
            EventType::Baptism => fl!(self.loader, "event-type-baptism"),
            EventType::Burial => fl!(self.loader, "event-type-burial"),
            EventType::Census => fl!(self.loader, "event-type-census"),
            EventType::Residence => fl!(self.loader, "event-type-residence"),
            EventType::Immigration => fl!(self.loader, "event-type-immigration"),
            EventType::Emigration => fl!(self.loader, "event-type-emigration"),
            EventType::Custom(value) => value.clone(),
        }
    }

    /// The localized sex label; a custom [`Sex::Other`] value renders verbatim.
    #[must_use]
    fn sex(&self, sex: &Sex) -> String {
        match sex {
            Sex::Male => fl!(self.loader, "sex-male"),
            Sex::Female => fl!(self.loader, "sex-female"),
            Sex::Unknown => fl!(self.loader, "sex-unknown"),
            Sex::Other(value) => value.clone(),
        }
    }

    /// The full error line, e.g. `error: no person with human_id "I9999"`.
    #[must_use]
    pub fn error(&self, error: &AppError) -> String {
        let message = self.error_message(error);
        fl!(self.loader, "error-prefix", message = message)
    }

    fn error_message(&self, error: &AppError) -> String {
        match error {
            AppError::Config(detail) => fl!(self.loader, "err-config", detail = detail.clone()),
            AppError::Workspace(detail) => fl!(self.loader, "err-workspace", detail = detail.clone()),
            AppError::HumanIdTaken(id) => fl!(self.loader, "err-human-id-taken", id = id.clone()),
            AppError::PersonNotFound(id) => fl!(self.loader, "err-person-not-found", id = id.clone()),
            AppError::FamilyNotFound(id) => fl!(self.loader, "err-family-not-found", id = id.clone()),
            AppError::PlaceNotFound(id) => fl!(self.loader, "err-place-not-found", id = id.clone()),
            AppError::SourceNotFound(id) => fl!(self.loader, "err-source-not-found", id = id.clone()),
            AppError::CitationNotFound(id) => fl!(self.loader, "err-citation-not-found", id = id.clone()),
            AppError::EventNotFound(id) => fl!(self.loader, "err-event-not-found", id = id.clone()),
            AppError::Domain(domain) => self.person_error(domain),
            AppError::FamilyDomain(domain) => self.family_error(domain),
            AppError::PlaceDomain(domain) => self.place_error(domain),
            AppError::SourceDomain(domain) => self.source_error(domain),
            AppError::CitationDomain(domain) => self.citation_error(domain),
            AppError::EventDomain(domain) => self.event_error(domain),
            AppError::Db(db) => self.db_error(db),
        }
    }

    fn citation_error(&self, error: &CitationError) -> String {
        match error {
            CitationError::NotFound(id) => fl!(self.loader, "err-citation-not-exist", id = id.to_string()),
            CitationError::AlreadyExists(id) => fl!(self.loader, "err-citation-exists", id = id.to_string()),
            CitationError::UnknownSource(id) => fl!(self.loader, "err-unknown-source", id = id.to_string()),
        }
    }

    fn event_error(&self, error: &EventError) -> String {
        match error {
            EventError::NotFound(id) => fl!(self.loader, "err-event-not-exist", id = id.to_string()),
            EventError::AlreadyExists(id) => fl!(self.loader, "err-event-exists", id = id.to_string()),
            EventError::UnknownPlace(id) => fl!(self.loader, "err-unknown-place", id = id.to_string()),
        }
    }

    fn place_error(&self, error: &PlaceError) -> String {
        match error {
            PlaceError::NotFound(id) => fl!(self.loader, "err-place-not-exist", id = id.to_string()),
            PlaceError::AlreadyExists(id) => fl!(self.loader, "err-place-exists", id = id.to_string()),
            PlaceError::EmptyName => fl!(self.loader, "err-place-empty-name"),
        }
    }

    fn source_error(&self, error: &SourceError) -> String {
        match error {
            SourceError::NotFound(id) => fl!(self.loader, "err-source-not-exist", id = id.to_string()),
            SourceError::AlreadyExists(id) => fl!(self.loader, "err-source-exists", id = id.to_string()),
        }
    }

    fn family_error(&self, error: &FamilyError) -> String {
        match error {
            FamilyError::NotFound(id) => fl!(self.loader, "err-family-not-exist", id = id.to_string()),
            FamilyError::AlreadyExists(id) => fl!(self.loader, "err-family-exists", id = id.to_string()),
            FamilyError::PartnerAlreadyPresent(id) => fl!(self.loader, "err-partner-present", id = id.to_string()),
            FamilyError::PartnerNotPresent(id) => fl!(self.loader, "err-partner-absent", id = id.to_string()),
            FamilyError::ChildAlreadyPresent(id) => fl!(self.loader, "err-child-present", id = id.to_string()),
            FamilyError::ChildNotPresent(id) => fl!(self.loader, "err-child-absent", id = id.to_string()),
            FamilyError::RetractsMissingAssertion(id) | FamilyError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }

    fn person_error(&self, error: &PersonError) -> String {
        match error {
            PersonError::NotFound(id) => fl!(self.loader, "err-person-not-exist", id = id.to_string()),
            PersonError::AlreadyExists(id) => fl!(self.loader, "err-person-exists", id = id.to_string()),
            PersonError::EmptyName => fl!(self.loader, "err-empty-name"),
            PersonError::RetractsMissingAssertion(id) | PersonError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
            PersonError::InvalidDate(detail) => fl!(self.loader, "err-invalid-date", detail = detail.clone()),
            PersonError::MergeConflict {
                surviving,
                merged,
                reason,
            } => fl!(
                self.loader,
                "err-merge-conflict",
                surviving = surviving.to_string(),
                merged = merged.to_string(),
                reason = reason.clone()
            ),
            PersonError::SelfAssociation(id) => fl!(self.loader, "err-self-association", id = id.to_string()),
        }
    }

    fn db_error(&self, error: &DbError) -> String {
        match error {
            DbError::Unsupported(detail) => fl!(self.loader, "err-db-unsupported", detail = detail.clone()),
            DbError::Backend(detail) => fl!(self.loader, "err-db-backend", detail = detail.clone()),
            DbError::Malformed(detail) => fl!(self.loader, "err-db-malformed", detail = detail.clone()),
        }
    }
}

/// Renders a [`GenealogicalDate`] as a plain ISO-ish `YYYY[-MM[-DD]]` string (or its verbatim text
/// when unparseable). This is a minimal, non-localized rendering; localized formatting via ICU4X
/// and Fluent date qualifiers lands with the localization work (roadmap Spike A i18n).
fn render_date(date: &GenealogicalDate) -> String {
    let point = match &date.modifier {
        GenealogicalDateBody::TextOnly { text } => return text.clone(),
        GenealogicalDateBody::Structured(modifier) => match modifier {
            DateModifier::None(point)
            | DateModifier::Before(point)
            | DateModifier::After(point)
            | DateModifier::About(point)
            | DateModifier::From(point)
            | DateModifier::To(point) => *point,
            DateModifier::Range { start, .. } | DateModifier::Span { start, .. } => *start,
        },
    };
    render_point(&point)
}

/// Renders a single [`DatePoint`] as `YYYY`, `YYYY-MM`, or `YYYY-MM-DD` (the known components).
fn render_point(point: &DatePoint) -> String {
    use std::fmt::Write as _;

    let Some(year) = point.year else {
        return "?".to_owned();
    };
    let mut rendered = year.to_string();
    if let Some(month) = point.month {
        let _ = write!(rendered, "-{month:02}");
        if let Some(day) = point.day {
            let _ = write!(rendered, "-{day:02}");
        }
    }
    rendered
}

/// Expands requested languages into an ordered, deduplicated load/fallback chain, ending with the
/// `fallback` language. Each request contributes its region form, its language-only form, and — for
/// Norwegian Bokmål/Nynorsk (`nb`/`nn`) — the generic `no` macrolanguage, so `nb-NO`/`nn-NO` resolve
/// to the `no` catalogue. Languages without a catalogue are simply skipped at load time.
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
    use super::*;
    use genealogy_app::PersonError;
    use genealogy_core::ids::PersonId;
    use uuid::Uuid;

    fn lang(tag: &str) -> LanguageIdentifier {
        tag.parse().expect("valid language tag")
    }

    fn localizer(tag: &str) -> Localizer {
        Localizer::with_languages(None, &[lang(tag)])
    }

    #[test]
    fn selects_the_requested_language() {
        assert_eq!(localizer("en").created("I0001"), "Created I0001");
        assert_eq!(localizer("no").created("I0001"), "Opprettet I0001");
    }

    #[test]
    fn norwegian_variants_resolve_to_the_generic_catalogue() {
        // nb-NO -> nb -> no and nn-NO -> nn -> no both land on the `no` catalogue.
        assert_eq!(localizer("nb-NO").created("I0001"), "Opprettet I0001");
        assert_eq!(localizer("nn-NO").created("I0001"), "Opprettet I0001");
    }

    #[test]
    fn empty_request_falls_back_to_english() {
        assert_eq!(Localizer::with_languages(None, &[]).created("I0001"), "Created I0001");
    }

    #[test]
    fn a_key_absent_in_the_locale_falls_back_to_english() {
        // The `no` catalogue omits `err-self-association`; it must fall back to the en baseline.
        let person = PersonId::from_uuid(Uuid::from_u128(7));
        let message = localizer("nb-NO").error(&AppError::Domain(PersonError::SelfAssociation(person)));
        assert!(message.contains("cannot be associated with itself"), "got: {message}");
    }

    #[test]
    fn family_summary_is_localized() {
        let summary = FamilySummary {
            human_id: "F0001".to_owned(),
            partners: vec!["I0001".to_owned(), "I0002".to_owned()],
            children: Vec::new(),
            private: false,
        };
        let line = localizer("no").family_summary_line(&summary);
        // Norwegian labels and the localized empty-list placeholder.
        assert!(line.contains("partnere: I0001, I0002"), "got: {line}");
        assert!(line.contains("barn: (ingen)"), "got: {line}");
        assert_eq!(localizer("no").family_list_empty(), "Ingen familier ennå.");
    }

    #[test]
    fn family_error_absent_in_the_locale_falls_back_to_english() {
        // The `no` catalogue omits `err-child-absent`; it must fall back to the en baseline.
        let person = PersonId::from_uuid(Uuid::from_u128(7));
        let message = localizer("nb-NO").error(&AppError::FamilyDomain(FamilyError::ChildNotPresent(person)));
        assert!(message.contains("is not a child of this family"), "got: {message}");
    }

    #[test]
    fn family_error_present_in_the_locale_is_translated() {
        let person = PersonId::from_uuid(Uuid::from_u128(7));
        let message = localizer("no").error(&AppError::FamilyDomain(FamilyError::PartnerAlreadyPresent(person)));
        assert!(message.contains("er allerede en partner"), "got: {message}");
    }

    #[test]
    fn sex_other_renders_verbatim_and_private_is_tagged() {
        let summary = PersonSummary {
            human_id: "I0001".to_owned(),
            display_name: Some("Ada".to_owned()),
            sex: Some(Sex::Other("intersex".to_owned())),
            private: true,
        };
        let line = localizer("en").summary_line(&summary);
        assert!(line.contains("intersex"), "got: {line}");
        assert!(line.contains("[private]"), "got: {line}");
    }

    #[test]
    fn errors_are_mapped_through_the_catalogue() {
        assert_eq!(
            localizer("en").error(&AppError::Db(DbError::Unsupported("postgres".to_owned()))),
            "error: unsupported: postgres"
        );
        assert_eq!(
            localizer("en").error(&AppError::Domain(PersonError::EmptyName)),
            "error: a name must have a given name or a surname"
        );
    }

    #[test]
    fn a_workspace_override_wins_over_the_embedded_baseline() {
        let dir = tempfile::tempdir().expect("tempdir");
        let en_dir = dir.path().join("i18n").join("en");
        std::fs::create_dir_all(&en_dir).expect("create override dir");
        std::fs::write(en_dir.join("genealogy-cli.ftl"), "created = OVERRIDDEN { $id }\n").expect("write override");

        let overridden = Localizer::with_languages(Some(dir.path()), &[lang("en")]);
        assert_eq!(overridden.created("I0001"), "OVERRIDDEN I0001");
        // Without the workspace layer the embedded baseline is used.
        assert_eq!(localizer("en").created("I0001"), "Created I0001");
    }
}
