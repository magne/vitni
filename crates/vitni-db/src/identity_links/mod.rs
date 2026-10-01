//! The identity cluster index (ADR 0039 §4).
//!
//! A merge decision (`PersonsMerged`, `EventsMerged`, `PlacesMerged`, …) is recorded once, on the
//! survivor's own stream, so the merged
//! record's projection never learns it was merged, and a chain of merges (C into B, then B into A) is
//! spread over several streams. Asking "which cluster is this record in, and what is its root?" needs
//! a cross-aggregate read no single projection can answer — the derived, rebuildable index this module
//! maintains (ADR 0010).
//!
//! `identity_edges` holds one row per live merge edge `(surviving, member)`, mirrored per survivor
//! from its projection. `identity_links` holds the transitive closure: one row per merged record,
//! naming its cluster's root — the survivor that is not itself merged. Both are keyed by the
//! [`MatchableKind`] so every matchable kind shares the tables.
//!
//! The app layer refuses cycles and double membership before it writes (ADR 0039 §4), but the index
//! stays well-defined without that guarantee: [`closure`] picks the lowest survivor id when a record is
//! merged into two clusters, and the lowest id on a cycle as the cycle's root.

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use std::collections::{BTreeMap, BTreeSet};

use cqrs_es::Aggregate;
use serde::de::DeserializeOwned;
use vitni_core::citation::{CitationEvent, CitationEventBody, CitationState, CitationView};
use vitni_core::event::{EventEventBody, EventState, EventView};
use vitni_core::family::{FamilyEventBody, FamilyState, FamilyView};
use vitni_core::identity::ClusterRecord;
use vitni_core::media::{MediaEvent, MediaEventBody, MediaState, MediaView};
use vitni_core::note::{NoteEvent, NoteEventBody, NoteState, NoteView};
use vitni_core::person::event::{PersonEvent, PersonEventBody};
use vitni_core::person::{PersonState, PersonView};
use vitni_core::place::{PlaceEvent, PlaceEventBody, PlaceState, PlaceView};
use vitni_core::repository::{RepositoryEvent, RepositoryEventBody, RepositoryState, RepositoryView};
use vitni_core::source::{SourceEvent, SourceEventBody, SourceState, SourceView};

use crate::tables::{
    CITATION_VIEW_TABLE, EVENT_VIEW_TABLE, FAMILY_VIEW_TABLE, MEDIA_VIEW_TABLE, NOTE_VIEW_TABLE, PERSON_VIEW_TABLE,
    PLACE_VIEW_TABLE, REPOSITORY_VIEW_TABLE, SOURCE_VIEW_TABLE,
};

/// The live merge edges: one row per `(kind, surviving, member)`.
const IDENTITY_EDGES_TABLE: &str = "identity_edges";
/// The transitive closure: one row per `(kind, member)`, naming the member's root.
const IDENTITY_LINKS_TABLE: &str = "identity_links";

/// A projection the index mirrors merge edges from: its aggregate, its table, and which of its events
/// can change the survivor's live edges — a merge adds one, and a retraction or supersession may remove
/// one.
pub(crate) trait IndexedRecord: ClusterRecord + DeserializeOwned + Send + Sync + 'static {
    /// The aggregate the projection folds.
    type State: Aggregate;

    /// The projection table the view is read from.
    const VIEW_TABLE: &'static str;

    /// Whether `event` can change the survivor's live merge edges.
    fn changes_edges(event: &<Self::State as Aggregate>::Event) -> bool;
}

impl IndexedRecord for PersonView {
    type State = PersonState;

    const VIEW_TABLE: &'static str = PERSON_VIEW_TABLE;

    fn changes_edges(event: &PersonEvent) -> bool {
        match &event.body {
            PersonEventBody::PersonsMerged { .. }
            | PersonEventBody::AssertionRetracted { .. }
            | PersonEventBody::AssertionSuperseded { .. } => true,
            PersonEventBody::PersonCreated { .. }
            | PersonEventBody::NameAsserted { .. }
            | PersonEventBody::SexAsserted { .. }
            | PersonEventBody::FactAsserted { .. }
            | PersonEventBody::ParticipationAsserted { .. }
            | PersonEventBody::AssociationAsserted { .. }
            | PersonEventBody::MediaAttached { .. }
            | PersonEventBody::NoteAttached { .. }
            | PersonEventBody::CitationAdded { .. }
            | PersonEventBody::ExternalIdAdded { .. }
            | PersonEventBody::Tagged { .. }
            | PersonEventBody::Untagged { .. }
            | PersonEventBody::RestrictionsChanged { .. }
            | PersonEventBody::HumanIdChanged { .. }
            | PersonEventBody::PersonsDistinguished { .. } => false,
        }
    }
}

impl IndexedRecord for EventView {
    type State = EventState;

    const VIEW_TABLE: &'static str = EVENT_VIEW_TABLE;

    fn changes_edges(event: &vitni_core::event::EventEvent) -> bool {
        match &event.body {
            EventEventBody::EventsMerged { .. }
            | EventEventBody::AssertionRetracted { .. }
            | EventEventBody::AssertionSuperseded { .. } => true,
            EventEventBody::EventCreated { .. }
            | EventEventBody::EventTypeSet { .. }
            | EventEventBody::DateAsserted { .. }
            | EventEventBody::DescriptionSet { .. }
            | EventEventBody::PlaceLinked { .. }
            | EventEventBody::AddressAdded { .. }
            | EventEventBody::CitationAdded { .. }
            | EventEventBody::MediaAttached { .. }
            | EventEventBody::NoteAttached { .. }
            | EventEventBody::Tagged { .. }
            | EventEventBody::Untagged { .. }
            | EventEventBody::RestrictionsChanged { .. }
            | EventEventBody::HumanIdChanged { .. }
            | EventEventBody::EventsDistinguished { .. } => false,
        }
    }
}

impl IndexedRecord for FamilyView {
    type State = FamilyState;

    const VIEW_TABLE: &'static str = FAMILY_VIEW_TABLE;

    fn changes_edges(event: &vitni_core::family::FamilyEvent) -> bool {
        match &event.body {
            FamilyEventBody::FamiliesMerged { .. }
            | FamilyEventBody::AssertionRetracted { .. }
            | FamilyEventBody::AssertionSuperseded { .. } => true,
            FamilyEventBody::FamilyCreated { .. }
            | FamilyEventBody::PartnerAdded { .. }
            | FamilyEventBody::PartnerRemoved { .. }
            | FamilyEventBody::ChildAdded { .. }
            | FamilyEventBody::ChildRelationshipAsserted { .. }
            | FamilyEventBody::ChildRemoved { .. }
            | FamilyEventBody::RestrictionsChanged { .. }
            | FamilyEventBody::CitationAdded { .. }
            | FamilyEventBody::FamilyEventLinked { .. }
            | FamilyEventBody::MediaAttached { .. }
            | FamilyEventBody::NoteAttached { .. }
            | FamilyEventBody::Tagged { .. }
            | FamilyEventBody::Untagged { .. }
            | FamilyEventBody::ExternalIdAdded { .. }
            | FamilyEventBody::HumanIdChanged { .. }
            | FamilyEventBody::FamiliesDistinguished { .. } => false,
        }
    }
}

impl IndexedRecord for PlaceView {
    type State = PlaceState;

    const VIEW_TABLE: &'static str = PLACE_VIEW_TABLE;

    fn changes_edges(event: &PlaceEvent) -> bool {
        match &event.body {
            PlaceEventBody::PlacesMerged { .. }
            | PlaceEventBody::AssertionRetracted { .. }
            | PlaceEventBody::AssertionSuperseded { .. } => true,
            PlaceEventBody::PlaceCreated { .. }
            | PlaceEventBody::PlaceTypeSet { .. }
            | PlaceEventBody::NameAsserted { .. }
            | PlaceEventBody::EnclosedByAsserted { .. }
            | PlaceEventBody::CoordinatesAsserted { .. }
            | PlaceEventBody::GeometryAsserted { .. }
            | PlaceEventBody::SuccessionAsserted { .. }
            | PlaceEventBody::CodeSet { .. }
            | PlaceEventBody::CitationAdded { .. }
            | PlaceEventBody::MediaAttached { .. }
            | PlaceEventBody::NoteAttached { .. }
            | PlaceEventBody::Tagged { .. }
            | PlaceEventBody::Untagged { .. }
            | PlaceEventBody::RestrictionsChanged { .. }
            | PlaceEventBody::HumanIdChanged { .. }
            | PlaceEventBody::PlacesDistinguished { .. } => false,
        }
    }
}

impl IndexedRecord for SourceView {
    type State = SourceState;

    const VIEW_TABLE: &'static str = SOURCE_VIEW_TABLE;

    fn changes_edges(event: &SourceEvent) -> bool {
        match &event.body {
            SourceEventBody::SourcesMerged { .. }
            | SourceEventBody::AssertionRetracted { .. }
            | SourceEventBody::AssertionSuperseded { .. } => true,
            SourceEventBody::SourceCreated { .. }
            | SourceEventBody::TitleSet { .. }
            | SourceEventBody::AuthorSet { .. }
            | SourceEventBody::PubInfoSet { .. }
            | SourceEventBody::AbbrevSet { .. }
            | SourceEventBody::RepositoryLinked { .. }
            | SourceEventBody::AttributeAdded { .. }
            | SourceEventBody::MediaAttached { .. }
            | SourceEventBody::NoteAttached { .. }
            | SourceEventBody::Tagged { .. }
            | SourceEventBody::Untagged { .. }
            | SourceEventBody::RestrictionsChanged { .. }
            | SourceEventBody::HumanIdChanged { .. }
            | SourceEventBody::SourcesDistinguished { .. } => false,
        }
    }
}

impl IndexedRecord for CitationView {
    type State = CitationState;

    const VIEW_TABLE: &'static str = CITATION_VIEW_TABLE;

    fn changes_edges(event: &CitationEvent) -> bool {
        match &event.body {
            CitationEventBody::CitationsMerged { .. }
            | CitationEventBody::AssertionRetracted { .. }
            | CitationEventBody::AssertionSuperseded { .. } => true,
            CitationEventBody::CitationCreated { .. }
            | CitationEventBody::PageSet { .. }
            | CitationEventBody::DateAsserted { .. }
            | CitationEventBody::ConfidenceSet { .. }
            | CitationEventBody::EvidenceAnalysisSet { .. }
            | CitationEventBody::AttributeAdded { .. }
            | CitationEventBody::MediaAttached { .. }
            | CitationEventBody::NoteAttached { .. }
            | CitationEventBody::Tagged { .. }
            | CitationEventBody::Untagged { .. }
            | CitationEventBody::RestrictionsChanged { .. }
            | CitationEventBody::HumanIdChanged { .. }
            | CitationEventBody::CitationsDistinguished { .. } => false,
        }
    }
}

impl IndexedRecord for RepositoryView {
    type State = RepositoryState;

    const VIEW_TABLE: &'static str = REPOSITORY_VIEW_TABLE;

    fn changes_edges(event: &RepositoryEvent) -> bool {
        match &event.body {
            RepositoryEventBody::RepositoriesMerged { .. }
            | RepositoryEventBody::AssertionRetracted { .. }
            | RepositoryEventBody::AssertionSuperseded { .. } => true,
            RepositoryEventBody::RepositoryCreated { .. }
            | RepositoryEventBody::RepositoryTypeSet { .. }
            | RepositoryEventBody::NameSet { .. }
            | RepositoryEventBody::AddressAdded { .. }
            | RepositoryEventBody::UrlAdded { .. }
            | RepositoryEventBody::NoteAttached { .. }
            | RepositoryEventBody::Tagged { .. }
            | RepositoryEventBody::Untagged { .. }
            | RepositoryEventBody::RestrictionsChanged { .. }
            | RepositoryEventBody::HumanIdChanged { .. }
            | RepositoryEventBody::RepositoriesDistinguished { .. } => false,
        }
    }
}

impl IndexedRecord for NoteView {
    type State = NoteState;

    const VIEW_TABLE: &'static str = NOTE_VIEW_TABLE;

    fn changes_edges(event: &NoteEvent) -> bool {
        match &event.body {
            NoteEventBody::NotesMerged { .. }
            | NoteEventBody::AssertionRetracted { .. }
            | NoteEventBody::AssertionSuperseded { .. } => true,
            NoteEventBody::NoteCreated { .. }
            | NoteEventBody::NoteTypeSet { .. }
            | NoteEventBody::RichTextSet { .. }
            | NoteEventBody::Tagged { .. }
            | NoteEventBody::Untagged { .. }
            | NoteEventBody::RestrictionsChanged { .. }
            | NoteEventBody::HumanIdChanged { .. }
            | NoteEventBody::NotesDistinguished { .. } => false,
        }
    }
}

impl IndexedRecord for MediaView {
    type State = MediaState;

    const VIEW_TABLE: &'static str = MEDIA_VIEW_TABLE;

    fn changes_edges(event: &MediaEvent) -> bool {
        match &event.body {
            MediaEventBody::MediaMerged { .. }
            | MediaEventBody::AssertionRetracted { .. }
            | MediaEventBody::AssertionSuperseded { .. } => true,
            MediaEventBody::MediaCreated { .. }
            | MediaEventBody::PathSet { .. }
            | MediaEventBody::ChecksumSet { .. }
            | MediaEventBody::MimeSet { .. }
            | MediaEventBody::DateAsserted { .. }
            | MediaEventBody::AttributeAdded { .. }
            | MediaEventBody::CitationAdded { .. }
            | MediaEventBody::NoteAttached { .. }
            | MediaEventBody::Tagged { .. }
            | MediaEventBody::Untagged { .. }
            | MediaEventBody::RestrictionsChanged { .. }
            | MediaEventBody::HumanIdChanged { .. }
            | MediaEventBody::MediaDistinguished { .. } => false,
        }
    }
}

/// The transitive closure of `edges` (`(surviving, member)` pairs): every member paired with its
/// cluster's root, ordered by member.
///
/// A member with several survivors follows the lowest survivor id. A cycle has no record that is not
/// merged, so the lowest id on it stands in as the root and gets no row of its own.
fn closure(edges: &[(String, String)]) -> Vec<(String, String)> {
    let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
    for (surviving, member) in edges {
        if surviving == member {
            continue;
        }
        let entry = parent.entry(member.as_str()).or_insert(surviving.as_str());
        if surviving.as_str() < *entry {
            *entry = surviving.as_str();
        }
    }
    let mut links = Vec::with_capacity(parent.len());
    for member in parent.keys() {
        let root = root_of(&parent, member);
        if root != *member {
            links.push(((*member).to_owned(), root.to_owned()));
        }
    }
    links
}

/// Follows `parent` from `start` to the first record with no parent, or, on a cycle, to the lowest id
/// on that cycle.
fn root_of<'a>(parent: &BTreeMap<&'a str, &'a str>, start: &'a str) -> &'a str {
    let mut path: Vec<&str> = vec![start];
    let mut seen: BTreeSet<&str> = BTreeSet::from([start]);
    let mut current = start;
    while let Some(next) = parent.get(current).copied() {
        if !seen.insert(next) {
            let cycle_start = path.iter().position(|id| *id == next).unwrap_or(0);
            return path.iter().skip(cycle_start).copied().min().unwrap_or(next);
        }
        path.push(next);
        current = next;
    }
    current
}

#[cfg(test)]
mod tests {
    use super::closure;

    fn edges(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(s, m)| ((*s).to_owned(), (*m).to_owned())).collect()
    }

    fn links(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        edges(pairs)
    }

    #[test]
    fn no_edges_mean_no_links() {
        assert!(closure(&[]).is_empty());
    }

    #[test]
    fn one_merge_links_the_member_to_the_survivor() {
        assert_eq!(closure(&edges(&[("a", "b")])), links(&[("b", "a")]));
    }

    #[test]
    fn a_chain_links_every_member_to_the_last_survivor() {
        assert_eq!(
            closure(&edges(&[("b", "c"), ("a", "b")])),
            links(&[("b", "a"), ("c", "a")])
        );
    }

    #[test]
    fn a_member_of_two_clusters_follows_the_lowest_survivor() {
        assert_eq!(closure(&edges(&[("z", "m"), ("a", "m")])), links(&[("m", "a")]));
    }

    #[test]
    fn a_cycle_is_rooted_at_its_lowest_id() {
        assert_eq!(
            closure(&edges(&[("a", "b"), ("b", "c"), ("c", "a"), ("c", "d")])),
            links(&[("b", "a"), ("c", "a"), ("d", "a")])
        );
    }

    #[test]
    fn a_tail_into_a_cycle_shares_the_cycle_root() {
        assert_eq!(
            closure(&edges(&[("c", "b"), ("b", "c"), ("b", "x")])),
            links(&[("c", "b"), ("x", "b")])
        );
    }

    #[test]
    fn a_self_edge_is_ignored() {
        assert!(closure(&edges(&[("a", "a")])).is_empty());
    }
}
