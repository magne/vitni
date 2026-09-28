//! [`BackupError`] — why a backup could not be written or an archive was refused (ADR 0041).

use std::path::PathBuf;

/// A backup or restore failure. Every refusal of an archive happens before the restore creates or
/// registers anything, so a refused restore leaves no trace.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    /// The backup destination already exists; a backup never overwrites a file.
    #[error("{} already exists", .0.display())]
    DestinationExists(PathBuf),
    /// The archive or a file it names could not be read or written.
    #[error("archive I/O failed: {0}")]
    Archive(String),
    /// The file is not a vitni backup (not a zip, or no readable `manifest.json`).
    #[error("not a vitni backup: {0}")]
    NotABackup(String),
    /// A member the manifest lists is absent from the archive.
    #[error("the archive is missing {0}")]
    MissingMember(String),
    /// The archive holds a member the manifest does not list.
    #[error("the archive holds {0}, which its manifest does not list")]
    UnexpectedMember(String),
    /// A member's content does not match the checksum the manifest records.
    #[error("{0} does not match its checksum; the archive is damaged")]
    ChecksumMismatch(String),
    /// The archive's format is older than this version restores (ADR 0041 §4).
    #[error(
        "backup format {found} is too old: this version restores {oldest_supported} and later; restore it with a \
         vitni release older than {readable_before}, then back up again"
    )]
    FormatTooOld {
        /// The archive's format version.
        found: String,
        /// The oldest format this version restores.
        oldest_supported: String,
        /// The first app version that no longer reads `found`.
        readable_before: String,
    },
    /// The archive was written by a newer vitni than this one.
    #[error("backup format {found} is newer than this version's {current}; upgrade vitni to restore it")]
    FormatTooNew {
        /// The archive's format version.
        found: String,
        /// The newest format this version writes and restores.
        current: String,
    },
    /// The archive names a format no vitni release has written.
    #[error("backup format {0} is not one any vitni release wrote")]
    UnknownFormat(String),
    /// An event row in `events.jsonl` does not decode, before or after upgrading.
    #[error("event on line {line} of events.jsonl is invalid: {detail}")]
    InvalidEvent {
        /// The 1-based line number.
        line: u64,
        /// Why it is invalid.
        detail: String,
    },
    /// The manifest's event count disagrees with the rows in `events.jsonl`.
    #[error("the manifest records {expected} events but the log holds {found}")]
    EventCountMismatch {
        /// The count the manifest records.
        expected: u64,
        /// The rows actually present.
        found: u64,
    },
    /// The restore target is a directory that is not empty.
    #[error("{} is not empty; restore into a new or empty directory", .0.display())]
    TargetNotEmpty(PathBuf),
    /// The restore target's database already holds events.
    #[error("the target database already holds events; restore into an empty database")]
    DatabaseNotEmpty,
}
