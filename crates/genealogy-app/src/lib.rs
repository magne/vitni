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
pub mod citation;
pub mod config;
pub mod dna_match;
pub mod dna_test;
pub mod error;
pub mod event;
pub mod family;
pub mod media;
pub mod note;
pub mod person;
pub mod place;
pub mod repository;
pub mod session;
pub mod source;
pub mod tag;
mod use_case;
pub mod workspace;

pub use citation::{
    CitationSummary, NewCitation, add_citation_attribute, assert_citation_date, attach_citation_media,
    attach_citation_note, create_citation, list_citations, set_citation_confidence, set_citation_evidence_analysis,
    set_page, show_citation, tag_citation,
};
pub use config::{AppDefaults, Config, Engine, IdFormats, OperatorConfig, WorkspaceDefaults, WorkspaceEntry};
pub use dna_match::{
    DnaMatchSummary, NewDnaMatch, add_dna_match_segment, assert_dna_match_shared_ancestor, attach_dna_match_note,
    list_dna_matches, observe_dna_match, set_dna_match_status, show_dna_match, tag_dna_match,
};
pub use dna_test::{
    DnaTestSummary, NewDnaTest, assert_dna_test_haplogroup, attach_dna_test_note, create_dna_test, list_dna_tests,
    set_dna_test_genome_build, set_dna_test_kit_id, set_dna_test_provider, set_dna_test_type, show_dna_test,
    tag_dna_test,
};
pub use error::AppError;
pub use event::{
    DateParts, EventSummary, NewEvent, add_event_citation, assert_event_date, attach_event_media, attach_event_note,
    create_event, link_place, list_events, set_event_description, set_event_type, set_participant_role, show_event,
    tag_event,
};
pub use family::{
    FamilySummary, add_child, add_partner, create_family, list_families, remove_child, remove_partner, show_family,
};
pub use genealogy_core::address::Address;
pub use genealogy_core::citation::CitationError;
pub use genealogy_core::dna::{
    Centimorgans, ChromosomeSide, DnaGenomeBuild, DnaProvider, DnaSegment, DnaTestType, PercentShared, SharedAncestor,
};
pub use genealogy_core::dna_match::{DnaMatchError, MatchStatus};
pub use genealogy_core::dna_test::DnaTestError;
pub use genealogy_core::enums::{
    ChildParentRelationship, EventType, NoteType, ParticipantRole, PlaceType, RepositoryType, Sex, SourceMediaType,
};
pub use genealogy_core::event::EventError;
pub use genealogy_core::family::FamilyError;
pub use genealogy_core::media::MediaError;
pub use genealogy_core::note::NoteError;
pub use genealogy_core::person::PersonError;
pub use genealogy_core::place::PlaceError;
pub use genealogy_core::provenance::{Confidence, EvidenceAnalysis, EvidenceKind, InformationKind, SourceQuality};
pub use genealogy_core::repository::RepositoryError;
pub use genealogy_core::source::SourceError;
pub use genealogy_core::tag::TagError;
pub use genealogy_core::text::Url;
pub use genealogy_db::DbError;
pub use media::{
    MediaSummary, NewMedia, add_media_attribute, add_media_citation, assert_media_date, attach_media_note,
    create_media, list_media, set_media_checksum, set_media_file_path, set_media_web_path, show_media, tag_media,
};
pub use note::{NewNote, NoteSummary, create_note, list_notes, set_note_text, set_note_type, show_note, tag_note};
pub use person::{NewPerson, PersonSummary, add_name, assert_participation, create_person, list_persons, show_person};
pub use place::{
    NewPlace, PlaceSummary, add_place_citation, add_place_name, assert_place_coordinates, assert_place_enclosed_by,
    attach_place_media, attach_place_note, create_place, list_places, set_place_code, set_place_type, show_place,
    tag_place,
};
pub use repository::{
    NewRepository, RepositorySummary, add_repository_address, add_repository_url, attach_repository_note,
    create_repository, list_repositories, set_repository_name, set_repository_type, show_repository, tag_repository,
};
pub use session::Session;
pub use source::{
    NewSource, SourceSummary, add_source_attribute, attach_source_media, attach_source_note, create_source,
    link_source_repository, list_sources, set_source_abbrev, set_source_author, set_source_pub_info, set_title,
    show_source, tag_source,
};
pub use tag::{TagSummary, create_tag, list_tags, rename_tag, set_tag_color, set_tag_priority, show_tag};
pub use use_case::Provenance;
pub use workspace::{OperatorRecord, Workspace, WorkspaceManifest};
