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

use genealogy_app::{
    AppError, AssociationRole, Calendar, ChildParentRelationship, DateModifier, DatePoint, DateQuality, DbError,
    FactType, GenealogicalDate, GenealogicalDateBody, NameType, ParticipantRole, Sex, config,
};
use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::{DesktopLanguageRequester, FileSystemAssets, LanguageLoader};
use i18n_embed_fl::fl;
use rust_embed::RustEmbed;
use tracing::warn;
use unic_langid::LanguageIdentifier;

use crate::presentation::{ConfidenceLevel, RestrictionKind};
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
            Some(Sex::Intersex) => fl!(self.loader, "sex-intersex"),
            Some(Sex::Other(value)) => value.clone(),
            None => fl!(self.loader, "no-value"),
        }
    }

    /// `No persons yet.`
    #[must_use]
    pub fn list_empty(&self) -> String {
        fl!(self.loader, "list-empty")
    }

    /// The localized label for a single privacy restriction (GEDCOM `RESN`).
    #[must_use]
    pub fn restriction_label(&self, kind: RestrictionKind) -> String {
        match kind {
            RestrictionKind::Confidential => fl!(self.loader, "restriction-confidential"),
            RestrictionKind::Locked => fl!(self.loader, "restriction-locked"),
            RestrictionKind::Privacy => fl!(self.loader, "restriction-privacy"),
        }
    }

    /// The localized label for a detail tab, keyed by its stable id (`overview`, `citations`, …).
    #[must_use]
    pub fn tab_label(&self, id: &str) -> String {
        match id {
            "names" => fl!(self.loader, "tab-names"),
            "facts" => fl!(self.loader, "tab-facts"),
            "events" => fl!(self.loader, "tab-events"),
            "associations" => fl!(self.loader, "tab-associations"),
            "families" => fl!(self.loader, "tab-families"),
            "citations" => fl!(self.loader, "tab-citations"),
            "media" => fl!(self.loader, "tab-media"),
            "notes" => fl!(self.loader, "tab-notes"),
            "tags" => fl!(self.loader, "tab-tags"),
            "history" => fl!(self.loader, "tab-history"),
            _ => fl!(self.loader, "tab-overview"),
        }
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

    /// The localized label for a generic field, keyed by a stable id (`nickname`, `prefix`, `value`,
    /// `confidence`, `citation`, `name-type`, `fact-type`, `role`, …) — used by the edit forms.
    #[must_use]
    pub fn field_label(&self, id: &str) -> String {
        match id {
            "nickname" => fl!(self.loader, "field-nickname"),
            "prefix" => fl!(self.loader, "field-prefix"),
            "suffix" => fl!(self.loader, "field-suffix"),
            "name-type" => fl!(self.loader, "field-name-type"),
            "fact-type" => fl!(self.loader, "field-fact-type"),
            "date" => fl!(self.loader, "field-date"),
            "place" => fl!(self.loader, "field-place"),
            "confidence" => fl!(self.loader, "field-confidence"),
            "citation" => fl!(self.loader, "field-citation"),
            "media" => fl!(self.loader, "field-media"),
            "note" => fl!(self.loader, "field-note"),
            "tag" => fl!(self.loader, "field-tag"),
            "association" => fl!(self.loader, "field-association"),
            "role" => fl!(self.loader, "field-role"),
            "language" => fl!(self.loader, "field-language"),
            "source" => fl!(self.loader, "field-source"),
            "surety" => fl!(self.loader, "field-surety"),
            "relationship" => fl!(self.loader, "field-relationship"),
            _ => fl!(self.loader, "field-value"),
        }
    }

    /// The Overview tab's evidence-first section note.
    #[must_use]
    pub fn overview_note(&self) -> String {
        fl!(self.loader, "overview-note")
    }

    /// The localized heading for an Overview section (`vitals`, `family`).
    #[must_use]
    pub fn section_label(&self, id: &str) -> String {
        match id {
            "family" => fl!(self.loader, "section-family"),
            _ => fl!(self.loader, "section-vitals"),
        }
    }

    /// The "Children" relation label for the Overview immediate-family card.
    #[must_use]
    pub fn family_children(&self) -> String {
        fl!(self.loader, "family-children")
    }

    /// The localized label for an action, keyed by id (`add-name`, `add-fact`, `edit`, `add-source`,
    /// `save`, `cancel`, `attach-citation`, …).
    #[must_use]
    pub fn action_label(&self, id: &str) -> String {
        match id {
            "add-name" => fl!(self.loader, "action-add-name"),
            "add-fact" => fl!(self.loader, "action-add-fact"),
            "add-source" => fl!(self.loader, "action-add-source"),
            "attach-citation" => fl!(self.loader, "action-attach-citation"),
            "attach-media" => fl!(self.loader, "action-attach-media"),
            "attach-note" => fl!(self.loader, "action-attach-note"),
            "add-tag" => fl!(self.loader, "action-add-tag"),
            "add-association" => fl!(self.loader, "action-add-association"),
            "compare" => fl!(self.loader, "action-compare"),
            "edit" => fl!(self.loader, "action-edit"),
            "cancel" => fl!(self.loader, "action-cancel"),
            "saved" => fl!(self.loader, "action-saved"),
            "dismiss" => fl!(self.loader, "action-dismiss"),
            _ => fl!(self.loader, "action-save"),
        }
    }

    /// The vital "born" affix for the detail header, e.g. `b. 1850`.
    #[must_use]
    pub fn vital_born(&self, date: &str) -> String {
        fl!(self.loader, "vital-born", date = date)
    }

    /// The vital "died" affix for the detail header, e.g. `d. 1920`.
    #[must_use]
    pub fn vital_died(&self, date: &str) -> String {
        fl!(self.loader, "vital-died", date = date)
    }

    /// The "no source" flag text shown on an unsourced fact (icon + text — colour-not-alone).
    #[must_use]
    pub fn no_source(&self) -> String {
        fl!(self.loader, "no-source")
    }

    /// The source-count link text, e.g. `2 sources`.
    #[must_use]
    pub fn source_count(&self, count: usize) -> String {
        fl!(self.loader, "source-count", count = count)
    }

    /// The provenance popover title ("Why we believe this").
    #[must_use]
    pub fn provenance_title(&self) -> String {
        fl!(self.loader, "provenance-title")
    }

    /// The empty-state text shown in an empty detail tab.
    #[must_use]
    pub fn tab_empty(&self) -> String {
        fl!(self.loader, "tab-empty")
    }

    /// The placeholder shown in the History tab until the change log lands (PR5).
    #[must_use]
    pub fn history_placeholder(&self) -> String {
        fl!(self.loader, "history-placeholder")
    }

    /// The localized label for a confidence level (data-model §8).
    #[must_use]
    pub fn confidence_label(&self, level: ConfidenceLevel) -> String {
        match level {
            ConfidenceLevel::VeryLow => fl!(self.loader, "confidence-very-low"),
            ConfidenceLevel::Low => fl!(self.loader, "confidence-low"),
            ConfidenceLevel::Normal => fl!(self.loader, "confidence-normal"),
            ConfidenceLevel::High => fl!(self.loader, "confidence-high"),
            ConfidenceLevel::VeryHigh => fl!(self.loader, "confidence-very-high"),
        }
    }

    /// The localized label for a fact type; a [`FactType::Custom`] value renders verbatim.
    #[must_use]
    pub fn fact_type_label(&self, fact_type: &FactType) -> String {
        match fact_type {
            FactType::Birth => fl!(self.loader, "fact-birth"),
            FactType::Death => fl!(self.loader, "fact-death"),
            FactType::Baptism => fl!(self.loader, "fact-baptism"),
            FactType::Burial => fl!(self.loader, "fact-burial"),
            FactType::Occupation => fl!(self.loader, "fact-occupation"),
            FactType::Residence => fl!(self.loader, "fact-residence"),
            FactType::Religion => fl!(self.loader, "fact-religion"),
            FactType::Caste => fl!(self.loader, "fact-caste"),
            FactType::PhysicalDescription => fl!(self.loader, "fact-physical-description"),
            FactType::Education => fl!(self.loader, "fact-education"),
            FactType::Ethnicity => fl!(self.loader, "fact-ethnicity"),
            FactType::NationalId => fl!(self.loader, "fact-national-id"),
            FactType::Nationality => fl!(self.loader, "fact-nationality"),
            FactType::NumberOfChildren => fl!(self.loader, "fact-number-of-children"),
            FactType::NumberOfMarriages => fl!(self.loader, "fact-number-of-marriages"),
            FactType::Property => fl!(self.loader, "fact-property"),
            FactType::SocialSecurityNumber => fl!(self.loader, "fact-social-security-number"),
            FactType::NobilityTitle => fl!(self.loader, "fact-nobility-title"),
            FactType::Custom(value) => value.clone(),
        }
    }

    /// Every non-custom fact type, for building the "Add fact" picker.
    #[must_use]
    pub fn fact_type_choices(&self) -> Vec<(FactType, String)> {
        let types = [
            FactType::Birth,
            FactType::Death,
            FactType::Baptism,
            FactType::Burial,
            FactType::Occupation,
            FactType::Residence,
            FactType::Religion,
            FactType::Caste,
            FactType::PhysicalDescription,
            FactType::Education,
            FactType::Ethnicity,
            FactType::NationalId,
            FactType::Nationality,
            FactType::NumberOfChildren,
            FactType::NumberOfMarriages,
            FactType::Property,
            FactType::SocialSecurityNumber,
            FactType::NobilityTitle,
        ];
        types
            .into_iter()
            .map(|kind| (kind.clone(), self.fact_type_label(&kind)))
            .collect()
    }

    /// The localized label for a name type; a [`NameType::Custom`] value renders verbatim.
    #[must_use]
    pub fn name_type_label(&self, name_type: &NameType) -> String {
        match name_type {
            NameType::BirthName => fl!(self.loader, "name-type-birth"),
            NameType::MarriedName => fl!(self.loader, "name-type-married"),
            NameType::Maiden => fl!(self.loader, "name-type-maiden"),
            NameType::Immigrant => fl!(self.loader, "name-type-immigrant"),
            NameType::Professional => fl!(self.loader, "name-type-professional"),
            NameType::AlsoKnownAs => fl!(self.loader, "name-type-aka"),
            NameType::ReligiousName => fl!(self.loader, "name-type-religious"),
            NameType::Custom(value) => value.clone(),
        }
    }

    /// The localized label for an event-participant role; [`ParticipantRole::Custom`] renders verbatim.
    #[must_use]
    pub fn participant_role_label(&self, role: &ParticipantRole) -> String {
        match role {
            ParticipantRole::Primary => self.role("primary"),
            ParticipantRole::Witness => self.role("witness"),
            ParticipantRole::Officiator => self.role("officiator"),
            ParticipantRole::Clergy => self.role("clergy"),
            ParticipantRole::Father => self.role("father"),
            ParticipantRole::Mother => self.role("mother"),
            ParticipantRole::Parent => self.role("parent"),
            ParticipantRole::Child => self.role("child"),
            ParticipantRole::Husband => self.role("husband"),
            ParticipantRole::Wife => self.role("wife"),
            ParticipantRole::Spouse => self.role("spouse"),
            ParticipantRole::Godparent => self.role("godparent"),
            ParticipantRole::Friend => self.role("friend"),
            ParticipantRole::Neighbour => self.role("neighbour"),
            ParticipantRole::Multiple => self.role("multiple"),
            ParticipantRole::Bride => self.role("bride"),
            ParticipantRole::Groom => self.role("groom"),
            ParticipantRole::Custom(value) => value.clone(),
        }
    }

    /// The localized label for a person-association role; [`AssociationRole::Custom`] renders verbatim.
    #[must_use]
    pub fn association_role_label(&self, role: &AssociationRole) -> String {
        match role {
            AssociationRole::Clergy => self.role("clergy"),
            AssociationRole::Friend => self.role("friend"),
            AssociationRole::Godparent => self.role("godparent"),
            AssociationRole::Neighbour => self.role("neighbour"),
            AssociationRole::Officiator => self.role("officiator"),
            AssociationRole::Witness => self.role("witness"),
            AssociationRole::Child => self.role("child"),
            AssociationRole::Father => self.role("father"),
            AssociationRole::Mother => self.role("mother"),
            AssociationRole::Parent => self.role("parent"),
            AssociationRole::Husband => self.role("husband"),
            AssociationRole::Wife => self.role("wife"),
            AssociationRole::Spouse => self.role("spouse"),
            AssociationRole::Multiple => self.role("multiple"),
            AssociationRole::Custom(value) => value.clone(),
        }
    }

    /// The localized "spouse/partner" role label for the Families tab's partners.
    #[must_use]
    pub fn partner_role_label(&self) -> String {
        self.role("spouse")
    }

    /// The localized label for one role token shared by participant and association roles, and by
    /// the Families tab's partner role.
    pub(crate) fn role(&self, id: &str) -> String {
        match id {
            "primary" => fl!(self.loader, "role-primary"),
            "witness" => fl!(self.loader, "role-witness"),
            "officiator" => fl!(self.loader, "role-officiator"),
            "clergy" => fl!(self.loader, "role-clergy"),
            "father" => fl!(self.loader, "role-father"),
            "mother" => fl!(self.loader, "role-mother"),
            "parent" => fl!(self.loader, "role-parent"),
            "child" => fl!(self.loader, "role-child"),
            "husband" => fl!(self.loader, "role-husband"),
            "wife" => fl!(self.loader, "role-wife"),
            "spouse" => fl!(self.loader, "role-spouse"),
            "godparent" => fl!(self.loader, "role-godparent"),
            "friend" => fl!(self.loader, "role-friend"),
            "neighbour" => fl!(self.loader, "role-neighbour"),
            "bride" => fl!(self.loader, "role-bride"),
            "groom" => fl!(self.loader, "role-groom"),
            _ => fl!(self.loader, "role-multiple"),
        }
    }

    /// The localized label for a child–parent relationship (data-model §6).
    #[must_use]
    pub fn relationship_label(&self, relationship: &ChildParentRelationship) -> String {
        match relationship {
            ChildParentRelationship::Birth => fl!(self.loader, "rel-birth"),
            ChildParentRelationship::Adopted => fl!(self.loader, "rel-adopted"),
            ChildParentRelationship::Foster => fl!(self.loader, "rel-foster"),
            ChildParentRelationship::Step => fl!(self.loader, "rel-step"),
            ChildParentRelationship::Sealed => fl!(self.loader, "rel-sealed"),
            ChildParentRelationship::Unknown => fl!(self.loader, "rel-unknown"),
            ChildParentRelationship::Custom(value) => value.clone(),
        }
    }

    /// Renders a [`GenealogicalDate`] as a locale-independent numeric string with localized
    /// qualifiers (before/about/range/…) and quality (estimated/calculated). Free-text dates render
    /// verbatim. (ICU-localized month/era names are the CLI's richer rendering; the UI keeps the
    /// numeric form and localizes only the genealogical qualifiers — ADR 0003.)
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

    fn date_modifier(&self, _calendar: Calendar, modifier: &DateModifier) -> String {
        match modifier {
            DateModifier::None(point) => numeric_point(point),
            DateModifier::Before(point) => fl!(self.loader, "date-before", date = numeric_point(point)),
            DateModifier::After(point) => fl!(self.loader, "date-after", date = numeric_point(point)),
            DateModifier::About(point) | DateModifier::Interpreted { date: point, .. } => {
                fl!(self.loader, "date-about", date = numeric_point(point))
            }
            DateModifier::From(point) => fl!(self.loader, "date-from", date = numeric_point(point)),
            DateModifier::To(point) => fl!(self.loader, "date-to", date = numeric_point(point)),
            DateModifier::Range { start, end } => fl!(
                self.loader,
                "date-range",
                start = numeric_point(start),
                end = numeric_point(end)
            ),
            DateModifier::Span { start, end } => fl!(
                self.loader,
                "date-span",
                start = numeric_point(start),
                end = numeric_point(end)
            ),
        }
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
            | AppError::EventNotFound(id)
            | AppError::DnaTestNotFound(id)
            | AppError::DnaMatchNotFound(id)
            | AppError::RepositoryNotFound(id)
            | AppError::NoteNotFound(id)
            | AppError::MediaNotFound(id)
            | AppError::TagNotFound(id) => fl!(self.loader, "err-not-found", id = id.clone()),
            AppError::Domain(_)
            | AppError::FamilyDomain(_)
            | AppError::PlaceDomain(_)
            | AppError::SourceDomain(_)
            | AppError::CitationDomain(_)
            | AppError::EventDomain(_)
            | AppError::DnaTestDomain(_)
            | AppError::DnaMatchDomain(_)
            | AppError::RepositoryDomain(_)
            | AppError::NoteDomain(_)
            | AppError::MediaDomain(_)
            | AppError::TagDomain(_) => fl!(self.loader, "err-domain"),
            AppError::Plugin(detail) => fl!(self.loader, "err-plugin", detail = detail.clone()),
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

/// Renders a single [`DatePoint`] numerically: `YYYY`, `YYYY-MM`, or `YYYY-MM-DD`, or `?` when the
/// year is unknown. Locale-independent, so it needs no Fluent catalogue.
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
