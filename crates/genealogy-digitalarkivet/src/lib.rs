//! Pure Digitalarkivet HTML/URL parsing: page classification, census/church-book
//! record extraction, and scan-URL resolution.
//!
//! HTML and URL strings in, typed records out — **zero I/O**, no async, no network.
//! The crate is the format logic of the Digitalarkivet import plugin (ADR 0017),
//! kept free of WASM/host types so it unit-tests on the host and still builds for
//! `wasm32-wasip2`. The import plugin (PR8) maps these records onto `genealogy-core`
//! aggregates and drives the network through the host `net`/`media-store`
//! capabilities — this crate never fetches anything.
//!
//! # Page/scan chain (prototype-proven, `sort-inbox.py`)
//! 1. [`classify_url`] a URL into a [`PageKind`].
//! 2. [`html::parse_person_page`] a `/census/person/` or `/view/<n>/pd…` record →
//!    focal fields, household links, and the scan-viewer URL.
//! 3. [`html::parse_residence_page`] a residence page → its household person links.
//! 4. [`html::parse_viewer_page`] a scan-viewer page → the permanent image URL.
//!
//! Church-book scans are served through the new `nye.digitalarkivet.no` IIIF viewer,
//! which carries no legacy permanent image; [`html::parse_viewer_page`] reports
//! [`error::ParseError::ImageUrlNotFound`] there (see [`api`] for the deferred path).

pub mod api;
pub mod classify;
pub mod error;
pub mod html;
pub mod model;
pub mod text;

pub use classify::{classify_url, record_id};
pub use error::{PageContext, ParseError};
pub use html::{parse_person_page, parse_residence_page, parse_viewer_page};
pub use model::{ExternalId, Field, PageKind, PersonRecord, ResidenceRecord, SourceMetadata};
pub use text::{
    AUTHORITY, COMMON_EVENTS, REPOSITORY, census_year, extract_urn, normalize_ws, slugify, suggest_filename,
};
