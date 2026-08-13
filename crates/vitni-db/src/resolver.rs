//! Cross-aggregate reference resolvers backing the aggregates' `cqrs-es` `Services` (ADR 0004 §3).
//!
//! ADR 0004 §3 reserves the `Services` slot for cross-aggregate projection reads — the "aggregate
//! tax" (data-model §9). Each resolver here answers the existence questions another aggregate's
//! pure `decide` needs, returning the engine-neutral `…Refs` value `vitni-core` defines.
//!
//! The *which command carries which cross-aggregate ref* logic is domain knowledge, so it lives
//! here once, engine-neutral. The only engine-specific part — reading a projection to answer "does
//! this view exist?" — is abstracted behind [`RefStore`], implemented once per backend. Each
//! engine's store constructs the resolvers with its own [`RefStore`]; the resolver code is shared.

use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::Pool;
#[cfg(feature = "postgres")]
use sqlx::Postgres;
#[cfg(feature = "sqlite")]
use sqlx::Sqlite;
use tracing::warn;
use vitni_core::citation::command::CitationCommand;
use vitni_core::citation::ref_resolver::{CitationRefResolver, CitationRefs};
use vitni_core::dna_match::command::DnaMatchCommand;
use vitni_core::dna_match::ref_resolver::{DnaMatchRefResolver, DnaMatchRefs};
use vitni_core::dna_test::command::DnaTestCommand;
use vitni_core::dna_test::ref_resolver::{DnaTestRefResolver, DnaTestRefs};
use vitni_core::event::command::EventCommand;
use vitni_core::event::ref_resolver::{EventRefResolver, EventRefs};
use vitni_core::ids::PlaceId;
use vitni_core::place::command::PlaceCommand;
use vitni_core::place::ref_resolver::{PlaceRefResolver, PlaceRefs};
use vitni_core::research_note::command::ResearchNoteCommand;
use vitni_core::research_note::ref_resolver::{ResearchNoteRefResolver, ResearchNoteRefs};
use vitni_core::research_note::subject::SubjectRef;
use vitni_core::source::command::SourceCommand;
use vitni_core::source::ref_resolver::{SourceRefResolver, SourceRefs};

use crate::store::DbError;
use crate::tables::{
    DNA_TEST_VIEW_TABLE, EVENT_VIEW_TABLE, FAMILY_VIEW_TABLE, PERSON_VIEW_TABLE, PLACE_VIEW_TABLE,
    REPOSITORY_VIEW_TABLE, SOURCE_VIEW_TABLE,
};

/// The one engine-specific operation the cross-aggregate resolvers need: does a view row with this
/// primary key exist? Implemented once per backend; the resolver logic above is engine-neutral.
#[async_trait]
pub(crate) trait RefStore: Send + Sync {
    /// Returns whether a view with primary key `view_id` exists in `table`.
    async fn view_exists(&self, table: &str, view_id: &str) -> Result<bool, DbError>;
}

/// Reads `table` for `view_id`, failing **closed**: a (practically impossible) lookup error on an
/// open pool is logged and treated as "absent", so an infrastructure hiccup never silently lets a
/// dangling reference through.
async fn exists_or_absent(store: &dyn RefStore, table: &str, view_id: &str) -> bool {
    match store.view_exists(table, view_id).await {
        Ok(exists) => exists,
        Err(error) => {
            warn!(%error, table, "cross-aggregate existence check failed; treating referent as absent");
            false
        }
    }
}

/// The SQLite-backed [`RefStore`]: existence checks read the conclusion projections over the
/// SQLite read-model pool.
#[cfg(feature = "sqlite")]
pub(crate) struct SqliteRefStore {
    pool: Pool<Sqlite>,
}

#[cfg(feature = "sqlite")]
impl SqliteRefStore {
    /// Wraps the read-model pool the existence checks query, as a shared [`RefStore`].
    pub(crate) fn shared(pool: Pool<Sqlite>) -> Arc<dyn RefStore> {
        Arc::new(Self { pool })
    }
}

#[cfg(feature = "sqlite")]
#[async_trait]
impl RefStore for SqliteRefStore {
    async fn view_exists(&self, table: &str, view_id: &str) -> Result<bool, DbError> {
        crate::sqlite_query::view_exists(&self.pool, table, view_id).await
    }
}

/// The Postgres-backed [`RefStore`]: existence checks read the conclusion projections over the
/// Postgres read-model pool.
#[cfg(feature = "postgres")]
pub(crate) struct PostgresRefStore {
    pool: Pool<Postgres>,
}

#[cfg(feature = "postgres")]
impl PostgresRefStore {
    /// Wraps the read-model pool the existence checks query, as a shared [`RefStore`].
    pub(crate) fn shared(pool: Pool<Postgres>) -> Arc<dyn RefStore> {
        Arc::new(Self { pool })
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl RefStore for PostgresRefStore {
    async fn view_exists(&self, table: &str, view_id: &str) -> Result<bool, DbError> {
        crate::postgres_query::view_exists(&self.pool, table, view_id).await
    }
}

/// Resolves Citation cross-aggregate refs (does the cited `Source` exist?) against the Source
/// projection — the `cqrs-es` `Services` value for the Citation aggregate.
pub(crate) struct CitationRefService {
    store: Arc<dyn RefStore>,
}

impl CitationRefService {
    /// Wraps the backend [`RefStore`] the resolver queries.
    pub(crate) fn new(store: Arc<dyn RefStore>) -> Arc<Self> {
        Arc::new(Self { store })
    }
}

#[async_trait]
impl CitationRefResolver for CitationRefService {
    async fn resolve(&self, command: &CitationCommand) -> CitationRefs {
        let source_exists = match command {
            CitationCommand::CreateCitation { source_id, .. } => {
                exists_or_absent(&*self.store, SOURCE_VIEW_TABLE, &source_id.to_string()).await
            }
            // No cross-aggregate reference to resolve.
            CitationCommand::SetPage { .. }
            | CitationCommand::AssertDate { .. }
            | CitationCommand::SetConfidence { .. }
            | CitationCommand::SetEvidenceAnalysis { .. }
            | CitationCommand::AddAttribute { .. }
            | CitationCommand::AttachMedia { .. }
            | CitationCommand::AttachNote { .. }
            | CitationCommand::Tag { .. }
            | CitationCommand::Untag { .. }
            | CitationCommand::SetRestrictions { .. }
            | CitationCommand::RetractAssertion { .. }
            | CitationCommand::SupersedeAssertion { .. }
            | CitationCommand::SetHumanId { .. } => true,
        };
        CitationRefs { source_exists }
    }
}

/// Resolves Event cross-aggregate refs (does the linked `Place` exist?) against the Place
/// projection — the `cqrs-es` `Services` value for the Event aggregate.
pub(crate) struct EventRefService {
    store: Arc<dyn RefStore>,
}

impl EventRefService {
    /// Wraps the backend [`RefStore`] the resolver queries.
    pub(crate) fn new(store: Arc<dyn RefStore>) -> Arc<Self> {
        Arc::new(Self { store })
    }
}

#[async_trait]
impl EventRefResolver for EventRefService {
    async fn resolve(&self, command: &EventCommand) -> EventRefs {
        let place_exists = match command {
            EventCommand::LinkPlace { place_id, .. } => {
                exists_or_absent(&*self.store, PLACE_VIEW_TABLE, &place_id.to_string()).await
            }
            // No cross-aggregate reference to resolve.
            EventCommand::CreateEvent { .. }
            | EventCommand::SetEventType { .. }
            | EventCommand::AssertDate { .. }
            | EventCommand::SetDescription { .. }
            | EventCommand::AddAddress { .. }
            | EventCommand::AddCitation { .. }
            | EventCommand::AttachMedia { .. }
            | EventCommand::AttachNote { .. }
            | EventCommand::Tag { .. }
            | EventCommand::Untag { .. }
            | EventCommand::SetRestrictions { .. }
            | EventCommand::RetractAssertion { .. }
            | EventCommand::SupersedeAssertion { .. }
            | EventCommand::SetHumanId { .. } => true,
        };
        EventRefs { place_exists }
    }
}

/// Resolves Place cross-aggregate refs (does the enclosing `Place` exist?) against the Place
/// projection — the `cqrs-es` `Services` value for the Place aggregate.
pub(crate) struct PlaceRefService {
    store: Arc<dyn RefStore>,
}

impl PlaceRefService {
    /// Wraps the backend [`RefStore`] the resolver queries.
    pub(crate) fn new(store: Arc<dyn RefStore>) -> Arc<Self> {
        Arc::new(Self { store })
    }
}

#[async_trait]
impl PlaceRefResolver for PlaceRefService {
    async fn resolve(&self, command: &PlaceCommand) -> PlaceRefs {
        let enclosing_exists = match command {
            PlaceCommand::AssertEnclosedBy { enclosed_by, .. } => {
                exists_or_absent(&*self.store, PLACE_VIEW_TABLE, &enclosed_by.place_id.to_string()).await
            }
            // No cross-aggregate reference to resolve.
            PlaceCommand::CreatePlace { .. }
            | PlaceCommand::SetPlaceType { .. }
            | PlaceCommand::AssertName { .. }
            | PlaceCommand::AssertCoordinates { .. }
            | PlaceCommand::AssertGeometry { .. }
            | PlaceCommand::AssertSuccession { .. }
            | PlaceCommand::SetCode { .. }
            | PlaceCommand::AddCitation { .. }
            | PlaceCommand::AttachMedia { .. }
            | PlaceCommand::AttachNote { .. }
            | PlaceCommand::Tag { .. }
            | PlaceCommand::Untag { .. }
            | PlaceCommand::SetRestrictions { .. }
            | PlaceCommand::RetractAssertion { .. }
            | PlaceCommand::SupersedeAssertion { .. }
            | PlaceCommand::SetHumanId { .. } => true,
        };
        let missing_succession_place = match command {
            PlaceCommand::AssertSuccession { from, to, .. } => {
                self.missing_succession_place(from.iter().chain(to.iter())).await
            }
            // No cross-aggregate reference to resolve.
            PlaceCommand::CreatePlace { .. }
            | PlaceCommand::SetPlaceType { .. }
            | PlaceCommand::AssertName { .. }
            | PlaceCommand::AssertEnclosedBy { .. }
            | PlaceCommand::AssertCoordinates { .. }
            | PlaceCommand::AssertGeometry { .. }
            | PlaceCommand::SetCode { .. }
            | PlaceCommand::AddCitation { .. }
            | PlaceCommand::AttachMedia { .. }
            | PlaceCommand::AttachNote { .. }
            | PlaceCommand::Tag { .. }
            | PlaceCommand::Untag { .. }
            | PlaceCommand::SetRestrictions { .. }
            | PlaceCommand::RetractAssertion { .. }
            | PlaceCommand::SupersedeAssertion { .. }
            | PlaceCommand::SetHumanId { .. } => None,
        };
        PlaceRefs {
            enclosing_exists,
            missing_succession_place,
        }
    }
}

impl PlaceRefService {
    /// The first place id in `ids` that does not exist in the Place projection, or `None` if every
    /// one does (ADR 0026 §4, the §9 aggregate-tax check for `AssertSuccession`).
    async fn missing_succession_place<'a>(&self, ids: impl Iterator<Item = &'a PlaceId>) -> Option<PlaceId> {
        for &id in ids {
            if !exists_or_absent(&*self.store, PLACE_VIEW_TABLE, &id.to_string()).await {
                return Some(id);
            }
        }
        None
    }
}

/// Resolves Source cross-aggregate refs (does the linked `Repository` exist?) against the
/// Repository projection — the `cqrs-es` `Services` value for the Source aggregate.
pub(crate) struct SourceRefService {
    store: Arc<dyn RefStore>,
}

impl SourceRefService {
    /// Wraps the backend [`RefStore`] the resolver queries.
    pub(crate) fn new(store: Arc<dyn RefStore>) -> Arc<Self> {
        Arc::new(Self { store })
    }
}

#[async_trait]
impl SourceRefResolver for SourceRefService {
    async fn resolve(&self, command: &SourceCommand) -> SourceRefs {
        let repository_exists = match command {
            SourceCommand::LinkRepository { repo_ref, .. } => {
                exists_or_absent(&*self.store, REPOSITORY_VIEW_TABLE, &repo_ref.repository_id.to_string()).await
            }
            // No cross-aggregate reference to resolve.
            SourceCommand::CreateSource { .. }
            | SourceCommand::SetTitle { .. }
            | SourceCommand::SetAuthor { .. }
            | SourceCommand::SetPubInfo { .. }
            | SourceCommand::SetAbbrev { .. }
            | SourceCommand::AddAttribute { .. }
            | SourceCommand::AttachMedia { .. }
            | SourceCommand::AttachNote { .. }
            | SourceCommand::Tag { .. }
            | SourceCommand::Untag { .. }
            | SourceCommand::SetRestrictions { .. }
            | SourceCommand::RetractAssertion { .. }
            | SourceCommand::SupersedeAssertion { .. }
            | SourceCommand::SetHumanId { .. } => true,
        };
        SourceRefs { repository_exists }
    }
}

/// Resolves `DnaTest` cross-aggregate refs (does the anchoring `Person` exist?) against the Person
/// projection — the `cqrs-es` `Services` value for the `DnaTest` aggregate.
pub(crate) struct DnaTestRefService {
    store: Arc<dyn RefStore>,
}

impl DnaTestRefService {
    /// Wraps the backend [`RefStore`] the resolver queries.
    pub(crate) fn new(store: Arc<dyn RefStore>) -> Arc<Self> {
        Arc::new(Self { store })
    }
}

#[async_trait]
impl DnaTestRefResolver for DnaTestRefService {
    async fn resolve(&self, command: &DnaTestCommand) -> DnaTestRefs {
        let person_exists = match command {
            DnaTestCommand::CreateDnaTest { person_id, .. } => {
                exists_or_absent(&*self.store, PERSON_VIEW_TABLE, &person_id.to_string()).await
            }
            // No cross-aggregate reference to resolve.
            DnaTestCommand::SetProvider { .. }
            | DnaTestCommand::SetKitId { .. }
            | DnaTestCommand::SetTestType { .. }
            | DnaTestCommand::SetGenomeBuild { .. }
            | DnaTestCommand::AssertHaplogroup { .. }
            | DnaTestCommand::AttachNote { .. }
            | DnaTestCommand::Tag { .. }
            | DnaTestCommand::Untag { .. }
            | DnaTestCommand::SetRestrictions { .. }
            | DnaTestCommand::RetractAssertion { .. }
            | DnaTestCommand::SupersedeAssertion { .. }
            | DnaTestCommand::SetHumanId { .. } => true,
        };
        DnaTestRefs { person_exists }
    }
}

/// Resolves `DnaMatch` cross-aggregate refs (do both `DnaTest`s exist?) against the `DnaTest`
/// projection — the `cqrs-es` `Services` value for the `DnaMatch` aggregate.
pub(crate) struct DnaMatchRefService {
    store: Arc<dyn RefStore>,
}

impl DnaMatchRefService {
    /// Wraps the backend [`RefStore`] the resolver queries.
    pub(crate) fn new(store: Arc<dyn RefStore>) -> Arc<Self> {
        Arc::new(Self { store })
    }

    /// Whether a test exists, failing closed on a (practically impossible) lookup error.
    async fn test_exists(&self, test_id: &str) -> bool {
        exists_or_absent(&*self.store, DNA_TEST_VIEW_TABLE, test_id).await
    }
}

#[async_trait]
impl DnaMatchRefResolver for DnaMatchRefService {
    async fn resolve(&self, command: &DnaMatchCommand) -> DnaMatchRefs {
        match command {
            DnaMatchCommand::ObserveMatch { test_a, test_b, .. } => DnaMatchRefs {
                test_a_exists: self.test_exists(&test_a.to_string()).await,
                test_b_exists: self.test_exists(&test_b.to_string()).await,
            },
            // No cross-aggregate reference to resolve.
            DnaMatchCommand::AddSegment { .. }
            | DnaMatchCommand::AssertSharedAncestor { .. }
            | DnaMatchCommand::ConfirmMatch { .. }
            | DnaMatchCommand::RejectMatch { .. }
            | DnaMatchCommand::AttachNote { .. }
            | DnaMatchCommand::Tag { .. }
            | DnaMatchCommand::Untag { .. }
            | DnaMatchCommand::SetRestrictions { .. }
            | DnaMatchCommand::RetractAssertion { .. }
            | DnaMatchCommand::SupersedeAssertion { .. }
            | DnaMatchCommand::SetHumanId { .. } => DnaMatchRefs {
                test_a_exists: true,
                test_b_exists: true,
            },
        }
    }
}

/// Resolves `ResearchNote` cross-aggregate refs (does the named subject exist?) against whichever
/// of the Person/Family/Event/Place projections `SubjectRef` names — the `cqrs-es` `Services` value
/// for the `ResearchNote` aggregate (ADR 0028).
pub(crate) struct ResearchNoteRefService {
    store: Arc<dyn RefStore>,
}

impl ResearchNoteRefService {
    /// Wraps the backend [`RefStore`] the resolver queries.
    pub(crate) fn new(store: Arc<dyn RefStore>) -> Arc<Self> {
        Arc::new(Self { store })
    }

    /// Whether `subject` exists, dispatching to the projection table its kind names.
    async fn subject_exists(&self, subject: SubjectRef) -> bool {
        let (table, id) = match subject {
            SubjectRef::Person(id) => (PERSON_VIEW_TABLE, id.to_string()),
            SubjectRef::Family(id) => (FAMILY_VIEW_TABLE, id.to_string()),
            SubjectRef::Event(id) => (EVENT_VIEW_TABLE, id.to_string()),
            SubjectRef::Place(id) => (PLACE_VIEW_TABLE, id.to_string()),
        };
        exists_or_absent(&*self.store, table, &id).await
    }
}

#[async_trait]
impl ResearchNoteRefResolver for ResearchNoteRefService {
    async fn resolve(&self, command: &ResearchNoteCommand) -> ResearchNoteRefs {
        let mut existing_subjects = BTreeSet::new();
        for subject in subjects_to_check(command) {
            if self.subject_exists(subject).await {
                existing_subjects.insert(subject);
            }
        }
        ResearchNoteRefs { existing_subjects }
    }
}

/// The subjects `command` names that need the aggregate-tax existence check — `CreateResearchNote`'s
/// full set, or `AddSubject`'s single subject, recursing through a `SupersedeAssertion` wrapper so a
/// corrected `AddSubject` still gets checked. Every other command carries no subject to resolve.
fn subjects_to_check(command: &ResearchNoteCommand) -> Vec<SubjectRef> {
    match command {
        ResearchNoteCommand::CreateResearchNote { subjects, .. } => subjects.iter().copied().collect(),
        ResearchNoteCommand::AddSubject { subject, .. } => vec![*subject],
        ResearchNoteCommand::SupersedeAssertion { replacement, .. } => subjects_to_check(replacement),
        ResearchNoteCommand::RemoveSubject { .. }
        | ResearchNoteCommand::SetBody { .. }
        | ResearchNoteCommand::Tag { .. }
        | ResearchNoteCommand::Untag { .. }
        | ResearchNoteCommand::SetRestrictions { .. }
        | ResearchNoteCommand::RetractAssertion { .. } => Vec::new(),
    }
}
