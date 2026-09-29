//! The profiles the matching engine compares (ADR 0038 §2): the evidence of one record, gathered by
//! the app layer from views and handed in as values.

use crate::date::GenealogicalDate;
use crate::enums::{EventType, ParticipantRole, Sex};
use crate::geo::GeoCoordinates;
use crate::ids::PlaceId;
use crate::matching::date::DateBasis;
use crate::name::PersonName;
use crate::origin::RecordOrigin;
use crate::place_name::PlaceName;
use crate::text::ExternalId;

/// A person's evidence: names, sex, vital events, occupations, the places that select name cultures, and
/// the relatives that tell two same-named people apart.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonProfile {
    /// Every name the person is recorded under, with its data language.
    pub names: Vec<PersonName>,
    /// The asserted sex, if any.
    pub sex: Option<Sex>,
    /// Birth, baptism, death and burial, each with its date and place.
    pub vitals: Vec<VitalEvent>,
    /// Places tied to the person beyond the vital events — residences, and the places of parents and
    /// ancestors (lineage) — which select name cultures but are not compared.
    pub lineage: Vec<PlaceMention>,
    /// What the person worked as, as recorded.
    pub occupations: Vec<String>,
    /// The person's parents.
    pub parents: Vec<Relative>,
    /// The person's partners.
    pub partners: Vec<Relative>,
    /// The person's children.
    pub children: Vec<Relative>,
    /// The origins of the assertions that created the person (ADR 0037 §1) — never those of its
    /// participations, which a record's father, mother and child share.
    pub origins: Vec<RecordOrigin>,
    /// Identifiers the person carries in external systems.
    pub external_ids: Vec<ExternalId>,
}

/// A relative as the matcher sees one: names, sex and birth.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Relative {
    /// Every name the relative is recorded under.
    pub names: Vec<PersonName>,
    /// The relative's asserted sex, which tells a father from a mother.
    pub sex: Option<Sex>,
    /// The relative's birth, or the baptism standing in for it.
    pub birth: Option<VitalEvent>,
}

/// The kind of a vital event. Baptism stands in for an unrecorded birth, and burial for death.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VitalKind {
    /// Birth.
    Birth,
    /// Baptism or christening.
    Baptism,
    /// Death.
    Death,
    /// Burial.
    Burial,
}

/// One vital event of a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VitalEvent {
    /// What happened.
    pub kind: VitalKind,
    /// When, if known.
    pub date: Option<GenealogicalDate>,
    /// How the date was arrived at.
    pub basis: DateBasis,
    /// Where, if known.
    pub place: Option<PlaceProfile>,
}

/// A place as the matcher sees it: its names, the places enclosing it, its country and coordinates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceProfile {
    /// The place's aggregate id, when it is a workspace place.
    pub id: Option<PlaceId>,
    /// Every name of the place, dated and with language.
    pub names: Vec<PlaceName>,
    /// Every place enclosing this one, transitively (farm → parish → county → country).
    pub enclosing: Vec<PlaceId>,
    /// The name of the country the place lies in, which selects name cultures by region.
    pub country: Option<String>,
    /// The place's coordinates, if known.
    pub coordinates: Option<GeoCoordinates>,
}

/// A country a person is tied to at some date, for name-culture selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceMention {
    /// The country's name.
    pub country: String,
    /// When the person (or the ancestor) was there.
    pub date: Option<GenealogicalDate>,
}

/// A family's evidence: its partners, its children and its marriage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FamilyProfile {
    /// The partners' person profiles, without their partners and children: the family compares those
    /// itself, so a partner's own profile carrying them would count them twice.
    pub partners: Vec<PersonProfile>,
    /// The family's children.
    pub children: Vec<Relative>,
    /// The family's marriage, if one is linked.
    pub marriage: Option<EventProfile>,
    /// The origins of the assertions that created the family (ADR 0037 §1).
    pub origins: Vec<RecordOrigin>,
    /// Identifiers the family carries in external systems.
    pub external_ids: Vec<ExternalId>,
}

/// An event's evidence: its type, date, place and the people taking part.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventProfile {
    /// What happened, if known.
    pub event_type: Option<EventType>,
    /// When, if known.
    pub date: Option<GenealogicalDate>,
    /// Where, if known.
    pub place: Option<PlaceProfile>,
    /// Everyone taking part, with their roles.
    pub participants: Vec<Participant>,
    /// The origins of the assertions that created the event (ADR 0037 §1).
    pub origins: Vec<RecordOrigin>,
}

/// Someone taking part in an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    /// The part they took.
    pub role: ParticipantRole,
    /// Who they are: names, sex and birth.
    pub person: Relative,
}
