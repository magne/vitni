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
use genealogy_core::event::command::EventCommand;
use genealogy_core::event::ref_resolver::{EventRefResolver, EventRefs};
use sqlx::{Pool, Sqlite};
use tracing::warn;

use crate::query;
use crate::sqlite::{PLACE_VIEW_TABLE, SOURCE_VIEW_TABLE};

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
            CitationCommand::SetPage { .. } => true,
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
            EventCommand::CreateEvent { .. } | EventCommand::SetEventType { .. } | EventCommand::AssertDate { .. } => {
                true
            }
        };
        EventRefs { place_exists }
    }
}
