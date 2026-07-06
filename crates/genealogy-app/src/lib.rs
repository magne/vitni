//! `genealogy-app` — the application coordination layer (ADR 0006).
//!
//! This crate sits between the pure domain core (`genealogy-core`) / engine-neutral event store
//! (`genealogy-db`) and the frontends (`genealogy-cli` today; a UI and web backend later). It owns
//! everything the frontends would otherwise duplicate:
//!
//! - **global configuration** — operator identity + workspace registry ([`config`], ADR 0005),
//! - **the impure inputs** — clock, UUID v7 ids, operator [`Agent`](genealogy_core::provenance::Agent)
//!   — isolated in [`Session`] (ADR 0004 §3),
//! - **workspace directories** — manifest, database, `exports/ backups/ media/` ([`workspace`]),
//! - **use-cases** returning frontend-neutral DTOs ([`person`]).
//!
//! The decision core stays pure and the database engine stays hidden in `genealogy-db`; this is the
//! only layer that reads a clock or generates an id.

mod aggregates;
pub mod change_set;
pub mod citation;
mod citation_usage;
pub mod config;
pub mod dna_match;
pub mod dna_test;
pub mod dto;
pub mod duplicates;
pub mod error;
pub mod event;
pub mod family;
pub mod history;
pub mod import;
pub mod media;
pub mod media_change_set;
mod media_usage;
mod merge_usage;
pub mod note;
pub mod note_change_set;
mod note_usage;
pub mod pedigree;
pub mod person;
pub mod person_change_set;
pub mod place;
pub mod repository;
pub mod repository_change_set;
pub mod session;
pub mod source;
pub mod source_change_set;
pub mod tag;
pub mod tag_change_set;
mod tag_usage;
mod use_case;
pub mod workspace;

pub use change_set::{CitationRefInput, NewCitationEntry, NewSourceEntry, PlaceholderRef, SourceRefInput};
pub use citation::{
    CitationSummary, NewCitation, TagRef, add_citation_attribute, assert_citation_date, attach_citation_media,
    attach_citation_note, create_citation, list_citations, set_citation_confidence, set_citation_evidence_analysis,
    set_page, set_restrictions as set_citation_restrictions, show_citation, tag_citation,
};
pub use config::{
    AppDefaults, Config, DateFormat, Engine, IdFormats, LocaleDefaults, NumberFormat, OperatorConfig, ThemeMode,
    UiDefaults, WorkspaceDefaults, WorkspaceEntry, set_default_workspace, set_operator_identity,
    set_workspace_default_id_formats, set_workspace_default_locale,
};
pub use dna_match::{
    DnaMatchSummary, NewDnaMatch, SharedAncestorRef, add_dna_match_segment, assert_dna_match_shared_ancestor,
    attach_dna_match_note, import_attach_dna_match_note, list_dna_matches, observe_dna_match, set_dna_match_status,
    set_restrictions as set_dna_match_restrictions, show_dna_match, tag_dna_match,
};
pub use dna_test::{
    DnaTestMatchRef, DnaTestSummary, NewDnaTest, assert_dna_test_haplogroup, attach_dna_test_note, create_dna_test,
    import_attach_dna_test_note, list_dna_tests, set_dna_test_genome_build, set_dna_test_kit_id, set_dna_test_provider,
    set_dna_test_type, set_restrictions as set_dna_test_restrictions, show_dna_test, tag_dna_test,
};
pub use dto::{
    AggRef, CitationRef, CitingContext, CitingKind, CitingRecordRef, MediaRefSummary, RepositoryLinkRef,
    SourceCitationRef, SourceLinkRef, SourceReliability, UsingKind, UsingRecordRef,
};
pub use duplicates::{DuplicateCandidate, MatchKind, find_duplicate_candidates};
pub use error::AppError;
pub use event::{
    DateInput, DateParts, EventSummary, NewEvent, ParticipantRef, PlaceRefSummary, add_event_citation,
    assert_event_address, assert_event_date, assert_event_date_value, attach_event_media, attach_event_note,
    build_genealogical_date, create_event, import_attach_event_media, import_attach_event_note, link_place,
    list_events, set_event_description, set_event_type, set_participant_role,
    set_restrictions as set_event_restrictions, show_event, tag_event,
};
pub use family::{
    ChildRef, FamilyEventRef, FamilyForPerson, FamilySummary, PartnerRef, PersonFamilyRole, add_child,
    add_external_id as add_family_external_id, add_family_citation, add_partner, attach_family_media,
    attach_family_note, create_family, families_for_person, link_family_event, list_families, remove_child,
    remove_partner, set_restrictions as set_family_restrictions, show_family, tag_family,
};
pub use genealogy_core::address::Address;
pub use genealogy_core::citation::CitationError;
pub use genealogy_core::date::{
    Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody,
};
pub use genealogy_core::dna::{
    Centimorgans, ChromosomeSide, DnaGenomeBuild, DnaProvider, DnaSegment, DnaTestType, PercentShared, SharedAncestor,
};
pub use genealogy_core::dna_match::{DnaMatchError, MatchStatus};
pub use genealogy_core::dna_test::DnaTestError;
pub use genealogy_core::enums::{
    AssociationRole, ChildParentRelationship, EventType, EvidenceLevel, FactType, NoteType, ParticipantRole, PlaceType,
    RepositoryType, Restriction, Sex, SourceMediaType,
};
pub use genealogy_core::event::EventError;
pub use genealogy_core::fact::Fact;
pub use genealogy_core::family::FamilyError;
pub use genealogy_core::geo::{GeoCoordinates, Microdegrees};
pub use genealogy_core::ids::AgentId;
pub use genealogy_core::media::MediaError;
pub use genealogy_core::name::{NameType, PersonName, Surname};
pub use genealogy_core::note::NoteError;
pub use genealogy_core::person::PersonError;
pub use genealogy_core::place::PlaceError;
pub use genealogy_core::provenance::{
    Agent, AgentKind, Confidence, EvidenceAnalysis, EvidenceKind, InformationKind, SourceQuality,
};
pub use genealogy_core::repository::RepositoryError;
pub use genealogy_core::source::SourceError;
pub use genealogy_core::tag::TagError;
pub use genealogy_core::text::{ExternalId, Url};
pub use genealogy_db::DbError;
pub use history::{
    ActivityDetail, ChangeLogEntry, OperatorKind, WorkspaceCounts, change_log_for_citation, change_log_for_dna_match,
    change_log_for_dna_test, change_log_for_event, change_log_for_family, change_log_for_media, change_log_for_note,
    change_log_for_person, change_log_for_place, change_log_for_repository, change_log_for_source, change_log_for_tag,
    recent_activity, undo_assertion, undo_citation_assertion, undo_dna_match_assertion, undo_dna_test_assertion,
    undo_event_assertion, undo_family_assertion, undo_media_assertion, undo_note_assertion, undo_place_assertion,
    undo_repository_assertion, undo_source_assertion, workspace_counts,
};
pub use import::{import_add_child, import_add_partner, import_family, import_person};
pub use media::{
    MediaAttributeRef, MediaSummary, NewMedia, add_media_attribute, add_media_citation, assert_media_date,
    attach_media_note, create_media, import_attach_media_note, list_media, set_media_checksum, set_media_file_path,
    set_media_mime, set_media_web_path, set_restrictions as set_media_restrictions, show_media, tag_media,
};
pub use media_change_set::{MediaChangeSet, commit_media_change_set};
pub use note::{
    NewNote, NoteSummary, TranslationRef, add_note_translation, create_note, list_notes, set_note_text, set_note_type,
    set_restrictions as set_note_restrictions, show_note, tag_note,
};
pub use note_change_set::{NoteChangeSet, commit_note_change_set};
pub use pedigree::{
    AncestorNode, AncestorSlot, DescendantChart, DescendantNode, Kinship, PedigreeChart,
    PersonRef as PedigreePersonRef, RelationshipResult, ancestors, descendants, relationship,
};
pub use person::{
    AssociationSummary, FactSummary, MergeResult, NameSummary, NewFact, NewPerson, ParticipationRef, PersonNameParts,
    PersonSummary, add_name, add_person_citation, assert_association, assert_fact, assert_participation, assert_sex,
    attach_person_media, attach_person_note, create_person, list_persons, merge_persons, set_restrictions, show_person,
    tag_person,
};
pub use person_change_set::{PersonChangeSet, PersonTarget, commit_person_change_set};
pub use place::{
    NewPlace, PlaceEnclosingRef, PlaceNameRef, PlaceSummary, add_place_citation, add_place_name,
    assert_place_coordinates, assert_place_enclosed_by, attach_place_media, attach_place_note, create_place,
    import_attach_place_media, import_attach_place_note, list_places, set_place_code, set_place_type,
    set_restrictions as set_place_restrictions, show_place, tag_place,
};
pub use repository::{
    NewRepository, RepositorySummary, add_repository_address, add_repository_url, attach_repository_note,
    create_repository, import_attach_repository_note, list_repositories, set_repository_name, set_repository_type,
    set_restrictions as set_repository_restrictions, show_repository, tag_repository,
};
pub use repository_change_set::{RepositoryChangeSet, commit_repository_change_set};
pub use session::Session;
pub use source::{
    NewSource, SourceAttributeRef, SourceSummary, add_source_attribute, attach_source_media, attach_source_note,
    create_source, import_attach_source_media, import_attach_source_note, link_source_repository, list_sources,
    set_restrictions as set_source_restrictions, set_source_abbrev, set_source_author, set_source_pub_info, set_title,
    show_source, tag_source,
};
pub use source_change_set::{SourceChangeSet, commit_source_change_set};
pub use tag::{
    TagSummary, create_tag, list_tags, rename_tag, set_restrictions as set_tag_restrictions, set_tag_color,
    set_tag_priority, show_tag,
};
pub use tag_change_set::{TagChangeSet, TagTarget, commit_tag_change_set};
pub use tag_usage::TagUsageGroup;
pub use use_case::{MutationMeta, Provenance};
pub use workspace::{
    IdFormatLayers, LayerKind, LocaleOverrides, OperatorRecord, PluginPreferences, PreferenceLayers, RECENT_LIMIT,
    RecentItem, ResolvedLocale, ResolvedUiPreferences, ThemeLayers, UiPreferences, WindowGeometry, Workspace,
    WorkspaceManifest, person_id_format_layers, push_recent, read_plugin_preferences, read_preference_layers,
    read_resolved_locale, read_ui_preferences, save_locale_overrides, save_plugin_enabled, save_recent,
    save_theme_mode, save_window_geometry, theme_layers,
};
