//! [`PlaceName`] — a dated, language-tagged name for a [`Place`](crate::place) (data-model §7, §14).
//!
//! A place is named differently across time and language (a Norwegian parish has a Norwegian and a
//! Latin church-record form; farm names shift spelling over centuries). So a place name is not one
//! string but a value carrying its text, the language it is in, and the date it was in use — the
//! same evidence-aware shape [`PersonName`](crate::name::PersonName) uses.

use serde::{Deserialize, Serialize};

use crate::date::GenealogicalDate;
use crate::name::LanguageTag;

/// One name a place is or was known by (data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceName {
    /// The name text.
    pub text: String,
    /// The language this name is in, if known.
    pub language: Option<LanguageTag>,
    /// The date this name was in use, if known.
    pub date: Option<GenealogicalDate>,
}

impl PlaceName {
    /// Returns `true` if the name has no non-whitespace text.
    ///
    /// Used by the Place aggregate to reject an empty `AssertName` (data-model §10.1
    /// `EmptyRequiredField`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::PlaceName;
    use crate::name::LanguageTag;

    fn plain(text: &str) -> PlaceName {
        PlaceName {
            text: text.to_owned(),
            language: Some(LanguageTag::new("nb")),
            date: None,
        }
    }

    #[test]
    fn place_name_round_trips_through_json() {
        let original = plain("Vågå");
        let json = serde_json::to_string(&original).unwrap();
        let back: PlaceName = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn blank_text_is_empty() {
        assert!(plain("   ").is_empty());
        assert!(!plain("Vågå").is_empty());
    }
}
