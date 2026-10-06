//! The Dashboard's narrow reads (#519): evidence health, the names of the few persons it lists, and the
//! death-before-birth check, each read without composing every person's summary — and each agreeing
//! with what the composed summaries say.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;

use uuid::Uuid;
use vitni_app::{
    AppDefaults, CheckFinding, DateParts, EvidenceHealth, IdentityDecision, MutationMeta, NewCitation, NewEvent,
    NewFact, NewParticipation, NewPerson, NewSource, OperatorConfig, PersonNameParts, Provenance, Session, Workspace,
    WorkspaceDefaults, assert_event_date, assert_fact, assert_participation, create_citation, create_event,
    create_person, create_source, evidence_health, list_persons, merge_persons, person_names, run_checks,
};
use vitni_core::enums::{EventType, EvidenceLevel, FactType, ParticipantRole};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn session() -> Session {
    Session::new(Agent {
        kind: AgentKind::Human,
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
    })
}

async fn workspace() -> (Workspace, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, dir)
}

/// Creates a person, named `given surname` when a name is given, returning its `human_id`.
async fn person(ws: &Workspace, name: Option<(&str, &str)>) -> String {
    let new = NewPerson {
        human_id: None,
        name: name.map(|(given, surname)| PersonNameParts::simple(Some(given.to_owned()), Some(surname.to_owned()))),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create person")
}

async fn merge(ws: &Workspace, surviving: &str, merged: &str) {
    merge_persons(ws, &session(), surviving, merged, IdentityDecision::default())
        .await
        .expect("merge");
}

async fn citation(ws: &Workspace) -> String {
    let source = create_source(
        ws,
        &session(),
        NewSource {
            human_id: None,
            title: Some("Census".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create source");
    create_citation(
        ws,
        &session(),
        NewCitation {
            human_id: None,
            source,
            page: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create citation")
}

async fn fact(ws: &Workspace, human_id: &str, citations: &[String]) {
    assert_fact(
        ws,
        &session(),
        human_id,
        NewFact {
            fact_type: FactType::Occupation,
            value: None,
            date: None,
        },
        MutationMeta {
            citations,
            ..MutationMeta::default()
        },
    )
    .await
    .expect("assert fact");
}

/// A dated vital event with `human_id` as its Primary participant (ADR 0021 §2).
async fn vital(ws: &Workspace, human_id: &str, event_type: EventType, year: i32) {
    let event = create_event(
        ws,
        &session(),
        NewEvent {
            human_id: None,
            event_type,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create event");
    let date = DateParts {
        year,
        month: None,
        day: None,
    };
    assert_event_date(ws, &session(), &event, date, MutationMeta::default())
        .await
        .expect("date event");
    assert_participation(
        ws,
        &session(),
        human_id,
        &event,
        NewParticipation::with_role(ParticipantRole::Primary),
        MutationMeta::default(),
    )
    .await
    .expect("participate");
}

/// The persons the death-before-birth check flags.
async fn flagged(ws: &Workspace) -> BTreeSet<String> {
    let quality = run_checks(ws, 0).await.expect("checks");
    let mut flagged = BTreeSet::new();
    for finding in quality.findings {
        if let CheckFinding::DeathBeforeBirth(record) = finding {
            flagged.insert(record.human_id);
        }
    }
    flagged
}

/// The persons whose composed summary has a death year before its birth year.
async fn reversed_in_summaries(ws: &Workspace) -> BTreeSet<String> {
    let mut reversed = BTreeSet::new();
    for summary in list_persons(ws).await.expect("persons") {
        if let (Some(birth), Some(death)) = (summary.birth_year(), summary.death_year())
            && death < birth
        {
            reversed.insert(summary.human_id);
        }
    }
    reversed
}

#[tokio::test]
async fn evidence_health_counts_every_fact_and_those_citing_a_source() {
    let (ws, _dir) = workspace().await;
    assert_eq!(
        evidence_health(&ws).await.expect("health"),
        EvidenceHealth { facts: 0, sourced: 0 }
    );
    let cited = citation(&ws).await;
    let ada = person(&ws, Some(("Ada", "Lovelace"))).await;
    let member = person(&ws, Some(("Ada", "Byron"))).await;
    fact(&ws, &ada, std::slice::from_ref(&cited)).await;
    fact(&ws, &ada, &[]).await;
    fact(&ws, &member, std::slice::from_ref(&cited)).await;
    merge(&ws, &ada, &member).await;

    let health = evidence_health(&ws).await.expect("health");
    assert_eq!(health, EvidenceHealth { facts: 3, sourced: 2 });
    let mut facts = 0;
    let mut sourced = 0;
    for summary in list_persons(&ws).await.expect("persons") {
        facts += summary.facts.len();
        sourced += summary.facts.iter().filter(|fact| !fact.citations.is_empty()).count();
    }
    assert_eq!(health, EvidenceHealth { facts, sourced }, "the summaries agree");
}

#[tokio::test]
async fn person_names_reads_only_the_persons_asked_for() {
    let (ws, _dir) = workspace().await;
    let ada = person(&ws, Some(("Ada", "Lovelace"))).await;
    let unnamed = person(&ws, None).await;
    let other = person(&ws, Some(("Bo", "Berg"))).await;

    let names = person_names(&ws, &[ada.clone(), unnamed.clone(), "I9999".to_owned()])
        .await
        .expect("names");
    assert_eq!(names.get(&ada).map(String::as_str), Some("Ada Lovelace"));
    assert!(
        !names.contains_key(&unnamed),
        "an unnamed person has no name: {names:?}"
    );
    assert!(!names.contains_key("I9999"), "an unknown id is left out: {names:?}");
    assert!(
        !names.contains_key(&other),
        "a person not asked for is left out: {names:?}"
    );
}

#[tokio::test]
async fn an_unnamed_root_reads_its_members_name_as_its_summary_does() {
    let (ws, _dir) = workspace().await;
    let root = person(&ws, None).await;
    let member = person(&ws, Some(("Cy", "Dahl"))).await;
    merge(&ws, &root, &member).await;

    let names = person_names(&ws, &[root.clone(), member.clone()]).await.expect("names");
    let summary = list_persons(&ws)
        .await
        .expect("persons")
        .into_iter()
        .find(|summary| summary.human_id == root)
        .expect("root listed");
    assert_eq!(names.get(&root), summary.display_name.as_ref());
    assert_eq!(names.get(&root).map(String::as_str), Some("Cy Dahl"));
    assert_eq!(names.get(&member).map(String::as_str), Some("Cy Dahl"));
}

#[tokio::test]
async fn death_before_birth_reads_a_cluster_as_its_summary_does() {
    let (ws, _dir) = workspace().await;
    let plain = person(&ws, Some(("Di", "Reversed"))).await;
    vital(&ws, &plain, EventType::Birth, 1900).await;
    vital(&ws, &plain, EventType::Death, 1880).await;
    let ordered = person(&ws, Some(("El", "Ordered"))).await;
    vital(&ws, &ordered, EventType::Birth, 1880).await;
    vital(&ws, &ordered, EventType::Death, 1950).await;

    // The root's death and its member's birth make the cluster's lifespan reversed.
    let root = person(&ws, Some(("Fi", "Split"))).await;
    vital(&ws, &root, EventType::Death, 1850).await;
    let member = person(&ws, Some(("Fi", "Splitt"))).await;
    vital(&ws, &member, EventType::Birth, 1900).await;
    merge(&ws, &root, &member).await;

    // The root's own birth wins over its member's.
    let kept = person(&ws, Some(("Gro", "Kept"))).await;
    vital(&ws, &kept, EventType::Birth, 1800).await;
    vital(&ws, &kept, EventType::Death, 1850).await;
    let late = person(&ws, Some(("Gro", "Kjept"))).await;
    vital(&ws, &late, EventType::Birth, 1900).await;
    merge(&ws, &kept, &late).await;

    let flagged = flagged(&ws).await;
    assert_eq!(flagged, BTreeSet::from([plain, root]));
    assert_eq!(flagged, reversed_in_summaries(&ws).await, "the summaries agree");
}
