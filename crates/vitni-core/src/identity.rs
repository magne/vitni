//! What a record that can be merged exposes to the identity cluster index and its readers (ADR 0039 §4).
//!
//! Person, Event and Family carry the same pair of identity decisions — `<Kind>sMerged` and
//! `<Kind>sDistinguished` — folded into the same two sets on the survivor's projection. This trait is
//! that shared shape, so the index and the cluster read side are written once over every kind.

use std::fmt;
use std::hash::Hash;

use uuid::Uuid;

use crate::assertions::Attributed;
use crate::event::EventView;
use crate::family::FamilyView;
use crate::ids::{AssertionId, EventId, FamilyId, HumanId, PersonId};
use crate::matching::MatchableKind;
use crate::person::PersonView;

/// A projection whose record can be merged into, or distinguished from, another of its kind.
pub trait ClusterRecord {
    /// The record's aggregate id.
    type Id: Copy + Eq + Ord + Hash + fmt::Display + fmt::Debug + Send + Sync;

    /// The kind the identity index files this record under.
    const KIND: MatchableKind;

    /// Wraps a UUID read back from the index as this kind's id.
    fn id_from_uuid(uuid: Uuid) -> Self::Id;

    /// The record's id, once created.
    fn record_id(&self) -> Option<Self::Id>;

    /// The record's user-facing identifier, once created.
    fn record_human_id(&self) -> Option<&HumanId>;

    /// The records currently merged into this one.
    fn merged(&self) -> Vec<Self::Id>;

    /// The live distinctions, each paired with the assertion that recorded it — the target an undo
    /// retracts.
    fn distinguished_with_assertions(&self) -> &[Attributed<Self::Id>];

    /// Whether `assertion` is live on this record's stream.
    fn holds_assertion(&self, assertion: AssertionId) -> bool;
}

/// Implements [`ClusterRecord`] for a view by delegating to its inherent accessors.
macro_rules! cluster_record {
    ($view:ty, $id:ty, $kind:expr, $record_id:ident) => {
        impl ClusterRecord for $view {
            type Id = $id;

            const KIND: MatchableKind = $kind;

            fn id_from_uuid(uuid: Uuid) -> Self::Id {
                <$id>::from_uuid(uuid)
            }

            fn record_id(&self) -> Option<Self::Id> {
                self.$record_id()
            }

            fn record_human_id(&self) -> Option<&HumanId> {
                self.human_id()
            }

            fn merged(&self) -> Vec<Self::Id> {
                <$view>::merged(self)
            }

            fn distinguished_with_assertions(&self) -> &[Attributed<Self::Id>] {
                <$view>::distinguished_with_assertions(self)
            }

            fn holds_assertion(&self, assertion: AssertionId) -> bool {
                <$view>::holds_assertion(self, assertion)
            }
        }
    };
}

cluster_record!(PersonView, PersonId, MatchableKind::Person, person_id);
cluster_record!(EventView, EventId, MatchableKind::Event, event_id);
cluster_record!(FamilyView, FamilyId, MatchableKind::Family, family_id);
