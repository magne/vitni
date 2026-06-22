//! Import use-case integration tests: resolve-or-create by external id, applied additively, so a
//! re-import of unchanged records produces no new persons, families, or names.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use genealogy_app::{
    AppDefaults, ExternalId, OperatorConfig, Session, Workspace, WorkspaceDefaults, import_add_partner, import_family,
    import_person, list_families, list_persons, show_person,
};
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

fn uid(value: &str) -> ExternalId {
    ExternalId {
        authority: "gedcom-uid".to_owned(),
        value: value.to_owned(),
        kind: None,
        url: None,
    }
}

#[tokio::test]
async fn re_importing_the_same_person_resolves_to_the_existing_one() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let (first, created) = import_person(
        &ws,
        &session,
        uid("P-1"),
        Some("Ada".to_owned()),
        Some("Lovelace".to_owned()),
    )
    .await
    .expect("first import");
    assert!(created, "first import creates the person");

    let (second, created_again) = import_person(
        &ws,
        &session,
        uid("P-1"),
        Some("Ada".to_owned()),
        Some("Lovelace".to_owned()),
    )
    .await
    .expect("second import");
    assert!(!created_again, "second import resolves the existing person");
    assert_eq!(first, second, "same human_id");

    // No duplicate person, and the identical name was not asserted again.
    assert_eq!(list_persons(&ws).await.expect("list").len(), 1);
    let view = show_person(&ws, &first).await.expect("show").expect("present");
    assert_eq!(view.display_name.as_deref(), Some("Ada Lovelace"));
}

#[tokio::test]
async fn a_new_name_on_an_existing_person_is_added_additively() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let (human_id, _) = import_person(
        &ws,
        &session,
        uid("P-1"),
        Some("Ada".to_owned()),
        Some("Lovelace".to_owned()),
    )
    .await
    .expect("import");
    // Re-import the same record but with a different name: it is added, not overwritten.
    import_person(
        &ws,
        &session,
        uid("P-1"),
        Some("Augusta Ada".to_owned()),
        Some("King".to_owned()),
    )
    .await
    .expect("re-import with new name");

    assert_eq!(list_persons(&ws).await.expect("list").len(), 1, "still one person");
    let view = show_person(&ws, &human_id).await.expect("show").expect("present");
    // The primary (first) name is unchanged; the new name is added alongside it.
    assert_eq!(view.display_name.as_deref(), Some("Ada Lovelace"));
}

#[tokio::test]
async fn re_importing_a_family_and_its_partners_is_idempotent() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let (husband, _) = import_person(
        &ws,
        &session,
        uid("I-1"),
        Some("John".to_owned()),
        Some("Smith".to_owned()),
    )
    .await
    .expect("husband");
    let (wife, _) = import_person(
        &ws,
        &session,
        uid("I-2"),
        Some("Jane".to_owned()),
        Some("Doe".to_owned()),
    )
    .await
    .expect("wife");

    for _ in 0..2 {
        let (family, _) = import_family(&ws, &session, uid("F-1")).await.expect("family");
        import_add_partner(&ws, &session, &family, &husband)
            .await
            .expect("partner husband");
        import_add_partner(&ws, &session, &family, &wife)
            .await
            .expect("partner wife");
    }

    let families = list_families(&ws).await.expect("list families");
    assert_eq!(families.len(), 1, "re-import creates no duplicate family");
    assert_eq!(families[0].partners.len(), 2, "partners added once, not duplicated");
}
