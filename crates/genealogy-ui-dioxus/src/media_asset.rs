//! Serving workspace-local media files to the webview (ADR 0017 §GUI).
//!
//! Multi-megabyte scans are served over a custom `/media/<rel>` asset handler rather than inlined as
//! data URIs. The path mapping is a pure, unit-tested [`resolve_media_path`] that mirrors the
//! host-side `media-store` sanitiser: it rejects traversal and only resolves paths that stay under the
//! workspace media root. The `use_asset_handler` registration is desktop-only (a webview is needed),
//! and is a no-op under the SSR tests, which never mount a window.

use std::path::{Component, Path, PathBuf};

/// Resolves a `/media/<rel>` asset request to an absolute path under `media_root`, or `None` when the
/// request is unsafe or malformed.
///
/// The `media` asset-handler prefix (`/media/`) is stripped, then the remainder must be a relative
/// path built only of normal components: an absolute path, a `..` parent component (literal or
/// percent-encoded), a backslash, a percent-encoded separator, or an empty path is rejected, so a
/// request can never escape `media_root`.
#[must_use]
pub fn resolve_media_path(media_root: &Path, request_path: &str) -> Option<PathBuf> {
    let rel = request_path
        .strip_prefix("/media/")
        .or_else(|| request_path.strip_prefix("media/"))
        .unwrap_or(request_path);

    if rel.is_empty() || rel.contains('\\') {
        return None;
    }
    let lowered = rel.to_ascii_lowercase();
    if lowered.contains("%2e") || lowered.contains("%2f") || lowered.contains("%5c") {
        return None;
    }

    let candidate = Path::new(rel);
    if candidate.is_absolute() {
        return None;
    }
    for component in candidate.components() {
        match component {
            Component::Normal(_) => {}
            Component::ParentDir | Component::CurDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    let resolved = media_root.join(candidate);
    resolved.starts_with(media_root).then_some(resolved)
}

/// The `Content-Type` for a served file, guessed from its extension. A small table covering the image
/// and document types the media library holds; unknown extensions fall back to `application/octet-stream`.
#[must_use]
pub fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
    {
        Some(ext) => match ext.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "tif" | "tiff" => "image/tiff",
            "bmp" => "image/bmp",
            "svg" => "image/svg+xml",
            "pdf" => "application/pdf",
            _ => "application/octet-stream",
        },
        None => "application/octet-stream",
    }
}

/// Registers the desktop `/media/<rel>` asset handler, serving files from `media_root` (the workspace's
/// media library). A no-op when the root is absent (e.g. an SSR test with no ready workspace).
#[cfg(feature = "desktop")]
pub fn use_media_asset_handler(media_root: Option<PathBuf>) {
    use dioxus::desktop::use_asset_handler;
    use dioxus::desktop::wry::RequestAsyncResponder;
    use dioxus::desktop::wry::http::Response;
    use dioxus::prelude::try_consume_context;

    // Registering an asset handler needs a real desktop window; under the SSR tests (which mount the
    // shell with the desktop feature compiled but no window context) this is a no-op. The context's
    // presence is fixed per mount, so the conditional hook keeps a consistent per-render order.
    if try_consume_context::<dioxus::desktop::DesktopContext>().is_none() {
        return;
    }
    let root = media_root.unwrap_or_default();
    use_asset_handler("media", move |request, responder: RequestAsyncResponder| {
        let body = resolve_media_path(&root, request.uri().path()).and_then(|path| {
            let bytes = std::fs::read(&path).ok()?;
            Some((content_type(&path), bytes))
        });
        let response = match body {
            Some((mime, bytes)) => Response::builder()
                .header("Content-Type", mime)
                .header("Access-Control-Allow-Origin", "*")
                .body(bytes),
            None => Response::builder().status(404).body(Vec::new()),
        };
        responder.respond(response.unwrap_or_else(|_| Response::new(Vec::new())));
    });
}

/// SSR/no-webview stub: nothing to serve without a desktop window.
#[cfg(not(feature = "desktop"))]
pub fn use_media_asset_handler(_media_root: Option<PathBuf>) {}

#[cfg(test)]
mod tests {
    use super::{content_type, resolve_media_path};
    use std::path::Path;

    fn root() -> &'static Path {
        Path::new("/workspace/media")
    }

    #[test]
    fn a_valid_nested_request_resolves_under_the_root() {
        let resolved = resolve_media_path(root(), "/media/05_personbilder/ada.jpg").expect("resolves");
        assert_eq!(resolved, Path::new("/workspace/media/05_personbilder/ada.jpg"));
    }

    #[test]
    fn a_request_without_the_prefix_still_resolves() {
        let resolved = resolve_media_path(root(), "photos/group.jpg").expect("resolves");
        assert_eq!(resolved, Path::new("/workspace/media/photos/group.jpg"));
    }

    #[test]
    fn a_parent_traversal_is_rejected() {
        assert_eq!(resolve_media_path(root(), "/media/../secret.txt"), None);
        assert_eq!(resolve_media_path(root(), "/media/a/../../etc/passwd"), None);
    }

    #[test]
    fn an_absolute_path_is_rejected() {
        assert_eq!(resolve_media_path(root(), "/media//etc/passwd"), None);
        assert_eq!(resolve_media_path(root(), "/etc/passwd"), None);
    }

    #[test]
    fn a_backslash_is_rejected() {
        assert_eq!(resolve_media_path(root(), "/media/..\\secret"), None);
        assert_eq!(resolve_media_path(root(), "/media/a\\b.jpg"), None);
    }

    #[test]
    fn percent_encoded_traversal_is_rejected() {
        assert_eq!(resolve_media_path(root(), "/media/%2e%2e/secret"), None);
        assert_eq!(resolve_media_path(root(), "/media/a%2Fb"), None);
    }

    #[test]
    fn an_empty_request_is_rejected() {
        assert_eq!(resolve_media_path(root(), "/media/"), None);
        assert_eq!(resolve_media_path(root(), ""), None);
    }

    #[test]
    fn content_type_is_guessed_from_the_extension() {
        assert_eq!(content_type(Path::new("a/b.JPG")), "image/jpeg");
        assert_eq!(content_type(Path::new("scan.png")), "image/png");
        assert_eq!(content_type(Path::new("doc.pdf")), "application/pdf");
        assert_eq!(content_type(Path::new("noext")), "application/octet-stream");
    }
}
