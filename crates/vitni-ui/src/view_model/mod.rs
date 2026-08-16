//! View-models: framework-neutral, render-ready shapes derived from `vitni-app` DTOs.
//!
//! A view-model carries already-localized display strings (built via the [`Localizer`]) so a
//! renderer stays dumb — it lays out fields, it does not format or localize. The structured parts a
//! renderer might branch on (e.g. `private`) stay typed. A list row is the generic [`RowVm`]; the
//! detail tab strip is [`DetailTab`]s.

use std::collections::HashMap;

use vitni_app::{
    AssociationSummary, ChangeLogEntry, ChildParentRelationship, CitationSummary, EventRow, EventType,
    EvidenceAnalysis, EvidenceKind, EvidenceLevel, FactSummary, FamilyForPerson, FamilyRow, FamilySummary,
    GenealogicalDate, InformationKind, MutationMeta, NameSummary, NameType, OperatorKind, PersonFamilyRole, PersonName,
    PersonNameParts, PersonRow, PersonSummary, Provenance, Sex, SourceQuality, TagRef, WorkspaceCounts,
};

use crate::action::ActionLabel;
use crate::detail::DetailTab;
use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{
    Category, CitationChangeSetRequest, CitationEdit, CitationSourceRequest, DnaMatchChangeSetRequest, DnaMatchEdit,
    DnaTestChangeSetRequest, DnaTestEdit, DraftCitationRef, DraftNewCitation, DraftNewSource, DraftSourceRef,
    EventChangeSetRequest, EventEdit, EventPlaceRequest, FamilyChangeSetRequest, FamilyEdit, MediaChangeSetRequest,
    MediaEdit, NoteChangeSetRequest, NoteEdit, PartnerRequest, PersonChangeSetRequest, PlaceChangeSetRequest,
    PlaceEdit, RecordRef, RepositoryChangeSetRequest, RepositoryEdit, ResearchNoteChangeSetRequest, ResearchNoteEdit,
    SourceChangeSetRequest, SourceEdit, SubjectRequest, TagChangeSetRequest,
};
use crate::presentation::{ConfidenceLevel, EvidenceAxis, RestrictionKind};

mod bulk_import;
mod citation;
mod common;
mod crop;
mod dashboard;
mod date_draft;
mod dna_match;
mod dna_test;
mod event;
mod export;
mod family;
mod geography;
mod history;
mod import;
mod media;
mod media_save;
mod merge;
mod note;
mod pedigree;
mod person;
mod place;
mod plugin;
mod provenance;
mod record_draft;
mod record_link;
mod repository;
mod research_note;
mod shortcuts_vm;
mod source;
mod tag;

pub use bulk_import::*;
pub use citation::*;
pub use common::*;
pub use crop::*;
pub use dashboard::*;
pub use date_draft::*;
pub use dna_match::*;
pub use dna_test::*;
pub use event::*;
pub use export::*;
pub use family::*;
pub use geography::*;
pub use history::*;
pub use import::*;
pub use media::*;
pub use media_save::*;
pub use merge::*;
pub use note::*;
pub use pedigree::*;
pub use person::*;
pub use place::*;
pub use plugin::*;
pub use provenance::*;
pub use record_draft::*;
pub use record_link::*;
pub use repository::*;
pub use research_note::*;
pub use shortcuts_vm::*;
pub use source::*;
pub use tag::*;

#[cfg(test)]
mod merge_tests;
#[cfg(test)]
mod pedigree_tests;
#[cfg(test)]
mod tests;
