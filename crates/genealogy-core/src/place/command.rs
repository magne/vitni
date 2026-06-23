//! Place commands — imperative operator intent (data-model §10).
//!
//! Like every aggregate, commands carry no clock or generated id; the application layer pairs one
//! with an [`AssertionMeta`] in a [`PlaceCommandEnvelope`] before the pure `decide` runs
//! (ADR 0004 §3).

use std::collections::BTreeSet;

use crate::enums::{PlaceType, Restriction};
use crate::geo::GeoCoordinates;
use crate::ids::{AssertionId, CitationId, HumanId, NoteId, PlaceId, TagId};
use crate::place_name::PlaceName;
use crate::place_ref::PlaceRef;
use crate::provenance::AssertionMeta;
use crate::text::MediaRef;

/// Operator intent against a Place aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceCommand {
    /// Create a new place.
    CreatePlace {
        /// The application-generated id for the new place.
        place_id: PlaceId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The place's type.
        place_type: PlaceType,
    },
    /// Set (or change) the place's type.
    SetPlaceType {
        /// The target place.
        place_id: PlaceId,
        /// The new place type.
        place_type: PlaceType,
    },
    /// Assert a name for the place.
    AssertName {
        /// The target place.
        place_id: PlaceId,
        /// The name to assert.
        name: PlaceName,
    },
    /// Assert the place this place is enclosed by, for a dated period.
    AssertEnclosedBy {
        /// The target (enclosed) place.
        place_id: PlaceId,
        /// The enclosing place and the date the enclosure held.
        enclosed_by: PlaceRef,
    },
    /// Assert the place's geographic coordinates.
    AssertCoordinates {
        /// The target place.
        place_id: PlaceId,
        /// The coordinates.
        coordinates: GeoCoordinates,
    },
    /// Set (or change) the place's code (a postal / administrative code).
    SetCode {
        /// The target place.
        place_id: PlaceId,
        /// The code.
        code: String,
    },
    /// Add a citation backing the place's claims.
    AddCitation {
        /// The target place.
        place_id: PlaceId,
        /// The citation to add.
        citation_id: CitationId,
    },
    /// Attach a media reference to the place.
    AttachMedia {
        /// The target place.
        place_id: PlaceId,
        /// The media reference.
        media: MediaRef,
    },
    /// Attach a note to the place.
    AttachNote {
        /// The target place.
        place_id: PlaceId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Apply a tag to the place.
    Tag {
        /// The target place.
        place_id: PlaceId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the place.
    Untag {
        /// The target place.
        place_id: PlaceId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Set (or change) the place's privacy restrictions (GEDCOM `RESN` — data-model §6).
    SetRestrictions {
        /// The target place.
        place_id: PlaceId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target place.
        place_id: PlaceId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target place.
        place_id: PlaceId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<PlaceCommand>,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Place aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: PlaceCommand,
}
