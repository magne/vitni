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
    PersonError, PersonSummary, PlaceError, PlaceSummary, PlaceType, RepositoryError, RepositorySummary,
    RepositoryType, Sex, SourceError, SourceSummary,
};
use genealogy_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::{DesktopLanguageRequester, LanguageLoader};
use i18n_embed_fl::fl;
use icu::calendar::Date;
use icu::datetime::DateTimeFormatter;
use icu::datetime::fieldsets::{Y, YM, YMD};
use icu::locale::Locale;
use rust_embed::RustEmbed;
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
        let shared = config::shared_i18n_dir().ok();
        genealogy_i18n::init(&loader, workspace_dir, shared.as_deref(), requested, Box::new(Embedded));
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

    /// One place line: `P0001  Vågå (Vaage)  type: parish  code: 0515  coords: 60.39,5.32`.
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
        let code = match &summary.code {
            Some(code) => code.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let coords = match &summary.coordinates {
            Some(coords) => coords.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "place-summary",
            id = summary.human_id.clone(),
            name = name,
            place_type = place_type,
            code = code,
            coords = coords
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

    /// One source line: `S0001  Folketelling 1801  author: Riksarkivet  repos: 1  attrs: 2`.
    #[must_use]
    pub fn source_summary_line(&self, summary: &SourceSummary) -> String {
        let title = match &summary.title {
            Some(title) => title.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let author = match &summary.author {
            Some(author) => author.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "source-summary",
            id = summary.human_id.clone(),
            title = title,
            author = author,
            repositories = summary.repositories.len().to_string(),
            attributes = summary.attributes.len().to_string()
        )
    }

    /// `No repositories yet.`
    #[must_use]
    pub fn repository_list_empty(&self) -> String {
        fl!(self.loader, "repository-list-empty")
    }

    /// One repository line: `R0001  Riksarkivet  type: archive  addresses: 1  urls: 2`.
    #[must_use]
    pub fn repository_summary_line(&self, summary: &RepositorySummary) -> String {
        let name = match &summary.name {
            Some(name) => name.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let repository_type = match &summary.repository_type {
            Some(repository_type) => self.repository_type(repository_type),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "repository-summary",
            id = summary.human_id.clone(),
            name = name,
            repository_type = repository_type,
            addresses = summary.address_count.to_string(),
            urls = summary.url_count.to_string()
        )
    }

    /// The localized repository-type label; a custom value renders verbatim.
    #[must_use]
    fn repository_type(&self, repository_type: &RepositoryType) -> String {
        match repository_type {
            RepositoryType::Library => fl!(self.loader, "repository-type-library"),
            RepositoryType::Archive => fl!(self.loader, "repository-type-archive"),
            RepositoryType::Church => fl!(self.loader, "repository-type-church"),
            RepositoryType::Cemetery => fl!(self.loader, "repository-type-cemetery"),
            RepositoryType::Museum => fl!(self.loader, "repository-type-museum"),
            RepositoryType::Website => fl!(self.loader, "repository-type-website"),
            RepositoryType::Collection => fl!(self.loader, "repository-type-collection"),
            RepositoryType::Custom(value) => value.clone(),
        }
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
            Some(date) => self.date(date),
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

    /// Renders a [`GenealogicalDate`] for the negotiated locale: ICU4X formats the calendar date
    /// (localized month/era names), and the genealogical qualifiers (before/about/range/…) and the
    /// quality (estimated/calculated) are composed from Fluent terms (ADR 0003). An unparseable
    /// date renders its verbatim text.
    #[must_use]
    pub fn date(&self, date: &GenealogicalDate) -> String {
        let core = match &date.modifier {
            GenealogicalDateBody::TextOnly { text } => return text.clone(),
            GenealogicalDateBody::Structured(modifier) => self.date_modifier(date.calendar, modifier),
        };
        match date.quality {
            DateQuality::Normal => core,
            DateQuality::Estimated => fl!(self.loader, "date-estimated", date = core),
            DateQuality::Calculated => fl!(self.loader, "date-calculated", date = core),
        }
    }

    /// Composes a [`DateModifier`] into a localized string, wrapping the formatted point(s) in the
    /// matching Fluent qualifier term.
    fn date_modifier(&self, calendar: Calendar, modifier: &DateModifier) -> String {
        match modifier {
            DateModifier::None(point) => self.date_point(calendar, point),
            DateModifier::Before(point) => fl!(self.loader, "date-before", date = self.date_point(calendar, point)),
            DateModifier::After(point) => fl!(self.loader, "date-after", date = self.date_point(calendar, point)),
            DateModifier::About(point) => fl!(self.loader, "date-about", date = self.date_point(calendar, point)),
            DateModifier::From(point) => fl!(self.loader, "date-from", date = self.date_point(calendar, point)),
            DateModifier::To(point) => fl!(self.loader, "date-to", date = self.date_point(calendar, point)),
            DateModifier::Range { start, end } => fl!(
                self.loader,
                "date-range",
                start = self.date_point(calendar, start),
                end = self.date_point(calendar, end)
            ),
            DateModifier::Span { start, end } => fl!(
                self.loader,
                "date-span",
                start = self.date_point(calendar, start),
                end = self.date_point(calendar, end)
            ),
        }
    }

    /// Formats a single [`DatePoint`] for the negotiated locale via ICU4X, choosing the field set
    /// from the known components (year / year-month / year-month-day). A Gregorian date is
    /// formatted by ICU; other calendars (and any ICU failure) fall back to a numeric rendering.
    fn date_point(&self, calendar: Calendar, point: &DatePoint) -> String {
        let Some(year) = point.year else {
            return "?".to_owned();
        };
        if calendar != Calendar::Gregorian {
            return numeric_point(point);
        }
        let Ok(date) = Date::try_new_iso(year, point.month.unwrap_or(1), point.day.unwrap_or(1)) else {
            return numeric_point(point);
        };
        let locale = self.icu_locale();
        let formatted = match (point.month, point.day) {
            (Some(_), Some(_)) => {
                DateTimeFormatter::try_new(locale.into(), YMD::long()).map(|f| f.format(&date).to_string())
            }
            (Some(_), None) => {
                DateTimeFormatter::try_new(locale.into(), YM::long()).map(|f| f.format(&date).to_string())
            }
            _ => DateTimeFormatter::try_new(locale.into(), Y::long()).map(|f| f.format(&date).to_string()),
        };
        formatted.unwrap_or_else(|_| numeric_point(point))
    }

    /// The ICU [`Locale`] for the negotiated UI language. The generic Norwegian `no` catalogue maps
    /// to Bokmål (`nb`) for CLDR date data; an unparseable tag falls back to `und` (root).
    fn icu_locale(&self) -> Locale {
        let language = self.loader.current_language();
        let tag = match language.language.as_str() {
            "no" => "nb",
            other => other,
        };
        tag.parse().unwrap_or(Locale::UNKNOWN)
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
            AppError::RepositoryNotFound(id) => fl!(self.loader, "err-repository-not-found", id = id.clone()),
            AppError::Domain(domain) => self.person_error(domain),
            AppError::FamilyDomain(domain) => self.family_error(domain),
            AppError::PlaceDomain(domain) => self.place_error(domain),
            AppError::SourceDomain(domain) => self.source_error(domain),
            AppError::CitationDomain(domain) => self.citation_error(domain),
            AppError::EventDomain(domain) => self.event_error(domain),
            AppError::RepositoryDomain(domain) => self.repository_error(domain),
            AppError::Db(db) => self.db_error(db),
        }
    }

    fn citation_error(&self, error: &CitationError) -> String {
        match error {
            CitationError::NotFound(id) => fl!(self.loader, "err-citation-not-exist", id = id.to_string()),
            CitationError::AlreadyExists(id) => fl!(self.loader, "err-citation-exists", id = id.to_string()),
            CitationError::UnknownSource(id) => fl!(self.loader, "err-unknown-source", id = id.to_string()),
            CitationError::RetractsMissingAssertion(id) | CitationError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }

    fn repository_error(&self, error: &RepositoryError) -> String {
        match error {
            RepositoryError::NotFound(id) => fl!(self.loader, "err-repository-not-exist", id = id.to_string()),
            RepositoryError::AlreadyExists(id) => fl!(self.loader, "err-repository-exists", id = id.to_string()),
            RepositoryError::EmptyName => fl!(self.loader, "err-repository-empty-name"),
            RepositoryError::RetractsMissingAssertion(id) | RepositoryError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }

    fn event_error(&self, error: &EventError) -> String {
        match error {
            EventError::NotFound(id) => fl!(self.loader, "err-event-not-exist", id = id.to_string()),
            EventError::AlreadyExists(id) => fl!(self.loader, "err-event-exists", id = id.to_string()),
            EventError::UnknownPlace(id) => fl!(self.loader, "err-unknown-place", id = id.to_string()),
            EventError::RetractsMissingAssertion(id) | EventError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }

    fn place_error(&self, error: &PlaceError) -> String {
        match error {
            PlaceError::NotFound(id) => fl!(self.loader, "err-place-not-exist", id = id.to_string()),
            PlaceError::AlreadyExists(id) => fl!(self.loader, "err-place-exists", id = id.to_string()),
            PlaceError::EmptyName => fl!(self.loader, "err-place-empty-name"),
            PlaceError::EmptyCode => fl!(self.loader, "err-place-empty-code"),
            PlaceError::UnknownPlace(id) => fl!(self.loader, "err-place-unknown-enclosing", id = id.to_string()),
            PlaceError::RetractsMissingAssertion(id) | PlaceError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }

    fn source_error(&self, error: &SourceError) -> String {
        match error {
            SourceError::NotFound(id) => fl!(self.loader, "err-source-not-exist", id = id.to_string()),
            SourceError::AlreadyExists(id) => fl!(self.loader, "err-source-exists", id = id.to_string()),
            SourceError::RetractsMissingAssertion(id) | SourceError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
            SourceError::UnknownRepository(id) => {
                fl!(self.loader, "err-source-unknown-repository", id = id.to_string())
            }
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

/// Renders a single [`DatePoint`] as a plain `YYYY`, `YYYY-MM`, or `YYYY-MM-DD` string — the
/// non-localized fallback used for non-Gregorian calendars or when ICU4X has no data for the
/// negotiated locale.
fn numeric_point(point: &DatePoint) -> String {
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
    fn a_translated_person_error_renders_in_norwegian() {
        // `err-self-association` is now translated in `no` (it used to be omitted).
        let person = PersonId::from_uuid(Uuid::from_u128(7));
        let message = localizer("no").error(&AppError::Domain(PersonError::SelfAssociation(person)));
        assert!(message.contains("kan ikke knyttes til seg selv"), "got: {message}");
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
    fn a_translated_family_error_renders_in_norwegian() {
        // `err-child-absent` is now translated in `no` (it used to be omitted).
        let person = PersonId::from_uuid(Uuid::from_u128(7));
        let message = localizer("no").error(&AppError::FamilyDomain(FamilyError::ChildNotPresent(person)));
        assert!(message.contains("er ikke et barn i denne familien"), "got: {message}");
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
            given: Some("Ada".to_owned()),
            surname: None,
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
