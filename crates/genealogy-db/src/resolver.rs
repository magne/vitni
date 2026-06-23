//! Cross-aggregate reference resolvers backing the aggregates' `cqrs-es` `Services` (ADR 0004 §3).
//!
//! ADR 0004 §3 reserves the `Services` slot for cross-aggregate projection reads — the "aggregate
//! tax" (data-model §9). Each resolver here answers the existence questions another aggregate's
//! pure `decide` needs, returning the engine-neutral `…Refs` value `genealogy-core` defines.
//!
//! The *which command carries which cross-aggregate ref* logic is domain knowledge, so it lives
//! here once, engine-neutral. The only engine-specific part — reading a projection to answer "does
//! this view exist?" — is abstracted behind [`RefStore`], implemented once per backend. Each
//! engine's store constructs the resolvers with its own [`RefStore`]; the resolver code is shared.

use std::sync::Arc;

use async_trait::async_trait;
use genealogy_core::citation::command::CitationCommand;
use genealogy_core::citation::ref_resolver::{CitationRefResolver, CitationRefs};
use genealogy_core::dna_match::command::DnaMatchCommand;
use genealogy_core::dna_match::ref_resolver::{DnaMatchRefResolver, DnaMatchRefs};
use genealogy_core::dna_test::command::DnaTestCommand;
use genealogy_core::dna_test::ref_resolver::{DnaTestRefResolver, DnaTestRefs};
use genealogy_core::event::command::EventCommand;
use genealogy_core::event::ref_resolver::{EventRefResolver, EventRefs};
use genealogy_core::place::command::PlaceCommand;
use genealogy_core::place::ref_resolver::{PlaceRefResolver, PlaceRefs};
use genealogy_core::source::command::SourceCommand;
use genealogy_core::source::ref_resolver::{SourceRefResolver, SourceRefs};
use sqlx::Pool;
#[cfg(feature = "postgres")]
use sqlx::Postgres;
#[cfg(feature = "sqlite")]
use sqlx::Sqlite;
use tracing::warn;

use crate::store::DbError;
use crate::tables::{
    DNA_TEST_VIEW_TABLE, PERSON_VIEW_TABLE, PLACE_VIEW_TABLE, REPOSITORY_VIEW_TABLE, SOURCE_VIEW_TABLE,
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
        crate::query::view_exists(&self.pool, table, view_id).await
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
            | CitationCommand::SupersedeAssertion { .. } => true,
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
            | EventCommand::AddParticipantRole { .. }
            | EventCommand::RemoveParticipantRole { .. }
            | EventCommand::AddCitation { .. }
            | EventCommand::AttachMedia { .. }
            | EventCommand::AttachNote { .. }
            | EventCommand::Tag { .. }
            | EventCommand::Untag { .. }
            | EventCommand::SetRestrictions { .. }
            | EventCommand::RetractAssertion { .. }
            | EventCommand::SupersedeAssertion { .. } => true,
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
            | PlaceCommand::SetCode { .. }
            | PlaceCommand::AddCitation { .. }
            | PlaceCommand::AttachMedia { .. }
            | PlaceCommand::AttachNote { .. }
            | PlaceCommand::Tag { .. }
            | PlaceCommand::Untag { .. }
            | PlaceCommand::SetRestrictions { .. }
            | PlaceCommand::RetractAssertion { .. }
            | PlaceCommand::SupersedeAssertion { .. } => true,
        };
        PlaceRefs { enclosing_exists }
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
            | SourceCommand::SupersedeAssertion { .. } => true,
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
            | DnaTestCommand::SupersedeAssertion { .. } => true,
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
            | DnaMatchCommand::SupersedeAssertion { .. } => DnaMatchRefs {
                test_a_exists: true,
                test_b_exists: true,
            },
        }
    }
}
