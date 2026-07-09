//! `MergeBlockedVm::from_error` classifies a real `AppError` from the app: a `MergeConflict`
//! rejection becomes a blocked view-model carrying the core's reason; any other failure returns
//! `None` (the screen keeps its toast). Built against genealogy-app's public surface only, so the
//! presentation layer never names a `genealogy-core` type, even in tests (ADR 0008).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use genealogy_app::{
    Agent, AgentId, AgentKind, AppDefaults, EvidenceLevel, NewPerson, OperatorConfig, PersonNameParts, Provenance,
    Session, Workspace, WorkspaceDefaults, create_person, merge_persons,
};
use genealogy_ui::{Localizer, MergeBlockedVm};
use uuid::Uuid;

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

async fn setup() -> (Workspace, Session, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, session(), dir)
}

async fn person(ws: &Workspace, session: &Session, given: &str, surname: &str) -> String {
    create_person(
        ws,
        session,
        NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(
                Some(given.to_owned()),
                Some(surname.to_owned()),
            )),
            evidence_level: EvidenceLevel::Conclusion,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("person")
}

#[tokio::test]
async fn a_merge_conflict_becomes_a_blocked_view_model_with_the_reason() {
    let (ws, session, dir) = setup().await;
    let loc = Localizer::for_workspace(&dir.path().join("ws"));
    let solo = person(&ws, &session, "John", "Smith").await;

    let error = merge_persons(&ws, &session, &solo, &solo, None)
        .await
        .expect_err("a self-merge is a conflict");
    let blocked = MergeBlockedVm::from_error(&error, &loc).expect("a conflict is a blocked merge");

    assert!(!blocked.heading.is_empty(), "the heading is localized");
    assert!(!blocked.guidance.is_empty(), "the guidance is localized");
    assert!(
        blocked.detail.contains("itself"),
        "the detail surfaces the core reason: {}",
        blocked.detail
    );
}

#[tokio::test]
async fn a_non_conflict_failure_is_not_a_blocked_merge() {
    let (ws, session, dir) = setup().await;
    let loc = Localizer::for_workspace(&dir.path().join("ws"));
    let survivor = person(&ws, &session, "John", "Smith").await;

    let error = merge_persons(&ws, &session, &survivor, "I9999", None)
        .await
        .expect_err("an unknown id is not found");
    assert!(
        MergeBlockedVm::from_error(&error, &loc).is_none(),
        "a not-found is not a blocked merge"
    );
}
