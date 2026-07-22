//! Enumerated type sets shared across aggregates (data-model §7).
//!
//! Each is a closed enum **plus a `Custom(String)` escape hatch**, mirroring Gramps' "custom
//! type" pattern: the common cases are coded and language-neutral, but the model never blocks an
//! unanticipated value. Human-readable labels are a UI concern (data-model §14), never stored.
//!
use serde::{Deserialize, Serialize};

/// Biological / recorded sex (GEDCOM 7 added `X` for intersex — data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Sex {
    /// Male.
    Male,
    /// Female.
    Female,
    /// Unknown / unrecorded.
    Unknown,
    /// Does not fit a binary male/female classification (GEDCOM 7 `X`).
    Intersex,
    /// Another recorded value not covered above.
    Other(String),
}

/// A privacy restriction on a record (GEDCOM v7 `RESN` — data-model §6, §16).
///
/// A record carries a *set* of these (`BTreeSet<Restriction>`); the empty set means unrestricted.
/// Closed set — GEDCOM `RESN` has exactly these three values and no custom escape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Restriction {
    /// Hide from general view.
    Confidential,
    /// Protected from edits.
    Locked,
    /// Living-person privacy.
    Privacy,
}

/// Whether a `Person` is a single-source persona or a synthesised conclusion (data-model §6, §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceLevel {
    /// Extracted from a single source (GEDCOM X persona).
    Persona,
    /// A researcher's synthesis across evidence.
    Conclusion,
}

/// The kind of single-person fact — an attribute-shaped claim about one person (closed set plus a
/// custom escape — data-model §7, §10).
///
/// Vital, shared-capable types (birth, death, baptism, burial) are **not** here: they are asserted
/// as [`EventType`] events with a `Primary` participant, so a birth that later gains a witness has
/// one representation, not two (ADR 0021 §2). The one deliberate overlap is `Residence`:
/// `FactType::Residence` is the GEDCOM `RESI` *attribute* (a person's stated place of residence),
/// while [`EventType::Residence`] is a dated residence *event*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum FactType {
    /// Occupation.
    Occupation,
    /// Residence.
    Residence,
    /// Religion (religious affiliation — GEDCOM `RELI`).
    Religion,
    /// Caste / caste name (GEDCOM `CAST`).
    Caste,
    /// Physical description (GEDCOM `DSCR`).
    PhysicalDescription,
    /// Education / scholastic achievement (GEDCOM `EDUC`).
    Education,
    /// Ethnicity (GEDCOM 7 `ETHN`).
    Ethnicity,
    /// A national / tribal identification number (GEDCOM `IDNO`).
    NationalId,
    /// National or tribal origin (GEDCOM `NATI`).
    Nationality,
    /// Number of children (GEDCOM `NCHI`).
    NumberOfChildren,
    /// Number of marriages (GEDCOM `NMR`).
    NumberOfMarriages,
    /// Property / possessions (GEDCOM `PROP`).
    Property,
    /// Social security / national insurance number (GEDCOM `SSN`).
    SocialSecurityNumber,
    /// A title of nobility (GEDCOM `TITL`).
    NobilityTitle,
    /// An application-defined fact type.
    Custom(String),
}

/// A participant's role in a shared `Event` (TMG-style, per participant — data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ParticipantRole {
    /// The principal of the event.
    Primary,
    /// A witness.
    Witness,
    /// An officiator (a civil/legal official — GEDCOM `OFFICIATOR`).
    Officiator,
    /// Clergy (a religious official — GEDCOM `CLERGY`).
    Clergy,
    /// The father.
    Father,
    /// The mother.
    Mother,
    /// A parent (neutral).
    Parent,
    /// A child.
    Child,
    /// A husband.
    Husband,
    /// A wife.
    Wife,
    /// A spouse (neutral).
    Spouse,
    /// A godparent.
    Godparent,
    /// A friend.
    Friend,
    /// A neighbour.
    Neighbour,
    /// Plays multiple roles (GEDCOM `MULTIPLE`).
    Multiple,
    /// The bride.
    Bride,
    /// The groom.
    Groom,
    /// An application-defined role.
    Custom(String),
}

/// How a child relates to the family's parents within a `Family` (GEDCOM `PEDI` — data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ChildParentRelationship {
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
    /// An application-defined relationship.
    Custom(String),
}

/// The kind of person-to-person association (GEDCOM 7 `ASSO.ROLE` — data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum AssociationRole {
    /// Clergy.
    Clergy,
    /// A friend.
    Friend,
    /// A godparent.
    Godparent,
    /// A neighbour.
    Neighbour,
    /// An officiator.
    Officiator,
    /// A witness.
    Witness,
    /// A child.
    Child,
    /// A father.
    Father,
    /// A mother.
    Mother,
    /// A parent (neutral).
    Parent,
    /// A husband.
    Husband,
    /// A wife.
    Wife,
    /// A spouse (neutral).
    Spouse,
    /// Holds multiple roles (GEDCOM `MULTIPLE`).
    Multiple,
    /// An application-defined association.
    Custom(String),
}

/// The kind of a `Place` in the enclosure hierarchy (closed set plus a custom escape — Gramps
/// place types, data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum PlaceType {
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
    /// An application-defined place type.
    Custom(String),
}

/// How a `Place`'s *identity* changed into (or out of) another place — a municipality merger,
/// county split, or absorption (ADR 0026 §2–§3). Cardinality carries the meaning: `Merged` is
/// many→one, `Split` is one→many, `Absorbed`/`Elevated`/`Renamed` are one→one. Distinct from a mere
/// rename of the same place (a dated `PlaceName` on the same aggregate — ADR 0026 §2); a closed set,
/// not a GEDCOM-derived open vocabulary, so no `Custom` escape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuccessionKind {
    /// Two or more places merged into one (many→one; e.g. Aker + Kristiania → Oslo, 1948).
    Merged,
    /// One place split into two or more (one→many; e.g. a county division).
    Split,
    /// One place was absorbed into another, which continues under its own identity (one→one).
    Absorbed,
    /// One place was elevated to a new administrative level, becoming a new identity (one→one).
    Elevated,
    /// One place's identity was replaced by a new one — distinct from a same-aggregate rename
    /// (one→one).
    Renamed,
}

/// The kind of a shared `Event` (closed set plus a custom escape — data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum EventType {
    /// Birth.
    Birth,
    /// Death.
    Death,
    /// Marriage.
    Marriage,
    /// Baptism (GEDCOM `BAPM`).
    Baptism,
    /// Christening (GEDCOM `CHR`) — distinct from infant/adult baptism.
    Christening,
    /// Burial.
    Burial,
    /// Cremation.
    Cremation,
    /// Census enumeration.
    Census,
    /// Residence.
    Residence,
    /// Immigration.
    Immigration,
    /// Emigration.
    Emigration,
    /// Adoption.
    Adoption,
    /// Confirmation.
    Confirmation,
    /// Bar Mitzvah.
    BarMitzvah,
    /// Bas / Bat Mitzvah.
    BasMitzvah,
    /// First communion.
    FirstCommunion,
    /// Graduation.
    Graduation,
    /// Naturalization.
    Naturalization,
    /// Ordination.
    Ordination,
    /// Probate.
    Probate,
    /// Retirement.
    Retirement,
    /// Will.
    Will,
    /// Engagement.
    Engagement,
    /// Annulment.
    Annulment,
    /// Divorce.
    Divorce,
    /// Divorce filed (GEDCOM `DIVF`).
    DivorceFiled,
    /// Marriage banns (GEDCOM `MARB`).
    MarriageBanns,
    /// Marriage contract (GEDCOM `MARC`).
    MarriageContract,
    /// Marriage license (GEDCOM `MARL`).
    MarriageLicense,
    /// Marriage settlement (GEDCOM `MARS`).
    MarriageSettlement,
    /// An application-defined event type.
    Custom(String),
}

/// The kind of a `Repository` that holds sources (closed set plus a custom escape — data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum RepositoryType {
    /// A library.
    Library,
    /// An archive (e.g. a national or regional archive).
    Archive,
    /// A church / parish holding registers.
    Church,
    /// A cemetery.
    Cemetery,
    /// A museum.
    Museum,
    /// A website / online collection.
    Website,
    /// A private or personal collection.
    Collection,
    /// An application-defined repository type.
    Custom(String),
}

/// The medium of a source as held in a repository (GEDCOM `MEDI` — data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum SourceMediaType {
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
    /// An application-defined medium.
    Custom(String),
}

/// The kind of a `Note` (closed set plus a custom escape — data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum NoteType {
    /// A general note.
    General,
    /// A research note.
    Research,
    /// A source transcript.
    Transcript,
    /// A citation note.
    Citation,
    /// An application-defined note type.
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::{AssociationRole, ChildParentRelationship, EventType, FactType, ParticipantRole, PlaceType, Sex};

    #[test]
    fn sex_other_round_trips() {
        let sex = Sex::Other("intersex".to_owned());
        let json = serde_json::to_string(&sex).unwrap();
        let back: Sex = serde_json::from_str(&json).unwrap();
        assert_eq!(sex, back);
    }

    #[test]
    fn fact_type_custom_is_tagged() {
        let json = serde_json::to_value(FactType::Custom("Emigration".to_owned())).unwrap();
        assert_eq!(json["type"], "Custom");
        assert_eq!(json["value"], "Emigration");
    }

    #[test]
    fn fact_type_rejects_vital_variants() {
        // Vitals (Birth/Death/Baptism/Burial) are asserted as Events with a Primary participant,
        // not Facts (ADR 0021 §2); a legacy vital-tagged fact no longer decodes.
        for tag in ["Birth", "Death", "Baptism", "Burial"] {
            let json = serde_json::json!({ "type": tag });
            assert!(
                serde_json::from_value::<FactType>(json).is_err(),
                "FactType must not decode the vital variant {tag}"
            );
        }
    }

    #[test]
    fn association_role_closed_variant_is_tagged() {
        let json = serde_json::to_value(AssociationRole::Witness).unwrap();
        assert_eq!(json["type"], "Witness");
    }

    #[test]
    fn child_parent_relationship_round_trips() {
        let relationship = ChildParentRelationship::Adopted;
        let json = serde_json::to_string(&relationship).unwrap();
        let back: ChildParentRelationship = serde_json::from_str(&json).unwrap();
        assert_eq!(relationship, back);
    }

    #[test]
    fn place_type_custom_is_tagged() {
        let json = serde_json::to_value(PlaceType::Custom("Sokn".to_owned())).unwrap();
        assert_eq!(json["type"], "Custom");
        assert_eq!(json["value"], "Sokn");
    }

    #[test]
    fn place_type_closed_variant_round_trips() {
        let place_type = PlaceType::Parish;
        let json = serde_json::to_string(&place_type).unwrap();
        let back: PlaceType = serde_json::from_str(&json).unwrap();
        assert_eq!(place_type, back);
    }

    #[test]
    fn event_type_custom_is_tagged() {
        let json = serde_json::to_value(EventType::Custom("Confirmation".to_owned())).unwrap();
        assert_eq!(json["type"], "Custom");
        assert_eq!(json["value"], "Confirmation");
    }

    #[test]
    fn event_type_closed_variant_round_trips() {
        let event_type = EventType::Marriage;
        let json = serde_json::to_string(&event_type).unwrap();
        let back: EventType = serde_json::from_str(&json).unwrap();
        assert_eq!(event_type, back);
    }

    #[test]
    fn gedcom_standard_event_types_are_first_class_not_custom() {
        for event_type in [
            EventType::Christening,
            EventType::Cremation,
            EventType::Adoption,
            EventType::Naturalization,
            EventType::DivorceFiled,
            EventType::MarriageBanns,
        ] {
            let json = serde_json::to_value(&event_type).unwrap();
            assert_ne!(json["type"], "Custom", "{event_type:?} should be a first-class variant");
            let back: EventType = serde_json::from_value(json).unwrap();
            assert_eq!(event_type, back);
        }
    }

    #[test]
    fn gedcom_standard_fact_types_are_first_class() {
        for fact_type in [
            FactType::Caste,
            FactType::PhysicalDescription,
            FactType::Ethnicity,
            FactType::NationalId,
            FactType::NobilityTitle,
        ] {
            let json = serde_json::to_value(&fact_type).unwrap();
            assert_ne!(json["type"], "Custom", "{fact_type:?} should be a first-class variant");
            let back: FactType = serde_json::from_value(json).unwrap();
            assert_eq!(fact_type, back);
        }
    }

    #[test]
    fn full_gedcom_role_set_round_trips_on_both_role_enums() {
        for role in [
            AssociationRole::Father,
            AssociationRole::Husband,
            AssociationRole::Spouse,
            AssociationRole::Multiple,
        ] {
            let back: AssociationRole = serde_json::from_str(&serde_json::to_string(&role).unwrap()).unwrap();
            assert_eq!(role, back);
        }
        for role in [
            ParticipantRole::Clergy,
            ParticipantRole::Friend,
            ParticipantRole::Spouse,
            ParticipantRole::Multiple,
        ] {
            let back: ParticipantRole = serde_json::from_str(&serde_json::to_string(&role).unwrap()).unwrap();
            assert_eq!(role, back);
        }
    }

    #[test]
    fn sex_intersex_is_first_class() {
        let json = serde_json::to_value(Sex::Intersex).unwrap();
        assert_eq!(json["type"], "Intersex");
        let back: Sex = serde_json::from_value(json).unwrap();
        assert_eq!(Sex::Intersex, back);
    }
}
