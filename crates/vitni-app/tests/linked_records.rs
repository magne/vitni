//! A conclusion person's *Linked records* (ADR 0039 §5): each record of its cluster with the import
//! record it came from, the origin of every claim, and *Unlink*, which splits a record back out.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, AppError, IdentityDecision, NewImportRun, NewPerson, OperatorConfig, PersonNameParts, Provenance,
    Session, Workspace, WorkspaceDefaults, create_person, linked_records, list_persons, merge_persons, record_url,
    start_import_run, unlink_person,
};
use vitni_core::enums::EvidenceLevel;
use vitni_core::ids::{AgentId, ImportRunId};
use vitni_core::origin::{DatasetId, RecordOrigin};
use vitni_core::person::PersonError;
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

fn new_person(given: &str, evidence_level: EvidenceLevel) -> NewPerson {
    NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some("Hansen".to_owned()),
        )),
        evidence_level,
        external_ids: Vec::new(),
    }
}

/// Creates a person entered by hand, returning its `human_id`.
async fn person(ws: &Workspace, given: &str) -> String {
    create_person(
        ws,
        &session(),
        new_person(given, EvidenceLevel::Conclusion),
        Provenance::default(),
        &[],
    )
    .await
    .expect("create person")
}

/// Starts a Digitalarkivet run, returning its id.
async fn run(ws: &Workspace) -> ImportRunId {
    let run = NewImportRun {
        plugin: "digitalarkivet-import".to_owned(),
        plugin_version: "1.0.0".to_owned(),
        dataset: DatasetId::global("digitalarkivet"),
        dataset_label: "Digitalarkivet".to_owned(),
        source_label: "1910 census".to_owned(),
        file_asserted_at: None,
        dataset_hint: None,
    };
    start_import_run(ws, &session(), run).await.expect("start run")
}

fn origin(record: &str, run: ImportRunId) -> RecordOrigin {
    RecordOrigin {
        dataset: DatasetId::global("digitalarkivet"),
        record: record.to_owned(),
        item: None,
        digest: None,
        run,
    }
}

/// Creates a persona imported from `record`, returning its `human_id`.
async fn persona(ws: &Workspace, given: &str, origin: RecordOrigin) -> String {
    let provenance = Provenance {
        origin: Some(origin),
        ..Provenance::default()
    };
    create_person(
        ws,
        &session(),
        new_person(given, EvidenceLevel::Persona),
        provenance,
        &[],
    )
    .await
    .expect("create persona")
}

async fn merge(ws: &Workspace, surviving: &str, merged: &str) {
    merge_persons(ws, &session(), surviving, merged, IdentityDecision::default())
        .await
        .expect("merge");
}

async fn listed(ws: &Workspace) -> Vec<String> {
    list_persons(ws)
        .await
        .expect("list")
        .into_iter()
        .map(|person| person.human_id)
        .collect()
}

#[tokio::test]
async fn a_root_lists_itself_then_every_member_with_its_origin() {
    let (ws, _dir) = workspace().await;
    let run = run(&ws).await;
    let a = person(&ws, "Ole").await;
    let b = persona(&ws, "Ola", origin("pf01", run)).await;
    let c = persona(&ws, "Olav", origin("pf02", run)).await;
    merge(&ws, &b, &c).await;
    merge(&ws, &a, &b).await;

    let linked = linked_records(&ws, &c).await.expect("linked");
    let ids: Vec<&str> = linked.records.iter().map(|r| r.record.human_id.as_str()).collect();
    assert_eq!(
        ids,
        [a.as_str(), b.as_str(), c.as_str()],
        "the root first, read from any member"
    );

    let root = &linked.records[0];
    assert!(root.root);
    assert_eq!(root.evidence_level, EvidenceLevel::Conclusion);
    assert_eq!(root.origin, None, "entered by hand");
    assert_eq!(root.via, None);

    let b_row = &linked.records[1];
    assert!(!b_row.root);
    assert_eq!(b_row.display_name.as_deref(), Some("Ola Hansen"));
    assert_eq!(b_row.evidence_level, EvidenceLevel::Persona);
    assert_eq!(b_row.via, None, "merged into the root itself");
    let b_origin = b_row.origin.as_ref().expect("imported");
    assert_eq!(b_origin.origin.record, "pf01");
    assert_eq!(b_origin.dataset_label.as_deref(), Some("Digitalarkivet"));
    assert_eq!(b_origin.source_label.as_deref(), Some("1910 census"));
    assert_eq!(b_origin.url.as_deref(), Some("https://www.digitalarkivet.no/pf01"));

    assert_eq!(linked.records[2].via.as_deref(), Some(b.as_str()), "linked through B");
}

#[tokio::test]
async fn every_imported_claim_of_the_cluster_names_its_origin() {
    let (ws, _dir) = workspace().await;
    let run = run(&ws).await;
    let a = person(&ws, "Ole").await;
    let b = persona(&ws, "Ola", origin("pf01", run)).await;
    merge(&ws, &a, &b).await;

    let linked = linked_records(&ws, &a).await.expect("linked");
    let summary = vitni_app::show_person(&ws, &a).await.expect("show").expect("found");
    let b_name = summary
        .names
        .iter()
        .find(|name| summary.owner_of(&name.assertion_id) == b)
        .expect("B's name");
    let a_name = summary
        .names
        .iter()
        .find(|name| summary.owner_of(&name.assertion_id) == a)
        .expect("A's name");
    let named = linked
        .claim_origins
        .get(&b_name.assertion_id)
        .expect("B's name has an origin");
    assert_eq!(named.origin.record, "pf01");
    assert!(
        !linked.claim_origins.contains_key(&a_name.assertion_id),
        "A's name was typed"
    );
}

#[tokio::test]
async fn a_person_with_no_members_lists_only_itself() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole").await;
    let linked = linked_records(&ws, &a).await.expect("linked");
    assert_eq!(linked.records.len(), 1);
    assert!(linked.records[0].root);
}

#[tokio::test]
async fn an_unknown_person_has_no_linked_records() {
    let (ws, _dir) = workspace().await;
    let result = linked_records(&ws, "I9999").await;
    assert!(matches!(result, Err(AppError::PersonNotFound(_))), "{result:?}");
}

#[tokio::test]
async fn unlinking_a_member_restores_two_people() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole").await;
    let b = person(&ws, "Ola").await;
    merge(&ws, &a, &b).await;
    assert_eq!(listed(&ws).await, std::slice::from_ref(&a));

    unlink_person(&ws, &session(), &a, &b, Some("different fathers".to_owned()))
        .await
        .expect("unlink");
    assert_eq!(listed(&ws).await, [a.clone(), b.clone()]);
    assert_eq!(linked_records(&ws, &a).await.expect("linked").records.len(), 1);
}

#[tokio::test]
async fn unlinking_a_member_linked_through_another_retracts_the_edge_on_its_holder() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole").await;
    let b = person(&ws, "Ola").await;
    let c = person(&ws, "Olav").await;
    merge(&ws, &b, &c).await;
    merge(&ws, &a, &b).await;

    unlink_person(&ws, &session(), &a, &c, None).await.expect("unlink");
    assert_eq!(listed(&ws).await, [a.clone(), c.clone()]);
    let ids: Vec<String> = linked_records(&ws, &a)
        .await
        .expect("linked")
        .records
        .into_iter()
        .map(|r| r.record.human_id)
        .collect();
    assert_eq!(ids, [a, b], "B stays linked");
}

#[tokio::test]
async fn unlinking_a_member_takes_the_records_linked_through_it_along() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole").await;
    let b = person(&ws, "Ola").await;
    let c = person(&ws, "Olav").await;
    merge(&ws, &b, &c).await;
    merge(&ws, &a, &b).await;

    unlink_person(&ws, &session(), &a, &b, None).await.expect("unlink");
    assert_eq!(listed(&ws).await, [a.clone(), b.clone()]);
    let ids: Vec<String> = linked_records(&ws, &b)
        .await
        .expect("linked")
        .records
        .into_iter()
        .map(|r| r.record.human_id)
        .collect();
    assert_eq!(ids, [b, c]);
}

#[tokio::test]
async fn a_record_outside_the_cluster_cannot_be_unlinked() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole").await;
    let b = person(&ws, "Ola").await;
    let result = unlink_person(&ws, &session(), &a, &b, None).await;
    assert!(
        matches!(result, Err(AppError::Domain(PersonError::NotLinked { .. }))),
        "{result:?}"
    );
    let result = unlink_person(&ws, &session(), &a, &a, None).await;
    assert!(
        matches!(result, Err(AppError::Domain(PersonError::NotLinked { .. }))),
        "the root itself: {result:?}"
    );
}

#[test]
fn only_a_dataset_with_a_url_form_links_out() {
    let run = ImportRunId::from_uuid(Uuid::from_u128(9));
    assert_eq!(
        record_url(&origin("pd00000020636420", run)).as_deref(),
        Some("https://www.digitalarkivet.no/pd00000020636420")
    );
    let gedcom = RecordOrigin {
        dataset: DatasetId::lineage("gedcom", Uuid::from_u128(1)),
        ..origin("I1", run)
    };
    assert_eq!(record_url(&gedcom), None);
}
