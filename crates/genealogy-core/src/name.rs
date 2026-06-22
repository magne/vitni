//! Personal-name value objects (data-model §7, §14).
//!
//! The surname *list* (kept from Gramps) handles patronymics and multi-part surnames better
//! than GEDCOM's single field. Names carry an optional language and a list of
//! transliterations (alternate scripts / romanisations of the same name — GEDCOM 7 `NAME.TRAN`).

use serde::{Deserialize, Serialize};

use crate::date::GenealogicalDate;

/// A BCP-47 language tag, e.g. `nb`, `en-US`, `zh-pinyin`, `sr-Cyrl` (data-model §7, §14).
///
/// The script is a BCP-47 subtag, so no separate script field is needed. Reused by
/// [`PersonName`], place names, and note text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LanguageTag(String);

impl LanguageTag {
    /// Wraps a BCP-47 language tag.
    #[must_use]
    pub fn new(tag: impl Into<String>) -> Self {
        Self(tag.into())
    }

    /// Returns the tag as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The kind of name (closed set plus a custom escape hatch — data-model §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum NameType {
    /// The name a person was born with.
    BirthName,
    /// A name taken at marriage.
    MarriedName,
    /// A maiden name held before marriage (GEDCOM 7 `MAIDEN`).
    Maiden,
    /// A name taken at immigration (GEDCOM 7 `IMMIGRANT`).
    Immigrant,
    /// A professional / occupational name (GEDCOM 7 `PROFESSIONAL`).
    Professional,
    /// A nickname / also-known-as.
    AlsoKnownAs,
    /// A name used in religious life.
    ReligiousName,
    /// An application-defined name type.
    Custom(String),
}

/// One surname element of a [`PersonName`] (data-model §7).
///
/// Keeping surnames as a list (rather than one field) models patronymics, multi-part names,
/// and prefixes/connectors faithfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surname {
    /// An optional prefix (e.g. `van`, `de`).
    pub prefix: Option<String>,
    /// The surname text itself.
    pub surname: String,
    /// Whether this is the primary surname for sorting/display.
    pub primary: bool,
    /// An optional connector to the next surname element (e.g. `y`, `e`).
    pub connector: Option<String>,
}

/// A personal name (data-model §7).
///
/// `transliterations` holds alternate-script / romanised forms of *this same name*
/// (GEDCOM 7 `NAME.TRAN`); each is itself a `PersonName` to carry its own structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonName {
    /// The kind of name.
    pub name_type: NameType,
    /// The given name(s).
    pub given: Option<String>,
    /// The surname element(s).
    pub surnames: Vec<Surname>,
    /// A suffix (e.g. `Jr.`).
    pub suffix: Option<String>,
    /// A title (e.g. `Dr.`).
    pub title: Option<String>,
    /// A nickname.
    pub nickname: Option<String>,
    /// The call name (the given name used in daily life).
    pub call_name: Option<String>,
    /// The date this name was in use, if known.
    pub date: Option<GenealogicalDate>,
    /// The language of this name.
    pub language: Option<LanguageTag>,
    /// Alternate-script / romanised forms of this same name (GEDCOM 7 `NAME.TRAN`).
    pub transliterations: Vec<PersonName>,
}

impl PersonName {
    /// Returns `true` if the name has neither a given name nor any surname.
    ///
    /// Used by the Person aggregate to reject an empty `NameAsserted` (data-model §10.1
    /// `EmptyName`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let has_given = self.given.as_ref().is_some_and(|g| !g.trim().is_empty());
        let has_surname = self.surnames.iter().any(|s| !s.surname.trim().is_empty());
        !has_given && !has_surname
    }
}

#[cfg(test)]
mod tests {
    use super::{LanguageTag, NameType, PersonName, Surname};

    fn surname(text: &str) -> Surname {
        Surname {
            prefix: None,
            surname: text.to_owned(),
            primary: true,
            connector: None,
        }
    }

    fn name(given: Option<&str>, surnames: Vec<Surname>) -> PersonName {
        PersonName {
            name_type: NameType::BirthName,
            given: given.map(ToOwned::to_owned),
            surnames,
            suffix: None,
            title: None,
            nickname: None,
            call_name: None,
            date: None,
            language: Some(LanguageTag::new("nb")),
            transliterations: Vec::new(),
        }
    }

    #[test]
    fn name_round_trips_through_json() {
        let original = name(Some("Ada"), vec![surname("Lovelace")]);
        let json = serde_json::to_string(&original).unwrap();
        let back: PersonName = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn name_with_given_only_is_not_empty() {
        assert!(!name(Some("Ada"), Vec::new()).is_empty());
    }

    #[test]
    fn name_with_surname_only_is_not_empty() {
        assert!(!name(None, vec![surname("Lovelace")]).is_empty());
    }

    #[test]
    fn name_with_neither_given_nor_surname_is_empty() {
        assert!(name(None, Vec::new()).is_empty());
        assert!(name(Some("   "), vec![surname("  ")]).is_empty());
    }

    #[test]
    fn custom_name_type_is_tagged() {
        let json = serde_json::to_value(NameType::Custom("Hypocorism".to_owned())).unwrap();
        assert_eq!(json["type"], "Custom");
        assert_eq!(json["value"], "Hypocorism");
    }
}
