//! Place-type vocabulary (ADR 0031 §3, §4): [`PlaceKind`], [`PlaceLevel`], [`PlaceTypeKey`] and the
//! [`PlaceTypeStatement`] that carries them.

use serde::{Deserialize, Serialize};

use crate::date::{GenealogicalDate, GenealogicalDateBody};
use crate::place::PlaceError;

/// What sort of structure a place is (ADR 0031 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    /// A civil administrative unit.
    Administrative,
    /// A church unit.
    Ecclesiastical,
    /// A court district.
    Judicial,
    /// A civil-registration district.
    Registration,
    /// A tax or fiscal unit.
    Fiscal,
    /// A military district.
    Military,
    /// A manorial unit.
    Manorial,
    /// A land-register (cadastral) unit.
    Cadastral,
    /// A statistical unit.
    Statistical,
    /// A populated place.
    Settlement,
    /// A building or structure.
    Building,
    /// A natural feature.
    Natural,
}

impl PlaceKind {
    /// Every kind, in declaration order.
    pub const ALL: [Self; 12] = [
        Self::Administrative,
        Self::Ecclesiastical,
        Self::Judicial,
        Self::Registration,
        Self::Fiscal,
        Self::Military,
        Self::Manorial,
        Self::Cadastral,
        Self::Statistical,
        Self::Settlement,
        Self::Building,
        Self::Natural,
    ];
}

/// A place's rank in a hierarchy, after Getty and `GeoNames` (ADR 0031 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceLevel {
    /// A country.
    Country,
    /// First administrative level.
    Adm1,
    /// Second administrative level.
    Adm2,
    /// Third administrative level.
    Adm3,
    /// Fourth administrative level.
    Adm4,
    /// Fifth administrative level.
    Adm5,
    /// A settlement.
    Settlement,
    /// A part of a settlement.
    SubSettlement,
}

impl PlaceLevel {
    /// Every level, in declaration order.
    pub const ALL: [Self; 8] = [
        Self::Country,
        Self::Adm1,
        Self::Adm2,
        Self::Adm3,
        Self::Adm4,
        Self::Adm5,
        Self::Settlement,
        Self::SubSettlement,
    ];
}

/// Why a string is not a valid [`PlaceTypeKey`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "invalid place type key {0:?}: expected `<namespace>:<name>` with a lower-case `[a-z0-9]+` namespace \
     and a `[a-z0-9_-]+` name, or `custom:<non-blank text>`"
)]
pub struct InvalidPlaceTypeKey(String);

const CUSTOM_NAMESPACE: &str = "custom";

/// A language-neutral place-type code, `<namespace>:<name>` (ADR 0031 §4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PlaceTypeKey(String);

impl PlaceTypeKey {
    /// Validates and wraps `text`.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidPlaceTypeKey`] naming `text` when it is not a well-formed key.
    pub fn new(text: &str) -> Result<Self, InvalidPlaceTypeKey> {
        let invalid = || InvalidPlaceTypeKey(text.to_owned());
        let (namespace, name) = text.split_once(':').ok_or_else(invalid)?;
        let valid = if namespace == CUSTOM_NAMESPACE {
            !name.trim().is_empty()
        } else {
            is_slug(namespace, false) && is_slug(name, true)
        };
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(invalid())
        }
    }

    /// Returns `true` if the key is in the `custom` namespace.
    #[must_use]
    pub fn is_custom(&self) -> bool {
        self.namespace() == CUSTOM_NAMESPACE
    }

    /// The whole key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The part before the first `:`.
    #[must_use]
    pub fn namespace(&self) -> &str {
        self.split().0
    }

    /// The part after the first `:`.
    #[must_use]
    pub fn name(&self) -> &str {
        self.split().1
    }

    fn split(&self) -> (&str, &str) {
        self.0.split_once(':').unwrap_or((&self.0, ""))
    }
}

fn is_slug(text: &str, allow_separators: bool) -> bool {
    !text.is_empty()
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || (allow_separators && matches!(byte, b'-' | b'_'))
        })
}

impl TryFrom<String> for PlaceTypeKey {
    type Error = InvalidPlaceTypeKey;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(&text)
    }
}

impl From<PlaceTypeKey> for String {
    fn from(key: PlaceTypeKey) -> Self {
        key.0
    }
}

/// One dated statement about a place's type (ADR 0031 §3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceTypeStatement {
    /// The kind, if stated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PlaceKind>,
    /// The level, if stated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<PlaceLevel>,
    /// The type key, if stated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_key: Option<PlaceTypeKey>,
    /// When the statement held, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<GenealogicalDate>,
}

impl PlaceTypeStatement {
    /// Returns `true` if the statement names no kind, level or key (a date alone says nothing).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kind.is_none() && self.level.is_none() && self.type_key.is_none()
    }

    /// Checks the statement against the domain rules every writer shares: a date must state a kind, a level
    /// or a key, and must be a calendar date, not free text, which has no sort value to resolve or supersede
    /// by (ADR 0054).
    ///
    /// # Errors
    ///
    /// [`PlaceError::EmptyPlaceType`] for a date with nothing stated, [`PlaceError::TextOnlyPlaceTypeDate`]
    /// for a free-text date.
    pub fn validate(&self) -> Result<(), PlaceError> {
        if self.date.is_some() && self.is_empty() {
            return Err(PlaceError::EmptyPlaceType);
        }
        if self.has_text_only_date() {
            return Err(PlaceError::TextOnlyPlaceTypeDate);
        }
        Ok(())
    }

    /// The date's `sort_value`, by which two statements count as stated at the same date (ADR 0054).
    #[must_use]
    pub fn date_sort_value(&self) -> Option<i64> {
        self.date.as_ref().map(|date| date.sort_value)
    }

    /// Returns `true` if the statement is dated by free text, which has no calendar date.
    #[must_use]
    pub fn has_text_only_date(&self) -> bool {
        self.date
            .as_ref()
            .is_some_and(|date| matches!(date.modifier, GenealogicalDateBody::TextOnly { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::{PlaceKind, PlaceLevel, PlaceTypeKey, PlaceTypeStatement};
    use crate::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
    use proptest::prelude::*;
    use std::collections::HashSet;

    fn a_date() -> GenealogicalDate {
        GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
                year: Some(1847),
                month: None,
                day: None,
            })),
            time: None,
            new_year_begins: None,
            sort_value: 18_470_000,
            original_text: None,
        }
    }

    #[test]
    fn place_kind_has_twelve_distinct_variants() {
        assert_eq!(PlaceKind::ALL.len(), 12);
        let distinct: HashSet<PlaceKind> = PlaceKind::ALL.into_iter().collect();
        assert_eq!(distinct.len(), PlaceKind::ALL.len());
    }

    #[test]
    fn place_level_has_eight_distinct_variants() {
        assert_eq!(PlaceLevel::ALL.len(), 8);
        let distinct: HashSet<PlaceLevel> = PlaceLevel::ALL.into_iter().collect();
        assert_eq!(distinct.len(), PlaceLevel::ALL.len());
    }

    #[test]
    fn place_level_serializes_as_pascal_case() {
        assert_eq!(serde_json::to_string(&PlaceLevel::Adm1).unwrap(), "\"Adm1\"");
        assert_eq!(
            serde_json::to_string(&PlaceLevel::SubSettlement).unwrap(),
            "\"SubSettlement\""
        );
        assert_eq!(serde_json::to_string(&PlaceLevel::Country).unwrap(), "\"Country\"");
    }

    #[test]
    fn place_kind_serializes_as_pascal_case() {
        assert_eq!(
            serde_json::to_string(&PlaceKind::Administrative).unwrap(),
            "\"Administrative\""
        );
        assert_eq!(
            serde_json::to_string(&PlaceKind::Ecclesiastical).unwrap(),
            "\"Ecclesiastical\""
        );
    }

    #[test]
    fn every_kind_and_level_round_trips_through_json() {
        for kind in PlaceKind::ALL {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(serde_json::from_str::<PlaceKind>(&json).unwrap(), kind);
        }
        for level in PlaceLevel::ALL {
            let json = serde_json::to_string(&level).unwrap();
            assert_eq!(serde_json::from_str::<PlaceLevel>(&json).unwrap(), level);
        }
    }

    #[test]
    fn key_accepts_valid_forms() {
        for input in [
            "std:farm",
            "no:prestegjeld",
            "se:harad",
            "us:county",
            "ie:townland",
            "custom:Sokn",
            "custom:with spaces:and colons",
            "custom:std:foo",
        ] {
            let key = PlaceTypeKey::new(input).unwrap_or_else(|error| panic!("{input:?} rejected: {error}"));
            assert_eq!(key.as_str(), input);
        }
    }

    #[test]
    fn key_exposes_namespace_and_name() {
        let key = PlaceTypeKey::new("no:prestegjeld").unwrap();
        assert_eq!(key.namespace(), "no");
        assert_eq!(key.name(), "prestegjeld");

        let custom = PlaceTypeKey::new("custom:std:foo").unwrap();
        assert_eq!(custom.namespace(), "custom");
        assert_eq!(custom.name(), "std:foo");
    }

    #[test]
    fn key_rejects_malformed_input_naming_it() {
        for input in ["farm", "", "No:x", "x:Y", "x:", ":x", "a b:c", "custom:", "custom:   "] {
            let error = PlaceTypeKey::new(input).expect_err(input);
            assert!(
                error.to_string().contains(input),
                "error {error:?} does not name {input:?}"
            );
        }
    }

    #[test]
    fn key_deserialization_validates() {
        let key: PlaceTypeKey = serde_json::from_str("\"std:farm\"").unwrap();
        assert_eq!(key.as_str(), "std:farm");
        assert_eq!(serde_json::to_string(&key).unwrap(), "\"std:farm\"");
        for input in ["\"farm\"", "\"\"", "\"No:x\"", "\"custom:   \""] {
            assert!(serde_json::from_str::<PlaceTypeKey>(input).is_err(), "{input} accepted");
        }
    }

    proptest! {
        #[test]
        fn accepted_keys_round_trip_through_json(
            standard in "[a-z][a-z0-9]{0,5}:[a-z][a-z0-9_-]{0,10}",
            custom in "custom:[A-Za-z0-9 :]{0,10}[A-Za-z0-9]",
        ) {
            for text in [standard, custom] {
                let key = PlaceTypeKey::new(&text).unwrap();
                prop_assert_eq!(key.as_str(), text.as_str());
                let json = serde_json::to_string(&key).unwrap();
                let back: PlaceTypeKey = serde_json::from_str(&json).unwrap();
                prop_assert_eq!(back, key);
            }
        }
    }

    #[test]
    fn default_statement_is_empty() {
        let statement = PlaceTypeStatement::default();
        assert!(statement.is_empty());
        assert_eq!(statement.kind, None);
        assert_eq!(statement.level, None);
        assert_eq!(statement.type_key, None);
        assert_eq!(statement.date, None);
        assert_eq!(serde_json::to_string(&statement).unwrap(), "{}");
    }

    #[test]
    fn statement_with_only_a_date_is_empty() {
        let statement = PlaceTypeStatement {
            date: Some(a_date()),
            ..PlaceTypeStatement::default()
        };
        assert!(statement.is_empty());
    }

    #[test]
    fn statement_with_any_of_kind_level_or_key_is_not_empty() {
        let kind = PlaceTypeStatement {
            kind: Some(PlaceKind::Ecclesiastical),
            ..PlaceTypeStatement::default()
        };
        let level = PlaceTypeStatement {
            level: Some(PlaceLevel::Adm2),
            ..PlaceTypeStatement::default()
        };
        let key = PlaceTypeStatement {
            type_key: Some(PlaceTypeKey::new("std:farm").unwrap()),
            ..PlaceTypeStatement::default()
        };
        assert!(!kind.is_empty());
        assert!(!level.is_empty());
        assert!(!key.is_empty());
    }

    #[test]
    fn statement_round_trips_through_json() {
        let statement = PlaceTypeStatement {
            kind: Some(PlaceKind::Administrative),
            level: Some(PlaceLevel::SubSettlement),
            type_key: Some(PlaceTypeKey::new("std:farm").unwrap()),
            date: Some(a_date()),
        };
        let json = serde_json::to_string(&statement).unwrap();
        assert_eq!(serde_json::from_str::<PlaceTypeStatement>(&json).unwrap(), statement);
    }
}
