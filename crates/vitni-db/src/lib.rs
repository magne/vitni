//! Persistence for the Vitni workspace: the event store and projection storage backing
//! `vitni-core` (ADR 0002).
//!
//! The whole point of this crate is to **abstract the database engine away**. Its only public type
//! is [`Store`], opened from a `database_url` and exposed in domain terms; the backend (SQLite or
//! Postgres, `sqlx`, `cqrs-es`) is selected by the URL scheme and kept private, so engine details
//! never reach `vitni-app` or the frontends. With both backends compiled in, one binary picks
//! the engine per workspace at runtime. This crate also owns the schema DDL; the domain rules live
//! entirely in `vitni-core`.
//!
//! # Licence
//!
//! `AGPL-3.0-or-later` (ADR 0034). Additional permission under GNU AGPL version 3 section 7: if you
//! modify this Program, or any covered work, by combining it with a WebAssembly component that
//! interacts with the Program solely through the versioned `vitni:host-api` WIT world (or any later
//! version of that world), the licensor grants you additional permission to convey the resulting
//! work. Such a component is not required to be licensed under the GNU AGPL.

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
