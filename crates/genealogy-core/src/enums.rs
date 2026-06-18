//! Enumerated type sets shared across aggregates (data-model §7).
//!
//! Each is a closed enum **plus a `Custom(String)` escape hatch**, mirroring Gramps' "custom
//! type" pattern: the common cases are coded and language-neutral, but the model never blocks an
//! unanticipated value. Human-readable labels are a UI concern (data-model §14), never stored.
//!
//! Only the sets the Person aggregate needs are defined here; the remaining sets
//! (`PlaceType`, `RepositoryType`, `SourceMediaType`, `AttributeType`, …) follow the same shape
//! and are added with their aggregates.

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
    /// Another recorded value (e.g. intersex).
    Other(String),
}

/// Whether a `Person` is a single-source persona or a synthesised conclusion (data-model §6, §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceLevel {
    /// Extracted from a single source (GEDCOM X persona).
    Persona,
    /// A researcher's synthesis across evidence.
    Conclusion,
}

/// The kind of single-person fact (closed set plus a custom escape — data-model §7, §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum FactType {
    /// Birth.
    Birth,
    /// Death.
    Death,
    /// Baptism / christening.
    Baptism,
    /// Burial.
    Burial,
    /// Occupation.
    Occupation,
    /// Residence.
    Residence,
    /// Religion.
    Religion,
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
    /// An application-defined association.
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::{AssociationRole, ChildParentRelationship, FactType, Sex};

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
}
