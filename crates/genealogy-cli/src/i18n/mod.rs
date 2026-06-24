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
    AppError, CitationError, CitationSummary, Confidence, DbError, DnaMatchError, DnaMatchSummary, DnaProvider,
    DnaTestError, DnaTestSummary, DnaTestType, EventError, EventSummary, EventType, FamilyError, FamilySummary,
    MatchStatus, MediaError, MediaSummary, NoteError, NoteSummary, NoteType, PersonError, PersonSummary, PlaceError,
    PlaceSummary, PlaceType, RepositoryError, RepositorySummary, RepositoryType, Restriction, Sex, SourceError,
    SourceSummary, TagError, TagSummary,
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

mod citation;
mod dna_match;
mod dna_test;
mod event;
mod family;
mod media;
mod note;
mod person;
mod place;
mod repository;
mod source;
mod tag;

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

    /// `Rebuilt all projections from the event log.`
    #[must_use]
    pub fn rebuild_success(&self) -> String {
        fl!(self.loader, "rebuild-success")
    }

    /// `Imported N record(s) with <plugin>.`
    #[must_use]
    pub fn import_success(&self, count: u32, plugin: &str) -> String {
        fl!(self.loader, "import-success", count = count, plugin = plugin)
    }

    /// `Exported N record(s) to <path>.`
    #[must_use]
    pub fn export_success(&self, count: u32, path: &str) -> String {
        fl!(self.loader, "export-success", count = count, path = path)
    }

    /// `Workspace "<name>" already contains N person(s). Import anyway? [y/N]`.
    #[must_use]
    pub fn import_confirm(&self, name: &str, count: usize) -> String {
        fl!(self.loader, "import-confirm", name = name, count = count)
    }

    /// `Import cancelled.`
    #[must_use]
    pub fn import_cancelled(&self) -> String {
        fl!(self.loader, "import-cancelled")
    }

    /// The localized confidence label (data-model §8).
    #[must_use]
    fn confidence(&self, confidence: Confidence) -> String {
        match confidence {
            Confidence::VeryLow => fl!(self.loader, "confidence-very-low"),
            Confidence::Low => fl!(self.loader, "confidence-low"),
            Confidence::Normal => fl!(self.loader, "confidence-normal"),
            Confidence::High => fl!(self.loader, "confidence-high"),
            Confidence::VeryHigh => fl!(self.loader, "confidence-very-high"),
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
            DateModifier::Interpreted { date, .. } => {
                fl!(self.loader, "date-about", date = self.date_point(calendar, date))
            }
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
            AppError::Plugin(detail) => fl!(self.loader, "err-plugin", detail = detail.clone()),
            AppError::PersonNotFound(id) => fl!(self.loader, "err-person-not-found", id = id.clone()),
            AppError::FamilyNotFound(id) => fl!(self.loader, "err-family-not-found", id = id.clone()),
            AppError::PlaceNotFound(id) => fl!(self.loader, "err-place-not-found", id = id.clone()),
            AppError::SourceNotFound(id) => fl!(self.loader, "err-source-not-found", id = id.clone()),
            AppError::CitationNotFound(id) => fl!(self.loader, "err-citation-not-found", id = id.clone()),
            AppError::EventNotFound(id) => fl!(self.loader, "err-event-not-found", id = id.clone()),
            AppError::DnaTestNotFound(id) => fl!(self.loader, "err-dna-test-not-found", id = id.clone()),
            AppError::DnaMatchNotFound(id) => fl!(self.loader, "err-dna-match-not-found", id = id.clone()),
            AppError::RepositoryNotFound(id) => fl!(self.loader, "err-repository-not-found", id = id.clone()),
            AppError::NoteNotFound(id) => fl!(self.loader, "err-note-not-found", id = id.clone()),
            AppError::MediaNotFound(id) => fl!(self.loader, "err-media-not-found", id = id.clone()),
            AppError::TagNotFound(id) => fl!(self.loader, "err-tag-not-found", id = id.clone()),
            AppError::Domain(domain) => self.person_error(domain),
            AppError::FamilyDomain(domain) => self.family_error(domain),
            AppError::PlaceDomain(domain) => self.place_error(domain),
            AppError::SourceDomain(domain) => self.source_error(domain),
            AppError::CitationDomain(domain) => self.citation_error(domain),
            AppError::EventDomain(domain) => self.event_error(domain),
            AppError::DnaTestDomain(domain) => self.dna_test_error(domain),
            AppError::DnaMatchDomain(domain) => self.dna_match_error(domain),
            AppError::RepositoryDomain(domain) => self.repository_error(domain),
            AppError::NoteDomain(domain) => self.note_error(domain),
            AppError::MediaDomain(domain) => self.media_error(domain),
            AppError::TagDomain(domain) => self.tag_error(domain),
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
    use std::collections::BTreeSet;
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
        let partner = |human_id: &str| genealogy_app::PartnerRef {
            human_id: human_id.to_owned(),
            id: String::new(),
            name: None,
            vitals: None,
            confidence: genealogy_app::Confidence::Normal,
            source_count: 0,
        };
        let summary = FamilySummary {
            human_id: "F0001".to_owned(),
            id: String::new(),
            partners: vec![partner("I0001"), partner("I0002")],
            children: Vec::new(),
            events: Vec::new(),
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: BTreeSet::new(),
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
            evidence_level: genealogy_app::EvidenceLevel::Conclusion,
            display_name: Some("Ada".to_owned()),
            given: Some("Ada".to_owned()),
            surname: None,
            surname_prefix: None,
            nickname: None,
            name_prefix: None,
            name_suffix: None,
            name_type: None,
            names: Vec::new(),
            sex: Some(Sex::Other("intersex".to_owned())),
            facts: Vec::new(),
            associations: Vec::new(),
            participations: Vec::new(),
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: BTreeSet::from([Restriction::Privacy]),
        };
        let line = localizer("en").summary_line(&summary);
        assert!(line.contains("intersex"), "got: {line}");
        assert!(line.contains("[privacy]"), "got: {line}");
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
