//! [`MediaPath`] — where a [`Media`](crate::media) artifact lives (data-model §6, §10).
//!
//! A media object is either a local file (a relative or absolute path) or a web reference. Modelled
//! as an enum so the two are distinguishable rather than collapsed into one ambiguous string.

use serde::{Deserialize, Serialize};

use crate::text::Url;

/// The location of a media artifact (data-model §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum MediaPath {
    /// A filesystem path (relative to the workspace media root, or absolute).
    File(String),
    /// A web reference.
    Web(Url),
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
