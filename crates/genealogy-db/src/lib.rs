//! Persistence for the genealogy workspace: the event store and projection storage backing
//! `genealogy-core` (ADR 0002).
//!
//! The whole point of this crate is to **abstract the database engine away**. Its only public type
//! is [`Store`], opened from a `database_url` and exposed in domain terms; the backend (SQLite or
//! Postgres, `sqlx`, `cqrs-es`) is selected by the URL scheme and kept private, so engine details
//! never reach `genealogy-app` or the frontends. With both backends compiled in, one binary picks
//! the engine per workspace at runtime. This crate also owns the schema DDL; the domain rules live
//! entirely in `genealogy-core`.

#[cfg(feature = "sqlite")]
mod geo_index;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
mod place_succession_index;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "postgres")]
mod postgres_query;
mod registry;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
mod resolver;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
mod schema;
#[cfg(feature = "sqlite")]
mod sqlite;
#[cfg(feature = "sqlite")]
mod sqlite_query;
mod store;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
mod tables;

pub use store::{CommandError, DbError, PlaceSuccessionRecord, Store, StoredEvent};
