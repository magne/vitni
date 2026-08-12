//! [`MediaPath`] — where a [`Media`](crate::media) artifact lives (data-model §6, §10).
//!
//! A media object is either a local file (a relative or absolute path) or a web reference. Modelled
//! as an enum so the two are distinguishable rather than collapsed into one ambiguous string.
//!
//! This module also owns the two rules every layer above it has to agree on, so a stored path and a
//! rendered URL cannot disagree: the name of the workspace media directory ([`MEDIA_DIR`], with
//! [`workspace_media_path`] adding it and [`media_root_relative`] taking it back off) and the
//! extension→MIME mapping ([`mime_for_path`]) used to recognise an image whose record carries no MIME.
//! All three are pure string functions, so they stay inside core's no-I/O rules.

use serde::{Deserialize, Serialize};

use crate::text::Url;

/// The location of a media artifact (data-model §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum MediaPath {
    /// A filesystem path. The stored form for a file inside the workspace is workspace-relative and
    /// carries the media directory (`media/portraits/ada.jpg`, see [`workspace_media_path`]); a file
    /// held outside the workspace is stored absolute and is not served to a frontend.
    File(String),
    /// A web reference.
    Web(Url),
}

/// The workspace subdirectory holding the media library — the one place its name is spelled.
pub const MEDIA_DIR: &str = "media";

/// The stored, workspace-relative path of a file held at `rel` below the workspace media root:
/// `media/<rel>`. The inverse of [`media_root_relative`].
#[must_use]
pub fn workspace_media_path(rel: &str) -> String {
    format!("{MEDIA_DIR}/{rel}")
}

/// The path below the workspace media root that `stored` names, or `None` when it names no file the
/// media root can serve.
///
/// One optional leading `media/` is stripped, so both the stored form (`media/portraits/ada.jpg`) and
/// a bare root-relative path (`portraits/ada.jpg`) resolve to the same file — a path whose own first
/// component is called `media` keeps it. `None` for an empty remainder, an absolute path, a `..`
/// component, or a backslash: the same predicate the desktop asset handler enforces, so a location it
/// would refuse to serve is never turned into an image URL in the first place.
#[must_use]
pub fn media_root_relative(stored: &str) -> Option<&str> {
    let rel = stored
        .strip_prefix(MEDIA_DIR)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(stored);
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') {
        return None;
    }
    for segment in rel.split('/') {
        if segment == ".." {
            return None;
        }
    }
    Some(rel)
}

/// The MIME type `path`'s extension implies, or `None` when it has no extension or an unknown one.
///
/// Backs both the `Content-Type` a frontend serves a media file with and the "is this an image?"
/// display gate for a record that carries no MIME of its own. Extensions match case-insensitively.
#[must_use]
pub fn mime_for_path(path: &str) -> Option<&'static str> {
    mime_guess::from_path(path).first_raw()
}

#[cfg(test)]
mod tests {
    use super::MediaPath;
    use crate::text::Url;

    #[test]
    fn file_path_is_tagged() {
        let json = serde_json::to_value(MediaPath::File("photos/ada.jpg".to_owned())).unwrap();
        assert_eq!(json["type"], "File");
        assert_eq!(json["value"], "photos/ada.jpg");
    }

    #[test]
    fn web_path_round_trips() {
        let path = MediaPath::Web(Url {
            url_type: None,
            href: "https://example.org/img.png".to_owned(),
            description: None,
        });
        let json = serde_json::to_string(&path).unwrap();
        let back: MediaPath = serde_json::from_str(&json).unwrap();
        assert_eq!(path, back);
    }
}

#[cfg(test)]
mod media_root_tests {
    use super::{media_root_relative, mime_for_path, workspace_media_path};

    #[test]
    fn the_stored_prefix_is_stripped_once() {
        assert_eq!(
            media_root_relative("media/portraits/ada.jpg"),
            Some("portraits/ada.jpg")
        );
    }

    #[test]
    fn a_bare_root_relative_path_is_kept_as_is() {
        assert_eq!(media_root_relative("portraits/ada.jpg"), Some("portraits/ada.jpg"));
    }

    #[test]
    fn a_directory_of_its_own_called_media_keeps_it() {
        assert_eq!(media_root_relative("media/media/family.jpg"), Some("media/family.jpg"));
        assert_eq!(media_root_relative("mediaeval/plate.jpg"), Some("mediaeval/plate.jpg"));
    }

    #[test]
    fn an_absolute_path_names_no_file_under_the_root() {
        assert_eq!(media_root_relative("/home/ada/photos/ada.jpg"), None);
        assert_eq!(media_root_relative("media//etc/passwd"), None);
    }

    #[test]
    fn a_parent_component_names_no_file_under_the_root() {
        assert_eq!(media_root_relative("media/../secret.txt"), None);
        assert_eq!(media_root_relative("media/a/../../etc/passwd"), None);
        assert_eq!(media_root_relative("../secret.txt"), None);
    }

    #[test]
    fn a_backslash_names_no_file_under_the_root() {
        assert_eq!(media_root_relative("media/a\\b.jpg"), None);
    }

    #[test]
    fn an_empty_remainder_names_no_file() {
        assert_eq!(media_root_relative(""), None);
        assert_eq!(media_root_relative("media/"), None);
    }

    #[test]
    fn the_stored_form_round_trips_through_the_root_relative_form() {
        let stored = workspace_media_path("portraits/ada.jpg");
        assert_eq!(stored, "media/portraits/ada.jpg");
        assert_eq!(media_root_relative(&stored), Some("portraits/ada.jpg"));
    }

    #[test]
    fn a_mime_is_guessed_from_the_extension_case_insensitively() {
        assert_eq!(mime_for_path("portraits/ada.jpg"), Some("image/jpeg"));
        assert_eq!(mime_for_path("portraits/ADA.JPG"), Some("image/jpeg"));
        assert_eq!(mime_for_path("scan.png"), Some("image/png"));
        assert_eq!(mime_for_path("deed.pdf"), Some("application/pdf"));
    }

    #[test]
    fn a_path_with_no_usable_extension_implies_no_mime() {
        assert_eq!(mime_for_path("noext"), None);
        assert_eq!(mime_for_path("archive.qqq"), None);
    }
}
