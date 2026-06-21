//! The engine-neutral workspace event store (ADR 0002, ADR 0006).
//!
//! [`Store`] is the entire public surface of `genealogy-db`: it is opened from a `database_url` and
//! exposes operations in *domain* terms (execute a Person command, allocate a `human_id`, read the
//! Person projection). The backend — SQLite or Postgres, `sqlx`, `cqrs-es` — is chosen by the URL
//! scheme and kept strictly private, so engine details never leak to `genealogy-app` or frontends.
//! Its per-aggregate operations are generated from the [`registry`](crate::registry); the backend
//! delegation pattern is identical for every aggregate.

use crate::registry::{for_each_db_aggregate, for_each_db_human_id_aggregate};

/// Postgres backend type, reserved per ADR 0002; wired when its read model lands. Referencing it
/// keeps the `postgres` feature's backend trait-compatible and compiling, as ADR 0002 commits.
#[cfg(feature = "postgres")]
#[expect(
    dead_code,
    reason = "postgres backend reserved (ADR 0002); wiring lands with its read model"
)]
type ReservedPostgresRepository = postgres_es::PostgresEventRepository;

/// An infrastructure failure (engine-neutral — no `sqlx`/`cqrs-es` types escape).
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// The requested backend or operation is unavailable in this build / not yet implemented.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// The storage backend failed (connection, query, serialization).
    #[error("storage backend error: {0}")]
    Backend(String),
    /// The input was malformed (e.g. an unrecognized `database_url`).
    #[error("malformed input: {0}")]
    Malformed(String),
}

/// The outcome of a rejected command: a domain rejection vs. an infrastructure failure.
///
/// `Rejected` carries the aggregate's own domain error `E` (the operator's fault — invalid input,
/// a 4xx); `Store` is the system's. Generic over `E` so every aggregate reuses one type.
#[derive(Debug, thiserror::Error)]
pub enum CommandError<E: std::error::Error + 'static> {
    /// A domain rule rejected the command (from `genealogy-core`).
    #[error(transparent)]
    Rejected(E),
    /// The event store failed for an infrastructure reason.
    #[error(transparent)]
    Store(DbError),
}

/// A workspace event store, bound at open time to whichever backend the `database_url` selects.
pub struct Store {
    #[cfg(feature = "sqlite")]
    sqlite: crate::sqlite::SqliteStore,
}

impl Store {
    /// Opens the store for `database_url`, dispatching on the URL scheme (ADR 0002).
    ///
    /// `sqlite:`/`sqlite://…` selects the embedded backend (the file is created and the schema
    /// initialized if needed). `postgres:`/`postgresql:` is reserved (ADR 0002) but not yet
    /// implemented. The SQLite backend is available only when the `sqlite` feature is compiled in.
    ///
    /// # Errors
    ///
    /// [`DbError::Unsupported`] for an unimplemented/uncompiled backend, [`DbError::Malformed`] for
    /// an unrecognized scheme, or [`DbError::Backend`] if opening/initialization fails.
    pub async fn open(database_url: &str) -> Result<Self, DbError> {
        if database_url.starts_with("sqlite:") {
            return Self::open_sqlite(database_url).await;
        }
        if database_url.starts_with("postgres:") || database_url.starts_with("postgresql:") {
            return Err(DbError::Unsupported(
                "the postgres backend is reserved (ADR 0002) but not yet implemented".to_owned(),
            ));
        }
        Err(DbError::Malformed(format!(
            "unrecognized database url scheme (expected sqlite:// or postgres://): {database_url}"
        )))
    }

    /// Constructs the SQLite-backed store, or reports it unavailable when not compiled in.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    async fn open_sqlite(database_url: &str) -> Result<Self, DbError> {
        #[cfg(feature = "sqlite")]
        {
            let sqlite = crate::sqlite::SqliteStore::open(database_url).await?;
            Ok(Self { sqlite })
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = database_url;
            Err(DbError::Unsupported(
                "this build was compiled without the sqlite backend".to_owned(),
            ))
        }
    }

    /// Rebuilds every projection from the event log, applying event upcasters (ADR 0010).
    ///
    /// Each read model is cleared and replayed from its aggregate's history, so a schema change or
    /// a corrupted projection is recovered without touching the (immutable) event log. A
    /// maintenance operation: the caller must ensure no commands run concurrently.
    ///
    /// # Errors
    ///
    /// [`DbError`] if clearing or replaying a projection fails.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn rebuild_projections(&self) -> Result<(), DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.rebuild_projections().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }
}

/// Generates the per-aggregate command/find/list facade methods, each delegating to the SQLite
/// backend or reporting `Unsupported` when no backend is compiled in.
macro_rules! store_methods {
    ($(($snake:ident, $State:ty, $View:ty, $Cmd:ty, $Err:ty, $table_const:ident, $table_str:literal, $execute:ident, $find:ident, $find_param:ident, $list:ident, $wiring:tt, $upcasters:expr,)),+ $(,)?) => {
        impl Store {
            $(
                #[doc = concat!("Executes one ", stringify!($snake), " command against the aggregate instance `aggregate_id`.")]
                ///
                /// # Errors
                ///
                /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on
                /// an infrastructure failure.
                pub async fn $execute(&self, aggregate_id: &str, command: $Cmd) -> Result<(), CommandError<$Err>> {
                    #[cfg(feature = "sqlite")]
                    {
                        self.sqlite.$execute(aggregate_id, command).await
                    }
                    #[cfg(not(feature = "sqlite"))]
                    {
                        let _ = (aggregate_id, command);
                        Err(CommandError::Store(DbError::Unsupported(
                            "no backend compiled in".to_owned(),
                        )))
                    }
                }

                #[doc = concat!("Loads the ", stringify!($snake), " projection for `", stringify!($find_param), "`, if any.")]
                ///
                /// # Errors
                ///
                /// [`DbError`] on a read-model failure.
                pub async fn $find(&self, $find_param: &str) -> Result<Option<$View>, DbError> {
                    #[cfg(feature = "sqlite")]
                    {
                        self.sqlite.$find($find_param).await
                    }
                    #[cfg(not(feature = "sqlite"))]
                    {
                        let _ = $find_param;
                        Err(DbError::Unsupported("no backend compiled in".to_owned()))
                    }
                }

                #[doc = concat!("Loads every ", stringify!($snake), " projection.")]
                ///
                /// # Errors
                ///
                /// [`DbError`] on a read-model failure.
                pub async fn $list(&self) -> Result<Vec<$View>, DbError> {
                    #[cfg(feature = "sqlite")]
                    {
                        self.sqlite.$list().await
                    }
                    #[cfg(not(feature = "sqlite"))]
                    {
                        Err(DbError::Unsupported("no backend compiled in".to_owned()))
                    }
                }
            )+
        }
    };
}

for_each_db_aggregate!(store_methods);

/// Generates the per-aggregate `next_*_human_id` allocators (every aggregate but Tag).
macro_rules! store_next_methods {
    ($(($snake:ident, $next:ident, $table_const:ident)),+ $(,)?) => {
        impl Store {
            $(
                #[doc = concat!("Allocates the next free ", stringify!($snake), " `human_id` for `format`.")]
                ///
                /// # Errors
                ///
                /// [`DbError`] on a read-model failure.
                pub async fn $next(&self, format: &genealogy_core::id_format::IdFormat) -> Result<String, DbError> {
                    #[cfg(feature = "sqlite")]
                    {
                        self.sqlite.$next(format).await
                    }
                    #[cfg(not(feature = "sqlite"))]
                    {
                        let _ = format;
                        Err(DbError::Unsupported("no backend compiled in".to_owned()))
                    }
                }
            )+
        }
    };
}

for_each_db_human_id_aggregate!(store_next_methods);
