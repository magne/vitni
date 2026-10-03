//! Workspace backup and restore (ADR 0041).
//!
//! A backup is the event log: one `.vitni-backup` zip holding `events.jsonl` (every stored row, in
//! store order), a portable `workspace.toml`, `media.json` and, with `--with-media`, the media library
//! files, all listed with their checksums in `manifest.json`. Internal provenance — record origins,
//! identity decisions — lives in the rows, so a backup keeps what an export drops.
//!
//! A restore checks the whole archive before it creates anything: the format is in the window, every
//! member is present and matches its checksum, and every row decodes after the upgraders ran. It then
//! registers a new workspace, inserts the rows as stored (no command is re-decided), and rebuilds the
//! projections. It is engine-neutral: a SQLite backup restores into Postgres and back. A restore that
//! fails after registering rolls the registration and the new directory back.
//!
//! A restore can instead replace the open workspace (ADR 0044). After the same checks it backs the
//! workspace up into `backups/` unless the workspace switched that off, then swaps the log and
//! rebuilds the projections in one transaction, so a failure at any step leaves the workspace as it
//! was.

mod archive;
mod error;
mod format;
mod media;
mod upgrade;

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use time::{OffsetDateTime, UtcOffset};
use vitni_core::provenance::Timestamp;
use vitni_db::{DbError, RawEvent};

use crate::backup::archive::{ArchiveReader, ArchiveWriter, HashingReader, io_error};
pub use crate::backup::error::BackupError;
use crate::backup::format::{BackupManifest, BackupRow, EVENTS, MANIFEST, MEDIA, MEDIA_PREFIX, MediaEntry, WORKSPACE};
use crate::backup::upgrade::Chain;
use crate::config::default_workspace_dir;
use crate::config_store::{ConfigStore, FileConfigStore};
use crate::error::AppError;
use crate::workspace::{BACKUPS_DIR, Workspace, read_manifest, write_manifest};
use crate::workspace_registry::{Registration, WorkspaceSummary, register_new_workspace, unregister_workspace};

/// How many rows one page of the log read holds while a backup streams it.
const PAGE: u32 = 1000;

/// What to back up, and where.
#[derive(Debug, Clone)]
pub struct BackupRequest<'a> {
    /// The workspace's registry name, recorded in the manifest.
    pub workspace_name: &'a str,
    /// The archive to write; it must not exist.
    pub destination: &'a Path,
    /// Whether to carry the media library files, not only their manifest.
    pub with_media: bool,
    /// When the backup is taken (from the session clock), recorded in the manifest.
    pub created_at: OffsetDateTime,
}

/// What a backup wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupReport {
    /// The archive written.
    pub destination: PathBuf,
    /// The events it holds.
    pub events: u64,
    /// The media library files it carries.
    pub media_files: usize,
    /// Files media records point at that were not on disk, by stored path.
    pub media_missing: Vec<String>,
}

/// Which archive to restore, and the new workspace to restore it into.
#[derive(Debug, Clone)]
pub struct RestoreRequest<'a> {
    /// The global config the new workspace is registered in.
    pub config_path: &'a Path,
    /// The archive to restore.
    pub archive: &'a Path,
    /// The new workspace's registry name.
    pub name: &'a str,
    /// The new workspace's directory (absent or empty); `None` uses the default location.
    pub dir: Option<&'a Path>,
    /// The new workspace's database; `None` uses the configured default engine.
    pub database_url: Option<&'a str>,
}

/// What a restore produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    /// The new workspace, registered and made the default.
    pub workspace: WorkspaceSummary,
    /// The archive's format version, as written.
    pub format_version: String,
    /// The events inserted.
    pub events: u64,
    /// The media library files put back from the archive.
    pub media_restored: usize,
    /// Listed media files not found after the restore, by stored path.
    pub media_missing: Vec<String>,
    /// Listed media files found with a different checksum, by stored path.
    pub media_mismatched: Vec<String>,
}

/// Which archive to restore over the open workspace (ADR 0044).
#[derive(Debug, Clone)]
pub struct ReplaceRequest<'a> {
    /// The archive to restore.
    pub archive: &'a Path,
    /// The open workspace's registry name, which names the pre-restore backup.
    pub workspace_name: &'a str,
    /// When the replace runs (from the session clock), which dates the pre-restore backup.
    pub now: OffsetDateTime,
}

/// What a replace produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceReport {
    /// The backup of the previous state, or `None` when the workspace switched it off.
    pub pre_restore_backup: Option<PathBuf>,
    /// The archive's format version, as written.
    pub format_version: String,
    /// The events the workspace now holds.
    pub events: u64,
    /// The media library files added from the archive.
    pub media_restored: usize,
    /// Listed media files not found after the replace, by stored path.
    pub media_missing: Vec<String>,
    /// Listed media files found with a different checksum, by stored path.
    pub media_mismatched: Vec<String>,
}

/// The backup format this version writes, as text (`0.1`).
#[must_use]
pub fn current_format_version() -> String {
    upgrade::current().to_string()
}

/// A digest of every projection row, the state a golden fixture must restore to (ADR 0041 §5).
///
/// Each row is hashed as one canonical JSON line — `[table, view_id, version, payload]`, object keys
/// sorted — so the digest depends on what the projections hold, not on the engine's JSON text.
///
/// # Errors
///
/// [`AppError::Db`] if a projection cannot be read.
pub async fn projection_digest(workspace: &Workspace) -> Result<String, AppError> {
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    for row in workspace.store().projection_rows().await? {
        let line = serde_json::json!([row.table, row.view_id, row.version, row.payload]);
        sha2::Digest::update(&mut hasher, line.to_string().as_bytes());
        sha2::Digest::update(&mut hasher, b"\n");
    }
    Ok(archive::checksum_text(hasher))
}

/// Writes a backup of `workspace` (ADR 0041 §1). A failed backup removes its partial archive.
///
/// # Errors
///
/// [`BackupError::DestinationExists`] if the destination exists, [`BackupError::Archive`] if it
/// cannot be written, or [`AppError`] if the log or a projection cannot be read.
pub async fn create_backup(workspace: &Workspace, request: &BackupRequest<'_>) -> Result<BackupReport, AppError> {
    let destination = request.destination;
    let file = fs::File::create_new(destination).map_err(|error| match error.kind() {
        std::io::ErrorKind::AlreadyExists => BackupError::DestinationExists(destination.to_path_buf()),
        _ => io_error(&destination.display().to_string(), &error),
    })?;
    let result = write_backup(workspace, request, ArchiveWriter::new(file)).await;
    if result.is_err()
        && let Err(remove_error) = fs::remove_file(destination)
    {
        tracing::warn!(%remove_error, path = %destination.display(), "could not remove a partial backup");
    }
    result
}

/// Streams the log, the portable manifest and the media into `archive`, then its manifest.
async fn write_backup(
    workspace: &Workspace,
    request: &BackupRequest<'_>,
    mut archive: ArchiveWriter,
) -> Result<BackupReport, AppError> {
    let events = write_events(workspace, &mut archive).await?;
    archive.member(WORKSPACE, portable_workspace_manifest(workspace.dir())?.as_bytes())?;
    let found = media::collect(workspace).await?;
    let entries: Vec<&MediaEntry> = found.iter().map(|item| &item.entry).collect();
    let media_json = serde_json::to_vec_pretty(&entries).map_err(|e| io_error(MEDIA, &e))?;
    archive.member(MEDIA, &media_json)?;
    let mut media_files = 0;
    let mut media_missing = Vec::new();
    for item in &found {
        let Some(file) = &item.file else {
            media_missing.push(item.entry.path.clone());
            continue;
        };
        if request.with_media && item.in_library {
            archive.file(&item.entry.path, file)?;
            media_files += 1;
        }
    }
    let manifest = BackupManifest {
        format_version: upgrade::current().to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        workspace: request.workspace_name.to_owned(),
        event_count: events,
        created_at: Timestamp::new(request.created_at),
        members: archive.checksums().clone(),
    };
    let manifest = serde_json::to_vec_pretty(&manifest).map_err(|e| io_error(MANIFEST, &e))?;
    archive.finish(MANIFEST, &manifest)?;
    Ok(BackupReport {
        destination: request.destination.to_path_buf(),
        events,
        media_files,
        media_missing,
    })
}

/// Streams every stored row into `events.jsonl`, a page at a time, returning the row count.
async fn write_events(workspace: &Workspace, archive: &mut ArchiveWriter) -> Result<u64, AppError> {
    archive.start(EVENTS)?;
    let mut count = 0;
    let mut after = None;
    loop {
        let page = workspace.store().read_raw_events(after.as_ref(), PAGE).await?;
        let Some(last) = page.last() else {
            return Ok(count);
        };
        after = Some(last.key());
        for event in page {
            let mut line = serde_json::to_vec(&BackupRow::from(event)).map_err(|e| io_error(EVENTS, &e))?;
            line.push(b'\n');
            archive.write(&line)?;
            count += 1;
        }
    }
}

/// The workspace manifest without its machine-local fields: the database location and the window
/// geometry belong to this machine, not to the research.
fn portable_workspace_manifest(dir: &Path) -> Result<String, AppError> {
    let mut manifest = read_manifest(dir)?;
    manifest.ui.window = None;
    let mut table = toml::Table::try_from(&manifest).map_err(|e| io_error(WORKSPACE, &e))?;
    table.remove("database_url");
    toml::to_string_pretty(&table).map_err(|e| io_error(WORKSPACE, &e).into())
}

/// An archive that passed every check, ready to restore.
struct CheckedArchive {
    format_version: String,
    chain: Chain,
    workspace: toml::Table,
    media: Vec<MediaEntry>,
    media_members: Vec<String>,
}

/// Restores an archive into a new workspace (ADR 0041 §2).
///
/// # Errors
///
/// A [`BackupError`] refusing the archive or the target, reported before anything is created;
/// [`AppError::Config`] if the name is taken; or [`AppError`] if the restore fails after
/// registering, in which case the registration and the new directory are rolled back.
pub async fn restore_backup(request: &RestoreRequest<'_>) -> Result<RestoreReport, AppError> {
    let dir = match request.dir {
        Some(dir) => dir.to_path_buf(),
        None => default_workspace_dir(request.name.trim())?,
    };
    check_target_dir(&dir)?;
    let mut archive = ArchiveReader::open(request.archive)?;
    let checked = check_archive(&mut archive)?;
    let registration =
        register_new_workspace(request.config_path, request.name, Some(&dir), request.database_url).await?;
    match populate(request.config_path, &mut archive, &checked, &registration).await {
        Ok(report) => Ok(report),
        Err(error) => {
            roll_back(request.config_path, &registration);
            Err(error)
        }
    }
}

/// Restores an archive over the open `workspace` (ADR 0044): its log and projections are replaced by
/// the archive's.
///
/// The archive is checked first. Then, unless the workspace switched it off, the workspace is backed
/// up into its `backups/` directory, and a failure there aborts the replace. The log is swapped and
/// the projections rebuilt in one transaction. The archived settings are adopted afterwards, keeping
/// this copy's database, window and backup setting, and archived media is added where no file exists.
/// The caller ensures no command runs against the workspace meanwhile.
///
/// # Errors
///
/// A [`BackupError`] refusing the archive, [`BackupError::PreRestoreBackup`], or [`AppError`] if the
/// swap fails; in each case the log and projections are unchanged. An error after the swap (the
/// manifest or the media) leaves the archive's log in place.
pub async fn replace_backup(workspace: &Workspace, request: &ReplaceRequest<'_>) -> Result<ReplaceReport, AppError> {
    let mut archive = ArchiveReader::open(request.archive)?;
    let checked = check_archive(&mut archive)?;
    let dir = workspace.dir();
    let pre_restore_backup = if read_manifest(dir)?.backup.pre_restore {
        Some(write_pre_restore_backup(workspace, request).await?)
    } else {
        None
    };
    let reader = BufReader::new(archive.open_member(EVENTS)?);
    let events = workspace
        .store()
        .replace_all_events(decoded_rows(reader, &checked.chain))
        .await?;
    replace_workspace_manifest(dir, &checked.workspace)?;
    let media_restored = media::extract(&mut archive, &checked.media_members, dir)?;
    let verification = media::verify(&checked.media, dir)?;
    Ok(ReplaceReport {
        pre_restore_backup,
        format_version: checked.format_version,
        events,
        media_restored,
        media_missing: verification.missing,
        media_mismatched: verification.mismatched,
    })
}

/// Backs `workspace` up to `backups/<name>-pre-restore-<UTC timestamp>.vitni-backup`, returning
/// the path, or [`BackupError::PreRestoreBackup`].
async fn write_pre_restore_backup(workspace: &Workspace, request: &ReplaceRequest<'_>) -> Result<PathBuf, AppError> {
    let now = request.now.to_offset(UtcOffset::UTC);
    let file_name = format!(
        "{}-pre-restore-{:04}{:02}{:02}T{:02}{:02}{:02}Z.vitni-backup",
        request.workspace_name.replace(['/', '\\'], "-"),
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    );
    let path = workspace.dir().join(BACKUPS_DIR).join(file_name);
    let backup = BackupRequest {
        workspace_name: request.workspace_name,
        destination: &path,
        with_media: false,
        created_at: request.now,
    };
    match create_backup(workspace, &backup).await {
        Ok(_) => Ok(path),
        Err(error) => Err(BackupError::PreRestoreBackup {
            path,
            source: Box::new(error),
        }
        .into()),
    }
}

/// Refuses a target directory that holds anything.
fn check_target_dir(dir: &Path) -> Result<(), BackupError> {
    let mut entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(&dir.display().to_string(), &error)),
    };
    if entries.next().is_some() {
        return Err(BackupError::TargetNotEmpty(dir.to_path_buf()));
    }
    Ok(())
}

/// Reads the manifest, puts it through the upgrade chain, and checks every member against it.
fn check_archive(archive: &mut ArchiveReader) -> Result<CheckedArchive, BackupError> {
    let (format_version, chain, manifest) = read_backup_manifest(archive)?;
    for name in archive.names() {
        if name != MANIFEST && !manifest.members.contains_key(&name) {
            return Err(BackupError::UnexpectedMember(name));
        }
    }
    let mut media_members = Vec::new();
    for (name, expected) in &manifest.members {
        if !archive.contains(name) {
            return Err(BackupError::MissingMember(name.clone()));
        }
        if name.starts_with(MEDIA_PREFIX) {
            media::library_member_rel(name)?;
            media_members.push(name.clone());
        }
        if name != EVENTS && archive.checksum(name)? != *expected {
            return Err(BackupError::ChecksumMismatch(name.clone()));
        }
    }
    for required in [EVENTS, WORKSPACE, MEDIA] {
        if !manifest.members.contains_key(required) {
            return Err(BackupError::MissingMember(required.to_owned()));
        }
    }
    check_events(archive, &chain, &manifest)?;
    let workspace = String::from_utf8(archive.read(WORKSPACE)?)
        .map_err(|e| e.to_string())
        .and_then(|text| toml::from_str(&text).map_err(|e| e.to_string()))
        .map_err(|detail| BackupError::NotABackup(format!("{WORKSPACE}: {detail}")))?;
    let media =
        serde_json::from_slice(&archive.read(MEDIA)?).map_err(|e| BackupError::NotABackup(format!("{MEDIA}: {e}")))?;
    Ok(CheckedArchive {
        format_version,
        chain,
        workspace,
        media,
        media_members,
    })
}

/// Reads `manifest.json`, finds the chain for its format and upgrades it to the current one.
fn read_backup_manifest(archive: &mut ArchiveReader) -> Result<(String, Chain, BackupManifest), BackupError> {
    let bytes = archive.read(MANIFEST).map_err(|error| match error {
        BackupError::MissingMember(_) => BackupError::NotABackup(format!("it has no {MANIFEST}")),
        other => other,
    })?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| BackupError::NotABackup(format!("{MANIFEST}: {e}")))?;
    let Some(format_version) = value.get("format_version").and_then(serde_json::Value::as_str) else {
        return Err(BackupError::NotABackup(format!("{MANIFEST} names no format_version")));
    };
    let format_version = format_version.to_owned();
    let chain = upgrade::chain_for(&format_version)?;
    let manifest = chain
        .manifest(value)
        .and_then(|value| serde_json::from_value(value).map_err(|e| e.to_string()))
        .map_err(|detail| BackupError::NotABackup(format!("{MANIFEST}: {detail}")))?;
    Ok((format_version, chain, manifest))
}

/// Upgrades and decodes one `events.jsonl` line into the row to insert.
fn decode_line(chain: &Chain, line: &str) -> Result<RawEvent, String> {
    let value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    let row: BackupRow = serde_json::from_value(chain.row(value)?).map_err(|e| e.to_string())?;
    let event = RawEvent::from(row);
    vitni_db::decode_raw_event(&event).map_err(|e| e.to_string())?;
    Ok(event)
}

/// Reads `events.jsonl` once: checks its checksum, that every row decodes, and the row count. A
/// damaged member is reported as such even when the damage also breaks a row.
fn check_events(archive: &mut ArchiveReader, chain: &Chain, manifest: &BackupManifest) -> Result<(), BackupError> {
    let mut reader = BufReader::new(HashingReader::new(archive.open_member(EVENTS)?));
    let mut count = 0_u64;
    let mut first_invalid = None;
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).map_err(|e| io_error(EVENTS, &e))?;
        if read == 0 {
            break;
        }
        count += 1;
        if first_invalid.is_none()
            && let Err(detail) = decode_line(chain, line.trim_end_matches('\n'))
        {
            first_invalid = Some(BackupError::InvalidEvent { line: count, detail });
        }
    }
    let Some(expected) = manifest.members.get(EVENTS) else {
        return Err(BackupError::MissingMember(EVENTS.to_owned()));
    };
    if reader.into_inner().checksum() != *expected {
        return Err(BackupError::ChecksumMismatch(EVENTS.to_owned()));
    }
    if let Some(invalid) = first_invalid {
        return Err(invalid);
    }
    if count != manifest.event_count {
        return Err(BackupError::EventCountMismatch {
            expected: manifest.event_count,
            found: count,
        });
    }
    Ok(())
}

/// Fills the registered workspace: its manifest settings, the rows, the projections and the media.
async fn populate(
    config_path: &Path,
    archive: &mut ArchiveReader,
    checked: &CheckedArchive,
    registration: &Registration,
) -> Result<RestoreReport, AppError> {
    let dir = &registration.summary.path;
    restore_workspace_manifest(dir, &checked.workspace)?;
    let config = FileConfigStore::new(config_path.to_path_buf(), None).load_or_bootstrap_config()?;
    let workspace = Workspace::open(dir, &config.operator, &config.workspace_defaults).await?;
    if workspace.store().event_count().await? != 0 {
        return Err(BackupError::DatabaseNotEmpty.into());
    }
    let events = insert_events(&workspace, archive, &checked.chain).await?;
    let finished = finish_restore(&workspace, archive, checked, dir).await;
    let (media_restored, verification) = match finished {
        Ok(finished) => finished,
        Err(error) => {
            // A server database outlives the directory `roll_back` removes, so empty it here.
            if let Err(discard_error) = workspace.store().discard_all_events().await {
                tracing::warn!(%discard_error, "could not empty the database after a failed restore");
            }
            return Err(error);
        }
    };
    Ok(RestoreReport {
        workspace: registration.summary.clone(),
        format_version: checked.format_version.clone(),
        events,
        media_restored,
        media_missing: verification.missing,
        media_mismatched: verification.mismatched,
    })
}

/// The steps after the rows are in: rebuild the projections, put the media back and check it.
async fn finish_restore(
    workspace: &Workspace,
    archive: &mut ArchiveReader,
    checked: &CheckedArchive,
    dir: &Path,
) -> Result<(usize, media::Verification), AppError> {
    workspace.rebuild_projections().await?;
    let media_restored = media::extract(archive, &checked.media_members, dir)?;
    let verification = media::verify(&checked.media, dir)?;
    Ok((media_restored, verification))
}

/// Streams `events.jsonl` into the store in one transaction.
async fn insert_events(workspace: &Workspace, archive: &mut ArchiveReader, chain: &Chain) -> Result<u64, AppError> {
    let reader = BufReader::new(archive.open_member(EVENTS)?);
    Ok(workspace.store().insert_raw_events(decoded_rows(reader, chain)).await?)
}

/// The rows of `events.jsonl`, read a line at a time and upgraded through `chain`.
fn decoded_rows<'a>(
    reader: impl BufRead + 'a,
    chain: &'a Chain,
) -> impl Iterator<Item = Result<RawEvent, DbError>> + 'a {
    reader.lines().map(move |line| {
        let line = line.map_err(|e| DbError::Backend(format!("reading {EVENTS}: {e}")))?;
        decode_line(chain, &line).map_err(DbError::Malformed)
    })
}

/// Writes the archived settings over the new workspace's manifest, keeping this machine's database
/// location and every operator either side knows.
fn restore_workspace_manifest(dir: &Path, archived: &toml::Table) -> Result<(), AppError> {
    let fresh = read_manifest(dir)?;
    let mut table = archived.clone();
    table.insert(
        "database_url".to_owned(),
        toml::Value::String(fresh.database_url.clone()),
    );
    let mut manifest: crate::workspace::WorkspaceManifest = table
        .try_into()
        .map_err(|e: toml::de::Error| BackupError::NotABackup(format!("{WORKSPACE}: {e}")))?;
    for (id, record) in fresh.operators {
        manifest.operators.entry(id).or_insert(record);
    }
    write_manifest(dir, &manifest)
}

/// Writes the archived settings over the replaced workspace's manifest, keeping what belongs to this
/// copy — its database location, window geometry and backup setting — and every operator either side
/// knows (ADR 0044 §4).
fn replace_workspace_manifest(dir: &Path, archived: &toml::Table) -> Result<(), AppError> {
    let current = read_manifest(dir)?;
    restore_workspace_manifest(dir, archived)?;
    let mut manifest = read_manifest(dir)?;
    manifest.ui.window = current.ui.window;
    manifest.backup = current.backup;
    write_manifest(dir, &manifest)
}

/// Undoes a registration whose restore failed: the config entry, then the directory's contents.
fn roll_back(config_path: &Path, registration: &Registration) {
    unregister_workspace(config_path, registration);
    let dir = &registration.summary.path;
    let result = if registration.dir_preexisted {
        fs::read_dir(dir).and_then(|entries| {
            for entry in entries {
                let path = entry?.path();
                if path.is_dir() {
                    fs::remove_dir_all(&path)?;
                } else {
                    fs::remove_file(&path)?;
                }
            }
            Ok(())
        })
    } else {
        fs::remove_dir_all(dir)
    };
    if let Err(remove_error) = result {
        tracing::warn!(%remove_error, path = %dir.display(), "could not remove a failed restore's workspace");
    }
}
