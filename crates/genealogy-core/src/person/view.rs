//! [`PersonView`] — the conclusion-layer read model for a Person (data-model §6).
//!
//! The view is rebuilt by folding the same events as the aggregate (it delegates to `evolve`), so
//! corrections — retractions and supersessions — are reflected correctly. A denormalized SQL
//! read schema is deferred (ADR 0002, data-model §17); for now the view exposes its projected
//! fields through accessor methods over the folded state.

use std::collections::BTreeSet;

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::enums::{EvidenceLevel, Restriction, Sex};
use crate::ids::{CitationId, HumanId, NoteId, PersonId, TagId};
use crate::name::PersonName;
use crate::person::decide::evolve;
use crate::person::state::{AssertedAssociation, AssertedFact, AssertedName, Association, Participation, PersonState};
use crate::text::{ExternalId, MediaRef};

/// The current best synthesis of a Person, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonView {
    state: PersonState,
}

impl PersonView {
    /// Returns `true` once the person has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The person's id, once created.
    #[must_use]
    pub fn person_id(&self) -> Option<PersonId> {
        self.state.person_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// Whether the person is a persona or a conclusion.
    #[must_use]
    pub fn evidence_level(&self) -> Option<EvidenceLevel> {
        self.state.evidence_level
    }

    /// All currently-live asserted names (retracted ones are excluded).
    #[must_use]
    pub fn names(&self) -> Vec<&PersonName> {
        self.state.names.iter().map(|n| &n.value.name).collect()
    }

    /// All currently-live asserted names with their provenance (surety + backing citations).
    #[must_use]
    pub fn asserted_names(&self) -> Vec<&AssertedName> {
        self.state.names.iter().map(|n| &n.value).collect()
    }

    /// The [`AssertionId`] of the primary (first-asserted, currently-live) name, if any — the target
    /// an edit supersedes when the operator changes the preferred name (data-model §10.1).
    #[must_use]
    pub fn primary_name_assertion(&self) -> Option<crate::ids::AssertionId> {
        self.state.names.first().map(|n| n.assertion_id)
    }

    /// The most recently asserted sex.
    #[must_use]
    pub fn sex(&self) -> Option<&Sex> {
        self.state.sex.as_ref().map(|s| &s.value)
    }

    /// All currently-live asserted facts, each with its assertion-time confidence.
    #[must_use]
    pub fn facts(&self) -> Vec<&AssertedFact> {
        self.state.facts.iter().map(|f| &f.value).collect()
    }

    /// All currently-live asserted person-to-person associations (data-model §10).
    #[must_use]
    pub fn associations(&self) -> Vec<&Association> {
        self.state.associations.iter().map(|a| &a.value.association).collect()
    }

    /// All currently-live asserted associations with their provenance (surety + backing citations).
    #[must_use]
    pub fn asserted_associations(&self) -> Vec<&AssertedAssociation> {
        self.state.associations.iter().map(|a| &a.value).collect()
    }

    /// All currently-live asserted event participations (data-model §6, §10).
    #[must_use]
    pub fn participations(&self) -> Vec<&Participation> {
        self.state.participations.iter().map(|p| &p.value).collect()
    }

    /// All currently-live citations backing the person's claims, in assertion order.
    #[must_use]
    pub fn citations(&self) -> Vec<CitationId> {
        self.state.citations.iter().map(|c| c.value).collect()
    }

    /// All currently-live attached media, in assertion order.
    #[must_use]
    pub fn media(&self) -> Vec<&MediaRef> {
        self.state.media.iter().map(|m| &m.value).collect()
    }

    /// All currently-live attached notes, in assertion order.
    #[must_use]
    pub fn notes(&self) -> Vec<NoteId> {
        self.state.notes.iter().map(|n| n.value).collect()
    }

    /// All currently-applied tags, in assertion order.
    #[must_use]
    pub fn tags(&self) -> Vec<TagId> {
        self.state.tags.iter().map(|t| t.value).collect()
    }

    /// The person's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// All currently-live external identifiers (data-model §11).
    #[must_use]
    pub fn external_ids(&self) -> Vec<&ExternalId> {
        self.state.external_ids.iter().map(|e| &e.value).collect()
    }

    /// The ids of persons currently merged into this survivor (data-model §9) — personas whose
    /// `PersonsMerged` assertion has not been undone.
    #[must_use]
    pub fn merged(&self) -> Vec<PersonId> {
        self.state.merged.iter().map(|m| m.value).collect()
    }

    /// Currently-live asserted names, each paired with the `AssertionId` that introduced it — the
    /// read side of the per-row correction (Edit supersedes it, Retract retracts it, data-model §8).
    #[must_use]
    pub fn names_with_assertions(&self) -> &[Attributed<AssertedName>] {
        &self.state.names
    }

    /// Currently-live asserted facts, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn facts_with_assertions(&self) -> &[Attributed<AssertedFact>] {
        &self.state.facts
    }

    /// Currently-live associations, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn associations_with_assertions(&self) -> &[Attributed<AssertedAssociation>] {
        &self.state.associations
    }

    /// Currently-live event participations, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn participations_with_assertions(&self) -> &[Attributed<Participation>] {
        &self.state.participations
    }

    /// Currently-live citations, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn citations_with_assertions(&self) -> &[Attributed<CitationId>] {
        &self.state.citations
    }

    /// Currently-live attached media, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn media_with_assertions(&self) -> &[Attributed<MediaRef>] {
        &self.state.media
    }

    /// Currently-live attached notes, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn notes_with_assertions(&self) -> &[Attributed<NoteId>] {
        &self.state.notes
    }
}

impl View<PersonState> for PersonView {
    fn update(&mut self, event: &EventEnvelope<PersonState>) {
        evolve(&mut self.state, &event.payload);
    }
}

#[cfg(test)]
mod tests {
    use super::PersonView;
    use crate::enums::EvidenceLevel;
    use crate::ids::{AgentId, AssertionId, HumanId, PersonId};
    use crate::name::{NameType, PersonName, Surname};
    use crate::person::command::PersonCommand;
    use crate::person::decide::{decide, evolve};
    use crate::person::state::PersonState;
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

    fn pid(n: u128) -> PersonId {
        PersonId::from_uuid(Uuid::from_u128(n))
    }

    fn aid(n: u128) -> AssertionId {
        AssertionId::from_uuid(Uuid::from_u128(n))
    }

    fn meta(assertion: u128) -> AssertionMeta {
        AssertionMeta {
            assertion_id: aid(assertion),
            context: EventContext {
                operator: Agent {
                    kind: AgentKind::Human,
                    id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                    display: None,
                },
                occurred_at: Timestamp::new(datetime!(2026-06-17 12:00:00 UTC)),
                rationale: None,
                confidence: Confidence::Normal,
                citations: Vec::new(),
                evidence_analysis: None,
            },
        }
    }

    fn name(given: &str) -> PersonName {
        PersonName {
            name_type: NameType::BirthName,
            given: Some(given.to_owned()),
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

    fn view_with(commands: Vec<(u128, PersonCommand)>) -> PersonView {
        let mut state = PersonState::default();
        for (assertion, command) in commands {
            let events = decide(&state, command, &meta(assertion)).expect("command decides");
            for event in &events {
                evolve(&mut state, event);
            }
        }
        PersonView { state }
    }

    fn create(person: u128) -> (u128, PersonCommand) {
        (
            1,
            PersonCommand::CreatePerson {
                person_id: pid(person),
                human_id: HumanId::new("I1"),
                evidence_level: EvidenceLevel::Conclusion,
            },
        )
    }

    #[test]
    fn names_with_assertions_carry_the_introducing_assertion_id() {
        let view = view_with(vec![
            create(1),
            (
                2,
                PersonCommand::AssertName {
                    person_id: pid(1),
                    name: name("Ada"),
                },
            ),
        ]);
        assert_eq!(view.names_with_assertions().len(), 1);
        assert_eq!(view.names_with_assertions()[0].assertion_id, aid(2));
    }

    #[test]
    fn a_retracted_name_leaves_no_row() {
        let view = view_with(vec![
            create(1),
            (
                2,
                PersonCommand::AssertName {
                    person_id: pid(1),
                    name: name("Ada"),
                },
            ),
            (
                3,
                PersonCommand::RetractAssertion {
                    person_id: pid(1),
                    target: aid(2),
                },
            ),
        ]);
        assert!(view.names_with_assertions().is_empty());
    }

    #[test]
    fn a_superseded_name_carries_the_replacement_assertion_id() {
        let view = view_with(vec![
            create(1),
            (
                2,
                PersonCommand::AssertName {
                    person_id: pid(1),
                    name: name("Ada"),
                },
            ),
            (
                3,
                PersonCommand::SupersedeAssertion {
                    person_id: pid(1),
                    target: aid(2),
                    replacement: Box::new(PersonCommand::AssertName {
                        person_id: pid(1),
                        name: name("Augusta"),
                    }),
                },
            ),
        ]);
        assert_eq!(view.names_with_assertions().len(), 1);
        assert_eq!(view.names_with_assertions()[0].assertion_id, aid(3));
        assert_eq!(
            view.names_with_assertions()[0].value.name.given.as_deref(),
            Some("Augusta")
        );
    }

    #[test]
    fn facts_with_assertions_carry_the_introducing_assertion_id() {
        use crate::enums::FactType;
        use crate::fact::Fact;
        let view = view_with(vec![
            create(1),
            (
                2,
                PersonCommand::AssertFact {
                    person_id: pid(1),
                    fact: Fact {
                        fact_type: FactType::Occupation,
                        value: Some("Carpenter".to_owned()),
                        date: None,
                        place_id: None,
                    },
                },
            ),
        ]);
        assert_eq!(view.facts_with_assertions().len(), 1);
        assert_eq!(view.facts_with_assertions()[0].assertion_id, aid(2));
    }
}
