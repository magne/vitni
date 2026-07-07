//! View-models: framework-neutral, render-ready shapes derived from `genealogy-app` DTOs.
//!
//! A view-model carries already-localized display strings (built via the [`Localizer`]) so a
//! renderer stays dumb — it lays out fields, it does not format or localize. The structured parts a
//! renderer might branch on (e.g. `private`) stay typed. A list row is the generic [`RowVm`]; the
//! detail tab strip is [`DetailTab`]s.

use std::collections::HashMap;

use genealogy_app::{
    AssociationSummary, ChangeLogEntry, ChildParentRelationship, CitationSummary, EventType, EvidenceAnalysis,
    EvidenceKind, EvidenceLevel, FactSummary, FactType, FamilyForPerson, FamilySummary, InformationKind, MutationMeta,
    NameSummary, NameType, OperatorKind, PersonFamilyRole, PersonName, PersonNameParts, PersonSummary, Provenance, Sex,
    SourceQuality, TagRef, WorkspaceCounts,
};

use crate::detail::DetailTab;
use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::navigation::{
    Category, CitationChangeSetRequest, CitationEdit, CitationSourceRequest, DnaMatchChangeSetRequest, DnaMatchEdit,
    DnaTestChangeSetRequest, DnaTestEdit, DraftCitationRef, DraftNewCitation, DraftNewSource, DraftSourceRef,
    EventChangeSetRequest, EventEdit, EventPlaceRequest, FamilyChangeSetRequest, FamilyEdit, MediaChangeSetRequest,
    MediaEdit, NoteChangeSetRequest, NoteEdit, PartnerRequest, PersonChangeSetRequest, PlaceChangeSetRequest,
    PlaceEdit, RecordRef, RepositoryChangeSetRequest, RepositoryEdit, SourceChangeSetRequest, SourceEdit,
    TagChangeSetRequest,
};
use crate::presentation::{ConfidenceLevel, EvidenceAxis, RestrictionKind};

mod citation;
mod common;
mod dashboard;
mod dna_match;
mod dna_test;
mod event;
mod family;
mod history;
mod media;
mod merge;
mod note;
mod pedigree;
mod person;
mod place;
mod provenance;
mod record_draft;
mod record_link;
mod repository;
mod source;
mod tag;

pub use citation::*;
pub use common::*;
pub use dashboard::*;
pub use dna_match::*;
pub use dna_test::*;
pub use event::*;
pub use family::*;
pub use history::*;
pub use media::*;
pub use merge::*;
pub use note::*;
pub use pedigree::*;
pub use person::*;
pub use place::*;
pub use provenance::*;
pub use record_draft::*;
pub use record_link::*;
pub use repository::*;
pub use source::*;
pub use tag::*;

#[cfg(test)]
mod merge_tests;
#[cfg(test)]
mod pedigree_tests;
#[cfg(test)]
mod tests;
