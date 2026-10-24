//! Paged lists (#607): a page is a window over the records a list shows — a record merged into another
//! is not listed and takes no slot (ADR 0039 §5) — and a cluster root on a page still reads with its
//! members' data, exactly as in the whole list.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, IdentityDecision, ListPage, ListQuery, ListedKind, NewPerson, NewPlace, OperatorConfig,
    PersonNameParts, PersonSummary, Provenance, Session, Workspace, WorkspaceDefaults, create_person, create_place,
    list_persons, list_persons_page, list_places, list_places_page, merge_persons, merge_places,
};
use vitni_core::enums::EvidenceLevel;
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

async fn person(ws: &Workspace, given: &str) -> String {
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some("Hansen".to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create person")
}

async fn place(ws: &Workspace, name: &str) -> String {
    let new = NewPlace {
        human_id: None,
        place_type: vitni_app::std_statement("farm").unwrap_or_default(),
        name: Some(name.to_owned()),
    };
    create_place(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create place")
}

const fn page(offset: u32, limit: u32) -> ListPage {
    ListPage {
        offset,
        limit: Some(limit),
    }
}

#[tokio::test]
async fn a_person_page_skips_merged_records_and_keeps_their_data_on_the_root() {
    let (ws, _dir) = workspace().await;
    let ole = person(&ws, "Ole").await;
    let per = person(&ws, "Per").await;
    let ola = person(&ws, "Ola").await;
    let jon = person(&ws, "Jon").await;
    merge_persons(&ws, &session(), &ole, &ola, IdentityDecision::default())
        .await
        .expect("merge");

    let first = list_persons_page(&ws, &ListQuery::new(ListedKind::Person), page(0, 2))
        .await
        .expect("first page")
        .items;
    let ids: Vec<&str> = first.iter().map(|p| p.human_id.as_str()).collect();
    assert_eq!(ids, [ole.as_str(), per.as_str()]);
    let merged: Vec<&str> = first[0].merged.iter().map(|m| m.human_id.as_str()).collect();
    assert_eq!(merged, [ola.as_str()], "the root carries its member");
    assert_eq!(first[0].names.len(), 2, "the member's name is folded into the root");

    let second = list_persons_page(&ws, &ListQuery::new(ListedKind::Person), page(2, 2))
        .await
        .expect("second page")
        .items;
    let ids: Vec<&str> = second.iter().map(|p| p.human_id.as_str()).collect();
    assert_eq!(ids, [jon.as_str()]);

    assert_eq!(
        list_persons_page(&ws, &ListQuery::new(ListedKind::Person), page(3, 2))
            .await
            .expect("past the end")
            .items,
        Vec::<PersonSummary>::new()
    );
    let tiled: Vec<PersonSummary> = first.into_iter().chain(second).collect();
    assert_eq!(
        tiled,
        list_persons(&ws).await.expect("whole list"),
        "the pages tile the whole list"
    );
}

#[tokio::test]
async fn a_place_page_skips_merged_records_and_keeps_their_data_on_the_root() {
    let (ws, _dir) = workspace().await;
    let haugen = place(&ws, "Haugen").await;
    let bakken = place(&ws, "Bakken").await;
    let hougen = place(&ws, "Hougen").await;
    merge_places(&ws, &session(), &haugen, &hougen, IdentityDecision::default())
        .await
        .expect("merge");

    let first = list_places_page(&ws, &ListQuery::new(ListedKind::Place), page(0, 1))
        .await
        .expect("first page")
        .items;
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].human_id, haugen);
    let merged: Vec<&str> = first[0].merged.iter().map(|m| m.human_id.as_str()).collect();
    assert_eq!(merged, [hougen.as_str()]);

    let second = list_places_page(&ws, &ListQuery::new(ListedKind::Place), page(1, 5))
        .await
        .expect("second page")
        .items;
    let ids: Vec<&str> = second.iter().map(|p| p.human_id.as_str()).collect();
    assert_eq!(ids, [bakken.as_str()]);
    let tiled: Vec<_> = first.into_iter().chain(second).collect();
    assert_eq!(
        tiled,
        list_places(&ws).await.expect("whole list"),
        "the pages tile the whole list"
    );
}
