//! Place events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! Each event carries its [`AssertionId`] and [`EventContext`] in the payload (ADR 0004 §1–§2);
//! the body is internally tagged (`type`) and flattened, so a stored event is one flat JSON object
//! (ADR 0004 §4).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Envelope, EventBody};
use crate::enums::{PlaceType, Restriction};
use crate::geo::GeoCoordinates;
use crate::ids::{AssertionId, CitationId, HumanId, NoteId, PlaceId, TagId};
use crate::place_name::PlaceName;
use crate::place_ref::PlaceRef;
use crate::text::MediaRef;

/// A single Place assertion plus its provenance envelope (ADR 0004 §1).
pub type PlaceEvent = Envelope<PlaceEventBody>;

/// The Place claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlaceEventBody {
    /// A place aggregate was created.
    PlaceCreated {
        /// The created place.
        place_id: PlaceId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The place's type.
        place_type: PlaceType,
    },
    /// The place's type was set / changed.
    PlaceTypeSet {
        /// The place.
        place_id: PlaceId,
        /// The new place type.
        place_type: PlaceType,
    },
    /// A name was asserted for the place.
    NameAsserted {
        /// The place the name belongs to.
        place_id: PlaceId,
        /// The asserted name.
        name: PlaceName,
    },
    /// An enclosing-place relationship was asserted (dated).
    EnclosedByAsserted {
        /// The enclosed place.
        place_id: PlaceId,
        /// The enclosing place and the date the enclosure held.
        enclosed_by: PlaceRef,
    },
    /// The place's coordinates were asserted.
    CoordinatesAsserted {
        /// The place.
        place_id: PlaceId,
        /// The coordinates.
        coordinates: GeoCoordinates,
    },
    /// The place's code was set / changed.
    CodeSet {
        /// The place.
        place_id: PlaceId,
        /// The code.
        code: String,
    },
    /// A citation was added to the place.
    CitationAdded {
        /// The place.
        place_id: PlaceId,
        /// The added citation.
        citation_id: CitationId,
    },
    /// A media reference was attached to the place.
    MediaAttached {
        /// The place.
        place_id: PlaceId,
        /// The media reference.
        media: MediaRef,
    },
    /// A note was attached to the place.
    NoteAttached {
        /// The place.
        place_id: PlaceId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the place.
    Tagged {
        /// The place.
        place_id: PlaceId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the place.
    Untagged {
        /// The place.
        place_id: PlaceId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The place's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The place.
        place_id: PlaceId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The place.
        place_id: PlaceId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The place.
        place_id: PlaceId,
        /// The assertion being superseded.
        target: AssertionId,
    },
    /// The place's user-facing identifier was changed (data-model §7).
    HumanIdChanged {
        /// The place.
        place_id: PlaceId,
        /// The new user-facing identifier.
        human_id: HumanId,
        /// The identifier in effect before this change (for the audit trail).
        old_human_id: HumanId,
    },
}

impl EventBody for PlaceEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::PlaceCreated { .. } => "PlaceCreated",
            Self::PlaceTypeSet { .. } => "PlaceTypeSet",
            Self::NameAsserted { .. } => "NameAsserted",
            Self::EnclosedByAsserted { .. } => "EnclosedByAsserted",
            Self::CoordinatesAsserted { .. } => "CoordinatesAsserted",
            Self::CodeSet { .. } => "CodeSet",
            Self::CitationAdded { .. } => "CitationAdded",
            Self::MediaAttached { .. } => "MediaAttached",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::RestrictionsChanged { .. } => "RestrictionsChanged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
            Self::HumanIdChanged { .. } => "HumanIdChanged",
        }
    }

    fn version(&self) -> &'static str {
        // Per-variant; bumped only on a payload change (ADR 0004 §4).
        // `MediaAttached` is "2.0" after `MediaRef.citations` widened to `EvidenceRef` (ADR 0023), no upcaster.
        match self {
            Self::MediaAttached { .. } => "2.0",
            _ => "1.0",
        }
    }
}
