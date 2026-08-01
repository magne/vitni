//! The Place succession cross-reference projection (ADR 0026 §4).
//!
//! `SuccessionAsserted` is recorded once, on the anchor place's own event stream (`place_id`, one of
//! `from`); its payload is self-contained (both endpoint id lists — ADR 0002), but the anchor's own
//! `PlaceView` only ever reflects assertions recorded on *its own* stream. Reading "what did this
//! place become?" from a `from` place that is *not* the anchor, or "what did this place come from?"
//! from any `to` place, needs a cross-aggregate read the anchor's own projection cannot answer —
//! exactly the derived, rebuildable index this module maintains (ADR 0010).
//!
//! `place_succession` holds one row per live succession assertion (its kind, date, and the assertion
//! id a correction targets); `place_succession_link` holds one row per `(from, to)` pair the
//! assertion names — the cartesian product of its endpoint lists — so a query on *either* endpoint's
//! id finds the assertion directly, symmetric in both directions (ADR 0026 §4).
//!
//! One engine-neutral half ([`succession_columns`], the table names) and one thin submodule per
//! backend, mirroring the `sqlite_query` / `postgres_query` split. Sharing the column serialization
//! is load-bearing: `genealogy-app` parses `kind` back with `serde_json`, so any drift between the
//! two engines would be an engine-dependent, user-visible bug.

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use genealogy_core::assertions::{Asserted, Attributed};
use genealogy_core::place_succession::PlaceSuccessionAssertion;

use crate::store::DbError;

/// The succession-assertion metadata table: one row per live `SuccessionAsserted` assertion.
const PLACE_SUCCESSION_TABLE: &str = "place_succession";
/// The endpoint cross-reference table: one row per `(from, to)` pair a succession assertion names.
const PLACE_SUCCESSION_LINK_TABLE: &str = "place_succession_link";

/// One live succession assertion as the Place projection carries it (ADR 0021 §3).
type SuccessionAssertion = Attributed<Asserted<PlaceSuccessionAssertion>>;

/// Serializes one succession assertion's `(kind, date_json)` columns — the two JSON strings both
/// backends store verbatim and `genealogy-app` parses back.
///
/// # Errors
///
/// [`DbError::Backend`] if either value fails to serialize.
fn succession_columns(attributed: &SuccessionAssertion) -> Result<(String, Option<String>), DbError> {
    let assertion = &attributed.value.value;
    let kind = serde_json::to_string(&assertion.kind)
        .map_err(|e| DbError::Backend(format!("serializing succession kind: {e}")))?;
    let date_json = assertion
        .date
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| DbError::Backend(format!("serializing succession date: {e}")))?;
    Ok((kind, date_json))
}
