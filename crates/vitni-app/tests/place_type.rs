//! A place's type is a dated statement of kind, level and key (ADR 0031 §3, ADR 0054): what the
//! app's readers do with it — citations, the country lookup and the history.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, ChangeValue, CitingContext, EvidenceLevel, MutationMeta, NewCitation, NewPerson, NewPlace, NewSource,
    OperatorConfig, Provenance, Session, Workspace, WorkspaceDefaults, add_person_citation, assert_place_enclosed_by,
    change_log_for_place, create_citation, create_person, create_place, create_source, place_profile, set_place_type,
    show_person, show_source,
};
use vitni_core::ids::AgentId;
use vitni_core::place_type::{PlaceKind, PlaceLevel, PlaceTypeKey, PlaceTypeStatement};
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

fn statement(kind: Option<PlaceKind>, level: Option<PlaceLevel>, key: Option<&str>) -> PlaceTypeStatement {
    PlaceTypeStatement {
        kind,
        level,
        type_key: key.map(|key| PlaceTypeKey::new(key).expect("key")),
        date: None,
    }
}

async fn create(ws: &Workspace, name: &str, type_statement: PlaceTypeStatement, citations: &[String]) -> String {
    let new = NewPlace {
        human_id: None,
        place_type: type_statement,
        name: Some(name.to_owned()),
    };
    create_place(ws, &session(), new, Provenance::default(), citations)
        .await
        .expect("create place")
}

#[tokio::test]
async fn a_citation_on_a_places_type_stays_counted_across_a_retype() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let source = create_source(
        &ws,
        &session,
        NewSource {
            human_id: None,
            title: Some("Parish register".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("source");
    let citation = create_citation(
        &ws,
        &session,
        NewCitation {
            human_id: None,
            source: source.clone(),
            page: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("citation");
    let farm = statement(Some(PlaceKind::Settlement), None, Some("std:farm"));
    let place = create(&ws, "Haugen", farm, std::slice::from_ref(&citation)).await;

    let parish = statement(Some(PlaceKind::Ecclesiastical), None, Some("std:parish"));
    set_place_type(&ws, &session, &place, parish, MutationMeta::default())
        .await
        .expect("retype");

    let summary = show_source(&ws, &source).await.expect("show").expect("source");
    let row = summary
        .citations
        .iter()
        .find(|row| row.citation.human_id == citation)
        .expect("the citation is listed");
    assert!(
        row.backers
            .iter()
            .any(|backer| backer.human_id == place && matches!(backer.context, CitingContext::PlaceType)),
        "{:?}",
        row.backers
    );
}

#[tokio::test]
async fn the_country_is_found_by_level_not_by_key() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let norge = create(
        &ws,
        "Norge",
        statement(
            Some(PlaceKind::Administrative),
            Some(PlaceLevel::Country),
            Some("custom:Rike"),
        ),
        &[],
    )
    .await;
    let farm = create(&ws, "Haugen", statement(None, None, Some("std:farm")), &[]).await;
    assert_place_enclosed_by(&ws, &session, &farm, &norge, MutationMeta::default())
        .await
        .expect("enclose");

    let profile = place_profile(&ws, &farm).await.expect("profile");

    assert_eq!(profile.country.as_deref(), Some("Norge"));
}

#[tokio::test]
async fn a_country_key_without_the_country_level_is_not_the_country() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let norge = create(&ws, "Norge", statement(None, None, Some("std:country")), &[]).await;
    let farm = create(&ws, "Haugen", statement(None, None, Some("std:farm")), &[]).await;
    assert_place_enclosed_by(&ws, &session, &farm, &norge, MutationMeta::default())
        .await
        .expect("enclose");

    let profile = place_profile(&ws, &farm).await.expect("profile");

    assert_eq!(profile.country, None);
}

#[tokio::test]
async fn the_history_of_a_place_created_without_a_type_shows_no_type_value() {
    let (ws, _dir) = workspace().await;
    let new = NewPlace {
        human_id: None,
        place_type: PlaceTypeStatement::default(),
        name: None,
    };
    let place = create_place(&ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create place");

    let log = change_log_for_place(&ws, &place).await.expect("log");

    let created: Vec<_> = log.iter().filter(|entry| entry.event_type == "PlaceCreated").collect();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].value, None);
}

#[tokio::test]
async fn the_history_of_a_typed_place_shows_the_statement() {
    let (ws, _dir) = workspace().await;
    let farm = statement(Some(PlaceKind::Settlement), None, Some("std:farm"));
    let place = create(&ws, "Haugen", farm.clone(), &[]).await;

    let log = change_log_for_place(&ws, &place).await.expect("log");

    let created = log
        .iter()
        .find(|entry| entry.event_type == "PlaceCreated")
        .expect("created entry");
    assert_eq!(created.value, Some(ChangeValue::PlaceType(farm)));
}

#[tokio::test]
async fn a_citation_on_a_retyped_places_creation_entry_still_counts_toward_its_backs() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let source = create_source(
        &ws,
        &session,
        NewSource {
            human_id: None,
            title: Some("Parish register".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("source");
    let citation = create_citation(
        &ws,
        &session,
        NewCitation {
            human_id: None,
            source,
            page: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("citation");
    let farm = statement(Some(PlaceKind::Settlement), None, Some("std:farm"));
    let place = create(&ws, "Haugen", farm, std::slice::from_ref(&citation)).await;
    let parish = statement(Some(PlaceKind::Ecclesiastical), None, Some("std:parish"));
    set_place_type(&ws, &session, &place, parish, MutationMeta::default())
        .await
        .expect("retype");
    let person = create_person(
        &ws,
        &session,
        NewPerson {
            human_id: None,
            name: None,
            evidence_level: EvidenceLevel::Conclusion,
            external_ids: Vec::new(),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("person");
    add_person_citation(&ws, &session, &person, &citation, MutationMeta::default())
        .await
        .expect("cite the person");

    let shown = show_person(&ws, &person).await.expect("show").expect("person");

    let row = shown
        .citations
        .iter()
        .find(|row| row.human_id == citation)
        .unwrap_or_else(|| panic!("{:?}", shown.citations));
    assert_eq!(row.backs_count, 2, "the person and the place's creation type entry");
}
