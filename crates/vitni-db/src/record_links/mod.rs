//! The record links index (ADR 0047): the references one record's projection holds to another, kept
//! so they can be followed backwards.
//!
//! A family names its partners, children and events, a person the events they take part in, an event
//! its place, a place the places enclosing it, a source its repositories, a citation its source — each
//! on its own projection. Reading a person's families, an event's participants, a place's events or a
//! source's citations from those projections means reading every one of them. This derived, rebuildable
//! index (ADR 0010) holds one row per live reference, `(relation, source, target)`, mirrored per source
//! record from its projection, and is read by target.
//!
//! A reference a record drops can no longer be followed back from its target, so the target is marked
//! dirty for its match keys and pairs (ADR 0048) when it is dropped: its profile read the reference.

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use cqrs_es::Aggregate;
use serde::de::DeserializeOwned;
use vitni_core::citation::{CitationEvent, CitationEventBody, CitationState, CitationView};
use vitni_core::dna_match::{DnaMatchEvent, DnaMatchEventBody, DnaMatchState, DnaMatchView};
use vitni_core::dna_test::{DnaTestEvent, DnaTestEventBody, DnaTestState, DnaTestView};
use vitni_core::event::{EventEvent, EventEventBody, EventState, EventView};
use vitni_core::family::{FamilyEvent, FamilyEventBody, FamilyState, FamilyView};
use vitni_core::ids::TagId;
use vitni_core::matching::MatchableKind;
use vitni_core::media::{MediaEvent, MediaEventBody, MediaState, MediaView};
use vitni_core::note::{NoteEvent, NoteEventBody, NoteState, NoteView};
use vitni_core::person::event::{PersonEvent, PersonEventBody};
use vitni_core::person::{PersonState, PersonView};
use vitni_core::place::{PlaceEvent, PlaceEventBody, PlaceState, PlaceView};
use vitni_core::repository::{RepositoryEvent, RepositoryEventBody, RepositoryState, RepositoryView};
use vitni_core::research_note::{ResearchNoteEvent, ResearchNoteEventBody, ResearchNoteState, ResearchNoteView};
use vitni_core::source::{SourceEvent, SourceEventBody, SourceState, SourceView};

use crate::tables::{
    CITATION_VIEW_TABLE, DNA_MATCH_VIEW_TABLE, DNA_TEST_VIEW_TABLE, EVENT_VIEW_TABLE, FAMILY_VIEW_TABLE,
    MEDIA_VIEW_TABLE, NOTE_VIEW_TABLE, PERSON_VIEW_TABLE, PLACE_VIEW_TABLE, REPOSITORY_VIEW_TABLE,
    RESEARCH_NOTE_VIEW_TABLE, SOURCE_VIEW_TABLE,
};

/// The index table.
const RECORD_LINKS_TABLE: &str = "record_links";

/// A kind of reference one record holds to another, named by what the source record is to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecordLink {
    /// A family (source) has the person (target) as a partner.
    FamilyPartner,
    /// A family (source) has the person (target) as a child.
    FamilyChild,
    /// A family (source) is linked to the event (target), its marriage among them.
    FamilyEvent,
    /// A person (source) takes part in the event (target).
    Participation,
    /// An event (source) took place at the place (target).
    EventPlace,
    /// A place (source) lies within the place (target).
    PlaceEnclosure,
    /// A source (source) is held by the repository (target).
    SourceRepository,
    /// A citation (source) points into the source (target).
    CitationSource,
    /// A record of any tag-bearing kind (source) carries the tag (target) — ADR 0052 §6.
    Tagged,
    /// A DNA test (source) was taken by the person (target).
    DnaTestPerson,
    /// A DNA match (source) compares the DNA test (target), one of its two.
    DnaMatchTest,
}

impl RecordLink {
    /// The matchable kind whose match profile reads the relation from its target's side, so a dropped
    /// reference marks the target for rematching; `None` where no target profile reads it.
    pub(crate) const fn rematched_kind(self) -> Option<MatchableKind> {
        match self {
            Self::FamilyPartner | Self::FamilyChild => Some(MatchableKind::Person),
            Self::FamilyEvent | Self::Participation => Some(MatchableKind::Event),
            Self::EventPlace | Self::PlaceEnclosure => Some(MatchableKind::Place),
            Self::SourceRepository => Some(MatchableKind::Repository),
            Self::CitationSource => Some(MatchableKind::Source),
            Self::Tagged | Self::DnaTestPerson | Self::DnaMatchTest => None,
        }
    }

    /// The relation as the index stores it.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::FamilyPartner => "family-partner",
            Self::FamilyChild => "family-child",
            Self::FamilyEvent => "family-event",
            Self::Participation => "participation",
            Self::EventPlace => "event-place",
            Self::PlaceEnclosure => "place-enclosure",
            Self::SourceRepository => "source-repository",
            Self::CitationSource => "citation-source",
            Self::Tagged => "tagged",
            Self::DnaTestPerson => "dna-test-person",
            Self::DnaMatchTest => "dna-match-test",
        }
    }
}

/// A projection whose references the index mirrors: its aggregate, its table, the relations it is the
/// source of, and which of its events can change them.
pub(crate) trait LinkingRecord: DeserializeOwned + Send + Sync + 'static {
    /// The aggregate the projection folds.
    type State: Aggregate;

    /// The projection table the view is read from.
    const VIEW_TABLE: &'static str;

    /// The relations this kind of record is the source of.
    const RELATIONS: &'static [RecordLink];

    /// The record's aggregate id, once created.
    fn linking_id(&self) -> Option<String>;

    /// The record's live references, as `(relation, target)`.
    fn links(&self) -> Vec<(RecordLink, String)>;

    /// Whether `event` can change the record's references.
    fn changes_links(event: &<Self::State as Aggregate>::Event) -> bool;
}

/// The `tagged` references of a record carrying `tags`.
fn tag_links(tags: Vec<TagId>) -> impl Iterator<Item = (RecordLink, String)> {
    tags.into_iter().map(|tag| (RecordLink::Tagged, tag.to_string()))
}

impl LinkingRecord for PersonView {
    type State = PersonState;

    const VIEW_TABLE: &'static str = PERSON_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::Participation, RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.person_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        let mut links = Vec::new();
        for participation in self.participations() {
            links.push((RecordLink::Participation, participation.event_id.to_string()));
        }
        links.extend(tag_links(self.tags()));
        links
    }

    fn changes_links(event: &PersonEvent) -> bool {
        match &event.body {
            PersonEventBody::ParticipationAsserted { .. }
            | PersonEventBody::AssertionRetracted { .. }
            | PersonEventBody::AssertionSuperseded { .. }
            | PersonEventBody::Tagged { .. }
            | PersonEventBody::Untagged { .. } => true,
            PersonEventBody::PersonCreated { .. }
            | PersonEventBody::NameAsserted { .. }
            | PersonEventBody::SexAsserted { .. }
            | PersonEventBody::FactAsserted { .. }
            | PersonEventBody::AssociationAsserted { .. }
            | PersonEventBody::MediaAttached { .. }
            | PersonEventBody::NoteAttached { .. }
            | PersonEventBody::CitationAdded { .. }
            | PersonEventBody::ExternalIdAdded { .. }
            | PersonEventBody::RestrictionsChanged { .. }
            | PersonEventBody::HumanIdChanged { .. }
            | PersonEventBody::PersonsMerged { .. }
            | PersonEventBody::PersonsDistinguished { .. } => false,
        }
    }
}

impl LinkingRecord for FamilyView {
    type State = FamilyState;

    const VIEW_TABLE: &'static str = FAMILY_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[
        RecordLink::FamilyPartner,
        RecordLink::FamilyChild,
        RecordLink::FamilyEvent,
        RecordLink::Tagged,
    ];

    fn linking_id(&self) -> Option<String> {
        self.family_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        let mut links = Vec::new();
        for partner in self.partners() {
            links.push((RecordLink::FamilyPartner, partner.to_string()));
        }
        for child in self.children() {
            links.push((RecordLink::FamilyChild, child.child_id.to_string()));
        }
        for event in self.linked_events() {
            links.push((RecordLink::FamilyEvent, event.to_string()));
        }
        links.extend(tag_links(self.tags()));
        links
    }

    fn changes_links(event: &FamilyEvent) -> bool {
        match &event.body {
            FamilyEventBody::PartnerAdded { .. }
            | FamilyEventBody::PartnerRemoved { .. }
            | FamilyEventBody::ChildAdded { .. }
            | FamilyEventBody::ChildRemoved { .. }
            | FamilyEventBody::FamilyEventLinked { .. }
            | FamilyEventBody::AssertionRetracted { .. }
            | FamilyEventBody::AssertionSuperseded { .. }
            | FamilyEventBody::Tagged { .. }
            | FamilyEventBody::Untagged { .. } => true,
            FamilyEventBody::FamilyCreated { .. }
            | FamilyEventBody::ChildRelationshipAsserted { .. }
            | FamilyEventBody::RestrictionsChanged { .. }
            | FamilyEventBody::CitationAdded { .. }
            | FamilyEventBody::MediaAttached { .. }
            | FamilyEventBody::NoteAttached { .. }
            | FamilyEventBody::ExternalIdAdded { .. }
            | FamilyEventBody::HumanIdChanged { .. }
            | FamilyEventBody::FamiliesMerged { .. }
            | FamilyEventBody::FamiliesDistinguished { .. } => false,
        }
    }
}

impl LinkingRecord for EventView {
    type State = EventState;

    const VIEW_TABLE: &'static str = EVENT_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::EventPlace, RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.event_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        let mut links = Vec::new();
        if let Some(place) = self.place_id() {
            links.push((RecordLink::EventPlace, place.to_string()));
        }
        links.extend(tag_links(self.tags()));
        links
    }

    fn changes_links(event: &EventEvent) -> bool {
        match &event.body {
            EventEventBody::PlaceLinked { .. }
            | EventEventBody::AssertionRetracted { .. }
            | EventEventBody::AssertionSuperseded { .. }
            | EventEventBody::Tagged { .. }
            | EventEventBody::Untagged { .. } => true,
            EventEventBody::EventCreated { .. }
            | EventEventBody::EventTypeSet { .. }
            | EventEventBody::DateAsserted { .. }
            | EventEventBody::DescriptionSet { .. }
            | EventEventBody::AddressAdded { .. }
            | EventEventBody::CitationAdded { .. }
            | EventEventBody::MediaAttached { .. }
            | EventEventBody::NoteAttached { .. }
            | EventEventBody::RestrictionsChanged { .. }
            | EventEventBody::HumanIdChanged { .. }
            | EventEventBody::EventsMerged { .. }
            | EventEventBody::EventsDistinguished { .. } => false,
        }
    }
}

impl LinkingRecord for PlaceView {
    type State = PlaceState;

    const VIEW_TABLE: &'static str = PLACE_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::PlaceEnclosure, RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.place_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        let mut links = Vec::new();
        for enclosing in self.enclosed_by() {
            links.push((RecordLink::PlaceEnclosure, enclosing.place_id.to_string()));
        }
        links.extend(tag_links(self.tags()));
        links
    }

    fn changes_links(event: &PlaceEvent) -> bool {
        match &event.body {
            PlaceEventBody::EnclosedByAsserted { .. }
            | PlaceEventBody::AssertionRetracted { .. }
            | PlaceEventBody::AssertionSuperseded { .. }
            | PlaceEventBody::Tagged { .. }
            | PlaceEventBody::Untagged { .. } => true,
            PlaceEventBody::PlaceCreated { .. }
            | PlaceEventBody::PlaceTypeSet { .. }
            | PlaceEventBody::NameAsserted { .. }
            | PlaceEventBody::CoordinatesAsserted { .. }
            | PlaceEventBody::GeometryAsserted { .. }
            | PlaceEventBody::SuccessionAsserted { .. }
            | PlaceEventBody::CodeSet { .. }
            | PlaceEventBody::CitationAdded { .. }
            | PlaceEventBody::MediaAttached { .. }
            | PlaceEventBody::NoteAttached { .. }
            | PlaceEventBody::RestrictionsChanged { .. }
            | PlaceEventBody::HumanIdChanged { .. }
            | PlaceEventBody::PlacesMerged { .. }
            | PlaceEventBody::PlacesDistinguished { .. } => false,
        }
    }
}

impl LinkingRecord for SourceView {
    type State = SourceState;

    const VIEW_TABLE: &'static str = SOURCE_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::SourceRepository, RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.source_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        let mut links = Vec::new();
        for held in self.repositories() {
            links.push((RecordLink::SourceRepository, held.repository_id.to_string()));
        }
        links.extend(tag_links(self.tags()));
        links
    }

    fn changes_links(event: &SourceEvent) -> bool {
        match &event.body {
            SourceEventBody::RepositoryLinked { .. }
            | SourceEventBody::AssertionRetracted { .. }
            | SourceEventBody::AssertionSuperseded { .. }
            | SourceEventBody::Tagged { .. }
            | SourceEventBody::Untagged { .. } => true,
            SourceEventBody::SourceCreated { .. }
            | SourceEventBody::TitleSet { .. }
            | SourceEventBody::AuthorSet { .. }
            | SourceEventBody::PubInfoSet { .. }
            | SourceEventBody::AbbrevSet { .. }
            | SourceEventBody::AttributeAdded { .. }
            | SourceEventBody::MediaAttached { .. }
            | SourceEventBody::NoteAttached { .. }
            | SourceEventBody::RestrictionsChanged { .. }
            | SourceEventBody::HumanIdChanged { .. }
            | SourceEventBody::SourcesMerged { .. }
            | SourceEventBody::SourcesDistinguished { .. } => false,
        }
    }
}

impl LinkingRecord for CitationView {
    type State = CitationState;

    const VIEW_TABLE: &'static str = CITATION_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::CitationSource, RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.citation_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        let mut links = Vec::new();
        if let Some(source) = self.source_id() {
            links.push((RecordLink::CitationSource, source.to_string()));
        }
        links.extend(tag_links(self.tags()));
        links
    }

    fn changes_links(event: &CitationEvent) -> bool {
        match &event.body {
            CitationEventBody::CitationCreated { .. }
            | CitationEventBody::AssertionRetracted { .. }
            | CitationEventBody::AssertionSuperseded { .. }
            | CitationEventBody::Tagged { .. }
            | CitationEventBody::Untagged { .. } => true,
            CitationEventBody::PageSet { .. }
            | CitationEventBody::DateAsserted { .. }
            | CitationEventBody::ConfidenceSet { .. }
            | CitationEventBody::EvidenceAnalysisSet { .. }
            | CitationEventBody::AttributeAdded { .. }
            | CitationEventBody::MediaAttached { .. }
            | CitationEventBody::NoteAttached { .. }
            | CitationEventBody::RestrictionsChanged { .. }
            | CitationEventBody::HumanIdChanged { .. }
            | CitationEventBody::CitationsMerged { .. }
            | CitationEventBody::CitationsDistinguished { .. } => false,
        }
    }
}

impl LinkingRecord for RepositoryView {
    type State = RepositoryState;

    const VIEW_TABLE: &'static str = REPOSITORY_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.repository_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        tag_links(self.tags()).collect()
    }

    fn changes_links(event: &RepositoryEvent) -> bool {
        match &event.body {
            RepositoryEventBody::Tagged { .. }
            | RepositoryEventBody::Untagged { .. }
            | RepositoryEventBody::AssertionRetracted { .. }
            | RepositoryEventBody::AssertionSuperseded { .. } => true,
            RepositoryEventBody::RepositoryCreated { .. }
            | RepositoryEventBody::RepositoryTypeSet { .. }
            | RepositoryEventBody::NameSet { .. }
            | RepositoryEventBody::AddressAdded { .. }
            | RepositoryEventBody::UrlAdded { .. }
            | RepositoryEventBody::NoteAttached { .. }
            | RepositoryEventBody::RestrictionsChanged { .. }
            | RepositoryEventBody::HumanIdChanged { .. }
            | RepositoryEventBody::RepositoriesMerged { .. }
            | RepositoryEventBody::RepositoriesDistinguished { .. } => false,
        }
    }
}

impl LinkingRecord for NoteView {
    type State = NoteState;

    const VIEW_TABLE: &'static str = NOTE_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.note_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        tag_links(self.tags()).collect()
    }

    fn changes_links(event: &NoteEvent) -> bool {
        match &event.body {
            NoteEventBody::Tagged { .. }
            | NoteEventBody::Untagged { .. }
            | NoteEventBody::AssertionRetracted { .. }
            | NoteEventBody::AssertionSuperseded { .. } => true,
            NoteEventBody::NoteCreated { .. }
            | NoteEventBody::NoteTypeSet { .. }
            | NoteEventBody::RichTextSet { .. }
            | NoteEventBody::RestrictionsChanged { .. }
            | NoteEventBody::HumanIdChanged { .. }
            | NoteEventBody::NotesMerged { .. }
            | NoteEventBody::NotesDistinguished { .. } => false,
        }
    }
}

impl LinkingRecord for MediaView {
    type State = MediaState;

    const VIEW_TABLE: &'static str = MEDIA_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.media_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        tag_links(self.tags()).collect()
    }

    fn changes_links(event: &MediaEvent) -> bool {
        match &event.body {
            MediaEventBody::Tagged { .. }
            | MediaEventBody::Untagged { .. }
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
            | MediaEventBody::RestrictionsChanged { .. }
            | MediaEventBody::HumanIdChanged { .. }
            | MediaEventBody::MediaMerged { .. }
            | MediaEventBody::MediaDistinguished { .. } => false,
        }
    }
}

impl LinkingRecord for ResearchNoteView {
    type State = ResearchNoteState;

    const VIEW_TABLE: &'static str = RESEARCH_NOTE_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.research_note_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        tag_links(self.tags()).collect()
    }

    fn changes_links(event: &ResearchNoteEvent) -> bool {
        match &event.body {
            ResearchNoteEventBody::Tagged { .. }
            | ResearchNoteEventBody::Untagged { .. }
            | ResearchNoteEventBody::AssertionRetracted { .. }
            | ResearchNoteEventBody::AssertionSuperseded { .. } => true,
            ResearchNoteEventBody::ResearchNoteCreated { .. }
            | ResearchNoteEventBody::SubjectAdded { .. }
            | ResearchNoteEventBody::SubjectRemoved { .. }
            | ResearchNoteEventBody::RichTextSet { .. }
            | ResearchNoteEventBody::RestrictionsChanged { .. } => false,
        }
    }
}

impl LinkingRecord for DnaTestView {
    type State = DnaTestState;

    const VIEW_TABLE: &'static str = DNA_TEST_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::DnaTestPerson, RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.dna_test_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        let mut links = Vec::new();
        if let Some(person) = self.person_id() {
            links.push((RecordLink::DnaTestPerson, person.to_string()));
        }
        links.extend(tag_links(self.tags()));
        links
    }

    fn changes_links(event: &DnaTestEvent) -> bool {
        match &event.body {
            DnaTestEventBody::DnaTestCreated { .. }
            | DnaTestEventBody::Tagged { .. }
            | DnaTestEventBody::Untagged { .. }
            | DnaTestEventBody::AssertionRetracted { .. }
            | DnaTestEventBody::AssertionSuperseded { .. } => true,
            DnaTestEventBody::ProviderSet { .. }
            | DnaTestEventBody::KitIdSet { .. }
            | DnaTestEventBody::TestTypeSet { .. }
            | DnaTestEventBody::GenomeBuildSet { .. }
            | DnaTestEventBody::HaplogroupAsserted { .. }
            | DnaTestEventBody::NoteAttached { .. }
            | DnaTestEventBody::RestrictionsChanged { .. }
            | DnaTestEventBody::HumanIdChanged { .. } => false,
        }
    }
}

impl LinkingRecord for DnaMatchView {
    type State = DnaMatchState;

    const VIEW_TABLE: &'static str = DNA_MATCH_VIEW_TABLE;
    const RELATIONS: &'static [RecordLink] = &[RecordLink::DnaMatchTest, RecordLink::Tagged];

    fn linking_id(&self) -> Option<String> {
        self.dna_match_id().map(|id| id.to_string())
    }

    fn links(&self) -> Vec<(RecordLink, String)> {
        let mut links = Vec::new();
        for test in [self.test_a(), self.test_b()].into_iter().flatten() {
            links.push((RecordLink::DnaMatchTest, test.to_string()));
        }
        links.extend(tag_links(self.tags()));
        links
    }

    fn changes_links(event: &DnaMatchEvent) -> bool {
        match &event.body {
            DnaMatchEventBody::DnaMatchObserved { .. }
            | DnaMatchEventBody::Tagged { .. }
            | DnaMatchEventBody::Untagged { .. }
            | DnaMatchEventBody::AssertionRetracted { .. }
            | DnaMatchEventBody::AssertionSuperseded { .. } => true,
            DnaMatchEventBody::SegmentAdded { .. }
            | DnaMatchEventBody::SharedAncestorAsserted { .. }
            | DnaMatchEventBody::MatchConfirmed { .. }
            | DnaMatchEventBody::MatchRejected { .. }
            | DnaMatchEventBody::NoteAttached { .. }
            | DnaMatchEventBody::RestrictionsChanged { .. }
            | DnaMatchEventBody::HumanIdChanged { .. } => false,
        }
    }
}
