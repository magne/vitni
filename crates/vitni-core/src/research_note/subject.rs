//! [`SubjectRef`] — one conclusion-bearing entity a `ResearchNote` argues about (ADR 0028 §2).
//!
//! Deliberately narrower than the general `EvidenceRef` union (ADR 0023: `Citation`/`DnaMatch`, the
//! things an assertion *cites*): a `SubjectRef` names what an argument is *about*, and only the four
//! conclusion-bearing aggregates — Person, Family, Event, Place — are subjects a proof argument
//! concludes about (Source/Citation/Repository/Media/Note/Tag/DnaTest/DnaMatch are not conclusions).

use serde::{Deserialize, Serialize};

use crate::ids::{EventId, FamilyId, PersonId, PlaceId};

/// One conclusion-bearing entity a `ResearchNote` is written about (ADR 0028 §2). A note names a
/// non-empty *set* of these (`ResearchNoteState::subjects`) — one analysis commonly resolves several
/// conclusions at once (e.g. "these two records are the same person"). `Ord` orders a `BTreeSet` of
/// them deterministically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SubjectRef {
    /// A Person aggregate.
    Person(PersonId),
    /// A Family aggregate.
    Family(FamilyId),
    /// An Event aggregate.
    Event(EventId),
    /// A Place aggregate.
    Place(PlaceId),
}
