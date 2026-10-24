//! `reference_label`: what a stored record id names, so a frontend shows a name and never an id
//! (ADR 0052 §3, §10).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, ListedKind, NewPlace, OperatorConfig, Provenance, Session, Workspace, WorkspaceDefaults, create_place,
    reference_id, reference_label,
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

async fn workspace() -> (Workspace, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, dir)
}

#[tokio::test]
async fn a_record_id_is_labelled_by_its_human_id_and_title() {
    let (ws, _dir) = workspace().await;
    let new = NewPlace {
        human_id: None,
        place_type: vitni_app::std_statement("parish").unwrap_or_default(),
        name: Some("Fana".to_owned()),
    };
    let human_id = create_place(&ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("place");
    let id = reference_id(&ws, ListedKind::Event, "place", &human_id)
        .await
        .expect("id");

    let label = reference_label(&ws, ListedKind::Event, "place", &id)
        .await
        .expect("label")
        .expect("the place exists");

    assert_eq!(label.human_id, human_id);
    assert_eq!(label.title.as_deref(), Some("Fana"));
}

#[tokio::test]
async fn an_id_nothing_has_is_no_label() {
    let (ws, _dir) = workspace().await;
    let gone = Uuid::from_u128(0xdead).to_string();

    for (kind, column) in [(ListedKind::Person, "tags"), (ListedKind::Event, "place")] {
        let label = reference_label(&ws, kind, column, &gone).await.expect("label");
        assert_eq!(label, None, "{kind:?} {column}");
    }
}

#[tokio::test]
async fn an_untitled_record_has_no_title_to_repeat_its_human_id() {
    let (ws, _dir) = workspace().await;
    let new = NewPlace {
        human_id: None,
        place_type: vitni_app::std_statement("parish").unwrap_or_default(),
        name: None,
    };
    let human_id = create_place(&ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("place");
    let id = reference_id(&ws, ListedKind::Event, "place", &human_id)
        .await
        .expect("id");

    let label = reference_label(&ws, ListedKind::Event, "place", &id)
        .await
        .expect("label")
        .expect("the place exists");

    assert_eq!(label.human_id, human_id);
    assert_eq!(label.title, None);
}
