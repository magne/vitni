//! Host-owned media-library writes for the `media-store` capability (ADR 0017 §3).
//!
//! Every write lands strictly under the workspace `media/` root. The plugin proposes a relative
//! path; the host sanitizes it (no absolute path, no `..`, no backslash; each component's forbidden
//! characters replaced) and refuses anything that would escape the root. Files are checksummed with
//! SHA-256 (`"sha256:<hex>"`); an identical file already at the target path is a dedup hit
//! (`existed = true`), while different bytes at a taken path uniquify (`-2`, `-3`, …). Writes are
//! atomic (temp file in the target directory, then rename).

use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::net::{self, NetPolicy};

/// Characters not allowed in a stored path component (Windows-reserved plus the path separators the
/// sanitizer handles structurally). Replaced with `_`; letters like `æøå` are kept.
const FORBIDDEN: &[char] = &['<', '>', ':', '"', '|', '?', '*', '/', '\\'];

/// The result of a media-store write, mirroring the WIT `stored-media` record.
#[derive(Debug)]
pub struct Stored {
    /// Workspace-relative path with forward slashes (`media/…`).
    pub relative_path: String,
    /// `"sha256:<hex>"`.
    pub checksum: String,
    /// The sniffed MIME type.
    pub mime: String,
    /// The file size in bytes.
    pub size: u64,
    /// Whether an identical file was already present (dedup hit).
    pub existed: bool,
}

/// Why a media-store write failed.
#[derive(Debug)]
pub enum MediaError {
    /// The suggested path was unsafe or unusable.
    InvalidPath(String),
    /// A network policy or transport failure while downloading (`fetch-and-store`).
    Net(String),
    /// A filesystem failure.
    Backend(String),
}

/// Sanitizes a plugin-proposed relative path into a safe, workspace-relative [`PathBuf`] (no leading
/// `media/` — that is added when the target is resolved). Rejects absolute paths, `..` components,
/// backslashes, and paths with no usable component.
pub fn sanitize_relative_path(suggested: &str) -> Result<PathBuf, MediaError> {
    if suggested.trim().is_empty() {
        return Err(MediaError::InvalidPath("the suggested path is empty".to_owned()));
    }
    if suggested.contains('\\') {
        return Err(MediaError::InvalidPath(
            "backslashes are not permitted in a path".to_owned(),
        ));
    }
    let mut out = PathBuf::new();
    for component in Path::new(suggested).components() {
        match component {
            Component::Normal(raw) => {
                let name = raw
                    .to_str()
                    .ok_or_else(|| MediaError::InvalidPath("path has a non-UTF-8 component".to_owned()))?;
                out.push(sanitize_component(name)?);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(MediaError::InvalidPath("`..` components are not permitted".to_owned()));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(MediaError::InvalidPath("absolute paths are not permitted".to_owned()));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(MediaError::InvalidPath("the path has no usable component".to_owned()));
    }
    Ok(out)
}

/// Resolves a workspace-relative media path (as `media-store` returns, e.g.
/// `"media/kirkebok/scan.jpg"`) to its absolute on-disk location, verifying it stays under
/// `media_root`. Reuses [`sanitize_relative_path`] to reject absolute paths, `..`, and backslashes,
/// then requires the resolved target to sit under the media root. Returns the absolute path (for
/// reading the file) and the sanitized relative path (to hand to a subprocess run with `workspace_dir`
/// as its cwd). Used by the `ai` capability to validate its `media-path` (ADR 0017 §4).
///
/// # Errors
///
/// [`MediaError::InvalidPath`] if the path is unsafe or resolves outside the media root;
/// [`MediaError::Backend`] if the media root cannot be canonicalized.
pub fn resolve_under_media_root(
    workspace_dir: &Path,
    media_root: &Path,
    path: &str,
) -> Result<(PathBuf, PathBuf), MediaError> {
    let rel = sanitize_relative_path(path)?;
    let abs = workspace_dir.join(&rel);
    let canonical_root = media_root
        .canonicalize()
        .map_err(|error| MediaError::Backend(format!("resolving media root {}: {error}", media_root.display())))?;
    if !abs.starts_with(&canonical_root) && !abs.starts_with(media_root) {
        return Err(MediaError::InvalidPath(
            "the media path is not under the workspace media root".to_owned(),
        ));
    }
    Ok((abs, rel))
}

/// Replaces forbidden and control characters in one path component, rejecting a component that is
/// empty or becomes empty (e.g. `.` / `..`-adjacent dot runs).
fn sanitize_component(name: &str) -> Result<String, MediaError> {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || FORBIDDEN.contains(&c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches(['.', ' ']);
    if trimmed.is_empty() {
        return Err(MediaError::InvalidPath(format!(
            "path component `{name}` sanitizes to nothing"
        )));
    }
    Ok(trimmed.to_owned())
}

/// Lowercase hex encoding of `bytes`.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// `"sha256:<hex>"` of `bytes`.
fn checksum_of(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex(&hasher.finalize()))
}

/// `"sha256:<hex>"` of a file already on disk (for dedup comparison).
fn checksum_of_file(path: &Path) -> Result<String, MediaError> {
    let bytes = fs::read(path).map_err(|error| MediaError::Backend(format!("reading {}: {error}", path.display())))?;
    Ok(checksum_of(&bytes))
}

/// Inserts `-<n>` before the extension of `base` (`dir/name.ext` → `dir/name-2.ext`).
fn uniquified(base: &Path, n: u32) -> PathBuf {
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let renamed = match base.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{stem}-{n}.{ext}"),
        None => format!("{stem}-{n}"),
    };
    match base.parent() {
        Some(parent) => parent.join(renamed),
        None => PathBuf::from(renamed),
    }
}

/// Resolves the final on-disk target for `rel` under `media_root`, applying dedup: an existing file
/// with the same `checksum` is a hit (`existed = true`); a different file at the path uniquifies.
/// Also enforces containment under the (canonical) media root.
fn resolve_target(media_root: &Path, rel: &Path, checksum: &str) -> Result<(PathBuf, bool), MediaError> {
    let base = media_root.join(rel);
    let canonical_root = media_root
        .canonicalize()
        .map_err(|error| MediaError::Backend(format!("resolving media root {}: {error}", media_root.display())))?;
    if !base.starts_with(&canonical_root) && !base.starts_with(media_root) {
        return Err(MediaError::InvalidPath(
            "the resolved path escapes the media root".to_owned(),
        ));
    }
    let mut candidate = base.clone();
    let mut n = 1;
    loop {
        if !candidate.exists() {
            return Ok((candidate, false));
        }
        if checksum_of_file(&candidate)? == checksum {
            return Ok((candidate, true));
        }
        n += 1;
        candidate = uniquified(&base, n);
    }
}

/// The MIME type: the response `Content-Type` when meaningful, else the path extension, else
/// `application/octet-stream`.
fn mime_of(content_type: Option<&str>, rel: &Path) -> String {
    if let Some(header) = content_type {
        let value = header.split(';').next().unwrap_or(header).trim();
        if !value.is_empty() && value != "application/octet-stream" {
            return value.to_owned();
        }
    }
    mime_guess::from_path(rel)
        .first_raw()
        .unwrap_or("application/octet-stream")
        .to_owned()
}

/// The workspace-relative path string (`media/…`, forward slashes) for a resolved target.
fn workspace_relative(media_root: &Path, target: &Path) -> String {
    let sub = target.strip_prefix(media_root).unwrap_or(target);
    let mut parts = vec!["media".to_owned()];
    for component in sub.components() {
        if let Component::Normal(raw) = component {
            parts.push(raw.to_string_lossy().into_owned());
        }
    }
    parts.join("/")
}

/// Writes `bytes` to `target` atomically: a uniquely-named temp file in the target's directory,
/// flushed and renamed into place.
fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), MediaError> {
    let parent = target
        .parent()
        .ok_or_else(|| MediaError::InvalidPath("the target has no parent directory".to_owned()))?;
    fs::create_dir_all(parent)
        .map_err(|error| MediaError::Backend(format!("creating {}: {error}", parent.display())))?;
    let temp = parent.join(format!(".tmp-{}", Uuid::now_v7()));
    let mut file =
        File::create(&temp).map_err(|error| MediaError::Backend(format!("creating {}: {error}", temp.display())))?;
    file.write_all(bytes)
        .map_err(|error| MediaError::Backend(format!("writing {}: {error}", temp.display())))?;
    file.sync_all()
        .map_err(|error| MediaError::Backend(format!("flushing {}: {error}", temp.display())))?;
    drop(file);
    fs::rename(&temp, target).map_err(|error| {
        let _ = fs::remove_file(&temp);
        MediaError::Backend(format!("renaming into {}: {error}", target.display()))
    })
}

/// Stores guest-held `bytes` under `suggested_path` within `media_root`. The MIME is inferred from
/// the path extension.
pub fn store_bytes(media_root: &Path, suggested_path: &str, bytes: &[u8]) -> Result<Stored, MediaError> {
    let rel = sanitize_relative_path(suggested_path)?;
    let checksum = checksum_of(bytes);
    let (target, existed) = resolve_target(media_root, &rel, &checksum)?;
    if !existed {
        write_atomic(&target, bytes)?;
    }
    Ok(Stored {
        relative_path: workspace_relative(media_root, &target),
        checksum,
        mime: mime_of(None, &rel),
        size: bytes.len() as u64,
        existed,
    })
}

/// Downloads `url` under `policy` and stores it under `suggested_path` within `media_root`, streaming
/// to a temp file (never holding the whole binary in memory) and checksumming as it goes. The MIME
/// prefers the response `Content-Type`, falling back to the path extension.
pub async fn fetch_and_store(
    policy: &NetPolicy,
    media_root: &Path,
    suggested_path: &str,
    url: &str,
) -> Result<Stored, MediaError> {
    let rel = sanitize_relative_path(suggested_path)?;
    let temp = media_root.join(format!(".tmp-{}", Uuid::now_v7()));
    fs::create_dir_all(media_root)
        .map_err(|error| MediaError::Backend(format!("creating {}: {error}", media_root.display())))?;
    let mut file =
        File::create(&temp).map_err(|error| MediaError::Backend(format!("creating {}: {error}", temp.display())))?;
    let mut hasher = Sha256::new();

    let download = net::stream_to(policy, url, |chunk| {
        hasher.update(chunk);
        file.write_all(chunk)
            .map_err(|error| net::NetError::Backend(error.to_string()))
    })
    .await;

    let meta = match download {
        Ok(meta) => meta,
        Err(error) => {
            drop(file);
            let _ = fs::remove_file(&temp);
            return Err(net_error(error));
        }
    };
    file.sync_all()
        .map_err(|error| MediaError::Backend(format!("flushing {}: {error}", temp.display())))?;
    drop(file);

    let checksum = format!("sha256:{}", hex(&hasher.finalize()));
    let resolve = resolve_target(media_root, &rel, &checksum);
    let (target, existed) = match resolve {
        Ok(resolved) => resolved,
        Err(error) => {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
    };

    if existed {
        let _ = fs::remove_file(&temp);
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| MediaError::Backend(format!("creating {}: {error}", parent.display())))?;
        }
        fs::rename(&temp, &target).map_err(|error| {
            let _ = fs::remove_file(&temp);
            MediaError::Backend(format!("renaming into {}: {error}", target.display()))
        })?;
    }

    Ok(Stored {
        relative_path: workspace_relative(media_root, &target),
        checksum,
        mime: mime_of(meta.content_type.as_deref(), &rel),
        size: meta.size,
        existed,
    })
}

/// Maps a [`net::NetError`] onto a [`MediaError`] for the `fetch-and-store` path.
fn net_error(error: net::NetError) -> MediaError {
    match error {
        net::NetError::Policy(message) | net::NetError::InvalidUrl(message) => MediaError::Net(message),
        net::NetError::TooLarge => MediaError::Net("the download exceeded the binary size cap".to_owned()),
        net::NetError::Timeout => MediaError::Net("the download timed out".to_owned()),
        net::NetError::Backend(message) => MediaError::Backend(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sha256_vector() {
        // NIST test vector: SHA-256("abc").
        assert_eq!(
            checksum_of(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn empty_path_is_rejected() {
        assert!(matches!(sanitize_relative_path("   "), Err(MediaError::InvalidPath(_))));
    }

    #[test]
    fn absolute_path_is_rejected() {
        assert!(matches!(
            sanitize_relative_path("/etc/passwd"),
            Err(MediaError::InvalidPath(_))
        ));
    }

    #[test]
    fn parent_traversal_is_rejected() {
        assert!(matches!(
            sanitize_relative_path("../../secret.txt"),
            Err(MediaError::InvalidPath(_))
        ));
        assert!(matches!(
            sanitize_relative_path("kirkebok/../../escape.jpg"),
            Err(MediaError::InvalidPath(_))
        ));
    }

    #[test]
    fn backslash_is_rejected() {
        assert!(matches!(
            sanitize_relative_path("kirkebok\\evil.jpg"),
            Err(MediaError::InvalidPath(_))
        ));
    }

    #[test]
    fn dot_components_are_dropped() {
        let path = sanitize_relative_path("./kirkebok/./scan.jpg").expect("sanitizes");
        assert_eq!(path, PathBuf::from("kirkebok/scan.jpg"));
    }

    #[test]
    fn nordic_letters_are_kept_and_forbidden_chars_replaced() {
        let path = sanitize_relative_path("01_kirkebøker/1801_ål_fødsel_bjørn:*.jpg").expect("sanitizes");
        assert_eq!(path, PathBuf::from("01_kirkebøker/1801_ål_fødsel_bjørn__.jpg"));
    }

    #[test]
    fn a_component_that_is_only_dots_is_rejected() {
        // A lone `...` is a `Normal` component (not `ParentDir`); trimming dots leaves nothing.
        assert!(matches!(
            sanitize_relative_path("kirkebok/..."),
            Err(MediaError::InvalidPath(_))
        ));
    }

    #[test]
    fn mime_prefers_header_then_extension() {
        assert_eq!(mime_of(Some("image/jpeg"), Path::new("scan.png")), "image/jpeg");
        assert_eq!(
            mime_of(Some("image/jpeg; charset=binary"), Path::new("scan.bin")),
            "image/jpeg"
        );
        assert_eq!(
            mime_of(Some("application/octet-stream"), Path::new("scan.png")),
            "image/png"
        );
        assert_eq!(mime_of(None, Path::new("scan.jpg")), "image/jpeg");
        assert_eq!(mime_of(None, Path::new("scan.unknownext")), "application/octet-stream");
    }

    #[test]
    fn uniquified_inserts_before_extension() {
        assert_eq!(
            uniquified(Path::new("dir/name.jpg"), 2),
            PathBuf::from("dir/name-2.jpg")
        );
        assert_eq!(uniquified(Path::new("name"), 3), PathBuf::from("name-3"));
    }

    #[test]
    fn resolve_under_media_root_accepts_media_paths_and_rejects_escapes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path();
        let media_root = ws.join("media");
        std::fs::create_dir_all(&media_root).expect("media dir");

        let (abs, rel) = resolve_under_media_root(ws, &media_root, "media/kirkebok/scan.jpg").expect("resolve");
        assert_eq!(rel, PathBuf::from("media/kirkebok/scan.jpg"));
        assert!(abs.ends_with("media/kirkebok/scan.jpg"));

        // A workspace-relative path that is not under `media/` is refused.
        assert!(matches!(
            resolve_under_media_root(ws, &media_root, "secret.txt"),
            Err(MediaError::InvalidPath(_))
        ));
        // Traversal and absolute paths are refused by the shared sanitizer.
        assert!(resolve_under_media_root(ws, &media_root, "../escape.jpg").is_err());
        assert!(resolve_under_media_root(ws, &media_root, "/etc/passwd").is_err());
    }

    #[test]
    fn store_dedup_and_uniquify_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        let first = store_bytes(root, "kirkebok/scan.jpg", b"content-a").expect("store");
        assert!(!first.existed);
        assert_eq!(first.relative_path, "media/kirkebok/scan.jpg");
        assert_eq!(first.size, 9);

        // Same path, same bytes → dedup hit, no new file.
        let again = store_bytes(root, "kirkebok/scan.jpg", b"content-a").expect("store");
        assert!(again.existed);
        assert_eq!(again.relative_path, "media/kirkebok/scan.jpg");

        // Same path, different bytes → uniquify to -2.
        let collision = store_bytes(root, "kirkebok/scan.jpg", b"content-b").expect("store");
        assert!(!collision.existed);
        assert_eq!(collision.relative_path, "media/kirkebok/scan-2.jpg");

        assert_eq!(fs::read(root.join("kirkebok/scan.jpg")).expect("read"), b"content-a");
        assert_eq!(fs::read(root.join("kirkebok/scan-2.jpg")).expect("read"), b"content-b");
    }
}
