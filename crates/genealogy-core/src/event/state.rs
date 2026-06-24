//! [`EventState`] — the folded aggregate state used by the decision core.
//!
//! Asserted fields (type, date, linked place) are kept attributed to the [`AssertionId`] that
//! introduced them, so a retraction or supersession can remove exactly the right entry
//! (data-model §10).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::address::Address;
use crate::assertions::{Asserted, Attributed};
use crate::date::GenealogicalDate;
use crate::enums::{EventType, ParticipantRole, Restriction};
use crate::ids::{AssertionId, CitationId, EventId, HumanId, NoteId, PersonId, PlaceId, TagId};
use crate::text::MediaRef;

/// One person's participation in an event, with their role (data-model §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventParticipant {
    /// The participating person.
    pub participant_id: PersonId,
    /// The participant's role.
    pub role: ParticipantRole,
}

/// The folded state of an Event aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventState {
    /// Whether `EventCreated` has been seen.
    pub exists: bool,
    /// The event's id (set on creation).
    pub event_id: Option<EventId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The kind of event (last writer wins), with its provenance.
    pub event_type: Option<Attributed<Asserted<EventType>>>,
    /// When the event occurred (last writer wins), with its provenance.
    pub date: Option<Attributed<Asserted<GenealogicalDate>>>,
    /// The event's free-text description (last writer wins), with its provenance.
    pub description: Option<Attributed<Asserted<String>>>,
    /// Where the event occurred (last writer wins), with its provenance.
    pub place_id: Option<Attributed<Asserted<PlaceId>>>,
    /// All currently-live postal addresses, in assertion order.
    pub addresses: Vec<Attributed<Address>>,
    /// The event's participants, in assertion order, each with its provenance.
    pub participants: Vec<Attributed<Asserted<EventParticipant>>>,
    /// All currently-live citations backing the event's claims.
    pub citations: Vec<Attributed<CitationId>>,
    /// All currently-live attached media.
    pub media: Vec<Attributed<MediaRef>>,
    /// All currently-live attached notes.
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// The event's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl EventState {
    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// This is the non-destructive-correction fold: the *event log* keeps the original assertion
    /// forever, but the derived state no longer reflects the retracted claim.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        if self.event_type.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.event_type = None;
        }
        if self.date.as_ref().is_some_and(|d| d.assertion_id == target) {
            self.date = None;
        }
        if self.place_id.as_ref().is_some_and(|p| p.assertion_id == target) {
            self.place_id = None;
        }
        if self.description.as_ref().is_some_and(|d| d.assertion_id == target) {
            self.description = None;
        }
        self.addresses.retain(|a| a.assertion_id != target);
        self.participants.retain(|p| p.assertion_id != target);
        self.citations.retain(|c| c.assertion_id != target);
        self.media.retain(|m| m.assertion_id != target);
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        self.live_assertions.remove(&target);
    }
}
