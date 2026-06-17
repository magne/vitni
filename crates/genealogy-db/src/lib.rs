//! Persistence for the genealogy workspace: the event store and projection storage backing
//! `genealogy-core` (ADR 0002).
//!
//! The database engine is a **per-workspace property chosen at runtime**, not a deploy-time
//! switch. Two Cargo features gate the backends — `sqlite` (default, zero-setup local) and
//! `postgres` (server / multi-user) — and [`WorkspaceStore`] dispatches to the active one behind
//! a single `execute` method. This crate also owns initial table creation (see [`schema`]); the
//! domain rules live entirely in `genealogy-core`.

pub mod schema;
pub mod store;

pub use store::WorkspaceStore;

#[cfg(feature = "sqlite")]
pub use store::{open_sqlite_pool, sqlite_store};
