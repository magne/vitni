//! The media side of a backup (ADR 0041 §1, §2): `media.json` lists every file a media record points
//! at, and with `--with-media` the library files themselves travel under `media/`. A restore puts
//! the archived files back, then checks every listed file against its checksum.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use vitni_core::media_path::{media_root_relative, workspace_media_path};

use crate::backup::archive::{ArchiveReader, io_error};
use crate::backup::error::BackupError;
use crate::backup::format::MediaEntry;
use crate::error::AppError;
use crate::media::{file_checksum, list_media};
use crate::workspace::Workspace;

/// A file the backup found: its manifest entry, and where it is on disk when it is there.
pub(crate) struct FoundMedia {
    /// The `media.json` entry.
    pub(crate) entry: MediaEntry,
    /// The file on disk, if present.
    pub(crate) file: Option<PathBuf>,
    /// Whether it is in the workspace media library, so `--with-media` archives it.
    pub(crate) in_library: bool,
}

/// Where a stored media path points: the workspace library file, or a file outside the workspace.
/// `None` for a path that is neither (a `..` escape).
fn locate(media_root: &Path, stored: &str) -> Option<(String, PathBuf, bool)> {
    if Path::new(stored).is_absolute() {
        return Some((stored.to_owned(), PathBuf::from(stored), false));
    }
    let rel = media_root_relative(stored)?;
    Some((workspace_media_path(rel), media_root.join(rel), true))
}

/// Every distinct file the workspace's media records point at, in path order, each hashed from disk.
///
/// # Errors
///
/// [`AppError`] if the media projection cannot be read, or [`BackupError::Archive`] if a present
/// file cannot be read.
pub(crate) async fn collect(workspace: &Workspace) -> Result<Vec<FoundMedia>, AppError> {
    let media_root = workspace.media_root();
    let mut found = BTreeMap::new();
    for media in list_media(workspace).await? {
        let Some(stored) = media.file_path else {
            continue;
        };
        let Some((path, file, in_library)) = locate(&media_root, &stored) else {
            continue;
        };
        if found.contains_key(&path) {
            continue;
        }
        let item = match fs::metadata(&file) {
            Ok(metadata) => FoundMedia {
                entry: MediaEntry {
                    path: path.clone(),
                    checksum: Some(file_checksum(&file).map_err(|e| io_error(&file.display().to_string(), &e))?),
                    size: Some(metadata.len()),
                },
                file: Some(file),
                in_library,
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => FoundMedia {
                entry: MediaEntry {
                    path: path.clone(),
                    checksum: media.checksum,
                    size: None,
                },
                file: None,
                in_library,
            },
            Err(error) => return Err(io_error(&file.display().to_string(), &error).into()),
        };
        found.insert(path, item);
    }
    Ok(found.into_values().collect())
}

/// Extracts every archived library file into `workspace_dir`'s media library, except where a file already
/// exists at its path: a restore adds media, it never overwrites (ADR 0044 §5). Returns how many were
/// written.
///
/// # Errors
///
/// [`BackupError::UnexpectedMember`] for a member name that would land outside the media library,
/// or [`BackupError::Archive`] if a file cannot be written.
pub(crate) fn extract(
    archive: &mut ArchiveReader,
    names: &[String],
    workspace_dir: &Path,
) -> Result<usize, BackupError> {
    let media_root = workspace_dir.join(vitni_core::media_path::MEDIA_DIR);
    let mut extracted = 0;
    for name in names {
        let target = media_root.join(library_member_rel(name)?);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| io_error(&parent.display().to_string(), &e))?;
        }
        let mut out = match fs::File::create_new(&target) {
            Ok(out) => out,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(&target.display().to_string(), &error)),
        };
        io::copy(&mut archive.open_member(name)?, &mut out).map_err(|e| io_error(name, &e))?;
        extracted += 1;
    }
    Ok(extracted)
}

/// The path below the media root that library member `name` (`media/<rel>`) names.
///
/// # Errors
///
/// [`BackupError::UnexpectedMember`] when `name` is not a safe `media/<rel>` path.
pub(crate) fn library_member_rel(name: &str) -> Result<&str, BackupError> {
    name.strip_prefix(crate::backup::format::MEDIA_PREFIX)
        .and_then(media_root_relative)
        .ok_or_else(|| BackupError::UnexpectedMember(name.to_owned()))
}

/// What the restore found when it checked every listed file.
#[derive(Debug, Default)]
pub(crate) struct Verification {
    /// Listed files absent from where they belong.
    pub(crate) missing: Vec<String>,
    /// Listed files present, but with a different checksum.
    pub(crate) mismatched: Vec<String>,
}

/// Checks each entry against the file it names, below the restored workspace or where it lives
/// outside it.
///
/// # Errors
///
/// [`BackupError::Archive`] if a present file cannot be read.
pub(crate) fn verify(entries: &[MediaEntry], workspace_dir: &Path) -> Result<Verification, BackupError> {
    let media_root = workspace_dir.join(vitni_core::media_path::MEDIA_DIR);
    let mut verification = Verification::default();
    for entry in entries {
        let Some((_, file, _)) = locate(&media_root, &entry.path) else {
            verification.missing.push(entry.path.clone());
            continue;
        };
        match file_checksum(&file) {
            Ok(checksum) => {
                if entry.checksum.as_deref().is_some_and(|expected| expected != checksum) {
                    verification.mismatched.push(entry.path.clone());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => verification.missing.push(entry.path.clone()),
            Err(error) => return Err(io_error(&file.display().to_string(), &error)),
        }
    }
    Ok(verification)
}
