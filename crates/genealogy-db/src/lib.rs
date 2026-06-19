//! Persistence for the genealogy workspace: the event store and projection storage backing
//! `genealogy-core` (ADR 0002).
//!
//! The whole point of this crate is to **abstract the database engine away**. Its only public type
//! is [`Store`], opened from a `database_url` and exposed in domain terms; the backend (SQLite or
//! Postgres, `sqlx`, `cqrs-es`) is selected by the URL scheme and kept private, so engine details
//! never reach `genealogy-app` or the frontends. This crate also owns the schema DDL; the domain
//! rules live entirely in `genealogy-core`.

#[cfg(feature = "sqlite")]
mod query;
#[cfg(feature = "sqlite")]
mod resolver;
#[cfg(feature = "sqlite")]
mod schema;
#[cfg(feature = "sqlite")]
mod sqlite;
mod store;

pub use store::{CommandError, DbError, Store};
