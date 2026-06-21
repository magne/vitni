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

pub mod citation;
pub mod config;
pub mod error;
pub mod event;
pub mod family;
pub mod person;
pub mod place;
pub mod session;
pub mod source;
mod use_case;
pub mod workspace;

pub use citation::{
    CitationSummary, NewCitation, add_citation_attribute, assert_citation_date, attach_citation_media,
    attach_citation_note, create_citation, list_citations, set_citation_confidence, set_citation_evidence_analysis,
    set_page, show_citation, tag_citation,
};
pub use config::{AppDefaults, Config, Engine, IdFormats, OperatorConfig, WorkspaceDefaults, WorkspaceEntry};
pub use error::AppError;
pub use event::{
    DateParts, EventSummary, NewEvent, assert_event_date, create_event, link_place, list_events, set_event_type,
    show_event,
};
pub use family::{
    FamilySummary, add_child, add_partner, create_family, list_families, remove_child, remove_partner, show_family,
};
pub use genealogy_core::citation::CitationError;
pub use genealogy_core::enums::{ChildParentRelationship, EventType, ParticipantRole, PlaceType, Sex};
pub use genealogy_core::event::EventError;
pub use genealogy_core::family::FamilyError;
pub use genealogy_core::person::PersonError;
pub use genealogy_core::place::PlaceError;
pub use genealogy_core::provenance::{Confidence, EvidenceAnalysis, EvidenceKind, InformationKind, SourceQuality};
pub use genealogy_core::source::SourceError;
pub use genealogy_db::DbError;
pub use person::{NewPerson, PersonSummary, add_name, assert_participation, create_person, list_persons, show_person};
pub use place::{
    NewPlace, PlaceSummary, add_place_citation, add_place_name, assert_place_coordinates, assert_place_enclosed_by,
    attach_place_media, attach_place_note, create_place, list_places, set_place_code, set_place_type, show_place,
    tag_place,
};
pub use session::Session;
pub use source::{NewSource, SourceSummary, create_source, list_sources, set_title, show_source};
pub use use_case::Provenance;
pub use workspace::{OperatorRecord, Workspace, WorkspaceManifest};
