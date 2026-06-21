//! Cross-aggregate reference resolvers backing the aggregates' `cqrs-es` `Services` (ADR 0004 §3).
//!
//! ADR 0004 §3 reserves the `Services` slot for cross-aggregate projection reads — the "aggregate
//! tax" (data-model §9). Each resolver here reads one aggregate's read model to answer the
//! existence questions another aggregate's pure `decide` needs, returning the engine-neutral
//! `…Refs` value `genealogy-core` defines. They are private to this crate; only the SQLite store
//! constructs and injects them.

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
use sqlx::{Pool, Sqlite};
use tracing::warn;

use crate::query;
use crate::sqlite::{DNA_TEST_VIEW_TABLE, PERSON_VIEW_TABLE, PLACE_VIEW_TABLE, SOURCE_VIEW_TABLE};

/// Resolves Citation cross-aggregate refs (does the cited `Source` exist?) against the Source
/// projection — the `cqrs-es` `Services` value for the Citation aggregate.
pub(crate) struct SqliteCitationRefResolver {
    pool: Pool<Sqlite>,
}

impl SqliteCitationRefResolver {
    /// Wraps the read-model pool the resolver queries.
    pub(crate) fn new(pool: Pool<Sqlite>) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[async_trait]
impl CitationRefResolver for SqliteCitationRefResolver {
    async fn resolve(&self, command: &CitationCommand) -> CitationRefs {
        let source_exists = match command {
            CitationCommand::CreateCitation { source_id, .. } => {
                match query::view_exists(&self.pool, SOURCE_VIEW_TABLE, &source_id.to_string()).await {
                    Ok(exists) => exists,
                    Err(error) => {
                        // Fail closed: if the source cannot be confirmed, do not let the citation
                        // claim it (a primary-key lookup on the open pool effectively never errors).
                        warn!(%error, "source existence check failed; treating source as absent");
                        false
                    }
                }
            }
            // No cross-aggregate reference to resolve.
            CitationCommand::SetPage { .. }
            | CitationCommand::RetractAssertion { .. }
            | CitationCommand::SupersedeAssertion { .. } => true,
        };
        CitationRefs { source_exists }
    }
}

/// Resolves Event cross-aggregate refs (does the linked `Place` exist?) against the Place
/// projection — the `cqrs-es` `Services` value for the Event aggregate.
pub(crate) struct SqliteEventRefResolver {
    pool: Pool<Sqlite>,
}

impl SqliteEventRefResolver {
    /// Wraps the read-model pool the resolver queries.
    pub(crate) fn new(pool: Pool<Sqlite>) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[async_trait]
impl EventRefResolver for SqliteEventRefResolver {
    async fn resolve(&self, command: &EventCommand) -> EventRefs {
        let place_exists = match command {
            EventCommand::LinkPlace { place_id, .. } => {
                match query::view_exists(&self.pool, PLACE_VIEW_TABLE, &place_id.to_string()).await {
                    Ok(exists) => exists,
                    Err(error) => {
                        // Fail closed: if the place cannot be confirmed, do not let the event link
                        // it (a primary-key lookup on the open pool effectively never errors).
                        warn!(%error, "place existence check failed; treating place as absent");
                        false
                    }
                }
            }
            // No cross-aggregate reference to resolve.
            EventCommand::CreateEvent { .. }
            | EventCommand::SetEventType { .. }
            | EventCommand::AssertDate { .. }
            | EventCommand::RetractAssertion { .. }
            | EventCommand::SupersedeAssertion { .. } => true,
        };
        EventRefs { place_exists }
    }
}

/// Resolves Place cross-aggregate refs (does the enclosing `Place` exist?) against the Place
/// projection — the `cqrs-es` `Services` value for the Place aggregate.
pub(crate) struct SqlitePlaceRefResolver {
    pool: Pool<Sqlite>,
}

impl SqlitePlaceRefResolver {
    /// Wraps the read-model pool the resolver queries.
    pub(crate) fn new(pool: Pool<Sqlite>) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[async_trait]
impl PlaceRefResolver for SqlitePlaceRefResolver {
    async fn resolve(&self, command: &PlaceCommand) -> PlaceRefs {
        let enclosing_exists = match command {
            PlaceCommand::AssertEnclosedBy { enclosed_by, .. } => {
                match query::view_exists(&self.pool, PLACE_VIEW_TABLE, &enclosed_by.place_id.to_string()).await {
                    Ok(exists) => exists,
                    Err(error) => {
                        // Fail closed: if the enclosing place cannot be confirmed, do not let the
                        // enclosure be asserted (a primary-key lookup on the open pool effectively
                        // never errors).
                        warn!(%error, "enclosing place existence check failed; treating place as absent");
                        false
                    }
                }
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
            | PlaceCommand::RetractAssertion { .. }
            | PlaceCommand::SupersedeAssertion { .. } => true,
        };
        PlaceRefs { enclosing_exists }
    }
}

/// Resolves `DnaTest` cross-aggregate refs (does the anchoring `Person` exist?) against the Person
/// projection — the `cqrs-es` `Services` value for the `DnaTest` aggregate.
pub(crate) struct SqliteDnaTestRefResolver {
    pool: Pool<Sqlite>,
}

impl SqliteDnaTestRefResolver {
    /// Wraps the read-model pool the resolver queries.
    pub(crate) fn new(pool: Pool<Sqlite>) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[async_trait]
impl DnaTestRefResolver for SqliteDnaTestRefResolver {
    async fn resolve(&self, command: &DnaTestCommand) -> DnaTestRefs {
        let person_exists = match command {
            DnaTestCommand::CreateDnaTest { person_id, .. } => {
                match query::view_exists(&self.pool, PERSON_VIEW_TABLE, &person_id.to_string()).await {
                    Ok(exists) => exists,
                    Err(error) => {
                        // Fail closed: if the person cannot be confirmed, do not let the test anchor
                        // to it (a primary-key lookup on the open pool effectively never errors).
                        warn!(%error, "person existence check failed; treating person as absent");
                        false
                    }
                }
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
            | DnaTestCommand::RetractAssertion { .. }
            | DnaTestCommand::SupersedeAssertion { .. } => true,
        };
        DnaTestRefs { person_exists }
    }
}

/// Resolves `DnaMatch` cross-aggregate refs (do both `DnaTest`s exist?) against the `DnaTest`
/// projection — the `cqrs-es` `Services` value for the `DnaMatch` aggregate.
pub(crate) struct SqliteDnaMatchRefResolver {
    pool: Pool<Sqlite>,
}

impl SqliteDnaMatchRefResolver {
    /// Wraps the read-model pool the resolver queries.
    pub(crate) fn new(pool: Pool<Sqlite>) -> Arc<Self> {
        Arc::new(Self { pool })
    }

    /// Whether a test exists, failing closed on a (practically impossible) lookup error.
    async fn test_exists(&self, test_id: &str) -> bool {
        match query::view_exists(&self.pool, DNA_TEST_VIEW_TABLE, test_id).await {
            Ok(exists) => exists,
            Err(error) => {
                warn!(%error, "dna test existence check failed; treating test as absent");
                false
            }
        }
    }
}

#[async_trait]
impl DnaMatchRefResolver for SqliteDnaMatchRefResolver {
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
            | DnaMatchCommand::RetractAssertion { .. }
            | DnaMatchCommand::SupersedeAssertion { .. } => DnaMatchRefs {
                test_a_exists: true,
                test_b_exists: true,
            },
        }
    }
}
