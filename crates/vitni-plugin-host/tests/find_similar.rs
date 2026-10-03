//! `query.find-similar` (ADR 0038 §8): a plugin asks the matching engine for the records similar to a
//! target and gets each with its band, score and features — the same answer the app gives — and only
//! when it holds the `query` grant.
//!
//! These require the fixture component to be built first: run `cargo xtask build-plugins`.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::{Path, PathBuf};

use uuid::Uuid;
use vitni_app::{
    AppDefaults, DateParts, MatchBand, MatchableKind, MutationMeta, NewEvent, NewParticipation, NewPerson,
    OperatorConfig, PersonNameParts, Provenance, Session, Workspace, WorkspaceDefaults, assert_event_date,
    assert_participation, assert_sex, create_event, create_person, find_similar,
};
use vitni_core::enums::{EventType, EvidenceLevel, ParticipantRole, Sex};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};
use vitni_plugin_host::{Capability, Grants, PluginError, ResourceBudget};

mod common;

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn session(kind: AgentKind) -> Session {
    Session::new(Agent {
        kind,
        id: AgentId::from_uuid(Uuid::from_u128(7)),
        display: Some("Tester".to_owned()),
    })
}

fn software_session() -> Session {
    session(AgentKind::Software {
        name: "vitni-fixture-plugin".to_owned(),
        version: "0.1.0".to_owned(),
    })
}

fn init_workspace() -> (PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("ws");
    Workspace::init(&root, &operator(), &AppDefaults::default(), None).expect("init");
    (root, dir)
}

async fn open_workspace(root: &Path) -> Workspace {
    Workspace::open(root, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace")
}

/// A man named `given` `surname`, born in `year`; returns his human id.
async fn born(workspace: &Workspace, given: &str, surname: &str, year: i32) -> String {
    let session = session(AgentKind::Human);
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some(surname.to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    let person = create_person(workspace, &session, new, Provenance::default(), &[])
        .await
        .expect("create person");
    assert_sex(workspace, &session, &person, Sex::Male, MutationMeta::default())
        .await
        .expect("assert sex");
    let birth = NewEvent {
        human_id: None,
        event_type: EventType::Birth,
    };
    let event = create_event(workspace, &session, birth, Provenance::default(), &[])
        .await
        .expect("create event");
    let date = DateParts {
        year,
        month: None,
        day: None,
    };
    assert_event_date(workspace, &session, &event, date, MutationMeta::default())
        .await
        .expect("date event");
    assert_participation(
        workspace,
        &session,
        &person,
        &event,
        NewParticipation::with_role(ParticipantRole::Primary),
        MutationMeta::default(),
    )
    .await
    .expect("participate");
    person
}

/// Runs the fixture's `try-find-similar` with `grants`.
async fn plugin_find_similar(
    workspace: Workspace,
    grants: Grants,
    target: &str,
    limit: u32,
) -> Result<String, PluginError> {
    let host = common::host();
    let component = common::component("fixture");
    host.fixture_try_find_similar(
        &component,
        workspace,
        software_session(),
        grants,
        ResourceBudget::default(),
        target,
        limit,
    )
    .await
    .map(|(summary, _)| summary)
}

#[tokio::test]
async fn find_similar_is_denied_without_the_query_grant() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let target = born(&workspace, "Ole", "Olsen", 1850).await;
    born(&workspace, "Ole", "Olsen", 1850).await;

    let grants = Grants::none().with(Capability::Log).with(Capability::Commands);
    let result = plugin_find_similar(workspace, grants, &target, 10).await;

    match result {
        Err(PluginError::Guest(message)) => assert!(message.contains("Denied"), "got: {message}"),
        other => panic!("expected a denied guest error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_plugin_gets_the_apps_candidates_with_band_score_and_features() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let target = born(&workspace, "Ole", "Olsen", 1850).await;
    born(&workspace, "Ole", "Olsen", 1850).await;
    born(&workspace, "Ola", "Olsen", 1851).await;
    let expected = find_similar(&workspace, MatchableKind::Person, &target, MatchBand::Possible, 10)
        .await
        .expect("find similar");
    assert!(
        expected.len() >= 2,
        "the seeded records should be similar: {expected:?}"
    );

    let summary = plugin_find_similar(workspace, Grants::none().with(Capability::Query), &target, 10)
        .await
        .expect("granted find-similar");

    let lines: Vec<&str> = summary.lines().collect();
    assert_eq!(lines.len(), expected.len(), "{summary}");
    for (line, similar) in lines.iter().zip(&expected) {
        let assessment = &similar.assessment;
        let head = format!(
            "{} MatchBand::{:?} {:.3} ",
            similar.record.human_id, assessment.band, assessment.score
        );
        assert!(line.starts_with(&head), "expected `{head}…`, got `{line}`");
        let terms: Vec<&str> = line[head.len()..].split(',').collect();
        assert_eq!(terms.len(), assessment.features.len(), "{line}");
    }
    assert!(lines.iter().all(|line| !line.starts_with(&format!("{target} "))));
    assert!(
        summary.contains("MatchFeature::GivenName=MatchOutcome::Agree"),
        "{summary}"
    );
}

#[tokio::test]
async fn a_plugin_gets_at_most_limit_candidates() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;
    let target = born(&workspace, "Ole", "Olsen", 1850).await;
    born(&workspace, "Ole", "Olsen", 1850).await;
    born(&workspace, "Ole", "Olsen", 1850).await;

    let summary = plugin_find_similar(workspace, Grants::none().with(Capability::Query), &target, 1)
        .await
        .expect("granted find-similar");

    assert_eq!(summary.lines().count(), 1, "{summary}");
}

#[tokio::test]
async fn an_unknown_target_is_invalid_input() {
    let (root, _dir) = init_workspace();
    let workspace = open_workspace(&root).await;

    let result = plugin_find_similar(workspace, Grants::none().with(Capability::Query), "I9999", 10).await;

    match result {
        Err(PluginError::Guest(message)) => assert!(message.contains("InvalidInput"), "got: {message}"),
        other => panic!("expected an invalid-input guest error, got {other:?}"),
    }
}
