//! The engine-neutral workspace event store (ADR 0002, ADR 0006).
//!
//! [`Store`] is the entire public surface of `genealogy-db`: it is opened from a `database_url` and
//! exposes operations in *domain* terms (execute a Person command, allocate a `human_id`, read the
//! Person projection). The backend — SQLite or Postgres, `sqlx`, `cqrs-es` — is chosen by the URL
//! scheme and kept strictly private, so engine details never leak to `genealogy-app` or frontends.
//! It currently hosts the Person aggregate; further aggregates extend this same handle.

use genealogy_core::id_format::IdFormat;
use genealogy_core::person::{PersonCommandEnvelope, PersonError, PersonView};

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
/// `Rejected` is the operator's fault (invalid input — a 4xx); `Store` is the system's.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// A domain rule rejected the command (from `genealogy-core`).
    #[error(transparent)]
    Rejected(PersonError),
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

    /// Executes one Person command against the aggregate instance `aggregate_id`.
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_person(&self, aggregate_id: &str, command: PersonCommandEnvelope) -> Result<(), CommandError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_person(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Allocates the next free Person `human_id` for `format` (e.g. `I0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_person_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_person_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Person projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_person(&self, human_id: &str) -> Result<Option<PersonView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_person(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Person projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_persons(&self) -> Result<Vec<PersonView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_persons().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }
}
