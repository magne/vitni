//! [`PlaceSuccessionAssertion`] — an identity change between `Place` aggregates (data-model §7,
//! ADR 0026).
//!
//! A place's *identity* can change — municipalities merge, counties split, one place is absorbed
//! into or elevated out of another — distinct from a mere rename (a dated `PlaceName` on the same
//! aggregate). A succession links two sets of places by id (self-contained — ADR 0002) and is dated
//! like the other Place assertions; unlike them, it names *other* Place aggregates rather than
//! describing this one.

use serde::{Deserialize, Serialize};

use crate::date::GenealogicalDate;
use crate::enums::SuccessionKind;
use crate::ids::PlaceId;

/// One succession a place took part in, and the date it held (data-model §7, ADR 0026).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceSuccessionAssertion {
    /// The place(s) that ceased (merge: many; split/absorb/elevate/rename: the one place).
    pub from: Vec<PlaceId>,
    /// The place(s) that resulted (split: many; merge/absorb/elevate/rename: the one place).
    pub to: Vec<PlaceId>,
    /// The kind of identity change (cardinality is implied by `from`/`to`'s lengths).
    pub kind: SuccessionKind,
    /// The date this succession took effect, if known.
    pub date: Option<GenealogicalDate>,
}

#[cfg(test)]
mod tests {
    use super::PlaceSuccessionAssertion;
    use crate::enums::SuccessionKind;
    use crate::ids::PlaceId;
    use uuid::Uuid;

    #[test]
    fn succession_assertion_round_trips_through_json() {
        let assertion = PlaceSuccessionAssertion {
            from: vec![
                PlaceId::from_uuid(Uuid::from_u128(1)),
                PlaceId::from_uuid(Uuid::from_u128(2)),
            ],
            to: vec![PlaceId::from_uuid(Uuid::from_u128(3))],
            kind: SuccessionKind::Merged,
            date: None,
        };
        let json = serde_json::to_string(&assertion).unwrap();
        let back: PlaceSuccessionAssertion = serde_json::from_str(&json).unwrap();
        assert_eq!(assertion, back);
    }
}
