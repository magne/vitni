//! The profiles the matching engine compares (ADR 0038 §2): the evidence of one record, gathered by
//! the app layer from views and handed in as values.

use crate::date::GenealogicalDate;
use crate::enums::Sex;
use crate::geo::GeoCoordinates;
use crate::ids::PlaceId;
use crate::matching::date::DateBasis;
use crate::name::PersonName;
use crate::origin::RecordOrigin;
use crate::place_name::PlaceName;
use crate::text::ExternalId;

/// A person's own evidence: names, sex, vital events and the places that select name cultures.
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
    /// The record origins the person was imported from (ADR 0037 §1).
    pub origins: Vec<RecordOrigin>,
    /// Identifiers the person carries in external systems.
    pub external_ids: Vec<ExternalId>,
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
