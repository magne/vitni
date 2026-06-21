//! [`PlaceRef`] — a dated "enclosed-by" link between two places (data-model §7, §9).
//!
//! A place's enclosing jurisdiction changes over time (a farm moves between parishes; a parish is
//! reassigned to a new municipality), so the enclosure link carries the date it held — the same
//! evidence-aware shape used elsewhere. The link is an id, never an embedded place (ADR 0002
//! self-contained events).

use serde::{Deserialize, Serialize};

use crate::date::GenealogicalDate;
use crate::ids::PlaceId;

/// A reference to an enclosing place, with the date the enclosure held (data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceRef {
    /// The enclosing place.
    pub place_id: PlaceId,
    /// The date this enclosure held, if known.
    pub date: Option<GenealogicalDate>,
}

#[cfg(test)]
mod tests {
    use super::PlaceRef;
    use crate::ids::PlaceId;
    use uuid::Uuid;

    #[test]
    fn place_ref_round_trips_through_json() {
        let reference = PlaceRef {
            place_id: PlaceId::from_uuid(Uuid::from_u128(0x42)),
            date: None,
        };
        let json = serde_json::to_string(&reference).unwrap();
        let back: PlaceRef = serde_json::from_str(&json).unwrap();
        assert_eq!(reference, back);
    }
}
