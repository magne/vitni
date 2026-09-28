//! The archive layout of the current backup format: member names, the manifest and the event row
//! (ADR 0041 §1). Everything here is the compatibility surface: a change an older archive would
//! not decode needs a format bump, an upgrader and a new fixture (ADR 0041 §5).

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use vitni_core::provenance::Timestamp;
use vitni_db::RawEvent;

/// The manifest member: format version, provenance of the archive, and member checksums.
pub(crate) const MANIFEST: &str = "manifest.json";
/// The event log member, one JSON row per line, in store order.
pub(crate) const EVENTS: &str = "events.jsonl";
/// The workspace manifest member, without machine-local fields.
pub(crate) const WORKSPACE: &str = "workspace.toml";
/// The media manifest member.
pub(crate) const MEDIA: &str = "media.json";
/// The prefix of the media library files an archive carries with `--with-media`.
pub(crate) const MEDIA_PREFIX: &str = "media/";

/// A backup format version: `0.N` before 1.0 and an integer `N` from 1 onward (ADR 0041 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FormatVersion {
    major: u32,
    minor: u32,
}

impl FormatVersion {
    /// The pre-1.0 format `0.minor`.
    pub(crate) const fn pre_release(minor: u32) -> Self {
        Self { major: 0, minor }
    }
}

impl fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.major == 0 {
            write!(f, "0.{}", self.minor)
        } else {
            write!(f, "{}", self.major)
        }
    }
}

impl FromStr for FormatVersion {
    type Err = String;

    /// Parses `0.N` or a positive integer `N`, without leading zeros; nothing else is a format
    /// version.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || format!("{text:?} is not a backup format version");
        if let Some(minor) = text.strip_prefix("0.") {
            if minor.len() > 1 && minor.starts_with('0') {
                return Err(invalid());
            }
            let minor = minor.parse().map_err(|_| invalid())?;
            return Ok(Self::pre_release(minor));
        }
        let major: u32 = text.parse().map_err(|_| invalid())?;
        if major == 0 || text.starts_with('0') {
            return Err(invalid());
        }
        Ok(Self { major, minor: 0 })
    }
}

/// `manifest.json` in the current format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupManifest {
    /// The archive's format version, as text (`0.1`).
    pub(crate) format_version: String,
    /// The vitni version that wrote the archive.
    pub(crate) app_version: String,
    /// The registry name of the workspace backed up.
    pub(crate) workspace: String,
    /// The number of rows in `events.jsonl`.
    pub(crate) event_count: u64,
    /// When the backup was written (RFC 3339).
    pub(crate) created_at: Timestamp,
    /// The `sha256:<hex>` checksum of every other member, by member name.
    pub(crate) members: BTreeMap<String, String>,
}

/// One `events.jsonl` row: an `events` table row, every column (ADR 0041 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupRow {
    aggregate_type: String,
    aggregate_id: String,
    sequence: i64,
    event_type: String,
    event_version: String,
    payload: serde_json::Value,
    metadata: serde_json::Value,
}

impl From<RawEvent> for BackupRow {
    fn from(event: RawEvent) -> Self {
        Self {
            aggregate_type: event.aggregate_type,
            aggregate_id: event.aggregate_id,
            sequence: event.sequence,
            event_type: event.event_type,
            event_version: event.event_version,
            payload: event.payload,
            metadata: event.metadata,
        }
    }
}

impl From<BackupRow> for RawEvent {
    fn from(row: BackupRow) -> Self {
        Self {
            aggregate_type: row.aggregate_type,
            aggregate_id: row.aggregate_id,
            sequence: row.sequence,
            event_type: row.event_type,
            event_version: row.event_version,
            payload: row.payload,
            metadata: row.metadata,
        }
    }
}

/// One `media.json` entry: a file a media record points at, as the backup found it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MediaEntry {
    /// The stored path: `media/<rel>` for a library file, absolute for a file outside the workspace.
    pub(crate) path: String,
    /// The file's `sha256:<hex>` checksum, or the recorded one when the file was missing.
    pub(crate) checksum: Option<String>,
    /// The file's size in bytes, or `None` when it was missing.
    pub(crate) size: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::FormatVersion;

    #[test]
    fn versions_read_and_print_both_before_and_after_one_point_zero() {
        for text in ["0.1", "0.12", "1", "7"] {
            let version: FormatVersion = text.parse().expect("a version");
            assert_eq!(version.to_string(), text);
        }
    }

    #[test]
    fn versions_order_pre_release_before_one() {
        let parse = |text: &str| text.parse::<FormatVersion>().expect("a version");
        assert!(parse("0.2") < parse("0.10"));
        assert!(parse("0.10") < parse("1"));
        assert!(parse("1") < parse("2"));
    }

    #[test]
    fn malformed_versions_are_refused() {
        for text in ["", "0", "01", "1.0", "0.x", "0.01", "v1", "-1", "0.-1"] {
            assert!(text.parse::<FormatVersion>().is_err(), "{text:?} parsed");
        }
    }
}
