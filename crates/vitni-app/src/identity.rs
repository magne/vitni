//! Identity clusters (ADR 0039 §4, §5): the decision use-cases and the read side of every
//! `<Kind>sMerged` — persons, events, families, places, sources, citations, repositories, notes and
//! media.
//!
//! A merge links records rather than rewriting them, so every merged record keeps its own stream.
//! The `identity_links` index names each merged record's cluster root, and this module turns it into
//! the one redirect every reader applies: a member resolves to its root, lists hide members, and a
//! reference to a member names the root ([`References`]).
//!
//! The decisions themselves are written the same way for every kind ([`merge`], [`distinguish`],
//! [`undo_distinction_and_merge`], [`pair_decision`]); a [`ClusterView`] supplies the kind's commands
//! and errors.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::future::Future;
use std::hash::Hash;

use uuid::Uuid;
use vitni_core::citation::{CitationCommand, CitationError, CitationView};
use vitni_core::event::command::EventCommand;
use vitni_core::event::{EventError, EventView};
use vitni_core::family::command::FamilyCommand;
use vitni_core::family::{FamilyError, FamilyView};
use vitni_core::identity::ClusterRecord;
use vitni_core::ids::{
    AssertionId, CitationId, EventId, FamilyId, MediaId, NoteId, PersonId, PlaceId, RepositoryId, SourceId,
};
use vitni_core::matching::MatchEvidence;
use vitni_core::media::{MediaCommand, MediaError, MediaView};
use vitni_core::note::{NoteCommand, NoteError, NoteView};
use vitni_core::person::PersonView;
use vitni_core::person::command::PersonCommand;
use vitni_core::person::error::PersonError;
use vitni_core::place::{PlaceCommand, PlaceError, PlaceView};
use vitni_core::repository::{RepositoryCommand, RepositoryError, RepositoryView};
use vitni_core::source::{SourceCommand, SourceError, SourceView};
use vitni_db::{DbError, Store};

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::Provenance;
use crate::workspace::Workspace;

/// An aggregate id that names a record of a cluster kind, tied to that kind's view.
pub(crate) trait ClusterId: Copy + Eq + Ord + Hash + fmt::Display + fmt::Debug + Send + Sync {
    /// The kind's projection.
    type View: ClusterView<Id = Self>;
}

impl ClusterId for PersonId {
    type View = PersonView;
}

impl ClusterId for EventId {
    type View = EventView;
}

impl ClusterId for FamilyId {
    type View = FamilyView;
}

impl ClusterId for PlaceId {
    type View = PlaceView;
}

impl ClusterId for SourceId {
    type View = SourceView;
}

impl ClusterId for CitationId {
    type View = CitationView;
}

impl ClusterId for RepositoryId {
    type View = RepositoryView;
}

impl ClusterId for NoteId {
    type View = NoteView;
}

impl ClusterId for MediaId {
    type View = MediaView;
}

/// What the decision use-cases and readers need of one cluster kind beyond [`ClusterRecord`]: how to
/// find a record, how to name a failure, and the commands that record a decision.
pub(crate) trait ClusterView: ClusterRecord<Id: ClusterId<View = Self>> + Sized + Send + Sync {
    /// The aggregate type the `human_id` index files this kind under.
    const AGGREGATE: &'static str;

    /// The record with `human_id`, if any.
    fn find(store: &Store, human_id: &str) -> impl Future<Output = Result<Option<Self>, AppError>> + Send;

    /// The error for a `human_id` that names no record of this kind.
    fn not_found(human_id: &str) -> AppError;

    /// The refusal for a pair that already holds a live identity decision.
    fn decided(first: Self::Id, second: Self::Id) -> AppError;

    /// Records the second record of `pair` as the same record as the first, on the first's stream.
    fn merge(
        store: &Store,
        session: &Session,
        pair: [Self::Id; 2],
        decision: IdentityDecision,
    ) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Records the second record of `pair` as different from the first, on the first's stream.
    fn distinguish(
        store: &Store,
        session: &Session,
        pair: [Self::Id; 2],
        decision: IdentityDecision,
    ) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Retracts `target` on `record`'s stream.
    fn retract(
        store: &Store,
        session: &Session,
        record: Self::Id,
        target: AssertionId,
        provenance: Provenance,
    ) -> impl Future<Output = Result<(), AppError>> + Send;
}

impl ClusterView for PersonView {
    const AGGREGATE: &'static str = "person";

    async fn find(store: &Store, human_id: &str) -> Result<Option<Self>, AppError> {
        Ok(store.find_person(human_id).await?)
    }

    fn not_found(human_id: &str) -> AppError {
        AppError::PersonNotFound(human_id.to_owned())
    }

    fn decided(person: PersonId, other: PersonId) -> AppError {
        PersonError::IdentityDecided { person, other }.into()
    }

    async fn merge(
        store: &Store,
        session: &Session,
        [surviving, merged]: [PersonId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = PersonCommand::MergePersons {
            surviving,
            merged,
            assessment: decision.assessment,
        };
        let id = surviving.to_string();
        crate::person::execute_person_command(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn distinguish(
        store: &Store,
        session: &Session,
        [person, other]: [PersonId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = PersonCommand::DistinguishPersons {
            person,
            other,
            assessment: decision.assessment,
        };
        let id = person.to_string();
        crate::person::execute_person_command(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn retract(
        store: &Store,
        session: &Session,
        person_id: PersonId,
        target: AssertionId,
        provenance: Provenance,
    ) -> Result<(), AppError> {
        let command = PersonCommand::RetractAssertion { person_id, target };
        let id = person_id.to_string();
        crate::person::execute_person_command(store, session, &id, command, provenance, Vec::new()).await
    }
}

impl ClusterView for EventView {
    const AGGREGATE: &'static str = "event";

    async fn find(store: &Store, human_id: &str) -> Result<Option<Self>, AppError> {
        Ok(store.find_event(human_id).await?)
    }

    fn not_found(human_id: &str) -> AppError {
        AppError::EventNotFound(human_id.to_owned())
    }

    fn decided(event: EventId, other: EventId) -> AppError {
        EventError::IdentityDecided { event, other }.into()
    }

    async fn merge(
        store: &Store,
        session: &Session,
        [surviving, merged]: [EventId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = EventCommand::MergeEvents {
            surviving,
            merged,
            assessment: decision.assessment,
        };
        let id = surviving.to_string();
        crate::event::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn distinguish(
        store: &Store,
        session: &Session,
        [event, other]: [EventId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = EventCommand::DistinguishEvents {
            event,
            other,
            assessment: decision.assessment,
        };
        let id = event.to_string();
        crate::event::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn retract(
        store: &Store,
        session: &Session,
        event_id: EventId,
        target: AssertionId,
        provenance: Provenance,
    ) -> Result<(), AppError> {
        let command = EventCommand::RetractAssertion { event_id, target };
        let id = event_id.to_string();
        crate::event::execute(store, session, &id, command, provenance, Vec::new()).await
    }
}

impl ClusterView for FamilyView {
    const AGGREGATE: &'static str = "family";

    async fn find(store: &Store, human_id: &str) -> Result<Option<Self>, AppError> {
        Ok(store.find_family(human_id).await?)
    }

    fn not_found(human_id: &str) -> AppError {
        AppError::FamilyNotFound(human_id.to_owned())
    }

    fn decided(family: FamilyId, other: FamilyId) -> AppError {
        FamilyError::IdentityDecided { family, other }.into()
    }

    async fn merge(
        store: &Store,
        session: &Session,
        [surviving, merged]: [FamilyId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = FamilyCommand::MergeFamilies {
            surviving,
            merged,
            assessment: decision.assessment,
        };
        let id = surviving.to_string();
        crate::family::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn distinguish(
        store: &Store,
        session: &Session,
        [family, other]: [FamilyId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = FamilyCommand::DistinguishFamilies {
            family,
            other,
            assessment: decision.assessment,
        };
        let id = family.to_string();
        crate::family::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn retract(
        store: &Store,
        session: &Session,
        family_id: FamilyId,
        target: AssertionId,
        provenance: Provenance,
    ) -> Result<(), AppError> {
        let command = FamilyCommand::RetractAssertion { family_id, target };
        let id = family_id.to_string();
        crate::family::execute(store, session, &id, command, provenance, Vec::new()).await
    }
}

impl ClusterView for PlaceView {
    const AGGREGATE: &'static str = "place";

    async fn find(store: &Store, human_id: &str) -> Result<Option<Self>, AppError> {
        Ok(store.find_place(human_id).await?)
    }

    fn not_found(human_id: &str) -> AppError {
        AppError::PlaceNotFound(human_id.to_owned())
    }

    fn decided(place: PlaceId, other: PlaceId) -> AppError {
        PlaceError::IdentityDecided { place, other }.into()
    }

    async fn merge(
        store: &Store,
        session: &Session,
        [surviving, merged]: [PlaceId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = PlaceCommand::MergePlaces {
            surviving,
            merged,
            assessment: decision.assessment,
        };
        let id = surviving.to_string();
        crate::place::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn distinguish(
        store: &Store,
        session: &Session,
        [place, other]: [PlaceId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = PlaceCommand::DistinguishPlaces {
            place,
            other,
            assessment: decision.assessment,
        };
        let id = place.to_string();
        crate::place::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn retract(
        store: &Store,
        session: &Session,
        place_id: PlaceId,
        target: AssertionId,
        provenance: Provenance,
    ) -> Result<(), AppError> {
        let command = PlaceCommand::RetractAssertion { place_id, target };
        let id = place_id.to_string();
        crate::place::execute(store, session, &id, command, provenance, Vec::new()).await
    }
}

impl ClusterView for SourceView {
    const AGGREGATE: &'static str = "source";

    async fn find(store: &Store, human_id: &str) -> Result<Option<Self>, AppError> {
        Ok(store.find_source(human_id).await?)
    }

    fn not_found(human_id: &str) -> AppError {
        AppError::SourceNotFound(human_id.to_owned())
    }

    fn decided(source: SourceId, other: SourceId) -> AppError {
        SourceError::IdentityDecided {
            source_id: source,
            other,
        }
        .into()
    }

    async fn merge(
        store: &Store,
        session: &Session,
        [surviving, merged]: [SourceId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = SourceCommand::MergeSources {
            surviving,
            merged,
            assessment: decision.assessment,
        };
        let id = surviving.to_string();
        crate::source::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn distinguish(
        store: &Store,
        session: &Session,
        [source, other]: [SourceId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = SourceCommand::DistinguishSources {
            source,
            other,
            assessment: decision.assessment,
        };
        let id = source.to_string();
        crate::source::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn retract(
        store: &Store,
        session: &Session,
        source_id: SourceId,
        target: AssertionId,
        provenance: Provenance,
    ) -> Result<(), AppError> {
        let command = SourceCommand::RetractAssertion { source_id, target };
        let id = source_id.to_string();
        crate::source::execute(store, session, &id, command, provenance, Vec::new()).await
    }
}

impl ClusterView for CitationView {
    const AGGREGATE: &'static str = "citation";

    async fn find(store: &Store, human_id: &str) -> Result<Option<Self>, AppError> {
        Ok(store.find_citation(human_id).await?)
    }

    fn not_found(human_id: &str) -> AppError {
        AppError::CitationNotFound(human_id.to_owned())
    }

    fn decided(citation: CitationId, other: CitationId) -> AppError {
        CitationError::IdentityDecided { citation, other }.into()
    }

    async fn merge(
        store: &Store,
        session: &Session,
        [surviving, merged]: [CitationId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = CitationCommand::MergeCitations {
            surviving,
            merged,
            assessment: decision.assessment,
        };
        let id = surviving.to_string();
        crate::citation::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn distinguish(
        store: &Store,
        session: &Session,
        [citation, other]: [CitationId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = CitationCommand::DistinguishCitations {
            citation,
            other,
            assessment: decision.assessment,
        };
        let id = citation.to_string();
        crate::citation::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn retract(
        store: &Store,
        session: &Session,
        citation_id: CitationId,
        target: AssertionId,
        provenance: Provenance,
    ) -> Result<(), AppError> {
        let command = CitationCommand::RetractAssertion { citation_id, target };
        let id = citation_id.to_string();
        crate::citation::execute(store, session, &id, command, provenance, Vec::new()).await
    }
}

impl ClusterView for RepositoryView {
    const AGGREGATE: &'static str = "repository";

    async fn find(store: &Store, human_id: &str) -> Result<Option<Self>, AppError> {
        Ok(store.find_repository(human_id).await?)
    }

    fn not_found(human_id: &str) -> AppError {
        AppError::RepositoryNotFound(human_id.to_owned())
    }

    fn decided(repository: RepositoryId, other: RepositoryId) -> AppError {
        RepositoryError::IdentityDecided { repository, other }.into()
    }

    async fn merge(
        store: &Store,
        session: &Session,
        [surviving, merged]: [RepositoryId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = RepositoryCommand::MergeRepositories {
            surviving,
            merged,
            assessment: decision.assessment,
        };
        let id = surviving.to_string();
        crate::repository::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn distinguish(
        store: &Store,
        session: &Session,
        [repository, other]: [RepositoryId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = RepositoryCommand::DistinguishRepositories {
            repository,
            other,
            assessment: decision.assessment,
        };
        let id = repository.to_string();
        crate::repository::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn retract(
        store: &Store,
        session: &Session,
        repository_id: RepositoryId,
        target: AssertionId,
        provenance: Provenance,
    ) -> Result<(), AppError> {
        let command = RepositoryCommand::RetractAssertion { repository_id, target };
        let id = repository_id.to_string();
        crate::repository::execute(store, session, &id, command, provenance, Vec::new()).await
    }
}

impl ClusterView for NoteView {
    const AGGREGATE: &'static str = "note";

    async fn find(store: &Store, human_id: &str) -> Result<Option<Self>, AppError> {
        Ok(store.find_note(human_id).await?)
    }

    fn not_found(human_id: &str) -> AppError {
        AppError::NoteNotFound(human_id.to_owned())
    }

    fn decided(note: NoteId, other: NoteId) -> AppError {
        NoteError::IdentityDecided { note, other }.into()
    }

    async fn merge(
        store: &Store,
        session: &Session,
        [surviving, merged]: [NoteId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = NoteCommand::MergeNotes {
            surviving,
            merged,
            assessment: decision.assessment,
        };
        let id = surviving.to_string();
        crate::note::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn distinguish(
        store: &Store,
        session: &Session,
        [note, other]: [NoteId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = NoteCommand::DistinguishNotes {
            note,
            other,
            assessment: decision.assessment,
        };
        let id = note.to_string();
        crate::note::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn retract(
        store: &Store,
        session: &Session,
        note_id: NoteId,
        target: AssertionId,
        provenance: Provenance,
    ) -> Result<(), AppError> {
        let command = NoteCommand::RetractAssertion { note_id, target };
        let id = note_id.to_string();
        crate::note::execute(store, session, &id, command, provenance, Vec::new()).await
    }
}

impl ClusterView for MediaView {
    const AGGREGATE: &'static str = "media";

    async fn find(store: &Store, human_id: &str) -> Result<Option<Self>, AppError> {
        Ok(store.find_media(human_id).await?)
    }

    fn not_found(human_id: &str) -> AppError {
        AppError::MediaNotFound(human_id.to_owned())
    }

    fn decided(media: MediaId, other: MediaId) -> AppError {
        MediaError::IdentityDecided { media, other }.into()
    }

    async fn merge(
        store: &Store,
        session: &Session,
        [surviving, merged]: [MediaId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = MediaCommand::MergeMedia {
            surviving,
            merged,
            assessment: decision.assessment,
        };
        let id = surviving.to_string();
        crate::media::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn distinguish(
        store: &Store,
        session: &Session,
        [media, other]: [MediaId; 2],
        decision: IdentityDecision,
    ) -> Result<(), AppError> {
        let command = MediaCommand::DistinguishMedia {
            media,
            other,
            assessment: decision.assessment,
        };
        let id = media.to_string();
        crate::media::execute(store, session, &id, command, decision.provenance, Vec::new()).await
    }

    async fn retract(
        store: &Store,
        session: &Session,
        media_id: MediaId,
        target: AssertionId,
        provenance: Provenance,
    ) -> Result<(), AppError> {
        let command = MediaCommand::RetractAssertion { media_id, target };
        let id = media_id.to_string();
        crate::media::execute(store, session, &id, command, provenance, Vec::new()).await
    }
}

/// Every cluster of one kind in the workspace, from the `identity_links` index.
#[derive(Debug, Clone)]
pub(crate) struct Clusters<I> {
    root_of: HashMap<I, I>,
    members_of: HashMap<I, Vec<I>>,
}

/// Every person cluster.
pub(crate) type PersonClusters = Clusters<PersonId>;
/// Every event cluster.
pub(crate) type EventClusters = Clusters<EventId>;
/// Every family cluster.
pub(crate) type FamilyClusters = Clusters<FamilyId>;
/// Every place cluster.
pub(crate) type PlaceClusters = Clusters<PlaceId>;
/// Every source cluster.
pub(crate) type SourceClusters = Clusters<SourceId>;
/// Every citation cluster.
pub(crate) type CitationClusters = Clusters<CitationId>;
/// Every repository cluster.
pub(crate) type RepositoryClusters = Clusters<RepositoryId>;
/// Every note cluster.
pub(crate) type NoteClusters = Clusters<NoteId>;
/// Every media cluster.
pub(crate) type MediaClusters = Clusters<MediaId>;

impl<I> Default for Clusters<I> {
    fn default() -> Self {
        Self {
            root_of: HashMap::new(),
            members_of: HashMap::new(),
        }
    }
}

impl<I: ClusterId> Clusters<I> {
    /// Loads the clusters of `I`'s kind.
    ///
    /// # Errors
    ///
    /// A store error, or [`DbError::Malformed`] if the index holds an id that is not a UUID.
    pub(crate) async fn load(store: &Store) -> Result<Self, AppError> {
        let mut root_of = HashMap::new();
        let mut members_of: HashMap<I, Vec<I>> = HashMap::new();
        for link in store.identity_links(I::View::KIND).await? {
            let (member, root) = (parse_id::<I>(&link.member)?, parse_id::<I>(&link.root)?);
            root_of.insert(member, root);
            members_of.entry(root).or_default().push(member);
        }
        Ok(Self { root_of, members_of })
    }

    /// Every merged record with its cluster root.
    pub(crate) fn links(&self) -> impl Iterator<Item = (I, I)> + '_ {
        self.root_of.iter().map(|(member, root)| (*member, *root))
    }

    /// The root of `id`'s cluster: `id` itself unless it is merged.
    pub(crate) fn root(&self, id: I) -> I {
        self.root_of.get(&id).copied().unwrap_or(id)
    }

    /// Whether `id` is merged into another record, and so hidden behind its root.
    pub(crate) fn is_member(&self, id: I) -> bool {
        self.root_of.contains_key(&id)
    }

    /// How many records are merged into another.
    pub(crate) fn member_count(&self) -> usize {
        self.root_of.len()
    }

    /// The members of `root`'s cluster, not including the root, in id order.
    pub(crate) fn members(&self, root: I) -> &[I] {
        self.members_of.get(&root).map_or(&[], Vec::as_slice)
    }

    /// `root` followed by every member of its cluster.
    pub(crate) fn cluster(&self, root: I) -> Vec<I> {
        let mut cluster = vec![root];
        cluster.extend_from_slice(self.members(root));
        cluster
    }

    /// Points every member's entry of `map` at a copy of its root's, so a lookup by a merged record's
    /// id reads its cluster's root (ADR 0039 §5). A member whose root has no entry keeps its own.
    pub(crate) fn redirect<V: Clone>(&self, map: &mut HashMap<I, V>) {
        for (member, root) in self.links() {
            if let Some(entry) = map.get(&root).cloned() {
                map.insert(member, entry);
            }
        }
    }

    /// Moves every member's entries of an inverse index onto its root's, each once, so the records that
    /// use any record of a cluster are listed under its root (ADR 0039 §5).
    pub(crate) fn fold<T: PartialEq>(&self, map: &mut HashMap<I, Vec<T>>) {
        self.fold_by(map, |held, entry| held == entry);
    }

    /// [`fold`](Self::fold), with `same` deciding when an entry is already held — for entries that name
    /// a record of another cluster by its root but keep their own copy's label.
    pub(crate) fn fold_by<T>(&self, map: &mut HashMap<I, Vec<T>>, same: impl Fn(&T, &T) -> bool) {
        let mut links: Vec<(I, I)> = self.links().collect();
        links.sort_unstable();
        for (member, root) in links {
            let Some(entries) = map.remove(&member) else { continue };
            let held = map.entry(root).or_default();
            for entry in entries {
                if !held.iter().any(|existing| same(existing, &entry)) {
                    held.push(entry);
                }
            }
        }
    }
}

/// Parses an aggregate id read from the identity index.
fn parse_id<I: ClusterId>(raw: &str) -> Result<I, AppError> {
    let uuid = Uuid::parse_str(raw).map_err(|e| {
        DbError::Malformed(format!(
            "{} id {raw:?} in identity index: {e}",
            <I::View as ClusterView>::AGGREGATE
        ))
    })?;
    Ok(I::View::id_from_uuid(uuid))
}

/// Groups `views` into clusters in list order (ADR 0039 §5): one group per record that is not merged
/// into another, its own view first, then its members' views in id order.
pub(crate) fn group_clusters<'a, V: ClusterView>(views: &'a [V], clusters: &Clusters<V::Id>) -> Vec<Vec<&'a V>> {
    let mut by_id: HashMap<V::Id, &V> = HashMap::new();
    for view in views {
        if let Some(id) = view.record_id() {
            by_id.insert(id, view);
        }
    }
    let mut groups = Vec::new();
    for view in views {
        let Some(id) = view.record_id() else { continue };
        if clusters.is_member(id) {
            continue;
        }
        let mut group = Vec::new();
        for record in clusters.cluster(id) {
            if let Some(view) = by_id.get(&record) {
                group.push(*view);
            }
        }
        groups.push(group);
    }
    groups
}

/// The projections of every record in `ids`, skipping any not found.
///
/// # Errors
///
/// A store error.
pub(crate) async fn views<I: ClusterId>(store: &Store, ids: &[I]) -> Result<Vec<I::View>, AppError> {
    let mut views = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(human_id) = store.human_id_of(I::View::AGGREGATE, &id.to_string()).await? else {
            continue;
        };
        if let Some(view) = I::View::find(store, &human_id).await? {
            views.push(view);
        }
    }
    Ok(views)
}

/// Every record of `id`'s cluster, its root first.
///
/// # Errors
///
/// A store error.
pub(crate) async fn cluster_records<I: ClusterId>(store: &Store, id: I) -> Result<Vec<I::View>, AppError> {
    let clusters = Clusters::<I>::load(store).await?;
    views(store, &clusters.cluster(clusters.root(id))).await
}

/// The `human_id` of the record of `human_id`'s cluster whose stream holds the live assertion
/// `assertion_id` — where an edit or retraction of that row is written (ADR 0039 §5). The cluster's
/// root when no record holds it, so the correction is refused there with the core's own error.
///
/// # Errors
///
/// `V`'s not-found error if `human_id` is unknown, [`AppError::Db`] if `assertion_id` is not a UUID,
/// or a store error.
pub(crate) async fn claim_owner<V: ClusterView>(
    workspace: &Workspace,
    human_id: &str,
    assertion_id: &str,
) -> Result<String, AppError> {
    let store = workspace.store();
    let id = resolve::<V>(store, human_id).await?;
    let target = crate::use_case::parse_assertion_id(assertion_id)?;
    let records = cluster_records(store, id).await?;
    let holder = records
        .iter()
        .find(|record| record.holds_assertion(target))
        .or_else(|| records.first());
    Ok(holder
        .and_then(ClusterRecord::record_human_id)
        .map_or_else(|| human_id.to_owned(), |id| id.as_str().to_owned()))
}

/// How a reference names each record of one kind: a member by its root's id and `human_id`, every
/// other record by its own (ADR 0039 §5).
pub(crate) struct References<I> {
    clusters: Clusters<I>,
    human_ids: HashMap<I, String>,
}

/// How a reference names each person.
pub(crate) type PersonReferences = References<PersonId>;
/// How a reference names each event.
pub(crate) type EventReferences = References<EventId>;
/// How a reference names each family.
pub(crate) type FamilyReferences = References<FamilyId>;
/// How a reference names each place.
pub(crate) type PlaceReferences = References<PlaceId>;
/// How a reference names each source.
pub(crate) type SourceReferences = References<SourceId>;
/// How a reference names each citation.
pub(crate) type CitationReferences = References<CitationId>;
/// How a reference names each repository.
pub(crate) type RepositoryReferences = References<RepositoryId>;
/// How a reference names each note.
pub(crate) type NoteReferences = References<NoteId>;
/// How a reference names each media object.
pub(crate) type MediaReferences = References<MediaId>;

impl<I: ClusterId> References<I> {
    /// Loads the clusters and every record's `human_id`.
    ///
    /// # Errors
    ///
    /// A store error.
    pub(crate) async fn load(store: &Store) -> Result<Self, AppError> {
        let mut human_ids = HashMap::new();
        for (id, human_id) in store.human_id_index(I::View::AGGREGATE).await? {
            human_ids.insert(parse_id::<I>(&id)?, human_id);
        }
        Ok(Self {
            clusters: Clusters::load(store).await?,
            human_ids,
        })
    }

    /// The id and `human_id` a reference to `id` names: its cluster root's.
    pub(crate) fn resolve(&self, id: I) -> (I, String) {
        let root = self.clusters.root(id);
        let human_id = self.human_ids.get(&root).cloned().unwrap_or_else(|| root.to_string());
        (root, human_id)
    }
}

/// The user's identity decision about a pair (ADR 0039 §1, §2): their surety and reason, and the
/// matching engine's assessment they decided on. Nothing is defaulted — a decision made without a
/// judgment records none, and one made without the engine carries no assessment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdentityDecision {
    /// The operator's confidence and rationale.
    pub provenance: Provenance,
    /// The engine's assessment of the pair as the user was shown it.
    pub assessment: Option<MatchEvidence>,
}

/// The live identity decision between two records' clusters (ADR 0039 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairDecision {
    /// Both records are in one cluster: a merge joined them.
    SameCluster,
    /// A record of one cluster is held distinct from a record of the other.
    Distinct,
}

/// A pair about to be decided, resolved to its cluster roots (ADR 0039 §4).
pub(crate) struct DecidablePair<V: ClusterView> {
    /// The first record as named.
    first_id: V::Id,
    /// The second record as named.
    second_id: V::Id,
    /// The first record's cluster root — the stream the decision is written on.
    pub(crate) first: V::Id,
    /// The first root's `human_id`.
    pub(crate) first_human_id: String,
    /// The second record's cluster root.
    second: V::Id,
    /// Every record of the first cluster.
    first_views: Vec<V>,
    /// Every record of the second cluster.
    second_views: Vec<V>,
}

impl<V: ClusterView> DecidablePair<V> {
    /// The refusal for a pair already decided.
    fn decided(&self) -> AppError {
        V::decided(self.first, self.second)
    }

    /// Every live distinction between the two clusters, with the record whose stream holds it — the
    /// rule that distinctness is judged between clusters (ADR 0039 §4).
    fn distinctions(&self) -> Vec<(V::Id, AssertionId)> {
        let ids = |views: &[V]| -> BTreeSet<V::Id> { views.iter().filter_map(ClusterRecord::record_id).collect() };
        let (first_ids, second_ids) = (ids(&self.first_views), ids(&self.second_views));
        let mut found = Vec::new();
        for (views, others) in [(&self.first_views, &second_ids), (&self.second_views, &first_ids)] {
            for view in views {
                let Some(holder) = view.record_id() else { continue };
                for distinction in view.distinguished_with_assertions() {
                    if others.contains(&distinction.value) {
                        found.push((holder, distinction.assertion_id));
                    }
                }
            }
        }
        found
    }
}

/// Merges `merged_human_id`'s cluster into `surviving_human_id`'s, recording a same-as link, and
/// returns the pair as resolved — its first root is the survivor.
///
/// Both records resolve to their cluster roots first (ADR 0039 §4), so a record already merged
/// elsewhere brings its whole cluster along and no record ends up in two clusters. One merge event is
/// emitted on the surviving root's stream, carrying the decision's provenance and assessment.
///
/// # Errors
///
/// `V`'s not-found error if either `human_id` does not resolve; its domain refusal if they resolve
/// to the same record, are already one cluster, or a record of one cluster is distinguished from a
/// record of the other; or a store error.
pub(crate) async fn merge<V: ClusterView>(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<DecidablePair<V>, AppError> {
    let store = workspace.store();
    let pair = decidable_pair::<V>(store, surviving_human_id, merged_human_id).await?;
    if !pair.distinctions().is_empty() {
        return Err(pair.decided());
    }
    V::merge(store, session, [pair.first, pair.second], decision).await?;
    Ok(pair)
}

/// Undoes every live distinction between the two clusters, then merges them — the compare view's
/// *Undo "not the same" and merge* (ADR 0039 §4). The retractions and the merge are written in
/// sequence; each is its own assertion, so the history shows the undo before the merge.
///
/// # Errors
///
/// As [`merge`], except that a distinction between the clusters no longer refuses the merge.
pub(crate) async fn undo_distinction_and_merge<V: ClusterView>(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<DecidablePair<V>, AppError> {
    let store = workspace.store();
    let pair = decidable_pair::<V>(store, surviving_human_id, merged_human_id).await?;
    for (holder, target) in pair.distinctions() {
        V::retract(store, session, holder, target, decision.provenance.clone()).await?;
    }
    V::merge(store, session, [pair.first, pair.second], decision).await?;
    Ok(pair)
}

/// Records that the two records are different (ADR 0039 §1), so neither cluster is proposed as a
/// duplicate of the other again. Both records resolve to their cluster roots first, and one
/// distinguish event is emitted on the first root's stream.
///
/// # Errors
///
/// As [`merge`], with the kind's "distinguished from itself" refusal for a record named twice.
pub(crate) async fn distinguish<V: ClusterView>(
    workspace: &Workspace,
    session: &Session,
    first_human_id: &str,
    other_human_id: &str,
    decision: IdentityDecision,
) -> Result<(), AppError> {
    let store = workspace.store();
    let pair = decidable_pair::<V>(store, first_human_id, other_human_id).await?;
    if !pair.distinctions().is_empty() {
        return Err(pair.decided());
    }
    V::distinguish(store, session, [pair.first, pair.second], decision).await
}

/// The live identity decision between the clusters of `first_human_id` and `other_human_id`, or `None`
/// when the pair is undecided — what the compare view shows before the user decides again (ADR 0039
/// §4). A record compared with itself is undecided.
///
/// # Errors
///
/// `V`'s not-found error if either `human_id` does not resolve, or a store error.
pub(crate) async fn pair_decision<V: ClusterView>(
    workspace: &Workspace,
    first_human_id: &str,
    other_human_id: &str,
) -> Result<Option<PairDecision>, AppError> {
    let pair = cluster_pair::<V>(workspace.store(), first_human_id, other_human_id).await?;
    if pair.first_id == pair.second_id {
        return Ok(None);
    }
    if pair.first == pair.second {
        return Ok(Some(PairDecision::SameCluster));
    }
    Ok((!pair.distinctions().is_empty()).then_some(PairDecision::Distinct))
}

/// Resolves a pair about to be decided to its cluster roots, refusing it when both are already one
/// cluster — checked against the lagging `identity_links` projection per ADR 0002. A record decided
/// about itself passes through, for the core to refuse.
async fn decidable_pair<V: ClusterView>(
    store: &Store,
    first: &str,
    second: &str,
) -> Result<DecidablePair<V>, AppError> {
    let pair = cluster_pair::<V>(store, first, second).await?;
    if pair.first_id != pair.second_id && pair.first == pair.second {
        return Err(V::decided(pair.first_id, pair.second_id));
    }
    Ok(pair)
}

/// Resolves two records to their cluster roots and reads every record of both clusters.
async fn cluster_pair<V: ClusterView>(store: &Store, first: &str, second: &str) -> Result<DecidablePair<V>, AppError> {
    let first_id = resolve::<V>(store, first).await?;
    let second_id = resolve::<V>(store, second).await?;
    let clusters = Clusters::<V::Id>::load(store).await?;
    let (first_root, second_root) = (clusters.root(first_id), clusters.root(second_id));
    let first_human_id = store
        .human_id_of(V::AGGREGATE, &first_root.to_string())
        .await?
        .ok_or_else(|| V::not_found(first))?;
    let first_views = views(store, &clusters.cluster(first_root)).await?;
    let second_views = if second_root == first_root {
        Vec::new()
    } else {
        views(store, &clusters.cluster(second_root)).await?
    };
    Ok(DecidablePair {
        first_id,
        second_id,
        first: first_root,
        first_human_id,
        second: second_root,
        first_views,
        second_views,
    })
}

/// Resolves a `human_id` to its aggregate id, or `V`'s not-found error.
async fn resolve<V: ClusterView>(store: &Store, human_id: &str) -> Result<V::Id, AppError> {
    V::find(store, human_id)
        .await?
        .and_then(|view| view.record_id())
        .ok_or_else(|| V::not_found(human_id))
}
