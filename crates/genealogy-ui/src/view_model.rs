//! View-models: framework-neutral, render-ready shapes derived from `genealogy-app` DTOs.
//!
//! A view-model carries already-localized display strings (built via the [`Localizer`]) so a
//! renderer stays dumb — it lays out fields, it does not format or localize. The structured parts a
//! renderer might branch on (e.g. `private`) stay typed. A list row is the generic [`RowVm`]; the
//! detail tab strip is [`DetailTab`]s.

use genealogy_app::PersonSummary;

use crate::detail::DetailTab;
use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::presentation::RestrictionKind;

/// Builds a generic list row from a [`PersonSummary`], localizing the name and sex via `loc`.
///
/// The subtitle is the localized sex label for now; later slices enrich it with vital dates and a
/// primary place. The avatar is the person's initials, or `?` when no name is known.
#[must_use]
pub fn person_row(summary: &PersonSummary, loc: &Localizer) -> RowVm {
    RowVm {
        id: summary.human_id.clone(),
        title: loc.display_name(summary.display_name.as_deref()),
        subtitle: Some(loc.sex_label(summary.sex.as_ref())),
        avatar: Some(initials(summary)),
    }
}

/// The person's initials from the structured given/surname, or `?` when neither is known.
fn initials(summary: &PersonSummary) -> String {
    let mut initials = String::new();
    for part in [summary.given.as_deref(), summary.surname.as_deref()] {
        if let Some(first) = part.and_then(|name| name.chars().next()) {
            initials.push(first.to_ascii_uppercase());
        }
    }
    if initials.is_empty() {
        initials.push('?');
    }
    initials
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
    /// The person's privacy restrictions (GEDCOM `RESN`), as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The human ids of the citations backing this person.
    pub citations: Vec<String>,
    /// The human ids of the media attached to this person.
    pub media: Vec<String>,
    /// The human ids of the notes attached to this person.
    pub notes: Vec<String>,
    /// The ids of the tags applied to this person.
    pub tags: Vec<String>,
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
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            citations: summary.citations.clone(),
            media: summary.media.clone(),
            notes: summary.notes.clone(),
            tags: summary.tags.clone(),
        }
    }
}

/// The tab strip for a person's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn person_tabs(detail: &PersonDetail, loc: &Localizer) -> Vec<DetailTab> {
    vec![
        DetailTab {
            id: "overview",
            label: loc.tab_label("overview"),
            count: None,
        },
        DetailTab {
            id: "citations",
            label: loc.tab_label("citations"),
            count: Some(detail.citations.len()),
        },
        DetailTab {
            id: "media",
            label: loc.tab_label("media"),
            count: Some(detail.media.len()),
        },
        DetailTab {
            id: "notes",
            label: loc.tab_label("notes"),
            count: Some(detail.notes.len()),
        },
        DetailTab {
            id: "tags",
            label: loc.tab_label("tags"),
            count: Some(detail.tags.len()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{PersonDetail, person_row, person_tabs};
    use crate::i18n::Localizer;
    use genealogy_app::{PersonSummary, Restriction, Sex};
    use std::collections::BTreeSet;

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
            facts: Vec::new(),
            associations: Vec::new(),
            participations: Vec::new(),
            citations: vec!["C0001".to_owned()],
            media: Vec::new(),
            notes: vec!["N0001".to_owned(), "N0002".to_owned()],
            tags: Vec::new(),
            restrictions: BTreeSet::new(),
        }
    }

    #[test]
    fn row_localizes_name_sex_and_initials() {
        let loc = Localizer::for_test("en");
        let row = person_row(&summary(), &loc);
        assert_eq!(row.id, "I0001");
        assert_eq!(row.title, "Ada Lovelace");
        assert_eq!(row.subtitle.as_deref(), Some("female"));
        assert_eq!(row.avatar.as_deref(), Some("AL"));
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
    fn tabs_carry_localized_labels_and_related_counts() {
        let loc = Localizer::for_test("en");
        let detail = PersonDetail::from_summary(&summary(), &loc);
        let tabs = person_tabs(&detail, &loc);
        assert_eq!(tabs[0].id, "overview");
        assert_eq!(tabs[0].label, "Overview");
        assert_eq!(tabs[0].count, None);
        assert_eq!(tabs[1].id, "citations");
        assert_eq!(tabs[1].count, Some(1));
        let notes = tabs.iter().find(|tab| tab.id == "notes").expect("notes tab");
        assert_eq!(notes.count, Some(2));
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
            facts: Vec::new(),
            associations: Vec::new(),
            participations: Vec::new(),
            citations: Vec::new(),
            media: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: BTreeSet::from([Restriction::Privacy]),
        };
        let row = person_row(&summary, &loc);
        assert_eq!(row.title, "(no name)");
        assert_eq!(row.subtitle.as_deref(), Some("-"));
        assert_eq!(row.avatar.as_deref(), Some("?"));
    }
}
