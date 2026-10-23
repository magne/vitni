//! Place types on merged clusters and the per-record base a retype builds on (ADR 0054): a cluster's
//! summary shows the root's type, falling back to a member's only for display; `place_type_at` reads
//! the record's own live entries at a date (never a member's), the latest of several at the same date;
//! and a member's type row is owned by that member.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, DateParts, GenealogicalDate, IdentityDecision, MutationMeta, NewPlace, OperatorConfig,
    PlaceTypeStatement, Provenance, Session, Workspace, WorkspaceDefaults, create_place, gregorian_date, merge_places,
    place_claim_owner, place_type_at, set_place_type, show_place,
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
async fn a_merged_cluster_shows_the_roots_type_and_the_member_keeps_its_own() {
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
    let member = place_type_at(&ws, &hougen, None)
        .await
        .expect("member")
        .expect("a type");

    let key = |statement: Option<&PlaceTypeStatement>| {
        statement
            .and_then(|s| s.type_key.as_ref())
            .map(|k| k.as_str().to_owned())
    };
    assert_eq!(key(summary.place_type.as_ref()).as_deref(), Some("std:farm"));
    assert_eq!(
        key(Some(&member)).as_deref(),
        Some("std:hamlet"),
        "the member keeps its own type"
    );
}

#[tokio::test]
async fn the_set_type_base_is_the_records_own_type_not_a_members() {
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

    let base = place_type_at(&ws, &haugen, None).await.expect("base").expect("a type");

    assert_eq!(base.type_key.expect("key").as_str(), "std:farm");
}

#[tokio::test]
async fn a_members_type_row_is_owned_by_that_member() {
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
    let member_view = ws.store().find_place(&hougen).await.expect("find").expect("member");
    let member_row = member_view
        .place_types_with_assertions()
        .first()
        .expect("the member's type row")
        .assertion_id
        .to_string();

    let owner = place_claim_owner(&ws, &haugen, &member_row).await.expect("owner");

    assert_eq!(owner, hougen);
}

async fn open_workspace(dir: &tempfile::TempDir) -> Workspace {
    let root = dir.path().join("ws");
    Workspace::init(&root, &operator(), &AppDefaults::default(), None).expect("init");
    Workspace::open(&root, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open")
}

fn year(year: i32) -> GenealogicalDate {
    gregorian_date(DateParts {
        year,
        month: None,
        day: None,
    })
}

fn std_at(key: &str, date: Option<GenealogicalDate>) -> PlaceTypeStatement {
    PlaceTypeStatement {
        date,
        ..vitni_app::std_statement(key).expect("a std key")
    }
}

fn key_of(statement: Option<PlaceTypeStatement>) -> Option<String> {
    statement.and_then(|s| s.type_key).map(|k| k.as_str().to_owned())
}

async fn set_type(ws: &Workspace, human_id: &str, statement: PlaceTypeStatement, supersedes: Option<&str>) {
    let meta = MutationMeta {
        supersedes,
        ..MutationMeta::default()
    };
    set_place_type(ws, &session(), human_id, statement, meta)
        .await
        .expect("set type");
}

#[tokio::test]
async fn a_dated_read_on_a_merged_place_sees_only_the_records_own_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = open_workspace(&dir).await;
    let session = session();
    let haugen = place(&ws, &session, "Haugen", "farm").await;
    let hougen = place(&ws, &session, "Hougen", "hamlet").await;
    set_type(&ws, &hougen, std_at("parish", Some(year(1838))), None).await;
    merge_places(&ws, &session, &haugen, &hougen, IdentityDecision::default())
        .await
        .expect("merge");

    let root = place_type_at(&ws, &haugen, Some(&year(1838))).await.expect("root");
    let member = place_type_at(&ws, &hougen, Some(&year(1838))).await.expect("member");

    assert_eq!(root, None, "the member's 1838 entry is not the root's");
    assert_eq!(key_of(member).as_deref(), Some("std:parish"));
}

#[tokio::test]
async fn a_root_with_no_type_shows_a_members_but_owns_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = open_workspace(&dir).await;
    let session = session();
    let untyped = NewPlace {
        human_id: None,
        place_type: PlaceTypeStatement::default(),
        name: Some("Haugen".to_owned()),
    };
    let haugen = create_place(&ws, &session, untyped, Provenance::default(), &[])
        .await
        .expect("place");
    let hougen = place(&ws, &session, "Hougen", "hamlet").await;
    set_type(&ws, &hougen, std_at("parish", Some(year(1838))), None).await;
    merge_places(&ws, &session, &haugen, &hougen, IdentityDecision::default())
        .await
        .expect("merge");

    let summary = show_place(&ws, &haugen).await.expect("show").expect("found");

    assert_eq!(key_of(summary.place_type).as_deref(), Some("std:parish"));
    assert_eq!(
        summary.own_place_type, None,
        "an edit of the root takes no date from the member"
    );
}

#[tokio::test]
async fn of_two_live_entries_at_one_date_the_latest_is_the_base() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = open_workspace(&dir).await;
    let haugen = place(&ws, &session(), "Haugen", "farm").await;
    set_type(&ws, &haugen, std_at("hamlet", None), None).await;
    let undated = place_type_at(&ws, &haugen, None).await.expect("undated");
    set_type(&ws, &haugen, std_at("parish", Some(year(1838))), None).await;
    let created = ws
        .store()
        .find_place(&haugen)
        .await
        .expect("find")
        .and_then(|view| view.created_assertion())
        .expect("created")
        .to_string();
    set_type(&ws, &haugen, std_at("county", Some(year(1838))), Some(&created)).await;

    let dated = place_type_at(&ws, &haugen, Some(&year(1838))).await.expect("dated");

    assert_eq!(
        key_of(undated).as_deref(),
        Some("std:hamlet"),
        "the write beside the live creation entry"
    );
    assert_eq!(
        key_of(dated).as_deref(),
        Some("std:county"),
        "a caller-named supersede leaves both 1838 live"
    );
}
