//! View-models: framework-neutral, render-ready shapes derived from `genealogy-app` DTOs.
//!
//! A view-model carries already-localized display strings (built via the [`Localizer`]) so a
//! renderer stays dumb — it lays out fields, it does not format or localize. The structured parts a
//! renderer might branch on (e.g. `private`) stay typed. A list row is the generic [`RowVm`]; the
//! detail tab strip is [`DetailTab`]s.

use genealogy_app::{FactSummary, FamilyForPerson, PersonFamilyRole, PersonName, PersonSummary};

use crate::detail::DetailTab;
use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::presentation::{ConfidenceLevel, RestrictionKind};

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

/// One asserted name variant, for the Names tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameVm {
    /// The localized name-type label.
    pub type_label: String,
    /// The rendered `given surname(s)` display string.
    pub display: String,
    /// The given name, if any.
    pub given: Option<String>,
    /// The primary surname, if any.
    pub surname: Option<String>,
    /// The nickname, if any.
    pub nickname: Option<String>,
}

/// One asserted fact, for the Facts tab — the evidence-first row (confidence + source count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactVm {
    /// The localized fact-type label.
    pub type_label: String,
    /// The fact's free-text value, if any.
    pub value: Option<String>,
    /// The localized rendered date, if any.
    pub date: Option<String>,
    /// The fact's confidence, as a presentation level (drives the badge colour token).
    pub confidence: ConfidenceLevel,
    /// The localized confidence label (shown beside the badge — colour is never alone).
    pub confidence_label: String,
    /// How many citations back this fact (its source count).
    pub source_count: usize,
}

impl FactVm {
    /// Whether the fact has at least one backing source (drives the no-source flag).
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source_count > 0
    }
}

/// One event participation, for the Events tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRefVm {
    /// The event's user-facing id (e.g. `E0001`).
    pub event_id: String,
    /// The localized participant-role label.
    pub role_label: String,
    /// The localized rendered event date, if known.
    pub date: Option<String>,
}

/// One person-to-person association, for the Associations tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationVm {
    /// The other person's user-facing id.
    pub other_id: String,
    /// The localized association-role label.
    pub role_label: String,
}

/// One family the person belongs to, for the Families tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyVm {
    /// The family's user-facing id (e.g. `F0001`).
    pub family_id: String,
    /// The localized role label (spouse/partner, or the child relationship).
    pub role_label: String,
    /// The partners' user-facing ids.
    pub partners: Vec<String>,
    /// The children: each child's id and localized relationship label.
    pub children: Vec<(String, String)>,
}

impl FamilyVm {
    /// Builds a family view-model from the app's [`FamilyForPerson`], localizing role labels.
    #[must_use]
    pub fn from_app(family: &FamilyForPerson, loc: &Localizer) -> Self {
        let role_label = match &family.role {
            PersonFamilyRole::Partner => loc.role("spouse"),
            PersonFamilyRole::Child(relationship) => loc.relationship_label(relationship),
        };
        Self {
            family_id: family.family_human_id.clone(),
            role_label,
            partners: family.partners.clone(),
            children: family
                .children
                .iter()
                .map(|(id, relationship)| (id.clone(), loc.relationship_label(relationship)))
                .collect(),
        }
    }
}

/// Builds a [`NameVm`] from an asserted [`PersonName`], localizing the type label.
fn name_vm(name: &PersonName, loc: &Localizer) -> NameVm {
    let surname = name.surnames.first().map(|element| element.surname.clone());
    NameVm {
        type_label: loc.name_type_label(&name.name_type),
        display: render_person_name(name),
        given: name.given.clone(),
        surname,
        nickname: name.nickname.clone(),
    }
}

/// Builds a [`FactVm`] from an app [`FactSummary`], localizing labels and the date.
fn fact_vm(summary: &FactSummary, loc: &Localizer) -> FactVm {
    let confidence = ConfidenceLevel::from(summary.confidence);
    FactVm {
        type_label: loc.fact_type_label(&summary.fact.fact_type),
        value: summary.fact.value.clone(),
        date: summary.fact.date.as_ref().map(|date| loc.date(date)),
        confidence,
        confidence_label: loc.confidence_label(confidence),
        source_count: summary.fact.citations.len(),
    }
}

/// Renders a [`PersonName`] as `given surname(s)` for display.
fn render_person_name(name: &PersonName) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(given) = name.given.as_deref() {
        parts.push(given);
    }
    for surname in &name.surnames {
        parts.push(&surname.surname);
    }
    parts.join(" ")
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
    /// Every asserted name variant (Names tab).
    pub names: Vec<NameVm>,
    /// Every asserted fact, with confidence + source count (Facts tab).
    pub facts: Vec<FactVm>,
    /// Event participations (Events tab); dates are joined by the dispatcher.
    pub events: Vec<EventRefVm>,
    /// Person-to-person associations (Associations tab).
    pub associations: Vec<AssociationVm>,
    /// Families this person belongs to (Families tab); filled by the dispatcher.
    pub families: Vec<FamilyVm>,
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
    /// Builds a detail view from a [`PersonSummary`], localizing labels via `loc`.
    ///
    /// The summary-derived tabs (names, facts, associations) are built here; the cross-aggregate
    /// tabs (events, families) start empty and are filled by the dispatcher
    /// ([`dispatch`](crate::intent::dispatch)), which has the joined event/family data.
    #[must_use]
    pub fn from_summary(summary: &PersonSummary, loc: &Localizer) -> Self {
        Self {
            human_id: summary.human_id.clone(),
            name: loc.display_name(summary.display_name.as_deref()),
            given: summary.given.clone(),
            surname: summary.surname.clone(),
            sex: loc.sex_label(summary.sex.as_ref()),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            names: summary.names.iter().map(|name| name_vm(name, loc)).collect(),
            facts: summary.facts.iter().map(|fact| fact_vm(fact, loc)).collect(),
            events: Vec::new(),
            associations: summary
                .associations
                .iter()
                .map(|(other_id, role)| AssociationVm {
                    other_id: other_id.clone(),
                    role_label: loc.association_role_label(role),
                })
                .collect(),
            families: Vec::new(),
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
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("names", Some(detail.names.len())),
        tab("facts", Some(detail.facts.len())),
        tab("events", Some(detail.events.len())),
        tab("associations", Some(detail.associations.len())),
        tab("families", Some(detail.families.len())),
        tab("citations", Some(detail.citations.len())),
        tab("media", Some(detail.media.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

#[cfg(test)]
mod tests {
    use super::{PersonDetail, person_row, person_tabs};
    use crate::i18n::Localizer;
    use crate::presentation::ConfidenceLevel;
    use genealogy_app::{
        AssociationRole, Confidence, Fact, FactSummary, FactType, NameType, PersonName, PersonSummary, Restriction,
        Sex, Surname,
    };
    use std::collections::BTreeSet;

    fn birth_name() -> PersonName {
        PersonName {
            name_type: NameType::BirthName,
            given: Some("Ada".to_owned()),
            surnames: vec![Surname {
                prefix: None,
                surname: "Lovelace".to_owned(),
                primary: true,
                connector: None,
            }],
            suffix: None,
            title: None,
            nickname: None,
            call_name: None,
            date: None,
            language: None,
            transliterations: Vec::new(),
        }
    }

    fn occupation_fact() -> FactSummary {
        FactSummary {
            fact: Fact {
                fact_type: FactType::Occupation,
                date: None,
                place_id: None,
                value: Some("Mathematician".to_owned()),
                citations: Vec::new(),
            },
            confidence: Confidence::High,
        }
    }

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
            names: vec![birth_name()],
            sex: Some(Sex::Female),
            facts: vec![occupation_fact()],
            associations: vec![("I0002".to_owned(), AssociationRole::Godparent)],
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
    fn detail_builds_name_fact_and_association_view_models() {
        let loc = Localizer::for_test("en");
        let detail = PersonDetail::from_summary(&summary(), &loc);

        assert_eq!(detail.names.len(), 1);
        assert_eq!(detail.names[0].type_label, "Birth name");
        assert_eq!(detail.names[0].display, "Ada Lovelace");

        assert_eq!(detail.facts.len(), 1);
        let fact = &detail.facts[0];
        assert_eq!(fact.type_label, "Occupation");
        assert_eq!(fact.value.as_deref(), Some("Mathematician"));
        assert_eq!(fact.confidence, ConfidenceLevel::High);
        assert_eq!(fact.confidence_label, "High");
        assert_eq!(fact.source_count, 0);
        assert!(!fact.has_source(), "no citations means no source");

        assert_eq!(detail.associations.len(), 1);
        assert_eq!(detail.associations[0].other_id, "I0002");
        assert_eq!(detail.associations[0].role_label, "Godparent");
    }

    #[test]
    fn tabs_carry_localized_labels_and_related_counts() {
        let loc = Localizer::for_test("en");
        let detail = PersonDetail::from_summary(&summary(), &loc);
        let tabs = person_tabs(&detail, &loc);
        assert_eq!(tabs[0].id, "overview");
        assert_eq!(tabs[0].label, "Overview");
        assert_eq!(tabs[0].count, None);
        assert_eq!(tabs[1].id, "names");
        assert_eq!(tabs[1].count, Some(1));
        let facts = tabs.iter().find(|tab| tab.id == "facts").expect("facts tab");
        assert_eq!(facts.count, Some(1));
        let notes = tabs.iter().find(|tab| tab.id == "notes").expect("notes tab");
        assert_eq!(notes.count, Some(2));
        let history = tabs.iter().find(|tab| tab.id == "history").expect("history tab");
        assert_eq!(history.count, None, "history count is unknown until PR5");
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
            names: Vec::new(),
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
