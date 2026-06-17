//! Verifies the `cqrs-es` adapter for the Person aggregate matches the pure decision core.
//!
//! Uses the framework's own given/when/then `TestFramework`, which drives the real
//! `Aggregate::handle`/`apply` path (not `decide`/`evolve` directly), so this proves the thin
//! adapter wires commands to events and applies them correctly.

use cqrs_es::test::TestFramework;
use genealogy_core::enums::EvidenceLevel;
use genealogy_core::ids::{AgentId, AssertionId, HumanId, PersonId};
use genealogy_core::name::{NameType, PersonName, Surname};
use genealogy_core::person::PersonError;
use genealogy_core::person::PersonState;
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::person::event::{PersonEvent, PersonEventBody};
use genealogy_core::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
use time::macros::datetime;
use uuid::Uuid;

type PersonTest = TestFramework<PersonState>;

fn pid(n: u128) -> PersonId {
    PersonId::from_uuid(Uuid::from_u128(n))
}

fn meta(assertion: u128) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
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

fn envelope(assertion: u128, command: PersonCommand) -> PersonCommandEnvelope {
    PersonCommandEnvelope {
        meta: meta(assertion),
        command,
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

fn created_event(assertion: u128, person: u128) -> PersonEvent {
    PersonEvent::new(
        &meta(assertion),
        PersonEventBody::PersonCreated {
            person_id: pid(person),
            human_id: HumanId::new("I1"),
            evidence_level: EvidenceLevel::Conclusion,
        },
    )
}

#[test]
fn create_person_produces_person_created_through_the_adapter() {
    PersonTest::with(())
        .given_no_previous_events()
        .when(envelope(
            1,
            PersonCommand::CreatePerson {
                person_id: pid(100),
                human_id: HumanId::new("I1"),
                evidence_level: EvidenceLevel::Conclusion,
            },
        ))
        .then_expect_events(vec![created_event(1, 100)]);
}

#[test]
fn asserting_a_name_on_an_existing_person_produces_name_asserted() {
    PersonTest::with(())
        .given(vec![created_event(1, 100)])
        .when(envelope(
            2,
            PersonCommand::AssertName {
                person_id: pid(100),
                name: name("Ada"),
            },
        ))
        .then_expect_events(vec![PersonEvent::new(
            &meta(2),
            PersonEventBody::NameAsserted {
                person_id: pid(100),
                name: name("Ada"),
            },
        )]);
}

#[test]
fn asserting_a_name_on_an_absent_person_is_rejected() {
    PersonTest::with(())
        .given_no_previous_events()
        .when(envelope(
            2,
            PersonCommand::AssertName {
                person_id: pid(100),
                name: name("Ada"),
            },
        ))
        .then_expect_error(PersonError::NotFound(pid(100)));
}
