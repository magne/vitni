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
/// The `media` asset-handler prefix (`/media/`) is **required** and stripped exactly once, then the
/// remainder is percent-decoded ([`media_url_decode`](genealogy_app::media_url_decode)) and must be a
/// relative path built only of normal components: an absolute path, a `..` parent component, a
/// backslash, a malformed escape, bytes that are not UTF-8, or an empty path is rejected, so a request
/// can never escape `media_root`.
///
/// Decoding is what lets a stored filename carrying `æøå`, a space or a `#` resolve at all: the webview
/// encodes the `src` it was given, and the file on disk is named in the decoded alphabet. It also
/// subsumes the string guard this once ran over the *encoded* request — a decoded `%2e%2e` is an
/// ordinary `..` component, rejected by the walk below.
///
/// Every URL reaching here is built by one owner (`genealogy_ui`'s `media_asset_src`, over the same
/// `genealogy_core::media_path` rules this reads back), so the mapping is literal: a request missing the
/// prefix is a bug, not a shape to accommodate. Accommodating it is what let a double-prefixed
/// `/media/media/…` half-resolve instead of failing loudly (#301).
#[must_use]
pub fn resolve_media_path(media_root: &Path, request_path: &str) -> Option<PathBuf> {
    let rel = genealogy_app::media_url_decode(request_path.strip_prefix("/media/")?)?;

    let candidate = Path::new(&rel);
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

/// The `Content-Type` for a served file, from the extension→MIME mapping in
/// [`mime_for_path`](genealogy_app::mime_for_path) — the same one that decides whether a record with no
/// recorded MIME displays as an image, so the two can never disagree. An unknown or absent extension
/// (or a path that is not valid UTF-8) falls back to `application/octet-stream`.
#[must_use]
pub fn content_type(path: &Path) -> &'static str {
    path.to_str()
        .and_then(genealogy_app::mime_for_path)
        .unwrap_or("application/octet-stream")
}

/// Registers the desktop `/media/<rel>` asset handler, serving files from `media_root` (the workspace's
/// media library). Registers nothing when the root is absent — see the `error!` below.
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
    // An absent root is a wiring bug, not a servable state: the shell is only mounted under a ready
    // workspace. Defaulting it (as this once did) would resolve every request against the process's
    // working directory. Registering nothing is the honest failure — the preview shows its placeholder
    // and the log says why. Fixed per mount like the context above, so the hook order stays consistent.
    let Some(root) = media_root else {
        tracing::error!("no workspace media root — the /media asset handler is not registered");
        return;
    };
    use_asset_handler("media", move |request, responder: RequestAsyncResponder| {
        let request_path = request.uri().path().to_owned();
        let body = resolve_media_path(&root, &request_path).and_then(|path| {
            let bytes = std::fs::read(&path).ok()?;
            Some((content_type(&path), bytes))
        });
        let respond = |response: Result<Response<Vec<u8>>, _>| {
            responder.respond(response.unwrap_or_else(|_| Response::new(Vec::new())));
        };
        let Some((mime, bytes)) = body else {
            tracing::warn!(
                request = %request_path,
                media_root = %root.display(),
                "media asset request resolved to no readable file — serving 404"
            );
            respond(Response::builder().status(404).body(Vec::new()));
            return;
        };
        respond(
            Response::builder()
                .header("Content-Type", mime)
                .header("Access-Control-Allow-Origin", "*")
                .body(bytes),
        );
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
    fn a_request_without_the_prefix_is_rejected() {
        // The prefix is the asset-handler scheme, not an optional decoration: one owner
        // (`genealogy_core::media_path`) builds every URL, so the handler can be literal. The old
        // `or_else`/`unwrap_or` fallbacks are what let a double-prefixed request half-resolve (#301).
        assert_eq!(resolve_media_path(root(), "photos/group.jpg"), None);
        assert_eq!(resolve_media_path(root(), "media/photos/group.jpg"), None);
    }

    #[test]
    fn a_double_prefixed_request_is_no_longer_papered_over() {
        // `/media/media/x.jpg` now resolves literally — to a `media` directory *below* the root, which
        // is not where the file is. #301's URL builder is what must not produce this shape; the
        // handler no longer hides it.
        let resolved = resolve_media_path(root(), "/media/media/x.jpg").expect("resolves literally");
        assert_eq!(resolved, Path::new("/workspace/media/media/x.jpg"));
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
        // Rejected *after* decoding now: `%2e%2e` is an ordinary `..` component once decoded, which the
        // component walk already refuses. That is what makes a string guard over the raw request
        // redundant rather than a second line of defence.
        assert_eq!(resolve_media_path(root(), "/media/%2e%2e/secret"), None);
        assert_eq!(resolve_media_path(root(), "/media/a/%2E%2E/%2E%2E/etc/passwd"), None);
        assert_eq!(resolve_media_path(root(), "/media/%2fetc%2fpasswd"), None);
        assert_eq!(resolve_media_path(root(), "/media/a%5cb.jpg"), None);
    }

    #[test]
    fn a_percent_encoded_non_ascii_name_resolves_to_its_file() {
        // The live #301 symptom: the webview percent-encodes the non-ASCII bytes of the `src`, and
        // nothing decoded them, so the handler looked for a file whose name contained a literal `%C3`.
        let resolved = resolve_media_path(root(), "/media/01_kirkeb%C3%B8ker/f%C3%B8dsel.jpg").expect("resolves");
        assert_eq!(resolved, Path::new("/workspace/media/01_kirkebøker/fødsel.jpg"));
    }

    #[test]
    fn an_escaped_space_and_hash_resolve_to_their_file() {
        assert_eq!(
            resolve_media_path(root(), "/media/a%20b.jpg").expect("resolves"),
            Path::new("/workspace/media/a b.jpg")
        );
        assert_eq!(
            resolve_media_path(root(), "/media/scans/deed%20%234.pdf").expect("resolves"),
            Path::new("/workspace/media/scans/deed #4.pdf")
        );
    }

    #[test]
    fn an_escaped_separator_is_an_ordinary_separator_inside_the_root() {
        // No escape from the root, so no reason to refuse it — the containment check still holds.
        let resolved = resolve_media_path(root(), "/media/a%2Fb.jpg").expect("resolves");
        assert_eq!(resolved, Path::new("/workspace/media/a/b.jpg"));
    }

    #[test]
    fn a_malformed_escape_is_rejected() {
        assert_eq!(resolve_media_path(root(), "/media/%zz.jpg"), None);
        assert_eq!(resolve_media_path(root(), "/media/a%2.jpg"), None);
        assert_eq!(resolve_media_path(root(), "/media/a.jpg%"), None);
        assert_eq!(resolve_media_path(root(), "/media/f%f8dsel.jpg"), None, "not UTF-8");
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
