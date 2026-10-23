//! A merged place cluster's summary lists the live type of every member (ADR 0054).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, IdentityDecision, NewPlace, OperatorConfig, Provenance, Session, Workspace, WorkspaceDefaults,
    create_place, merge_places, show_place,
};
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

async fn place(ws: &Workspace, session: &Session, name: &str, key: &str) -> String {
    create_place(
        ws,
        session,
        NewPlace {
            human_id: None,
            place_type: vitni_app::std_statement(key).expect("a std key"),
            name: Some(name.to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place")
}

#[tokio::test]
async fn a_merged_cluster_lists_both_members_types() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("ws");
    Workspace::init(&root, &operator(), &AppDefaults::default(), None).expect("init");
    let ws = Workspace::open(&root, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open");
    let session = session();
    let haugen = place(&ws, &session, "Haugen", "farm").await;
    let hougen = place(&ws, &session, "Hougen", "hamlet").await;
    merge_places(&ws, &session, &haugen, &hougen, IdentityDecision::default())
        .await
        .expect("merge");

    let summary = show_place(&ws, &haugen).await.expect("show").expect("found");

    let mut keys: Vec<String> = summary
        .place_types
        .iter()
        .filter_map(|entry| entry.statement.type_key.as_ref().map(|key| key.as_str().to_owned()))
        .collect();
    keys.sort();
    assert_eq!(keys, ["std:farm".to_owned(), "std:hamlet".to_owned()]);
}
