//! [`Fact`] — a claimed single-person characteristic (data-model §7, §15).
//!
//! A `Fact` is the payload of `FactAsserted` and shapes the Person `facts` list. It is distinct
//! from a full `Event` aggregate: a `Fact` is a single-person attribute (birth, death,
//! occupation, residence, …), whereas an `Event` is shared between participants.

use serde::{Deserialize, Serialize};

use crate::date::GenealogicalDate;
use crate::enums::FactType;
use crate::ids::PlaceId;
use crate::provenance::CitationRef;

/// A claimed characteristic or event-like attribute of a single person (data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    /// The kind of fact (birth, death, occupation, …).
    pub fact_type: FactType,
    /// When the fact occurred / applied, if dated.
    pub date: Option<GenealogicalDate>,
    /// Where the fact occurred, if placed.
    pub place_id: Option<PlaceId>,
    /// A free-text value (e.g. an occupation title).
    pub value: Option<String>,
    /// Citations backing this fact.
    pub citations: Vec<CitationRef>,
}

#[cfg(test)]
mod tests {
    use super::Fact;
    use crate::enums::FactType;

    #[test]
    fn occupation_fact_round_trips_through_json() {
        let fact = Fact {
            fact_type: FactType::Occupation,
            date: None,
            place_id: None,
            value: Some("mathematician".to_owned()),
            citations: Vec::new(),
        };
        let json = serde_json::to_string(&fact).unwrap();
        let back: Fact = serde_json::from_str(&json).unwrap();
        assert_eq!(fact, back);
    }
}
