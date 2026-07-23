//! Domain model and event-sourcing engine for the genealogy workspace.
//!
//! This crate is the **conclusion-derived-from-evidence** core described in `docs/data-model.md`
//! and the ADRs. It has no I/O frontend: persistence lives in `genealogy-db`, and CLI/UI concerns
//! live in their own crates.
//!
//! # Layers
//!
//! - **Foundation** ([`ids`], [`provenance`], [`date`], [`age`], [`name`], [`enums`], [`text`],
//!   [`fact`], [`geo`], [`dna`], [`address`], [`place_ref`], [`repo_ref`], [`media_path`]) — the immutable
//!   value objects (data-model §7) that event payloads and projections are built from. [`fixed`]
//!   backs the scaled-integer decimals.
//! - **Aggregates** (e.g. [`person`]) — each owns a pure decision core
//!   `decide(state, command, meta) -> Result<Vec<Event>, Error>` plus an `evolve` fold, and a thin
//!   `cqrs-es` adapter. The `decide`/`evolve` functions are framework-agnostic (ADR 0002); only
//!   the adapter touches the framework.
//!
//! # Event-sourcing contract (ADR 0004)
//!
//! Every event embeds its [`provenance::EventContext`] and an [`ids::AssertionId`] in the payload
//! — never in `cqrs-es` metadata. The decision core is pure: the clock, generated ids, and the
//! operator are supplied by the application layer via [`provenance::AssertionMeta`], never sampled.

pub mod address;
pub mod age;
pub mod assertions;
pub mod citation;
pub mod date;
pub mod dna;
pub mod dna_match;
pub mod dna_test;
pub mod enums;
pub mod event;
pub mod fact;
pub mod family;
pub mod fixed;
pub mod geo;
pub mod id_format;
pub mod ids;
pub mod media;
pub mod media_path;
pub mod name;
pub mod note;
pub mod person;
pub mod place;
pub mod place_geometry;
pub mod place_name;
pub mod place_ref;
pub mod provenance;
pub mod repo_ref;
pub mod repository;
pub mod source;
pub mod tag;
pub mod text;
