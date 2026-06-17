//! The per-workspace event store: one trait, runtime backend selection (ADR 0002).
//!
//! A workspace's database engine is chosen *at runtime*, not at deploy time: a binary with more
//! than one backend feature compiled in decides per workspace. [`WorkspaceStore`] is the enum that
//! holds whichever `cqrs-es` framework is active and forwards [`execute`](WorkspaceStore::execute)
//! to it, so engine differences never leak into domain or application code.

use cqrs_es::persist::PersistedEventStore;
use cqrs_es::{Aggregate, AggregateError, CqrsFramework};

#[cfg(feature = "sqlite")]
use cqrs_es::Query;
#[cfg(feature = "sqlite")]
use sqlite_es::{SqliteEventRepository, default_sqlite_pool};
#[cfg(feature = "sqlite")]
use sqlx::{Pool, Sqlite};

/// A `cqrs-es` framework bound to whichever backend a workspace uses (ADR 0002).
///
/// Both backends implement the same `PersistedEventRepository`, so commands are executed through
/// one method regardless of engine. SQLite serves embedded / single-user workspaces; Postgres
/// serves server / multi-user workspaces.
pub enum WorkspaceStore<A>
where
    A: Aggregate,
{
    /// A SQLite-backed workspace (the zero-setup default).
    #[cfg(feature = "sqlite")]
    Sqlite(CqrsFramework<A, PersistedEventStore<SqliteEventRepository, A>>),
    /// A Postgres-backed workspace (server / multi-user).
    #[cfg(feature = "postgres")]
    Postgres(CqrsFramework<A, PersistedEventStore<postgres_es::PostgresEventRepository, A>>),
}

impl<A> WorkspaceStore<A>
where
    A: Aggregate,
{
    /// Applies a command to an aggregate instance, dispatching to the active backend.
    ///
    /// # Errors
    ///
    /// Returns the framework's [`AggregateError`], which wraps either a domain rejection
    /// (`A::Error`) or an infrastructure failure (a concurrency conflict or database error).
    pub async fn execute(&self, aggregate_id: &str, command: A::Command) -> Result<(), AggregateError<A::Error>> {
        match self {
            #[cfg(feature = "sqlite")]
            Self::Sqlite(cqrs) => cqrs.execute(aggregate_id, command).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(cqrs) => cqrs.execute(aggregate_id, command).await,
        }
    }
}

/// Opens a SQLite connection pool for a workspace database at `connection_string`.
///
/// Creates the file if missing and enables WAL journaling (the `sqlite-es` defaults).
#[cfg(feature = "sqlite")]
pub async fn open_sqlite_pool(connection_string: &str) -> Pool<Sqlite> {
    default_sqlite_pool(connection_string).await
}

/// Builds a SQLite-backed [`WorkspaceStore`] for aggregate `A`, with its projection queries.
///
/// The caller passes the query processors (e.g. a `GenericQuery` over a view repository) and the
/// aggregate's services. The schema must already exist (see [`crate::schema::init_sqlite`]).
#[cfg(feature = "sqlite")]
pub fn sqlite_store<A>(pool: Pool<Sqlite>, queries: Vec<Box<dyn Query<A>>>, services: A::Services) -> WorkspaceStore<A>
where
    A: Aggregate,
{
    WorkspaceStore::Sqlite(sqlite_es::sqlite_cqrs(pool, queries, services))
}
