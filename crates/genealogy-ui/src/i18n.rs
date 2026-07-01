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
    ActivityDetail, AppError, AssociationRole, Calendar, ChangeLogEntry, ChildParentRelationship, ChromosomeSide,
    CitingContext, DateModifier, DatePoint, DateQuality, DbError, DnaGenomeBuild, DnaProvider, DnaTestType,
    EvidenceKind, EvidenceLevel, FactType, GenealogicalDate, GenealogicalDateBody, InformationKind, Kinship,
    MatchStatus, NameType, NoteType, OperatorKind, ParticipantRole, RepositoryType, Sex, SourceMediaType,
    SourceQuality, UsingKind, config,
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

    /// Resolves a help-article Fluent message id (`help-*`) to its display text; an unknown id
    /// renders as itself (graceful — the help render test surfaces a missing key, never a panic).
    /// Backs the data-driven help content ([`help`](crate::help)): headings, section labels, topic
    /// titles, and block prose.
    #[must_use]
    pub fn help_text(&self, id: &str) -> String {
        self.loader.get(id)
    }

    /// The "Most tools" label on a help contrast block.
    #[must_use]
    pub fn help_contrast_most(&self) -> String {
        fl!(self.loader, "help-label-most")
    }

    /// The "This app" label on a help contrast block.
    #[must_use]
    pub fn help_contrast_ours(&self) -> String {
        fl!(self.loader, "help-label-ours")
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
            "children" => fl!(self.loader, "tab-children"),
            "participants" => fl!(self.loader, "tab-participants"),
            "hierarchy" => fl!(self.loader, "tab-hierarchy"),
            "citations" => fl!(self.loader, "tab-citations"),
            "media" => fl!(self.loader, "tab-media"),
            "notes" => fl!(self.loader, "tab-notes"),
            "tags" => fl!(self.loader, "tab-tags"),
            "attributes" => fl!(self.loader, "tab-attributes"),
            "repositories" => fl!(self.loader, "tab-repositories"),
            "sources" => fl!(self.loader, "tab-sources"),
            "addresses" => fl!(self.loader, "tab-addresses"),
            "urls" => fl!(self.loader, "tab-urls"),
            "content" => fl!(self.loader, "tab-content"),
            "language" => fl!(self.loader, "tab-language"),
            "references" => fl!(self.loader, "tab-references"),
            "usage" => fl!(self.loader, "tab-usage"),
            "haplogroups" => fl!(self.loader, "tab-haplogroups"),
            "matches" => fl!(self.loader, "tab-matches"),
            "segments" => fl!(self.loader, "tab-segments"),
            "ancestors" => fl!(self.loader, "tab-ancestors"),
            "history" => fl!(self.loader, "tab-history"),
            _ => fl!(self.loader, "tab-overview"),
        }
    }

    /// The localized label for a note type; a [`NoteType::Custom`] value renders verbatim.
    #[must_use]
    pub fn note_type_label(&self, note_type: &NoteType) -> String {
        match note_type {
            NoteType::General => fl!(self.loader, "note-type-general"),
            NoteType::Research => fl!(self.loader, "note-type-research"),
            NoteType::Transcript => fl!(self.loader, "note-type-transcript"),
            NoteType::Citation => fl!(self.loader, "note-type-citation"),
            NoteType::Custom(value) => value.clone(),
        }
    }

    /// The localized label for the kind of record that references a media object or note (the Media
    /// "Used by" / Note "References" row chip), driving the navigation route.
    #[must_use]
    pub fn using_kind_label(&self, kind: UsingKind) -> String {
        match kind {
            UsingKind::Person => fl!(self.loader, "using-kind-person"),
            UsingKind::Family => fl!(self.loader, "using-kind-family"),
            UsingKind::Event => fl!(self.loader, "using-kind-event"),
            UsingKind::Place => fl!(self.loader, "using-kind-place"),
            UsingKind::Source => fl!(self.loader, "using-kind-source"),
            UsingKind::Citation => fl!(self.loader, "using-kind-citation"),
            UsingKind::Repository => fl!(self.loader, "using-kind-repository"),
            UsingKind::Media => fl!(self.loader, "using-kind-media"),
            UsingKind::Note => fl!(self.loader, "using-kind-note"),
            UsingKind::DnaTest => fl!(self.loader, "using-kind-dna-test"),
            UsingKind::DnaMatch => fl!(self.loader, "using-kind-dna-match"),
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
            "name" => fl!(self.loader, "field-name"),
            "year" => fl!(self.loader, "field-year"),
            "month" => fl!(self.loader, "field-month"),
            "day" => fl!(self.loader, "field-day"),
            "code" => fl!(self.loader, "field-code"),
            "web-path" => fl!(self.loader, "field-web-path"),
            "coordinates" => fl!(self.loader, "field-coordinates"),
            "latitude" => fl!(self.loader, "field-latitude"),
            "longitude" => fl!(self.loader, "field-longitude"),
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
            "page" => fl!(self.loader, "field-page"),
            "attribute-type" => fl!(self.loader, "field-attribute-type"),
            "evidence" => fl!(self.loader, "field-evidence"),
            "born" => fl!(self.loader, "field-born"),
            "partner" => fl!(self.loader, "field-partner"),
            "child" => fl!(self.loader, "field-child"),
            "title" => fl!(self.loader, "field-title"),
            "author" => fl!(self.loader, "field-author"),
            "publication" => fl!(self.loader, "field-publication"),
            "abbreviation" => fl!(self.loader, "field-abbreviation"),
            "call-number" => fl!(self.loader, "field-call-number"),
            "media-type" => fl!(self.loader, "field-media-type"),
            "used-by" => fl!(self.loader, "field-used-by"),
            "typical-surety" => fl!(self.loader, "field-typical-surety"),
            "type" => fl!(self.loader, "field-type"),
            "street" => fl!(self.loader, "field-street"),
            "locality" => fl!(self.loader, "field-locality"),
            "region" => fl!(self.loader, "field-region"),
            "postal-code" => fl!(self.loader, "field-postal-code"),
            "country" => fl!(self.loader, "field-country"),
            "phone" => fl!(self.loader, "field-phone"),
            "email" => fl!(self.loader, "field-email"),
            "url" => fl!(self.loader, "field-url"),
            "description" => fl!(self.loader, "field-description"),
            "backs-record" => fl!(self.loader, "field-backs-record"),
            "sources" => fl!(self.loader, "field-sources"),
            "citations" => fl!(self.loader, "field-citations"),
            "file-path" => fl!(self.loader, "field-file-path"),
            "mime" => fl!(self.loader, "field-mime"),
            "checksum" => fl!(self.loader, "field-checksum"),
            "translator" => fl!(self.loader, "field-translator"),
            "translation" => fl!(self.loader, "field-translation"),
            "object" => fl!(self.loader, "field-object"),
            "id" => fl!(self.loader, "field-id"),
            "priority" => fl!(self.loader, "field-priority"),
            "color" => fl!(self.loader, "field-color"),
            "provider" => fl!(self.loader, "field-provider"),
            "test-type" => fl!(self.loader, "field-test-type"),
            "kit-id" => fl!(self.loader, "field-kit-id"),
            "genome-build" => fl!(self.loader, "field-genome-build"),
            "person" => fl!(self.loader, "field-person"),
            "haplogroup" => fl!(self.loader, "field-haplogroup"),
            "lineage" => fl!(self.loader, "field-lineage"),
            "terminal-snp" => fl!(self.loader, "field-terminal-snp"),
            "shared-cm" => fl!(self.loader, "field-shared-cm"),
            "percent-shared" => fl!(self.loader, "field-percent-shared"),
            "largest-segment" => fl!(self.loader, "field-largest-segment"),
            "segment-count" => fl!(self.loader, "field-segment-count"),
            "predicted" => fl!(self.loader, "field-predicted"),
            "status" => fl!(self.loader, "field-status"),
            "compared-test" => fl!(self.loader, "field-compared-test"),
            "test-a" => fl!(self.loader, "field-test-a"),
            "test-b" => fl!(self.loader, "field-test-b"),
            "ancestor" => fl!(self.loader, "field-ancestor"),
            "chromosome" => fl!(self.loader, "field-chromosome"),
            "start" => fl!(self.loader, "field-start"),
            "end" => fl!(self.loader, "field-end"),
            "centimorgans" => fl!(self.loader, "field-centimorgans"),
            "snps" => fl!(self.loader, "field-snps"),
            "side" => fl!(self.loader, "field-side"),
            "object-type" => fl!(self.loader, "field-object-type"),
            "count" => fl!(self.loader, "field-count"),
            "examples" => fl!(self.loader, "field-examples"),
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
            "partners" => fl!(self.loader, "section-partners"),
            "marriage" => fl!(self.loader, "section-marriage"),
            "bibliographic" => fl!(self.loader, "section-bibliographic"),
            "reliability" => fl!(self.loader, "section-reliability"),
            "repository" => fl!(self.loader, "section-repository"),
            "contact" => fl!(self.loader, "section-contact"),
            "file" => fl!(self.loader, "section-file"),
            "primary-language" => fl!(self.loader, "section-primary-language"),
            "tag" => fl!(self.loader, "section-tag"),
            "color" => fl!(self.loader, "section-color"),
            "kit" => fl!(self.loader, "section-kit"),
            "tested-person" => fl!(self.loader, "section-tested-person"),
            "ethnicity" => fl!(self.loader, "section-ethnicity"),
            "compared-tests" => fl!(self.loader, "section-compared-tests"),
            "shared-dna" => fl!(self.loader, "section-shared-dna"),
            "inferred-relationship" => fl!(self.loader, "section-inferred-relationship"),
            _ => fl!(self.loader, "section-vitals"),
        }
    }

    /// The Family list empty-state message.
    #[must_use]
    pub fn family_list_empty(&self) -> String {
        fl!(self.loader, "family-list-empty")
    }

    /// The Family Overview neutral-roles / evidence-first section note.
    #[must_use]
    pub fn family_overview_note(&self) -> String {
        fl!(self.loader, "family-overview-note")
    }

    /// The localized label for an event type; a [`EventType::Custom`] value renders verbatim.
    #[must_use]
    pub fn event_type_label(&self, event_type: &genealogy_app::EventType) -> String {
        use genealogy_app::EventType;
        match event_type {
            EventType::Birth => fl!(self.loader, "event-type-birth"),
            EventType::Death => fl!(self.loader, "event-type-death"),
            EventType::Marriage => fl!(self.loader, "event-type-marriage"),
            EventType::Baptism => fl!(self.loader, "event-type-baptism"),
            EventType::Christening => fl!(self.loader, "event-type-christening"),
            EventType::Burial => fl!(self.loader, "event-type-burial"),
            EventType::Cremation => fl!(self.loader, "event-type-cremation"),
            EventType::Census => fl!(self.loader, "event-type-census"),
            EventType::Residence => fl!(self.loader, "event-type-residence"),
            EventType::Immigration => fl!(self.loader, "event-type-immigration"),
            EventType::Emigration => fl!(self.loader, "event-type-emigration"),
            EventType::Adoption => fl!(self.loader, "event-type-adoption"),
            EventType::Confirmation => fl!(self.loader, "event-type-confirmation"),
            EventType::BarMitzvah => fl!(self.loader, "event-type-bar-mitzvah"),
            EventType::BasMitzvah => fl!(self.loader, "event-type-bas-mitzvah"),
            EventType::FirstCommunion => fl!(self.loader, "event-type-first-communion"),
            EventType::Graduation => fl!(self.loader, "event-type-graduation"),
            EventType::Naturalization => fl!(self.loader, "event-type-naturalization"),
            EventType::Ordination => fl!(self.loader, "event-type-ordination"),
            EventType::Probate => fl!(self.loader, "event-type-probate"),
            EventType::Retirement => fl!(self.loader, "event-type-retirement"),
            EventType::Will => fl!(self.loader, "event-type-will"),
            EventType::Engagement => fl!(self.loader, "event-type-engagement"),
            EventType::Annulment => fl!(self.loader, "event-type-annulment"),
            EventType::Divorce => fl!(self.loader, "event-type-divorce"),
            EventType::DivorceFiled => fl!(self.loader, "event-type-divorce-filed"),
            EventType::MarriageBanns => fl!(self.loader, "event-type-marriage-banns"),
            EventType::MarriageContract => fl!(self.loader, "event-type-marriage-contract"),
            EventType::MarriageLicense => fl!(self.loader, "event-type-marriage-license"),
            EventType::MarriageSettlement => fl!(self.loader, "event-type-marriage-settlement"),
            EventType::Custom(value) => value.clone(),
        }
    }

    /// The localized label for a place type; a [`PlaceType::Custom`] value renders verbatim.
    #[must_use]
    pub fn place_type_label(&self, place_type: &genealogy_app::PlaceType) -> String {
        use genealogy_app::PlaceType;
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

    /// The Event list empty-state message.
    #[must_use]
    pub fn event_list_empty(&self) -> String {
        fl!(self.loader, "event-list-empty")
    }

    /// The Event Overview structured-date / evidence-first section note.
    #[must_use]
    pub fn event_overview_note(&self) -> String {
        fl!(self.loader, "event-overview-note")
    }

    /// The Place list empty-state message.
    #[must_use]
    pub fn place_list_empty(&self) -> String {
        fl!(self.loader, "place-list-empty")
    }

    /// The Place Overview name-history / jurisdiction-chain section note.
    #[must_use]
    pub fn place_overview_note(&self) -> String {
        fl!(self.loader, "place-overview-note")
    }

    /// The Place Names tab's dated / language-tagged section note.
    #[must_use]
    pub fn place_names_note(&self) -> String {
        fl!(self.loader, "place-names-note")
    }

    /// The Place Hierarchy tab's dated-jurisdiction section note.
    #[must_use]
    pub fn place_hierarchy_note(&self) -> String {
        fl!(self.loader, "place-hierarchy-note")
    }

    /// The Source list empty-state message.
    #[must_use]
    pub fn source_list_empty(&self) -> String {
        fl!(self.loader, "source-list-empty")
    }

    /// The Source Overview master-record / two-way-provenance section note.
    #[must_use]
    pub fn source_overview_note(&self) -> String {
        fl!(self.loader, "source-overview-note")
    }

    /// The Source Citations tab's "citations that use this source" section note.
    #[must_use]
    pub fn source_citations_note(&self) -> String {
        fl!(self.loader, "source-citations-note")
    }

    /// The Repository list empty-state message.
    #[must_use]
    pub fn repository_list_empty(&self) -> String {
        fl!(self.loader, "repository-list-empty")
    }

    /// The Media list empty-state message.
    #[must_use]
    pub fn media_list_empty(&self) -> String {
        fl!(self.loader, "media-list-empty")
    }

    /// The Note list empty-state message.
    #[must_use]
    pub fn note_list_empty(&self) -> String {
        fl!(self.loader, "note-list-empty")
    }

    /// The Media Overview "Used by" / provenance section note.
    #[must_use]
    pub fn media_used_by_note(&self) -> String {
        fl!(self.loader, "media-used-by-note")
    }

    /// The Note References tab's "what references this note" section note.
    #[must_use]
    pub fn note_references_note(&self) -> String {
        fl!(self.loader, "note-references-note")
    }

    /// The Note Content tab's "type + rich text" section note.
    #[must_use]
    pub fn note_content_note(&self) -> String {
        fl!(self.loader, "note-content-note")
    }

    /// The Media Overview preview-card caption.
    #[must_use]
    pub fn media_preview(&self) -> String {
        fl!(self.loader, "media-preview")
    }

    /// The Tag list empty-state message.
    #[must_use]
    pub fn tag_list_empty(&self) -> String {
        fl!(self.loader, "tag-list-empty")
    }

    /// The DNA-test list empty-state message.
    #[must_use]
    pub fn dna_test_list_empty(&self) -> String {
        fl!(self.loader, "dna-test-list-empty")
    }

    /// The DNA-match list empty-state message.
    #[must_use]
    pub fn dna_match_list_empty(&self) -> String {
        fl!(self.loader, "dna-match-list-empty")
    }

    /// The Tag Overview colour/priority section note.
    #[must_use]
    pub fn tag_overview_note(&self) -> String {
        fl!(self.loader, "tag-overview-note")
    }

    /// The Tag Usage tab's grouped-by-type section note.
    #[must_use]
    pub fn tag_usage_note(&self) -> String {
        fl!(self.loader, "tag-usage-note")
    }

    /// The DNA-test Overview auditable-record section note.
    #[must_use]
    pub fn dna_test_overview_note(&self) -> String {
        fl!(self.loader, "dna-test-overview-note")
    }

    /// The DNA-test ethnicity-estimate later-phase section note.
    #[must_use]
    pub fn dna_test_ethnicity_note(&self) -> String {
        fl!(self.loader, "dna-test-ethnicity-note")
    }

    /// The DNA-match Overview observation-vs-conclusion section note.
    #[must_use]
    pub fn dna_match_overview_note(&self) -> String {
        fl!(self.loader, "dna-match-overview-note")
    }

    /// The DNA-match Segments tab's phasing section note.
    #[must_use]
    pub fn dna_match_segments_note(&self) -> String {
        fl!(self.loader, "dna-match-segments-note")
    }

    /// The DNA-match Shared ancestors tab's inferred-conclusion section note.
    #[must_use]
    pub fn dna_match_ancestors_note(&self) -> String {
        fl!(self.loader, "dna-match-ancestors-note")
    }

    /// The localized DNA-provider label; a [`DnaProvider::Custom`] value renders verbatim.
    #[must_use]
    pub fn dna_provider_label(&self, provider: &DnaProvider) -> String {
        match provider {
            DnaProvider::AncestryDna => fl!(self.loader, "dna-provider-ancestry"),
            DnaProvider::TwentyThreeAndMe => fl!(self.loader, "dna-provider-23andme"),
            DnaProvider::MyHeritage => fl!(self.loader, "dna-provider-myheritage"),
            DnaProvider::FamilyTreeDna => fl!(self.loader, "dna-provider-ftdna"),
            DnaProvider::GedMatch => fl!(self.loader, "dna-provider-gedmatch"),
            DnaProvider::LivingDna => fl!(self.loader, "dna-provider-livingdna"),
            DnaProvider::Custom(value) => value.clone(),
        }
    }

    /// The localized DNA-test-type label.
    #[must_use]
    pub fn dna_test_type_label(&self, test_type: DnaTestType) -> String {
        match test_type {
            DnaTestType::Autosomal => fl!(self.loader, "dna-test-type-autosomal"),
            DnaTestType::YDna => fl!(self.loader, "dna-test-type-ydna"),
            DnaTestType::MtDna => fl!(self.loader, "dna-test-type-mtdna"),
            DnaTestType::XDna => fl!(self.loader, "dna-test-type-xdna"),
        }
    }

    /// The localized DNA genome-build label.
    #[must_use]
    pub fn dna_genome_build_label(&self, build: DnaGenomeBuild) -> String {
        match build {
            DnaGenomeBuild::GRCh37 => fl!(self.loader, "dna-genome-build-37"),
            DnaGenomeBuild::GRCh38 => fl!(self.loader, "dna-genome-build-38"),
        }
    }

    /// The localized chromosome-side (segment phasing) label.
    #[must_use]
    pub fn chromosome_side_label(&self, side: ChromosomeSide) -> String {
        match side {
            ChromosomeSide::Maternal => fl!(self.loader, "chromosome-side-maternal"),
            ChromosomeSide::Paternal => fl!(self.loader, "chromosome-side-paternal"),
            ChromosomeSide::Unknown => fl!(self.loader, "chromosome-side-unknown"),
        }
    }

    /// The localized DNA-match status label; `None` is the "undecided" placeholder.
    #[must_use]
    pub fn match_status_label(&self, status: Option<MatchStatus>) -> String {
        match status {
            Some(MatchStatus::Confirmed) => fl!(self.loader, "match-status-confirmed"),
            Some(MatchStatus::Rejected) => fl!(self.loader, "match-status-rejected"),
            None => fl!(self.loader, "match-status-undecided"),
        }
    }

    /// The Repository Overview holds-sources / follow-provenance section note.
    #[must_use]
    pub fn repository_overview_note(&self) -> String {
        fl!(self.loader, "repository-overview-note")
    }

    /// The localized label for a repository type; a [`RepositoryType::Custom`] value renders verbatim.
    #[must_use]
    pub fn repository_type_label(&self, repository_type: &RepositoryType) -> String {
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

    /// The localized label for a source medium; a [`SourceMediaType::Custom`] value renders verbatim.
    #[must_use]
    pub fn source_media_type_label(&self, media_type: &SourceMediaType) -> String {
        match media_type {
            SourceMediaType::Book => fl!(self.loader, "media-type-book"),
            SourceMediaType::Card => fl!(self.loader, "media-type-card"),
            SourceMediaType::Electronic => fl!(self.loader, "media-type-electronic"),
            SourceMediaType::Fiche => fl!(self.loader, "media-type-fiche"),
            SourceMediaType::Film => fl!(self.loader, "media-type-film"),
            SourceMediaType::Magazine => fl!(self.loader, "media-type-magazine"),
            SourceMediaType::Manuscript => fl!(self.loader, "media-type-manuscript"),
            SourceMediaType::Map => fl!(self.loader, "media-type-map"),
            SourceMediaType::Newspaper => fl!(self.loader, "media-type-newspaper"),
            SourceMediaType::Photo => fl!(self.loader, "media-type-photo"),
            SourceMediaType::Tombstone => fl!(self.loader, "media-type-tombstone"),
            SourceMediaType::Video => fl!(self.loader, "media-type-video"),
            SourceMediaType::Audio => fl!(self.loader, "media-type-audio"),
            SourceMediaType::Custom(value) => value.clone(),
        }
    }

    /// The localized sub-context for a "Backs record" cell (the fact type, the participant role, …).
    /// A row-level citation has no sub-context, so it renders empty.
    #[must_use]
    pub fn citing_context_label(&self, context: &CitingContext) -> String {
        match context {
            CitingContext::Record => String::new(),
            CitingContext::Name => fl!(self.loader, "citing-name"),
            CitingContext::Fact(fact_type) => self.fact_type_label(fact_type),
            CitingContext::Association(role) => self.association_role_label(role),
            CitingContext::Participant(role) => self.participant_role_label(role),
            CitingContext::Partner => fl!(self.loader, "citing-partner"),
            CitingContext::Child => fl!(self.loader, "citing-child"),
            CitingContext::FamilyEvent => fl!(self.loader, "citing-family-event"),
            CitingContext::PlaceType => fl!(self.loader, "citing-place-type"),
        }
    }

    /// The "Children" relation label for the Overview immediate-family card.
    #[must_use]
    pub fn family_children(&self) -> String {
        fl!(self.loader, "family-children")
    }

    /// The "{count} children" summary label for the Family list row subtitle.
    #[must_use]
    pub fn family_children_count(&self, count: usize) -> String {
        fl!(self.loader, "family-children-count", count = count.to_string())
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
            "remove-tag" => fl!(self.loader, "action-remove-tag"),
            "add-association" => fl!(self.loader, "action-add-association"),
            "set-page" => fl!(self.loader, "action-set-page"),
            "set-date" => fl!(self.loader, "action-set-date"),
            "set-confidence" => fl!(self.loader, "action-set-confidence"),
            "set-evidence" => fl!(self.loader, "action-set-evidence"),
            "add-attribute" => fl!(self.loader, "action-add-attribute"),
            "add-translation" => fl!(self.loader, "action-add-translation"),
            "add-haplogroup" => fl!(self.loader, "action-add-haplogroup"),
            "set-name" => fl!(self.loader, "action-set-name"),
            "set-priority" => fl!(self.loader, "action-set-priority"),
            "set-color" => fl!(self.loader, "action-set-color"),
            "confirm" => fl!(self.loader, "action-confirm"),
            "reject" => fl!(self.loader, "action-reject"),
            "add-partner" => fl!(self.loader, "action-add-partner"),
            "add-child" => fl!(self.loader, "action-add-child"),
            "link-event" => fl!(self.loader, "action-link-event"),
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

    /// The reference-count subtitle text, e.g. `2 references` (Note list row / header).
    #[must_use]
    pub fn reference_count(&self, count: usize) -> String {
        fl!(self.loader, "reference-count", count = count)
    }

    /// The provenance popover title ("Why we believe this").
    #[must_use]
    pub fn provenance_title(&self) -> String {
        fl!(self.loader, "provenance-title")
    }

    /// The per-claim provenance popover title ("Why we believe: {claim}").
    #[must_use]
    pub fn provenance_title_claim(&self, claim: &str) -> String {
        fl!(self.loader, "provenance-title-claim", claim = claim)
    }

    /// The "asserted by {who} · {when}" provenance line under a claim, or a who-only line when the
    /// creation timestamp is unknown.
    #[must_use]
    pub fn provenance_asserted_by(&self, who: &str, when: Option<&str>) -> String {
        match when {
            Some(when) => fl!(self.loader, "provenance-asserted-by", who = who, when = when),
            None => fl!(self.loader, "provenance-asserted-by-undated", who = who),
        }
    }

    /// `No citations yet.` — the citation list's empty state.
    #[must_use]
    pub fn citation_list_empty(&self) -> String {
        fl!(self.loader, "citation-list-empty")
    }

    /// The localized value of the *source* Evidence Explained axis (original vs derivative).
    #[must_use]
    pub fn evidence_source_label(&self, quality: SourceQuality) -> String {
        match quality {
            SourceQuality::Original => fl!(self.loader, "evidence-original"),
            SourceQuality::Derivative => fl!(self.loader, "evidence-derivative"),
        }
    }

    /// The localized value of the *information* Evidence Explained axis (primary vs secondary).
    #[must_use]
    pub fn evidence_information_label(&self, kind: InformationKind) -> String {
        match kind {
            InformationKind::Primary => fl!(self.loader, "evidence-primary"),
            InformationKind::Secondary => fl!(self.loader, "evidence-secondary"),
        }
    }

    /// The localized value of the *evidence* Evidence Explained axis (direct / indirect / negative).
    #[must_use]
    pub fn evidence_kind_label(&self, kind: EvidenceKind) -> String {
        match kind {
            EvidenceKind::Direct => fl!(self.loader, "evidence-direct"),
            EvidenceKind::Indirect => fl!(self.loader, "evidence-indirect"),
            EvidenceKind::Negative => fl!(self.loader, "evidence-negative"),
        }
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

    /// The empty-state text shown when a record has no change log yet.
    #[must_use]
    pub fn history_empty(&self) -> String {
        fl!(self.loader, "history-empty")
    }

    /// The History tab's explanatory note (the audit-trail differentiator).
    #[must_use]
    pub fn history_note(&self) -> String {
        fl!(self.loader, "history-note")
    }

    /// A localized phrase summarizing what an entry recorded.
    ///
    /// A payload-derived [`ActivityDetail`] gives the specific phrase (the fact's kind, a collapsed
    /// import's count); otherwise the event-type verb is used (one phrase per type across all 12
    /// aggregates), with a generic "recorded a change" only for an unmapped type.
    #[must_use]
    pub fn change_summary(&self, entry: &ChangeLogEntry) -> String {
        match &entry.detail {
            Some(ActivityDetail::Fact { fact_type }) => {
                let fact = self.fact_type_label(fact_type);
                fl!(self.loader, "history-fact-asserted-kind", fact = fact)
            }
            Some(ActivityDetail::ImportBatch { count }) => {
                let count = i64::from(*count);
                fl!(self.loader, "dashboard-import-batch", count = count)
            }
            None => self.event_type_summary(&entry.event_type),
        }
    }

    /// The localized verb phrase for an event type — one per variant across the 12 aggregates.
    ///
    /// An unrecognized type falls back to a generic "recorded a change"; the `change_summary_covers_*`
    /// tests assert every aggregate's variant names map to a specific phrase.
    fn event_type_summary(&self, event_type: &str) -> String {
        match event_type {
            "PersonCreated" => fl!(self.loader, "history-person-created"),
            "NameAsserted" => fl!(self.loader, "history-name-asserted"),
            "SexAsserted" => fl!(self.loader, "history-sex-asserted"),
            "FactAsserted" => fl!(self.loader, "history-fact-asserted"),
            "ParticipationAsserted" => fl!(self.loader, "history-participation-asserted"),
            "AssociationAsserted" => fl!(self.loader, "history-association-asserted"),
            "MediaAttached" => fl!(self.loader, "history-media-attached"),
            "NoteAttached" => fl!(self.loader, "history-note-attached"),
            "CitationAdded" => fl!(self.loader, "history-citation-added"),
            "ExternalIdAdded" => fl!(self.loader, "history-external-id-added"),
            "Tagged" => fl!(self.loader, "history-tagged"),
            "Untagged" => fl!(self.loader, "history-untagged"),
            "RestrictionsChanged" => fl!(self.loader, "history-restrictions-changed"),
            "AssertionRetracted" => fl!(self.loader, "history-assertion-retracted"),
            "AssertionSuperseded" => fl!(self.loader, "history-assertion-superseded"),
            "PersonsMerged" => fl!(self.loader, "history-persons-merged"),
            "CitationCreated" => fl!(self.loader, "history-citation-created"),
            "PageSet" => fl!(self.loader, "history-page-set"),
            "DateAsserted" => fl!(self.loader, "history-date-asserted"),
            "ConfidenceSet" => fl!(self.loader, "history-confidence-set"),
            "EvidenceAnalysisSet" => fl!(self.loader, "history-evidence-analysis-set"),
            "AttributeAdded" => fl!(self.loader, "history-attribute-added"),
            "DnaMatchObserved" => fl!(self.loader, "history-dna-match-observed"),
            "SegmentAdded" => fl!(self.loader, "history-segment-added"),
            "SharedAncestorAsserted" => fl!(self.loader, "history-shared-ancestor-asserted"),
            "MatchConfirmed" => fl!(self.loader, "history-match-confirmed"),
            "MatchRejected" => fl!(self.loader, "history-match-rejected"),
            "DnaTestCreated" => fl!(self.loader, "history-dna-test-created"),
            "ProviderSet" => fl!(self.loader, "history-provider-set"),
            "KitIdSet" => fl!(self.loader, "history-kit-id-set"),
            "TestTypeSet" => fl!(self.loader, "history-test-type-set"),
            "GenomeBuildSet" => fl!(self.loader, "history-genome-build-set"),
            "HaplogroupAsserted" => fl!(self.loader, "history-haplogroup-asserted"),
            "FamilyCreated" => fl!(self.loader, "history-family-created"),
            "PartnerAdded" => fl!(self.loader, "history-partner-added"),
            "PartnerRemoved" => fl!(self.loader, "history-partner-removed"),
            "ChildAdded" => fl!(self.loader, "history-child-added"),
            "ChildRemoved" => fl!(self.loader, "history-child-removed"),
            "FamilyEventLinked" => fl!(self.loader, "history-family-event-linked"),
            "MediaCreated" => fl!(self.loader, "history-media-created"),
            "PathSet" => fl!(self.loader, "history-path-set"),
            "ChecksumSet" => fl!(self.loader, "history-checksum-set"),
            "MimeSet" => fl!(self.loader, "history-mime-set"),
            "NoteCreated" => fl!(self.loader, "history-note-created"),
            "NoteTypeSet" => fl!(self.loader, "history-note-type-set"),
            "RichTextSet" => fl!(self.loader, "history-rich-text-set"),
            "PlaceCreated" => fl!(self.loader, "history-place-created"),
            "PlaceTypeSet" => fl!(self.loader, "history-place-type-set"),
            "EnclosedByAsserted" => fl!(self.loader, "history-enclosed-by-asserted"),
            "CoordinatesAsserted" => fl!(self.loader, "history-coordinates-asserted"),
            "CodeSet" => fl!(self.loader, "history-code-set"),
            "RepositoryCreated" => fl!(self.loader, "history-repository-created"),
            "RepositoryTypeSet" => fl!(self.loader, "history-repository-type-set"),
            "NameSet" => fl!(self.loader, "history-name-set"),
            "AddressAdded" => fl!(self.loader, "history-address-added"),
            "UrlAdded" => fl!(self.loader, "history-url-added"),
            "SourceCreated" => fl!(self.loader, "history-source-created"),
            "TitleSet" => fl!(self.loader, "history-title-set"),
            "AuthorSet" => fl!(self.loader, "history-author-set"),
            "PubInfoSet" => fl!(self.loader, "history-pub-info-set"),
            "AbbrevSet" => fl!(self.loader, "history-abbrev-set"),
            "RepositoryLinked" => fl!(self.loader, "history-repository-linked"),
            "TagCreated" => fl!(self.loader, "history-tag-created"),
            "TagRenamed" => fl!(self.loader, "history-tag-renamed"),
            "TagColorSet" => fl!(self.loader, "history-tag-color-set"),
            "TagPrioritySet" => fl!(self.loader, "history-tag-priority-set"),
            "EventCreated" => fl!(self.loader, "history-event-created"),
            "EventTypeSet" => fl!(self.loader, "history-event-type-set"),
            "DescriptionSet" => fl!(self.loader, "history-description-set"),
            "PlaceLinked" => fl!(self.loader, "history-place-linked"),
            "ParticipantRoleAdded" => fl!(self.loader, "history-participant-role-added"),
            "ParticipantRoleRemoved" => fl!(self.loader, "history-participant-role-removed"),
            _ => fl!(self.loader, "history-generic"),
        }
    }

    /// The operator line for a change-log entry: a human shows `name · <confidence>`, a software or AI
    /// agent shows `name (<kind>)`. Falls back to a localized "unknown operator" when no name was
    /// recorded.
    #[must_use]
    pub fn operator_line(&self, entry: &ChangeLogEntry) -> String {
        let name = entry
            .operator_display
            .clone()
            .unwrap_or_else(|| fl!(self.loader, "history-operator-unknown"));
        match entry.operator_kind {
            OperatorKind::Human => {
                let confidence = self.confidence_label(ConfidenceLevel::from(entry.confidence));
                fl!(
                    self.loader,
                    "history-operator-human",
                    name = name,
                    confidence = confidence
                )
            }
            OperatorKind::Software => {
                let kind = fl!(self.loader, "history-operator-software");
                fl!(self.loader, "history-operator-agent", name = name, kind = kind)
            }
            OperatorKind::AiModel => {
                let kind = fl!(self.loader, "history-operator-ai");
                fl!(self.loader, "history-operator-agent", name = name, kind = kind)
            }
        }
    }

    /// The localized label for a dashboard string, keyed by a stable id.
    #[must_use]
    pub fn dashboard_label(&self, id: &str) -> String {
        match id {
            "stat-people" => fl!(self.loader, "dashboard-stat-people"),
            "stat-evidence" => fl!(self.loader, "dashboard-stat-evidence"),
            "stat-evidence-caption" => fl!(self.loader, "dashboard-stat-evidence-caption"),
            "stat-attention" => fl!(self.loader, "dashboard-stat-attention"),
            "recent-activity" => fl!(self.loader, "dashboard-recent-activity"),
            "jump-back" => fl!(self.loader, "dashboard-jump-back"),
            "data-quality" => fl!(self.loader, "dashboard-data-quality"),
            "no-source-facts" => fl!(self.loader, "dashboard-no-source-facts"),
            "later-milestone" => fl!(self.loader, "dashboard-later-milestone"),
            "activity-empty" => fl!(self.loader, "dashboard-activity-empty"),
            _ => fl!(self.loader, "dashboard-title"),
        }
    }

    /// The `families · events` caption under the People stat card.
    #[must_use]
    pub fn dashboard_people_caption(&self, families: u64, events: u64) -> String {
        fl!(
            self.loader,
            "dashboard-people-caption",
            families = families,
            events = events
        )
    }

    /// The collapsed-import activity summary, e.g. `142 records imported`.
    #[must_use]
    pub fn activity_import_batch(&self, count: usize) -> String {
        fl!(self.loader, "dashboard-import-batch", count = count)
    }

    /// The undo control's accessible label for a history entry, naming the change it reverts.
    #[must_use]
    pub fn history_undo_label(&self, what: &str) -> String {
        fl!(self.loader, "history-undo", what = what)
    }

    /// The short visible "Undo" button text.
    #[must_use]
    pub fn history_undo_short(&self) -> String {
        fl!(self.loader, "history-undo-short")
    }

    /// The localized label for a person's evidence level — the personas badge (data-model §7).
    #[must_use]
    pub fn evidence_level_label(&self, level: EvidenceLevel) -> String {
        match level {
            EvidenceLevel::Persona => fl!(self.loader, "evidence-level-persona"),
            EvidenceLevel::Conclusion => fl!(self.loader, "evidence-level-conclusion"),
        }
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

    /// The "father of {name}" placeholder hint for an unresearched ancestor slot whose descendant is
    /// known.
    #[must_use]
    pub fn pedigree_unknown_father_of(&self, name: &str) -> String {
        fl!(self.loader, "pedigree-unknown-father-of", name = name)
    }

    /// The "mother of {name}" placeholder hint for an unresearched ancestor slot whose descendant is
    /// known.
    #[must_use]
    pub fn pedigree_unknown_mother_of(&self, name: &str) -> String {
        fl!(self.loader, "pedigree-unknown-mother-of", name = name)
    }

    /// The generic "father (line unresearched)" hint, once the branch above is itself unknown.
    #[must_use]
    pub fn pedigree_father_unresearched(&self) -> String {
        fl!(self.loader, "pedigree-father-unresearched")
    }

    /// The generic "mother (line unresearched)" hint, once the branch above is itself unknown.
    #[must_use]
    pub fn pedigree_mother_unresearched(&self) -> String {
        fl!(self.loader, "pedigree-mother-unresearched")
    }

    /// The "Focus: {name} · {n} generations" caption above the pedigree/descendant chart.
    #[must_use]
    pub fn pedigree_focus(&self, name: &str, generations: usize) -> String {
        fl!(
            self.loader,
            "pedigree-focus",
            name = name,
            generations = u64::try_from(generations).unwrap_or(u64::MAX)
        )
    }

    /// The "No known relationship found" result when the calculator finds none.
    #[must_use]
    pub fn kinship_not_found(&self) -> String {
        fl!(self.loader, "kinship-not-found")
    }

    /// The localized sentence describing the kinship the calculator found between two named people.
    #[must_use]
    pub fn kinship_summary(&self, name_a: &str, name_b: &str, kinship: &Kinship) -> String {
        match kinship {
            Kinship::Same => fl!(self.loader, "kinship-same", a = name_a),
            Kinship::Ancestor { generations } => {
                let term = self.ancestor_term(*generations);
                fl!(self.loader, "kinship-a-is-b-term", a = name_a, b = name_b, term = term)
            }
            Kinship::Descendant { generations } => {
                let term = self.descendant_term(*generations);
                fl!(self.loader, "kinship-a-is-b-term", a = name_a, b = name_b, term = term)
            }
            Kinship::Sibling { full } => {
                let term = if *full {
                    fl!(self.loader, "kinship-full-sibling")
                } else {
                    fl!(self.loader, "kinship-half-sibling")
                };
                fl!(self.loader, "kinship-a-and-b-are", a = name_a, b = name_b, term = term)
            }
            Kinship::CommonAncestor { up_a, up_b, .. } => {
                let term = self.cousin_or_aunt_term(*up_a, *up_b);
                fl!(self.loader, "kinship-a-is-b-term", a = name_a, b = name_b, term = term)
            }
        }
    }

    /// The direct-ancestor term for `generations` generations up (1 = parent, 2 = grandparent, 3 =
    /// great-grandparent, further as "N× great-grandparent").
    fn ancestor_term(&self, generations: u32) -> String {
        match generations {
            1 => fl!(self.loader, "kinship-parent"),
            2 => fl!(self.loader, "kinship-grandparent"),
            3 => fl!(self.loader, "kinship-great-grandparent"),
            n => {
                let n: u32 = n - 2;
                fl!(self.loader, "kinship-great-n-grandparent", n = n)
            }
        }
    }

    /// The direct-descendant term for `generations` generations down — the mirror of
    /// [`Self::ancestor_term`].
    fn descendant_term(&self, generations: u32) -> String {
        match generations {
            1 => fl!(self.loader, "kinship-child"),
            2 => fl!(self.loader, "kinship-grandchild"),
            3 => fl!(self.loader, "kinship-great-grandchild"),
            n => {
                let n: u32 = n - 2;
                fl!(self.loader, "kinship-great-n-grandchild", n = n)
            }
        }
    }

    /// The cousin/aunt-or-uncle term for a nearest common ancestor `up_a`/`up_b` generations from
    /// each person. `up_a == up_b` (siblings) is handled by the caller before reaching here.
    fn cousin_or_aunt_term(&self, up_a: u32, up_b: u32) -> String {
        let degree = up_a.min(up_b) - 1;
        let removed = up_a.abs_diff(up_b);
        if degree > 0 {
            let cousins = self.cousin_degree_label(degree);
            return match self.removed_label(removed) {
                Some(removed) => fl!(
                    self.loader,
                    "kinship-cousins-removed",
                    cousins = cousins,
                    removed = removed
                ),
                None => cousins,
            };
        }
        // `up_a`/`up_b` cannot be equal here (that is the sibling case above), so exactly one side
        // is closer to the common ancestor — that side is the elder relative to the other. `removed`
        // (>= 1) counts how many "great"s: 1 = aunt/uncle, 2 = great-aunt/uncle, 3+ = "N× great-…".
        match (up_a < up_b, removed) {
            (true, 1) => fl!(self.loader, "kinship-aunt-uncle"),
            (true, 2) => fl!(self.loader, "kinship-great-aunt-uncle"),
            (true, n) => {
                let n: u32 = n - 2;
                fl!(self.loader, "kinship-great-n-aunt-uncle", n = n)
            }
            (false, 1) => fl!(self.loader, "kinship-niece-nephew"),
            (false, 2) => fl!(self.loader, "kinship-great-niece-nephew"),
            (false, n) => {
                let n: u32 = n - 2;
                fl!(self.loader, "kinship-great-n-niece-nephew", n = n)
            }
        }
    }

    /// The "Nth cousins" label for a cousin degree (1 = first cousins, 2 = second, …).
    fn cousin_degree_label(&self, degree: u32) -> String {
        match degree {
            1 => fl!(self.loader, "cousin-first"),
            2 => fl!(self.loader, "cousin-second"),
            3 => fl!(self.loader, "cousin-third"),
            n => fl!(self.loader, "cousin-nth", n = n),
        }
    }

    /// The "once/twice/N× removed" suffix for a cousin generation gap, or `None` when there is none.
    fn removed_label(&self, removed: u32) -> Option<String> {
        match removed {
            0 => None,
            1 => Some(fl!(self.loader, "removed-once")),
            2 => Some(fl!(self.loader, "removed-twice")),
            n => Some(fl!(self.loader, "removed-n-times", n = n)),
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
    use genealogy_app::{AppError, ChangeLogEntry, Confidence, DbError, OperatorKind, Sex};

    /// Every event variant's `type_name()` across the 12 aggregates (genealogy-core `*/event.rs`).
    /// Keep in sync when a new event variant lands — an unmapped type renders as "Recorded a change".
    const EVENT_TYPES: &[&str] = &[
        "PersonCreated",
        "NameAsserted",
        "SexAsserted",
        "FactAsserted",
        "ParticipationAsserted",
        "AssociationAsserted",
        "MediaAttached",
        "NoteAttached",
        "CitationAdded",
        "ExternalIdAdded",
        "Tagged",
        "Untagged",
        "RestrictionsChanged",
        "AssertionRetracted",
        "AssertionSuperseded",
        "PersonsMerged",
        "CitationCreated",
        "PageSet",
        "DateAsserted",
        "ConfidenceSet",
        "EvidenceAnalysisSet",
        "AttributeAdded",
        "DnaMatchObserved",
        "SegmentAdded",
        "SharedAncestorAsserted",
        "MatchConfirmed",
        "MatchRejected",
        "DnaTestCreated",
        "ProviderSet",
        "KitIdSet",
        "TestTypeSet",
        "GenomeBuildSet",
        "HaplogroupAsserted",
        "FamilyCreated",
        "PartnerAdded",
        "PartnerRemoved",
        "ChildAdded",
        "ChildRemoved",
        "FamilyEventLinked",
        "MediaCreated",
        "PathSet",
        "ChecksumSet",
        "MimeSet",
        "NoteCreated",
        "NoteTypeSet",
        "RichTextSet",
        "PlaceCreated",
        "PlaceTypeSet",
        "EnclosedByAsserted",
        "CoordinatesAsserted",
        "CodeSet",
        "RepositoryCreated",
        "RepositoryTypeSet",
        "NameSet",
        "AddressAdded",
        "UrlAdded",
        "SourceCreated",
        "TitleSet",
        "AuthorSet",
        "PubInfoSet",
        "AbbrevSet",
        "RepositoryLinked",
        "TagCreated",
        "TagRenamed",
        "TagColorSet",
        "TagPrioritySet",
        "EventCreated",
        "EventTypeSet",
        "DescriptionSet",
        "PlaceLinked",
        "ParticipantRoleAdded",
        "ParticipantRoleRemoved",
    ];

    fn typed_entry(event_type: &str) -> ChangeLogEntry {
        ChangeLogEntry {
            aggregate_kind: "person".to_owned(),
            aggregate_human_id: None,
            assertion_id: String::new(),
            sequence: 1,
            event_type: event_type.to_owned(),
            occurred_at: String::new(),
            operator_display: None,
            operator_kind: OperatorKind::Human,
            confidence: Confidence::Normal,
            rationale: None,
            detail: None,
            can_undo: false,
        }
    }

    #[test]
    fn change_summary_covers_every_event_type() {
        let loc = Localizer::for_test("en");
        let generic = loc.change_summary(&typed_entry("NoSuchEventType"));
        for event_type in EVENT_TYPES {
            let summary = loc.change_summary(&typed_entry(event_type));
            assert_ne!(summary, generic, "event type {event_type} has no specific phrase");
        }
    }

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

    #[test]
    fn kinship_summary_covers_direct_lines_cousins_and_aunts() {
        use genealogy_app::Kinship;

        let loc = Localizer::for_test("en");
        assert_eq!(
            loc.kinship_summary("Alice", "Bob", &Kinship::Ancestor { generations: 2 }),
            "Alice is Bob’s grandparent."
        );
        assert_eq!(
            loc.kinship_summary("Alice", "Bob", &Kinship::Ancestor { generations: 4 }),
            "Alice is Bob’s 2× great-grandparent."
        );
        assert_eq!(
            loc.kinship_summary("Alice", "Bob", &Kinship::Descendant { generations: 1 }),
            "Alice is Bob’s child."
        );
        assert_eq!(
            loc.kinship_summary("Alice", "Bob", &Kinship::Sibling { full: false }),
            "Alice and Bob are half siblings."
        );
        // A common ancestor 2 generations from each side: first cousins.
        assert_eq!(
            loc.kinship_summary(
                "Alice",
                "Bob",
                &Kinship::CommonAncestor {
                    common_ancestor: person_ref("I0099", "Great Gran"),
                    up_a: 2,
                    up_b: 2,
                }
            ),
            "Alice is Bob’s first cousins."
        );
        // A common ancestor 1 generation from Alice and 2 from Bob: Alice is Bob's aunt/uncle.
        assert_eq!(
            loc.kinship_summary(
                "Alice",
                "Bob",
                &Kinship::CommonAncestor {
                    common_ancestor: person_ref("I0099", "Great Gran"),
                    up_a: 1,
                    up_b: 2,
                }
            ),
            "Alice is Bob’s aunt/uncle."
        );
        // Reversed and one generation further: Alice is Bob's great-niece/nephew.
        assert_eq!(
            loc.kinship_summary(
                "Alice",
                "Bob",
                &Kinship::CommonAncestor {
                    common_ancestor: person_ref("I0099", "Great Gran"),
                    up_a: 3,
                    up_b: 1,
                }
            ),
            "Alice is Bob’s great-niece/nephew."
        );
        assert_eq!(
            loc.kinship_summary("Alice", "Alice", &Kinship::Same),
            "Alice — the same person."
        );
        assert_eq!(
            loc.kinship_not_found(),
            "No known relationship found within the searched generations."
        );
    }

    fn person_ref(human_id: &str, name: &str) -> genealogy_app::PedigreePersonRef {
        genealogy_app::PedigreePersonRef {
            human_id: human_id.to_owned(),
            id: format!("{human_id}-id"),
            name: Some(name.to_owned()),
            vitals: None,
            restrictions: std::collections::BTreeSet::new(),
        }
    }
}
