//! Future: `api.digitalarkivet.no` — a documented REST endpoint or a graduated
//! IIIF Image API over the census/church-book corpus.
//!
//! As of 2026-07 no anonymous public API exists (`api.digitalarkivet.no` is 404
//! and IIIF is an experimental, photo-scoped technology test) — see
//! `docs/research/digitalarkivet.md`. Every feature ships HTML-first through the
//! [`crate::html`] parsers. This module is a deliberate seam: an `info.json`/image
//! path or a search endpoint can be added here without touching the plugin flow.
