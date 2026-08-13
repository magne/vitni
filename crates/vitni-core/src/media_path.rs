//! [`MediaPath`] — where a [`Media`](crate::media) artifact lives (data-model §6, §10).
//!
//! A media object is either a local file (a relative or absolute path) or a web reference. Modelled
//! as an enum so the two are distinguishable rather than collapsed into one ambiguous string.
//!
//! This module also owns the rules every layer above it has to agree on, so a stored path and a
//! rendered URL cannot disagree: the name of the workspace media directory ([`MEDIA_DIR`], with
//! [`workspace_media_path`] adding it and [`media_root_relative`] taking it back off), the
//! percent-encoding of the served `/media/<rel>` URL space ([`media_url_path`] building it and
//! [`media_url_decode`] reading it back), and the extension→MIME mapping ([`mime_for_path`]) used to
//! recognise an image whose record carries no MIME. All are pure string functions, so they stay inside
//! core's no-I/O rules.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
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
    is_root_relative(rel).then_some(rel)
}

/// Whether `rel` names a file the media root can serve: non-empty, relative, no `..` component and no
/// backslash. The one predicate [`media_root_relative`] and [`media_url_decode`] share, so a stored
/// path and a decoded request are judged by the same rule.
fn is_root_relative(rel: &str) -> bool {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') {
        return false;
    }
    for segment in rel.split('/') {
        if segment == ".." {
            return false;
        }
    }
    true
}

/// Everything outside RFC 3986's unreserved set (`ALPHA / DIGIT / - . _ ~`) is escaped, so a Nordic
/// letter, a space, and each of `#`, `?`, `%` and `+` survives a round trip through the URL.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_').remove(b'~');

/// The `/media/…` URL body serving the file at `rel` below the workspace media root: each
/// `/`-separated segment percent-encoded, the separators kept.
///
/// The inverse of [`media_url_decode`]. The webview would encode a non-ASCII byte on its own, but
/// only encoding here makes the rule symmetric: a literal `#`, `?` or `%` in a filename otherwise
/// takes on its URL meaning before the request is even made.
#[must_use]
pub fn media_url_path(rel: &str) -> String {
    let mut encoded = String::with_capacity(rel.len());
    for (index, segment) in rel.split('/').enumerate() {
        if index > 0 {
            encoded.push('/');
        }
        encoded.extend(utf8_percent_encode(segment, SEGMENT));
    }
    encoded
}

/// The path below the workspace media root that the percent-encoded URL body `encoded` names, or
/// `None` when it names no file the media root can serve.
///
/// Decoded as UTF-8 first, then judged by the same predicate as [`media_root_relative`] — which is
/// why no separate guard over the *encoded* form is needed: a `%2e%2e` is an ordinary `..` component
/// once decoded, and so is rejected there. A decoded `%2f` is an ordinary separator *inside* the root,
/// not an escape from it, so it is allowed. `None` for a malformed escape (non-hex or truncated),
/// bytes that are not UTF-8, an absolute result, a `..` component, a backslash, or an empty path.
#[must_use]
pub fn media_url_decode(encoded: &str) -> Option<String> {
    if !escapes_are_well_formed(encoded) {
        return None;
    }
    let decoded = percent_decode_str(encoded).decode_utf8().ok()?;
    is_root_relative(&decoded).then(|| decoded.into_owned())
}

/// Whether every `%` in `encoded` introduces a complete two-hex-digit escape. `percent-encoding`
/// passes a malformed escape through as literal text; a request built by [`media_url_path`] never
/// contains one, so it is a malformed request rather than a filename that happens to hold a `%`.
fn escapes_are_well_formed(encoded: &str) -> bool {
    let mut parts = encoded.split('%');
    parts.next();
    for part in parts {
        let Some(escape) = part.as_bytes().get(..2) else {
            return false;
        };
        if !escape.iter().all(u8::is_ascii_hexdigit) {
            return false;
        }
    }
    true
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
mod media_url_tests {
    use super::{media_url_decode, media_url_path};
    use proptest::prelude::{Strategy, prop, prop_assert_eq, proptest};

    #[test]
    fn a_nordic_name_survives_the_round_trip() {
        let rel = "02_folketelling/1920/1920_greipstad_folketelling_asbjørn-andreassen-bergstøl.jpg";
        let encoded = media_url_path(rel);
        assert_eq!(
            encoded,
            "02_folketelling/1920/1920_greipstad_folketelling_asbj%C3%B8rn-andreassen-bergst%C3%B8l.jpg"
        );
        assert_eq!(media_url_decode(&encoded).as_deref(), Some(rel));
    }

    #[test]
    fn every_character_a_stored_name_may_carry_survives_the_round_trip() {
        // `slugify` and the plugin host's `sanitize_component` both keep `æøå`; an operator's own file
        // may carry a space, and `#`, `?`, `%` and `+` each have a URL meaning that must not leak.
        for rel in [
            "01_kirkebøker/1801_ål_fødsel_bjørn.jpg",
            "portraits/a b.png",
            "scans/deed #4.pdf",
            "scans/what?.pdf",
            "scans/100% sure.pdf",
            "scans/a+b.pdf",
            "01_kirkebøker/fødsel.jpg",
        ] {
            let encoded = media_url_path(rel);
            assert!(
                encoded.is_ascii(),
                "the served URL is ASCII: {rel:?} encoded to {encoded:?}"
            );
            assert_eq!(
                media_url_decode(&encoded).as_deref(),
                Some(rel),
                "round trip of {rel:?}"
            );
        }
    }

    #[test]
    fn the_separator_is_kept_and_everything_else_is_escaped() {
        assert_eq!(media_url_path("a/b/c.jpg"), "a/b/c.jpg");
        assert_eq!(media_url_path("a b/c#d.jpg"), "a%20b/c%23d.jpg");
        assert_eq!(
            media_url_path("~x-y_z.1"),
            "~x-y_z.1",
            "unreserved characters stay literal"
        );
    }

    #[test]
    fn a_percent_encoded_parent_component_names_no_file() {
        // Decode-then-validate is what makes a string guard over the raw request redundant: once
        // decoded, `%2e%2e` is an ordinary `..` component the predicate already rejects.
        assert_eq!(media_url_decode("%2e%2e/secret"), None);
        assert_eq!(media_url_decode("a/%2E%2E/%2E%2E/etc/passwd"), None);
        assert_eq!(media_url_decode("../secret"), None);
    }

    #[test]
    fn a_percent_encoded_separator_is_an_ordinary_separator_inside_the_root() {
        assert_eq!(media_url_decode("a%2fb.jpg").as_deref(), Some("a/b.jpg"));
        assert_eq!(media_url_decode("a%2Fb.jpg").as_deref(), Some("a/b.jpg"));
    }

    #[test]
    fn a_malformed_escape_names_no_file() {
        assert_eq!(media_url_decode("%zz.jpg"), None, "not hex");
        assert_eq!(media_url_decode("a%2.jpg"), None, "truncated by a following character");
        assert_eq!(media_url_decode("a.jpg%"), None, "a trailing percent");
        assert_eq!(media_url_decode("a%%41.jpg"), None, "a percent introducing a percent");
    }

    #[test]
    fn an_escape_that_is_not_utf8_names_no_file() {
        assert_eq!(media_url_decode("f%f8dsel.jpg"), None, "latin-1 ø is not UTF-8");
    }

    #[test]
    fn a_decoded_absolute_path_or_backslash_names_no_file() {
        assert_eq!(media_url_decode("%2fetc%2fpasswd"), None, "absolute once decoded");
        assert_eq!(media_url_decode("a%5cb.jpg"), None, "a backslash once decoded");
        assert_eq!(media_url_decode("a\\b.jpg"), None, "a literal backslash");
        assert_eq!(media_url_decode(""), None, "nothing to serve");
    }

    /// Path segments of the alphabet real stored names use: Nordic letters, spaces and the URL-special
    /// characters, but no `/`, no leading/trailing space-only segment trouble, and never `..`.
    fn segment() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop::sample::select(vec![
                'a', 'Z', '0', 'ø', 'æ', 'å', ' ', '#', '?', '%', '+', '.', '-', '_', '~',
            ]),
            1..8,
        )
        .prop_map(|chars| chars.into_iter().collect::<String>())
        .prop_filter("a `..` segment is not servable", |segment| segment != "..")
    }

    proptest! {
        #[test]
        fn any_relative_path_of_normal_segments_round_trips(segments in prop::collection::vec(segment(), 1..5)) {
            let rel = segments.join("/");
            let decoded = media_url_decode(&media_url_path(&rel));
            prop_assert_eq!(decoded.as_deref(), Some(rel.as_str()));
        }
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
