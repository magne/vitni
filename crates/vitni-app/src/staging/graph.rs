//! The record graph an importer submits (ADR 0040 §1): one source record, as the entities it holds and
//! the links between them, before anything is resolved or written.
//!
//! Each entity carries a local id, unique within its graph, and an item key naming it within the record
//! (ADR 0037 §1) — `None` for the record's own entity. A link carries the item it is stamped with, so a
//! link's origin is stated by the importer rather than derived from its ends. A link's ends, and a
//! citation's source, are [`EntityRef`]s: an entity of the same graph, one of another graph of the same
//! run by its origin, or an existing record by its human id.

use std::collections::{BTreeSet, HashSet};

use vitni_core::address::Address;
use vitni_core::age::Age;
use vitni_core::date::GenealogicalDate;
use vitni_core::enums::{
    AssociationRole, ChildParentRelationship, EventType, NoteType, ParticipantRole, PlaceType, Restriction, Sex,
    SourceMediaType,
};
use vitni_core::geo::GeoCoordinates;
use vitni_core::matching::MatchableKind;
use vitni_core::provenance::Confidence;
use vitni_core::text::{Attribute, ExternalId, Rect};

use crate::person::{NewFact, PersonNameParts};

/// An entity's id within its graph.
pub type LocalId = u32;

/// One source record, as an importer read it.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordGraph {
    /// The record's id within the importer's dataset (a GEDCOM xref, a Gramps handle).
    pub record: String,
    /// The entities the record holds.
    pub entities: Vec<StagedEntity>,
    /// The links between them, and to entities of other records.
    pub links: Vec<StagedLink>,
}

/// One entity of a record.
#[derive(Debug, Clone, PartialEq)]
pub struct StagedEntity {
    /// The entity's id within its graph.
    pub local_id: LocalId,
    /// The entity's key within its record (`event:BIRT:0`); `None` for the record's own entity.
    pub item: Option<String>,
    /// What the record says about it.
    pub fields: EntityFields,
}

/// An entity's incoming fields, by kind.
#[derive(Debug, Clone, PartialEq)]
pub enum EntityFields {
    /// A person.
    Person(StagedPerson),
    /// A family.
    Family(StagedFamily),
    /// An event.
    Event(StagedEvent),
    /// A place.
    Place(StagedPlace),
    /// A source.
    Source(StagedSource),
    /// A citation.
    Citation(StagedCitation),
    /// A media object.
    Media(StagedMedia),
    /// A note.
    Note(StagedNote),
    /// A repository.
    Repository(StagedRepository),
    /// A tag.
    Tag(StagedTag),
}

impl EntityFields {
    /// The entity's kind.
    #[must_use]
    pub fn kind(&self) -> MatchableKind {
        match self {
            Self::Person(_) => MatchableKind::Person,
            Self::Family(_) => MatchableKind::Family,
            Self::Event(_) => MatchableKind::Event,
            Self::Place(_) => MatchableKind::Place,
            Self::Source(_) => MatchableKind::Source,
            Self::Citation(_) => MatchableKind::Citation,
            Self::Media(_) => MatchableKind::Media,
            Self::Note(_) => MatchableKind::Note,
            Self::Repository(_) => MatchableKind::Repository,
            Self::Tag(_) => MatchableKind::Tag,
        }
    }
}

/// A person: every name (the first is primary), sex, facts, external ids and restrictions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StagedPerson {
    /// Every name, in record order.
    pub names: Vec<PersonNameParts>,
    /// The recorded sex.
    pub sex: Option<Sex>,
    /// Single-person facts (occupation, religion, …).
    pub facts: Vec<NewFact>,
    /// Identifiers the person carries in external systems.
    pub external_ids: Vec<ExternalId>,
    /// Privacy restrictions.
    pub restrictions: BTreeSet<Restriction>,
}

/// A family: its external ids and restrictions. Its partners and children are links.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StagedFamily {
    /// Identifiers the family carries in external systems.
    pub external_ids: Vec<ExternalId>,
    /// Privacy restrictions.
    pub restrictions: BTreeSet<Restriction>,
}

/// An event: its type, date, addresses and restrictions. Its place and participants are links.
#[derive(Debug, Clone, PartialEq)]
pub struct StagedEvent {
    /// What happened.
    pub event_type: EventType,
    /// When.
    pub date: Option<GenealogicalDate>,
    /// Postal addresses (GEDCOM `ADDR`).
    pub addresses: Vec<Address>,
    /// Privacy restrictions.
    pub restrictions: BTreeSet<Restriction>,
}

/// A place: its name, type, point and restrictions. The place enclosing it is a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedPlace {
    /// The place's name.
    pub name: String,
    /// The kind of place, when the record says.
    pub place_type: Option<PlaceType>,
    /// Where the place lies, when the record gives a point.
    pub coordinates: Option<GeoCoordinates>,
    /// Privacy restrictions.
    pub restrictions: BTreeSet<Restriction>,
}

/// A source: its title, author, publication, abbreviation and restrictions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StagedSource {
    /// The title.
    pub title: Option<String>,
    /// The author (GEDCOM `AUTH`).
    pub author: Option<String>,
    /// Publication information (GEDCOM `PUBL`).
    pub pub_info: Option<String>,
    /// The abbreviation (GEDCOM `ABBR`).
    pub abbrev: Option<String>,
    /// Privacy restrictions.
    pub restrictions: BTreeSet<Restriction>,
}

/// A citation: the source it cites, its page, confidence and restrictions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedCitation {
    /// The cited source.
    pub source: EntityRef,
    /// Where in the source.
    pub page: Option<String>,
    /// The recorded surety (GEDCOM `QUAY`).
    pub confidence: Option<Confidence>,
    /// Privacy restrictions.
    pub restrictions: BTreeSet<Restriction>,
}

/// A media object: its path, MIME type and restrictions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StagedMedia {
    /// The file path or URL.
    pub path: Option<String>,
    /// The MIME type.
    pub mime: Option<String>,
    /// Privacy restrictions.
    pub restrictions: BTreeSet<Restriction>,
}

/// A note: its text, type and restrictions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedNote {
    /// The text, as Markdown.
    pub text: String,
    /// The kind of note, when the record says.
    pub note_type: Option<NoteType>,
    /// Privacy restrictions.
    pub restrictions: BTreeSet<Restriction>,
}

/// A repository: its name and restrictions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedRepository {
    /// The name.
    pub name: String,
    /// Privacy restrictions.
    pub restrictions: BTreeSet<Restriction>,
}

/// A tag: its name, which is its identity (ADR 0038 §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedTag {
    /// The name.
    pub name: String,
}

/// Where a link end, or a citation's source, points.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntityRef {
    /// An entity of the same graph.
    Local(LocalId),
    /// An entity of another graph of the same run, or one an earlier run created from that origin.
    Origin {
        /// The entity's kind.
        kind: MatchableKind,
        /// The record holding it.
        record: String,
        /// Its key within the record.
        item: Option<String>,
    },
}

/// One link, and the item of the graph's record it is stamped with.
#[derive(Debug, Clone, PartialEq)]
pub struct StagedLink {
    /// The item the link's assertion is stamped with; `None` for the record's own entity.
    pub item: Option<String>,
    /// What it links.
    pub link: LinkKind,
}

/// A link between two entities, with the link's own fields.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkKind {
    /// A person taking part in an event.
    Participation {
        /// The participant.
        person: EntityRef,
        /// The event.
        event: EntityRef,
        /// The part they took.
        role: ParticipantRole,
        /// Their age at the event.
        age: Option<Age>,
        /// Typed attributes of the participation.
        attributes: Vec<Attribute>,
        /// Notes about the participation.
        notes: Vec<EntityRef>,
        /// Citations backing it.
        citations: Vec<EntityRef>,
    },
    /// A family partner.
    Partner {
        /// The family.
        family: EntityRef,
        /// The partner.
        person: EntityRef,
    },
    /// A family child, with its relationship to each partner.
    Child {
        /// The family.
        family: EntityRef,
        /// The child.
        child: EntityRef,
        /// The child's relationship to each partner the record states one for.
        relationships: Vec<(EntityRef, ChildParentRelationship)>,
    },
    /// An event linked to a family.
    FamilyEvent {
        /// The family.
        family: EntityRef,
        /// The event.
        event: EntityRef,
    },
    /// An event's place.
    EventPlace {
        /// The event.
        event: EntityRef,
        /// The place.
        place: EntityRef,
    },
    /// The place enclosing another.
    Enclosure {
        /// The enclosed place.
        place: EntityRef,
        /// The place enclosing it.
        enclosing: EntityRef,
    },
    /// A citation attached to a person, family or event.
    CitationOf {
        /// The cited record.
        owner: EntityRef,
        /// The citation.
        citation: EntityRef,
    },
    /// A media object attached to a person, family or event.
    MediaOf {
        /// The record the media is attached to.
        owner: EntityRef,
        /// The media object.
        media: EntityRef,
        /// The region of interest within it.
        crop: Option<Rect>,
        /// A caption for this use.
        caption: Option<String>,
    },
    /// A note attached to a person, family, event or citation.
    NoteOf {
        /// The annotated record.
        owner: EntityRef,
        /// The note.
        note: EntityRef,
    },
    /// A tag applied to a person, family or event.
    TagOf {
        /// The tagged record.
        owner: EntityRef,
        /// The tag.
        tag: EntityRef,
    },
    /// A repository holding a source.
    SourceRepository {
        /// The source.
        source: EntityRef,
        /// The repository.
        repository: EntityRef,
        /// The call number (GEDCOM `CALN`).
        call_number: Option<String>,
        /// The medium it is held in (GEDCOM `CALN.MEDI`).
        media_type: SourceMediaType,
    },
    /// A person-to-person association (GEDCOM `ASSO`).
    Association {
        /// The person the association is recorded on.
        person: EntityRef,
        /// The associated person.
        other: EntityRef,
        /// The association's role.
        role: AssociationRole,
    },
}

/// A reference a link end makes, and the kinds it may point at.
pub(crate) struct End<'a> {
    pub reference: &'a EntityRef,
    pub kinds: &'static [MatchableKind],
}

/// Records a citation, media object, note or tag may be attached to.
const OWNERS: &[MatchableKind] = &[MatchableKind::Person, MatchableKind::Family, MatchableKind::Event];
/// Records a note may be attached to: the owners, and a citation.
const NOTE_OWNERS: &[MatchableKind] = &[
    MatchableKind::Person,
    MatchableKind::Family,
    MatchableKind::Event,
    MatchableKind::Citation,
];

const fn end<'a>(reference: &'a EntityRef, kinds: &'static [MatchableKind]) -> End<'a> {
    End { reference, kinds }
}

impl LinkKind {
    /// The end that owns the link, whose record states it.
    pub(crate) fn owner(&self) -> End<'_> {
        use MatchableKind::{Family, Person, Place, Source};
        match self {
            Self::Participation { person, .. } | Self::Association { person, .. } => end(person, &[Person]),
            Self::Partner { family, .. } | Self::Child { family, .. } | Self::FamilyEvent { family, .. } => {
                end(family, &[Family])
            }
            Self::EventPlace { event, .. } => end(event, &[MatchableKind::Event]),
            Self::Enclosure { place, .. } => end(place, &[Place]),
            Self::CitationOf { owner, .. } | Self::MediaOf { owner, .. } | Self::TagOf { owner, .. } => {
                end(owner, OWNERS)
            }
            Self::NoteOf { owner, .. } => end(owner, NOTE_OWNERS),
            Self::SourceRepository { source, .. } => end(source, &[Source]),
        }
    }

    /// The ends the link points at, beyond its owner.
    pub(crate) fn targets(&self) -> Vec<End<'_>> {
        use MatchableKind::{Citation, Event, Media, Note, Person, Place, Repository, Tag};
        match self {
            Self::Participation {
                event,
                notes,
                citations,
                ..
            } => {
                let mut ends = vec![end(event, &[Event])];
                ends.extend(notes.iter().map(|note| end(note, &[Note])));
                ends.extend(citations.iter().map(|citation| end(citation, &[Citation])));
                ends
            }
            Self::Partner { person, .. } => vec![end(person, &[Person])],
            Self::Child {
                child, relationships, ..
            } => {
                let mut ends = vec![end(child, &[Person])];
                ends.extend(relationships.iter().map(|(partner, _)| end(partner, &[Person])));
                ends
            }
            Self::FamilyEvent { event, .. } => vec![end(event, &[Event])],
            Self::EventPlace { place, .. } => vec![end(place, &[Place])],
            Self::Enclosure { enclosing, .. } => vec![end(enclosing, &[Place])],
            Self::CitationOf { citation, .. } => vec![end(citation, &[Citation])],
            Self::MediaOf { media, .. } => vec![end(media, &[Media])],
            Self::NoteOf { note, .. } => vec![end(note, &[Note])],
            Self::TagOf { tag, .. } => vec![end(tag, &[Tag])],
            Self::SourceRepository { repository, .. } => vec![end(repository, &[Repository])],
            Self::Association { other, .. } => vec![end(other, &[Person])],
        }
    }
}

/// Why a graph cannot be planned.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphError {
    /// Two entities of the graph share a local id.
    #[error("record {record}: local id {local_id} is used twice")]
    DuplicateLocalId {
        /// The graph's record.
        record: String,
        /// The repeated id.
        local_id: LocalId,
    },
    /// Two entities of one kind share an item key.
    #[error("record {record}: two {kind:?} entities share the item {item:?}")]
    DuplicateItem {
        /// The graph's record.
        record: String,
        /// Their kind.
        kind: MatchableKind,
        /// The shared item.
        item: Option<String>,
    },
    /// A reference names a local id the graph does not hold.
    #[error("record {record}: no entity has local id {local_id}")]
    UnknownLocalId {
        /// The graph's record.
        record: String,
        /// The missing id.
        local_id: LocalId,
    },
    /// A reference points at a kind its place in the link cannot hold.
    #[error("record {record}: a {found:?} cannot be referenced here")]
    WrongKind {
        /// The graph's record.
        record: String,
        /// The kind it points at.
        found: MatchableKind,
    },
}

impl RecordGraph {
    /// The kind of the entity `local_id`, if the graph holds it.
    #[must_use]
    pub fn kind_of(&self, local_id: LocalId) -> Option<MatchableKind> {
        self.entities
            .iter()
            .find(|entity| entity.local_id == local_id)
            .map(|entity| entity.fields.kind())
    }

    /// Checks the graph is well-formed: local ids unique, one entity per kind and item, and every
    /// reference pointing at an entity of a kind its place can hold.
    ///
    /// # Errors
    ///
    /// The first [`GraphError`] found.
    pub fn validate(&self) -> Result<(), GraphError> {
        let mut ids = HashSet::new();
        let mut items = HashSet::new();
        for entity in &self.entities {
            if !ids.insert(entity.local_id) {
                return Err(GraphError::DuplicateLocalId {
                    record: self.record.clone(),
                    local_id: entity.local_id,
                });
            }
            let kind = entity.fields.kind();
            if !items.insert((kind, entity.item.clone())) {
                return Err(GraphError::DuplicateItem {
                    record: self.record.clone(),
                    kind,
                    item: entity.item.clone(),
                });
            }
            if let EntityFields::Citation(citation) = &entity.fields {
                self.check(&end(&citation.source, &[MatchableKind::Source]))?;
            }
        }
        for link in &self.links {
            self.check(&link.link.owner())?;
            for target in link.link.targets() {
                self.check(&target)?;
            }
        }
        Ok(())
    }

    fn check(&self, end: &End<'_>) -> Result<(), GraphError> {
        let found = match end.reference {
            EntityRef::Local(local_id) => self.kind_of(*local_id).ok_or_else(|| GraphError::UnknownLocalId {
                record: self.record.clone(),
                local_id: *local_id,
            })?,
            EntityRef::Origin { kind, .. } => *kind,
        };
        if end.kinds.contains(&found) {
            Ok(())
        } else {
            Err(GraphError::WrongKind {
                record: self.record.clone(),
                found,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EntityFields, EntityRef, GraphError, LinkKind, RecordGraph, StagedEntity, StagedLink, StagedTag};
    use vitni_core::matching::MatchableKind;

    fn tag(local_id: u32, item: Option<&str>) -> StagedEntity {
        StagedEntity {
            local_id,
            item: item.map(str::to_owned),
            fields: EntityFields::Tag(StagedTag { name: "t".to_owned() }),
        }
    }

    fn graph(entities: Vec<StagedEntity>, links: Vec<StagedLink>) -> RecordGraph {
        RecordGraph {
            record: "R".to_owned(),
            entities,
            links,
        }
    }

    fn tag_of(owner: EntityRef, tag: EntityRef) -> StagedLink {
        StagedLink {
            item: None,
            link: LinkKind::TagOf { owner, tag },
        }
    }

    #[test]
    fn a_well_formed_graph_is_accepted() {
        let person = EntityRef::Origin {
            kind: MatchableKind::Person,
            record: "I1".to_owned(),
            item: None,
        };
        let graph = graph(vec![tag(0, None)], vec![tag_of(person, EntityRef::Local(0))]);
        assert_eq!(graph.validate(), Ok(()));
    }

    #[test]
    fn a_duplicate_local_id_is_refused() {
        let graph = graph(vec![tag(0, None), tag(0, Some("b"))], Vec::new());
        assert!(matches!(
            graph.validate(),
            Err(GraphError::DuplicateLocalId { local_id: 0, .. })
        ));
    }

    #[test]
    fn two_entities_of_one_kind_and_item_are_refused() {
        let graph = graph(vec![tag(0, Some("a")), tag(1, Some("a"))], Vec::new());
        assert!(matches!(graph.validate(), Err(GraphError::DuplicateItem { .. })));
    }

    #[test]
    fn a_reference_to_a_missing_local_id_is_refused() {
        let graph = graph(
            vec![tag(0, None)],
            vec![tag_of(EntityRef::Local(7), EntityRef::Local(0))],
        );
        assert!(matches!(
            graph.validate(),
            Err(GraphError::UnknownLocalId { local_id: 7, .. })
        ));
    }

    #[test]
    fn a_reference_of_the_wrong_kind_is_refused() {
        let graph = graph(
            vec![tag(0, None)],
            vec![tag_of(EntityRef::Local(0), EntityRef::Local(0))],
        );
        assert!(matches!(
            graph.validate(),
            Err(GraphError::WrongKind {
                found: MatchableKind::Tag,
                ..
            })
        ));
    }
}
