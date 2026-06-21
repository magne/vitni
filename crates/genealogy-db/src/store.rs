//! The engine-neutral workspace event store (ADR 0002, ADR 0006).
//!
//! [`Store`] is the entire public surface of `genealogy-db`: it is opened from a `database_url` and
//! exposes operations in *domain* terms (execute a Person command, allocate a `human_id`, read the
//! Person projection). The backend — SQLite or Postgres, `sqlx`, `cqrs-es` — is chosen by the URL
//! scheme and kept strictly private, so engine details never leak to `genealogy-app` or frontends.
//! It currently hosts the Person aggregate; further aggregates extend this same handle.

use genealogy_core::citation::{CitationCommandEnvelope, CitationError, CitationView};
use genealogy_core::dna_match::{DnaMatchCommandEnvelope, DnaMatchError, DnaMatchView};
use genealogy_core::dna_test::{DnaTestCommandEnvelope, DnaTestError, DnaTestView};
use genealogy_core::event::{EventCommandEnvelope, EventError, EventView};
use genealogy_core::family::{FamilyCommandEnvelope, FamilyError, FamilyView};
use genealogy_core::id_format::IdFormat;
use genealogy_core::media::{MediaCommandEnvelope, MediaError, MediaView};
use genealogy_core::note::{NoteCommandEnvelope, NoteError, NoteView};
use genealogy_core::person::{PersonCommandEnvelope, PersonError, PersonView};
use genealogy_core::place::{PlaceCommandEnvelope, PlaceError, PlaceView};
use genealogy_core::repository::{RepositoryCommandEnvelope, RepositoryError, RepositoryView};
use genealogy_core::source::{SourceCommandEnvelope, SourceError, SourceView};
use genealogy_core::tag::{TagCommandEnvelope, TagError, TagView};

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
    pub async fn execute_person(
        &self,
        aggregate_id: &str,
        command: PersonCommandEnvelope,
    ) -> Result<(), CommandError<PersonError>> {
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

    /// Executes one Family command against the aggregate instance `aggregate_id`.
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_family(
        &self,
        aggregate_id: &str,
        command: FamilyCommandEnvelope,
    ) -> Result<(), CommandError<FamilyError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_family(aggregate_id, command).await
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

    /// Allocates the next free Family `human_id` for `format` (e.g. `F0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_family_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_family_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Family projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_family(&self, human_id: &str) -> Result<Option<FamilyView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_family(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Family projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_families(&self) -> Result<Vec<FamilyView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_families().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Executes one Place command against the aggregate instance `aggregate_id`.
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_place(
        &self,
        aggregate_id: &str,
        command: PlaceCommandEnvelope,
    ) -> Result<(), CommandError<PlaceError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_place(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Allocates the next free Place `human_id` for `format` (e.g. `P0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_place_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_place_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Place projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_place(&self, human_id: &str) -> Result<Option<PlaceView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_place(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Place projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_places(&self) -> Result<Vec<PlaceView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_places().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Executes one Source command against the aggregate instance `aggregate_id`.
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_source(
        &self,
        aggregate_id: &str,
        command: SourceCommandEnvelope,
    ) -> Result<(), CommandError<SourceError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_source(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Allocates the next free Source `human_id` for `format` (e.g. `S0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_source_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_source_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Source projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_source(&self, human_id: &str) -> Result<Option<SourceView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_source(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Source projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_sources(&self) -> Result<Vec<SourceView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_sources().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Executes one Citation command against the aggregate instance `aggregate_id`.
    ///
    /// The cited source's existence is checked against the (possibly-lagging) Source projection by
    /// the aggregate's `Services` resolver; a missing source surfaces as
    /// [`CitationError::UnknownSource`](genealogy_core::citation::CitationError::UnknownSource)
    /// through [`CommandError::Rejected`] (ADR 0004 §3, data-model §9).
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_citation(
        &self,
        aggregate_id: &str,
        command: CitationCommandEnvelope,
    ) -> Result<(), CommandError<CitationError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_citation(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Allocates the next free Citation `human_id` for `format` (e.g. `C0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_citation_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_citation_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Citation projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_citation(&self, human_id: &str) -> Result<Option<CitationView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_citation(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Citation projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_citations(&self) -> Result<Vec<CitationView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_citations().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Executes one Event command against the aggregate instance `aggregate_id`.
    ///
    /// A `LinkPlace` to a place the Place projection does not know surfaces as
    /// [`EventError::UnknownPlace`](genealogy_core::event::EventError::UnknownPlace) through
    /// [`CommandError::Rejected`], checked by the aggregate's `Services` resolver against the
    /// (possibly-lagging) projection (ADR 0004 §3, data-model §9).
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_event(
        &self,
        aggregate_id: &str,
        command: EventCommandEnvelope,
    ) -> Result<(), CommandError<EventError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_event(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Allocates the next free Event `human_id` for `format` (e.g. `E0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_event_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_event_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Event projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_event(&self, human_id: &str) -> Result<Option<EventView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_event(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Event projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_events(&self) -> Result<Vec<EventView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_events().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Executes one `DnaTest` command against the aggregate instance `aggregate_id`.
    ///
    /// A test anchored to a person the Person projection does not know surfaces as
    /// `DnaTestError::UnknownPerson` through [`CommandError::Rejected`] (ADR 0004 §3).
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_dna_test(
        &self,
        aggregate_id: &str,
        command: DnaTestCommandEnvelope,
    ) -> Result<(), CommandError<DnaTestError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_dna_test(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Allocates the next free `DnaTest` `human_id` for `format` (e.g. `D0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_dna_test_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_dna_test_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the `DnaTest` projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_dna_test(&self, human_id: &str) -> Result<Option<DnaTestView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_dna_test(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every `DnaTest` projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_dna_tests(&self) -> Result<Vec<DnaTestView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_dna_tests().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Executes one `DnaMatch` command against the aggregate instance `aggregate_id`.
    ///
    /// A match referencing a test the `DnaTest` projection does not know surfaces as
    /// `DnaMatchError::UnknownTest` through [`CommandError::Rejected`] (ADR 0004 §3).
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_dna_match(
        &self,
        aggregate_id: &str,
        command: DnaMatchCommandEnvelope,
    ) -> Result<(), CommandError<DnaMatchError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_dna_match(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Executes one Repository command against the aggregate instance `aggregate_id`.
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_repository(
        &self,
        aggregate_id: &str,
        command: RepositoryCommandEnvelope,
    ) -> Result<(), CommandError<RepositoryError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_repository(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Executes one Tag command against the aggregate instance `aggregate_id`.
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_tag(
        &self,
        aggregate_id: &str,
        command: TagCommandEnvelope,
    ) -> Result<(), CommandError<TagError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_tag(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Executes one Note command against the aggregate instance `aggregate_id`.
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_note(
        &self,
        aggregate_id: &str,
        command: NoteCommandEnvelope,
    ) -> Result<(), CommandError<NoteError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_note(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Allocates the next free `DnaMatch` `human_id` for `format` (e.g. `X0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_dna_match_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_dna_match_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Allocates the next free Repository `human_id` for `format` (e.g. `R0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_repository_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_repository_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the `DnaMatch` projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_dna_match(&self, human_id: &str) -> Result<Option<DnaMatchView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_dna_match(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Tag projection for `tag_id` (the aggregate id; tags have no `human_id`), if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_tag(&self, tag_id: &str) -> Result<Option<TagView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_tag(tag_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = tag_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Tag projection.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_tags(&self) -> Result<Vec<TagView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_tags().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Allocates the next free Note `human_id` for `format` (e.g. `N0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_note_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_note_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Repository projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_repository(&self, human_id: &str) -> Result<Option<RepositoryView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_repository(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every `DnaMatch` projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_dna_matches(&self) -> Result<Vec<DnaMatchView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_dna_matches().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Note projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_note(&self, human_id: &str) -> Result<Option<NoteView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_note(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Repository projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_repositories(&self) -> Result<Vec<RepositoryView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_repositories().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Note projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_notes(&self) -> Result<Vec<NoteView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_notes().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Executes one Media command against the aggregate instance `aggregate_id`.
    ///
    /// # Errors
    ///
    /// [`CommandError::Rejected`] if a domain rule rejects it, [`CommandError::Store`] on an
    /// infrastructure failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn execute_media(
        &self,
        aggregate_id: &str,
        command: MediaCommandEnvelope,
    ) -> Result<(), CommandError<MediaError>> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.execute_media(aggregate_id, command).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (aggregate_id, command);
            Err(CommandError::Store(DbError::Unsupported(
                "no backend compiled in".to_owned(),
            )))
        }
    }

    /// Allocates the next free Media `human_id` for `format` (e.g. `M0001`).
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn next_media_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.next_media_human_id(format).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = format;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads the Media projection for `human_id`, if any.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn find_media(&self, human_id: &str) -> Result<Option<MediaView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.find_media(human_id).await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = human_id;
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
        }
    }

    /// Loads every Media projection, ordered by `human_id`.
    ///
    /// # Errors
    ///
    /// [`DbError`] on a read-model failure.
    #[cfg_attr(
        not(feature = "sqlite"),
        expect(clippy::unused_async, reason = "neutral async API; no backend compiled in")
    )]
    pub async fn list_media(&self) -> Result<Vec<MediaView>, DbError> {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.list_media().await
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Err(DbError::Unsupported("no backend compiled in".to_owned()))
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
