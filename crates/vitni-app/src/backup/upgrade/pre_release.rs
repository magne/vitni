//! **Temporary.** The pre-1.0 backup formats and their upgraders (ADR 0041 §4).
//!
//! Before 1.0 a restore reads the current format and the two before it. At 1.0 the format in force
//! is frozen as v1 and this whole module is deleted, together with the older `v0.*` fixtures, keeping
//! only the permanent `0.K` → v1 alias. `cargo xtask check` fails while this file exists at a
//! workspace version of 1.0.0 or later, so the deletion cannot be forgotten (#392).
//!
//! Bumping the format: append a [`FormatRecord`] for the new version with `upgrade: None`, give the
//! previous last record its `vN → vN+1` [`Upgrader`] (pure functions over `serde_json::Value`, in
//! this module), and add the new golden fixture under `crates/vitni-app/tests/fixtures/backup/`.

use super::FormatRecord;
#[cfg(doc)]
use super::Upgrader;
use crate::backup::format::FormatVersion;

/// Every pre-1.0 format, oldest first; the last is the one this version writes.
pub(super) const HISTORY: &[FormatRecord] = &[FormatRecord {
    version: FormatVersion::pre_release(1),
    first_app_version: "0.1.0",
    upgrade: None,
}];
