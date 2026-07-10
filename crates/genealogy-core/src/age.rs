//! [`Age`] — a participant's age at an event, as a duration (data-model §7).
//!
//! An age is a *span* (how old someone was), not a calendar point, so it is its own value object
//! rather than a reuse of [`GenealogicalDate`](crate::date::GenealogicalDate) (ADR 0019). It carries
//! the decomposed years/months/days a genealogical source records, an optional [`AgeBound`] for
//! GEDCOM's `<`/`>` qualifiers, and a free-text `phrase` for an age that does not decompose. Weeks
//! are deliberately absent: GEDCOM's `w` unit is normalized to days at import (ADR 0019).

use serde::{Deserialize, Serialize};

/// A one-sided bound on an age (GEDCOM `AGE` `<` / `>` qualifiers — data-model §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgeBound {
    /// The age is strictly less than the stated value (GEDCOM `<`).
    LessThan,
    /// The age is strictly greater than the stated value (GEDCOM `>`).
    GreaterThan,
}

/// A participant's age at an event, expressed as a duration (data-model §7, ADR 0019).
///
/// Every field is optional: a partially-recorded age (just years, say) is common, and an all-`None`
/// age is [`is_empty`](Age::is_empty) and normalized to `None` at the boundary rather than stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Age {
    /// A one-sided bound qualifying the age (GEDCOM `<` / `>`), if any.
    pub bound: Option<AgeBound>,
    /// Whole years.
    pub years: Option<u16>,
    /// Whole months.
    pub months: Option<u16>,
    /// Whole days (GEDCOM weeks are normalized to days at import — ADR 0019).
    pub days: Option<u16>,
    /// A free-text age that does not decompose into parts (GEDCOM `AGE` phrase).
    pub phrase: Option<String>,
}

impl Age {
    /// Whether every field is absent, so no age should be recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bound.is_none()
            && self.years.is_none()
            && self.months.is_none()
            && self.days.is_none()
            && self.phrase.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::{Age, AgeBound};

    #[test]
    fn full_age_round_trips_through_json() {
        let age = Age {
            bound: Some(AgeBound::LessThan),
            years: Some(25),
            months: Some(3),
            days: Some(10),
            phrase: Some("in his prime".to_owned()),
        };
        let json = serde_json::to_string(&age).unwrap();
        let back: Age = serde_json::from_str(&json).unwrap();
        assert_eq!(age, back);
    }

    #[test]
    fn all_none_age_round_trips_and_is_empty() {
        let age = Age::default();
        assert!(age.is_empty());
        let json = serde_json::to_string(&age).unwrap();
        let back: Age = serde_json::from_str(&json).unwrap();
        assert_eq!(age, back);
        assert!(back.is_empty());
    }

    #[test]
    fn any_present_field_makes_the_age_non_empty() {
        assert!(
            !Age {
                years: Some(40),
                ..Age::default()
            }
            .is_empty()
        );
        assert!(
            !Age {
                bound: Some(AgeBound::GreaterThan),
                ..Age::default()
            }
            .is_empty()
        );
        assert!(
            !Age {
                phrase: Some("elderly".to_owned()),
                ..Age::default()
            }
            .is_empty()
        );
    }
}
