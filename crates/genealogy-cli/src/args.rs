//! CLI mirrors of the closed domain enums, keeping clap's `ValueEnum` off the domain types.
//!
//! Each domain enum carries a `Custom(String)` escape the CLI does not expose yet; these mirrors
//! cover the closed variants and convert into the domain type via `From`.

use clap::ValueEnum;
use genealogy_app::{
    ChildParentRelationship, Confidence, EventType, ParticipantRole, PlaceType, RepositoryType, SourceMediaType,
};
use genealogy_core::enums::EvidenceLevel;

/// CLI mirror of [`Confidence`] — the operator's surety in an assertion (data-model §8).
#[derive(Clone, Copy, ValueEnum)]
pub enum ConfidenceArg {
    /// Lowest surety.
    VeryLow,
    /// Low surety.
    Low,
    /// The default, middling surety.
    Normal,
    /// High surety.
    High,
    /// Highest surety.
    VeryHigh,
}

impl From<ConfidenceArg> for Confidence {
    fn from(value: ConfidenceArg) -> Self {
        match value {
            ConfidenceArg::VeryLow => Self::VeryLow,
            ConfidenceArg::Low => Self::Low,
            ConfidenceArg::Normal => Self::Normal,
            ConfidenceArg::High => Self::High,
            ConfidenceArg::VeryHigh => Self::VeryHigh,
        }
    }
}

/// CLI mirror of [`EvidenceLevel`].
#[derive(Clone, Copy, ValueEnum)]
pub enum EvidenceArg {
    /// A single-source persona.
    Persona,
    /// A researcher's conclusion.
    Conclusion,
}

impl From<EvidenceArg> for EvidenceLevel {
    fn from(value: EvidenceArg) -> Self {
        match value {
            EvidenceArg::Persona => Self::Persona,
            EvidenceArg::Conclusion => Self::Conclusion,
        }
    }
}

/// CLI mirror of [`ChildParentRelationship`].
#[derive(Clone, Copy, ValueEnum)]
pub enum RelationshipArg {
    /// A biological / birth relationship.
    Birth,
    /// An adoptive relationship.
    Adopted,
    /// A foster relationship.
    Foster,
    /// A step relationship.
    Step,
    /// A sealed relationship (LDS).
    Sealed,
    /// An unknown / unrecorded relationship.
    Unknown,
}

impl From<RelationshipArg> for ChildParentRelationship {
    fn from(value: RelationshipArg) -> Self {
        match value {
            RelationshipArg::Birth => Self::Birth,
            RelationshipArg::Adopted => Self::Adopted,
            RelationshipArg::Foster => Self::Foster,
            RelationshipArg::Step => Self::Step,
            RelationshipArg::Sealed => Self::Sealed,
            RelationshipArg::Unknown => Self::Unknown,
        }
    }
}

/// CLI mirror of [`PlaceType`]'s closed variants. The domain's `Custom` escape is not exposed yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum PlaceTypeArg {
    /// A country.
    Country,
    /// A first-level division (county, state, province).
    County,
    /// A municipality / kommune.
    Municipality,
    /// An ecclesiastical parish.
    Parish,
    /// A city.
    City,
    /// A town.
    Town,
    /// A village.
    Village,
    /// A farm / gård.
    Farm,
    /// A single building.
    Building,
}

impl From<PlaceTypeArg> for PlaceType {
    fn from(value: PlaceTypeArg) -> Self {
        match value {
            PlaceTypeArg::Country => Self::Country,
            PlaceTypeArg::County => Self::County,
            PlaceTypeArg::Municipality => Self::Municipality,
            PlaceTypeArg::Parish => Self::Parish,
            PlaceTypeArg::City => Self::City,
            PlaceTypeArg::Town => Self::Town,
            PlaceTypeArg::Village => Self::Village,
            PlaceTypeArg::Farm => Self::Farm,
            PlaceTypeArg::Building => Self::Building,
        }
    }
}

/// CLI mirror of [`EventType`]'s closed variants. The domain's `Custom` escape is not exposed yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum EventTypeArg {
    /// Birth.
    Birth,
    /// Death.
    Death,
    /// Marriage.
    Marriage,
    /// Baptism / christening.
    Baptism,
    /// Burial.
    Burial,
    /// Census enumeration.
    Census,
    /// Residence.
    Residence,
    /// Immigration.
    Immigration,
    /// Emigration.
    Emigration,
}

impl From<EventTypeArg> for EventType {
    fn from(value: EventTypeArg) -> Self {
        match value {
            EventTypeArg::Birth => Self::Birth,
            EventTypeArg::Death => Self::Death,
            EventTypeArg::Marriage => Self::Marriage,
            EventTypeArg::Baptism => Self::Baptism,
            EventTypeArg::Burial => Self::Burial,
            EventTypeArg::Census => Self::Census,
            EventTypeArg::Residence => Self::Residence,
            EventTypeArg::Immigration => Self::Immigration,
            EventTypeArg::Emigration => Self::Emigration,
        }
    }
}

/// CLI mirror of [`ParticipantRole`]'s closed variants. The domain's `Custom` escape is not exposed
/// yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum ParticipantRoleArg {
    /// The principal of the event.
    Primary,
    /// A witness.
    Witness,
    /// An officiator (e.g. clergy).
    Officiator,
    /// The father.
    Father,
    /// The mother.
    Mother,
    /// A parent (neutral).
    Parent,
    /// A child.
    Child,
    /// A godparent.
    Godparent,
    /// The bride.
    Bride,
    /// The groom.
    Groom,
}

impl From<ParticipantRoleArg> for ParticipantRole {
    fn from(value: ParticipantRoleArg) -> Self {
        match value {
            ParticipantRoleArg::Primary => Self::Primary,
            ParticipantRoleArg::Witness => Self::Witness,
            ParticipantRoleArg::Officiator => Self::Officiator,
            ParticipantRoleArg::Father => Self::Father,
            ParticipantRoleArg::Mother => Self::Mother,
            ParticipantRoleArg::Parent => Self::Parent,
            ParticipantRoleArg::Child => Self::Child,
            ParticipantRoleArg::Godparent => Self::Godparent,
            ParticipantRoleArg::Bride => Self::Bride,
            ParticipantRoleArg::Groom => Self::Groom,
        }
    }
}

/// CLI mirror of [`RepositoryType`]'s closed variants. The domain's `Custom` escape is not exposed
/// yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum RepositoryTypeArg {
    /// A library.
    Library,
    /// An archive.
    Archive,
    /// A church / parish.
    Church,
    /// A cemetery.
    Cemetery,
    /// A museum.
    Museum,
    /// A website / online collection.
    Website,
    /// A private or personal collection.
    Collection,
}

impl From<RepositoryTypeArg> for RepositoryType {
    fn from(value: RepositoryTypeArg) -> Self {
        match value {
            RepositoryTypeArg::Library => Self::Library,
            RepositoryTypeArg::Archive => Self::Archive,
            RepositoryTypeArg::Church => Self::Church,
            RepositoryTypeArg::Cemetery => Self::Cemetery,
            RepositoryTypeArg::Museum => Self::Museum,
            RepositoryTypeArg::Website => Self::Website,
            RepositoryTypeArg::Collection => Self::Collection,
        }
    }
}

/// CLI mirror of [`SourceMediaType`]'s closed variants. The domain's `Custom` escape is not exposed
/// yet.
#[derive(Clone, Copy, ValueEnum)]
pub enum SourceMediaTypeArg {
    /// A book.
    Book,
    /// A card / index card.
    Card,
    /// An electronic / digital record.
    Electronic,
    /// Microfiche.
    Fiche,
    /// Microfilm.
    Film,
    /// A magazine / periodical.
    Magazine,
    /// A manuscript.
    Manuscript,
    /// A map.
    Map,
    /// A newspaper.
    Newspaper,
    /// A photograph.
    Photo,
    /// A tombstone / grave marker.
    Tombstone,
    /// A video recording.
    Video,
    /// An audio recording.
    Audio,
}

impl From<SourceMediaTypeArg> for SourceMediaType {
    fn from(value: SourceMediaTypeArg) -> Self {
        match value {
            SourceMediaTypeArg::Book => Self::Book,
            SourceMediaTypeArg::Card => Self::Card,
            SourceMediaTypeArg::Electronic => Self::Electronic,
            SourceMediaTypeArg::Fiche => Self::Fiche,
            SourceMediaTypeArg::Film => Self::Film,
            SourceMediaTypeArg::Magazine => Self::Magazine,
            SourceMediaTypeArg::Manuscript => Self::Manuscript,
            SourceMediaTypeArg::Map => Self::Map,
            SourceMediaTypeArg::Newspaper => Self::Newspaper,
            SourceMediaTypeArg::Photo => Self::Photo,
            SourceMediaTypeArg::Tombstone => Self::Tombstone,
            SourceMediaTypeArg::Video => Self::Video,
            SourceMediaTypeArg::Audio => Self::Audio,
        }
    }
}
