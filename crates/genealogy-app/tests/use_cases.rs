//! Person use-case integration tests: create / name / show / list against a temp workspace dir.
//!
//! These exercise the full application path — id generation, meta stamping, command execution
//! through the engine-neutral store, projection query — over a real on-disk workspace directory.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use genealogy_app::{
    AppDefaults, NewPerson, OperatorConfig, PersonNameParts, Provenance, Session, Workspace, WorkspaceDefaults,
    add_name, assert_association, assert_fact, create_person, list_persons, show_person,
};
use genealogy_core::enums::{AssociationRole, EvidenceLevel, FactType};
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
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

async fn workspace() -> (Workspace, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, dir)
}

fn new_person(given: &str, surname: &str) -> NewPerson {
    NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some(surname.to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
    }
}

#[tokio::test]
async fn create_auto_allocates_sequential_human_ids() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let first = create_person(&ws, &session, new_person("Ada", "Lovelace"))
        .await
        .expect("create");
    let second = create_person(&ws, &session, new_person("Alan", "Turing"))
        .await
        .expect("create");
    assert_eq!(first, "I0001");
    assert_eq!(second, "I0002");
}

#[tokio::test]
async fn create_honors_a_supplied_id_then_rejects_a_duplicate() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let supplied = NewPerson {
        human_id: Some("I0500".to_owned()),
        name: Some(PersonNameParts::simple(
            Some("Grace".to_owned()),
            Some("Hopper".to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
    };
    let assigned = create_person(&ws, &session, supplied.clone()).await.expect("create");
    assert_eq!(assigned, "I0500");

    let err = create_person(&ws, &session, supplied).await;
    assert!(matches!(err, Err(genealogy_app::AppError::HumanIdTaken(id)) if id == "I0500"));
}

#[tokio::test]
async fn show_reflects_an_added_name() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let id = create_person(
        &ws,
        &session,
        NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(Some("Ada".to_owned()), None)),
            evidence_level: EvidenceLevel::Conclusion,
        },
    )
    .await
    .expect("create");

    add_name(
        &ws,
        &session,
        &id,
        PersonNameParts::simple(Some("Augusta".to_owned()), Some("Lovelace".to_owned())),
        Provenance::default(),
        &[],
    )
    .await
    .expect("add name");

    let summary = show_person(&ws, &id).await.expect("show").expect("person exists");
    assert_eq!(summary.human_id, "I0001");
    // The first asserted name (given-only "Ada") is the display name.
    assert_eq!(summary.display_name.as_deref(), Some("Ada"));
}

#[tokio::test]
async fn list_returns_persons_in_human_id_order() {
    let (ws, _dir) = workspace().await;
    let session = session();
    create_person(&ws, &session, new_person("Ada", "Lovelace"))
        .await
        .expect("create");
    create_person(&ws, &session, new_person("Alan", "Turing"))
        .await
        .expect("create");

    let people = list_persons(&ws).await.expect("list");
    let ids: Vec<&str> = people.iter().map(|p| p.human_id.as_str()).collect();
    assert_eq!(ids, ["I0001", "I0002"]);
    assert_eq!(people[0].display_name.as_deref(), Some("Ada Lovelace"));
}

#[tokio::test]
async fn list_surfaces_facts_and_resolves_association_targets_to_human_ids() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let john = create_person(&ws, &session, new_person("John", "Smith"))
        .await
        .expect("create john");
    let jane = create_person(&ws, &session, new_person("Jane", "Doe"))
        .await
        .expect("create jane");

    assert_fact(
        &ws,
        &session,
        &john,
        FactType::Occupation,
        Some("Carpenter".to_owned()),
        None,
    )
    .await
    .expect("assert fact");
    assert_association(&ws, &session, &john, &jane, AssociationRole::Witness)
        .await
        .expect("assert association");

    let summary = show_person(&ws, &john).await.expect("show").expect("john exists");
    assert_eq!(summary.facts.len(), 1, "the occupation fact surfaces");
    assert_eq!(summary.facts[0].fact_type, FactType::Occupation);
    assert_eq!(summary.facts[0].value.as_deref(), Some("Carpenter"));
    assert_eq!(
        summary.associations,
        vec![(jane.clone(), AssociationRole::Witness)],
        "the association target resolves to its human_id"
    );
}

#[tokio::test]
async fn missing_person_and_empty_name_surface_distinct_errors() {
    let (ws, _dir) = workspace().await;
    let session = session();
    create_person(&ws, &session, new_person("Ada", "Lovelace"))
        .await
        .expect("create");

    let missing = add_name(
        &ws,
        &session,
        "I9999",
        PersonNameParts::simple(Some("X".to_owned()), None),
        Provenance::default(),
        &[],
    )
    .await;
    assert!(matches!(missing, Err(genealogy_app::AppError::PersonNotFound(id)) if id == "I9999"));

    let empty = add_name(
        &ws,
        &session,
        "I0001",
        PersonNameParts::simple(None, None),
        Provenance::default(),
        &[],
    )
    .await;
    assert!(matches!(empty, Err(genealogy_app::AppError::Domain(_))));
}
