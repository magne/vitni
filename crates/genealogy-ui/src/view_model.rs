//! View-models: framework-neutral, render-ready shapes derived from `genealogy-app` DTOs.
//!
//! A view-model carries already-localized display strings (built via the [`Localizer`]) so a
//! renderer stays dumb — it lays out fields, it does not format or localize. The structured parts a
//! renderer might branch on (e.g. `private`) stay typed.

use genealogy_app::PersonSummary;

use crate::i18n::Localizer;

/// A person as one row in the list view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRow {
    /// The user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// The localized display name, or the localized "no name" placeholder.
    pub name: String,
    /// The localized sex label, or the localized "no value" placeholder.
    pub sex: String,
    /// Whether the person is marked private.
    pub private: bool,
}

impl PersonRow {
    /// Builds a row from a [`PersonSummary`], localizing the name and sex via `loc`.
    #[must_use]
    pub fn from_summary(summary: &PersonSummary, loc: &Localizer) -> Self {
        Self {
            human_id: summary.human_id.clone(),
            name: loc.display_name(summary.display_name.as_deref()),
            sex: loc.sex_label(summary.sex.as_ref()),
            private: summary.private,
        }
    }
}

/// A person's detail view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonDetail {
    /// The user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// The localized display name, or the localized "no name" placeholder.
    pub name: String,
    /// The structured given name, if asserted.
    pub given: Option<String>,
    /// The structured primary surname, if asserted.
    pub surname: Option<String>,
    /// The localized sex label, or the localized "no value" placeholder.
    pub sex: String,
    /// Whether the person is marked private.
    pub private: bool,
}

impl PersonDetail {
    /// Builds a detail view from a [`PersonSummary`], localizing the name and sex via `loc`.
    #[must_use]
    pub fn from_summary(summary: &PersonSummary, loc: &Localizer) -> Self {
        Self {
            human_id: summary.human_id.clone(),
            name: loc.display_name(summary.display_name.as_deref()),
            given: summary.given.clone(),
            surname: summary.surname.clone(),
            sex: loc.sex_label(summary.sex.as_ref()),
            private: summary.private,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PersonDetail, PersonRow};
    use crate::i18n::Localizer;
    use genealogy_app::{PersonSummary, Sex};

    fn summary() -> PersonSummary {
        PersonSummary {
            human_id: "I0001".to_owned(),
            display_name: Some("Ada Lovelace".to_owned()),
            given: Some("Ada".to_owned()),
            surname: Some("Lovelace".to_owned()),
            surname_prefix: None,
            nickname: None,
            name_prefix: None,
            name_suffix: None,
            name_type: None,
            sex: Some(Sex::Female),
            private: false,
        }
    }

    #[test]
    fn row_localizes_name_and_sex() {
        let loc = Localizer::for_test("en");
        let row = PersonRow::from_summary(&summary(), &loc);
        assert_eq!(row.human_id, "I0001");
        assert_eq!(row.name, "Ada Lovelace");
        assert_eq!(row.sex, "female");
        assert!(!row.private);
    }

    #[test]
    fn detail_keeps_structured_parts_and_localizes_in_norwegian() {
        let loc = Localizer::for_test("no");
        let detail = PersonDetail::from_summary(&summary(), &loc);
        assert_eq!(detail.given.as_deref(), Some("Ada"));
        assert_eq!(detail.surname.as_deref(), Some("Lovelace"));
        assert_eq!(detail.sex, "kvinne");
    }

    #[test]
    fn missing_name_and_sex_use_placeholders() {
        let loc = Localizer::for_test("en");
        let summary = PersonSummary {
            human_id: "I0002".to_owned(),
            display_name: None,
            given: None,
            surname: None,
            surname_prefix: None,
            nickname: None,
            name_prefix: None,
            name_suffix: None,
            name_type: None,
            sex: None,
            private: true,
        };
        let row = PersonRow::from_summary(&summary, &loc);
        assert_eq!(row.name, "(no name)");
        assert_eq!(row.sex, "-");
        assert!(row.private);
    }
}
