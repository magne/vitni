//! Place temporal-resolution and succession integration tests (ADR 0026): the transitive
//! hierarchy walk, the date-aware resolution rule, and place succession — end to end against a
//! temp workspace.
//!
//! The app-layer `assert_place_enclosed_by` use-case does not yet accept a date (a pre-existing
//! gap, not part of this ADR's scope), so the dated-enclosure setup here executes a raw
//! `PlaceCommand::AssertEnclosedBy` through the public `Store` directly — everything under test
//! (the resolution rule, the walk, succession) still runs through the public `vitni_app` API.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, AppError, MutationMeta, NewPlace, OperatorConfig, PlaceError, PlaceSuccessionInput, PlaceType,
    Provenance, Session, SuccessionKind, Workspace, WorkspaceDefaults, assert_place_enclosed_by,
    assert_place_succession, create_place, show_place, show_place_as_of,
};
use vitni_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use vitni_core::ids::AgentId;
use vitni_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use vitni_core::place_ref::PlaceRef;
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

/// A minimal exact-year `GenealogicalDate`, with `sort_value` set directly (the resolution rule
/// reads only this field — data-model §7.1).
fn year(value: i32) -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(value),
            month: None,
            day: None,
        })),
        time: None,
        new_year_begins: None,
        sort_value: i64::from(value),
        original_text: None,
    }
}

async fn create(ws: &Workspace, session: &Session, name: &str, place_type: PlaceType) -> String {
    create_place(
        ws,
        session,
        NewPlace {
            human_id: None,
            place_type,
            name: Some(name.to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place")
}

/// Directly asserts a dated `AssertEnclosedBy` through the public `Store` (see module docs).
async fn assert_dated_enclosure(
    ws: &Workspace,
    session: &Session,
    human_id: &str,
    enclosing_human_id: &str,
    date: Option<GenealogicalDate>,
) {
    let store = ws.store();
    let place_id = store
        .find_place(human_id)
        .await
        .expect("find")
        .expect("place exists")
        .place_id()
        .expect("place id");
    let enclosing_id = store
        .find_place(enclosing_human_id)
        .await
        .expect("find")
        .expect("place exists")
        .place_id()
        .expect("place id");
    let envelope = PlaceCommandEnvelope {
        meta: session.new_meta(Provenance::default(), Vec::new()),
        command: PlaceCommand::AssertEnclosedBy {
            place_id,
            enclosed_by: PlaceRef {
                place_id: enclosing_id,
                date,
            },
        },
    };
    store
        .execute_place(&place_id.to_string(), envelope)
        .await
        .expect("enclosure");
}

#[tokio::test]
async fn hierarchy_walk_resolves_the_full_transitive_chain_and_title() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let farm = create(&ws, &session, "Nordgarden", PlaceType::Farm).await;
    let parish = create(&ws, &session, "Vågå", PlaceType::Parish).await;
    let county = create(&ws, &session, "Innlandet", PlaceType::County).await;
    let country = create(&ws, &session, "Norway", PlaceType::Country).await;

    assert_place_enclosed_by(&ws, &session, &farm, &parish, MutationMeta::default())
        .await
        .expect("enclose farm");
    assert_place_enclosed_by(&ws, &session, &parish, &county, MutationMeta::default())
        .await
        .expect("enclose parish");
    assert_place_enclosed_by(&ws, &session, &county, &country, MutationMeta::default())
        .await
        .expect("enclose county");

    let summary = show_place(&ws, &farm).await.expect("show").expect("found");
    let chain: Vec<_> = summary.enclosing.iter().map(|e| e.human_id.clone()).collect();
    assert_eq!(chain, vec![parish, county, country]);
    assert_eq!(summary.generated_title, "Nordgarden, Vågå, Innlandet, Norway");
}

#[tokio::test]
async fn a_cycle_in_the_enclosure_chain_terminates_the_walk() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let a = create(&ws, &session, "A", PlaceType::Farm).await;
    let b = create(&ws, &session, "B", PlaceType::Parish).await;
    assert_place_enclosed_by(&ws, &session, &a, &b, MutationMeta::default())
        .await
        .expect("enclose a in b");
    assert_place_enclosed_by(&ws, &session, &b, &a, MutationMeta::default())
        .await
        .expect("enclose b in a");

    // The walk must terminate (the cycle guard), not hang or panic.
    let summary = show_place(&ws, &a).await.expect("show").expect("found");
    assert_eq!(
        summary.enclosing.len(),
        1,
        "the cycle stops the walk after the first repeat"
    );
}

#[tokio::test]
async fn date_aware_resolution_picks_the_enclosure_in_effect_at_the_query_year() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let farm = create(&ws, &session, "Nedre Vågå", PlaceType::Farm).await;
    let old_parish = create(&ws, &session, "Vågå prestegjeld", PlaceType::Parish).await;
    let new_parish = create(&ws, &session, "Lom prestegjeld", PlaceType::Parish).await;

    // Two dated jurisdictions: valid from 1801, then reassigned from 1900 — no undated fallback.
    assert_dated_enclosure(&ws, &session, &farm, &old_parish, Some(year(1801))).await;
    assert_dated_enclosure(&ws, &session, &farm, &new_parish, Some(year(1900))).await;

    let as_of_1850 = show_place_as_of(&ws, &farm, year(1850))
        .await
        .expect("show")
        .expect("found");
    assert_eq!(as_of_1850.enclosing[0].human_id, old_parish);

    let as_of_1950 = show_place_as_of(&ws, &farm, year(1950))
        .await
        .expect("show")
        .expect("found");
    assert_eq!(as_of_1950.enclosing[0].human_id, new_parish);

    // No date context: the primary (first-asserted) link — the 1801 assignment, asserted first.
    let primary = show_place(&ws, &farm).await.expect("show").expect("found");
    assert_eq!(primary.enclosing[0].human_id, old_parish);
}

#[tokio::test]
async fn a_merge_is_reachable_from_both_the_survivor_and_each_merged_place() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let aker = create(&ws, &session, "Aker", PlaceType::Municipality).await;
    let kristiania = create(&ws, &session, "Kristiania", PlaceType::Municipality).await;
    let oslo = create(&ws, &session, "Oslo", PlaceType::Municipality).await;

    assert_place_succession(
        &ws,
        &session,
        &aker,
        PlaceSuccessionInput {
            from_human_ids: vec![aker.clone(), kristiania.clone()],
            to_human_ids: vec![oslo.clone()],
            kind: SuccessionKind::Merged,
            date: Some(year(1948)),
        },
        MutationMeta::default(),
    )
    .await
    .expect("merge");

    let oslo_summary = show_place(&ws, &oslo).await.expect("show").expect("found");
    let mut predecessors: Vec<_> = oslo_summary.predecessors.iter().map(|p| p.human_id.clone()).collect();
    predecessors.sort();
    let mut expected = vec![aker.clone(), kristiania.clone()];
    expected.sort();
    assert_eq!(predecessors, expected);
    assert_eq!(oslo_summary.predecessors[0].kind, SuccessionKind::Merged);

    let aker_summary = show_place(&ws, &aker).await.expect("show").expect("found");
    assert_eq!(aker_summary.successors[0].human_id, oslo);

    // Kristiania is not the anchor (Aker is), but is still reachable — the symmetric projection.
    let kristiania_summary = show_place(&ws, &kristiania).await.expect("show").expect("found");
    assert_eq!(kristiania_summary.successors[0].human_id, oslo);
}

#[tokio::test]
async fn a_split_is_reachable_from_every_resulting_place() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let county = create(&ws, &session, "Old County", PlaceType::County).await;
    let north = create(&ws, &session, "North County", PlaceType::County).await;
    let south = create(&ws, &session, "South County", PlaceType::County).await;

    assert_place_succession(
        &ws,
        &session,
        &county,
        PlaceSuccessionInput {
            from_human_ids: vec![county.clone()],
            to_human_ids: vec![north.clone(), south.clone()],
            kind: SuccessionKind::Split,
            date: None,
        },
        MutationMeta::default(),
    )
    .await
    .expect("split");

    let county_summary = show_place(&ws, &county).await.expect("show").expect("found");
    let mut successors: Vec<_> = county_summary.successors.iter().map(|s| s.human_id.clone()).collect();
    successors.sort();
    let mut expected = vec![north.clone(), south.clone()];
    expected.sort();
    assert_eq!(successors, expected);

    let north_summary = show_place(&ws, &north).await.expect("show").expect("found");
    assert_eq!(north_summary.predecessors[0].human_id, county);
}

#[tokio::test]
async fn an_unknown_referenced_place_is_a_domain_error() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create(&ws, &session, "Somewhere", PlaceType::Village).await;

    let err = assert_place_succession(
        &ws,
        &session,
        &place,
        PlaceSuccessionInput {
            from_human_ids: vec![place.clone()],
            to_human_ids: vec!["P9999".to_owned()],
            kind: SuccessionKind::Renamed,
            date: None,
        },
        MutationMeta::default(),
    )
    .await;
    assert!(matches!(err, Err(AppError::PlaceNotFound(_))));
}

#[tokio::test]
async fn an_anchor_not_among_the_from_places_is_a_domain_error() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let a = create(&ws, &session, "A", PlaceType::Village).await;
    let b = create(&ws, &session, "B", PlaceType::Village).await;
    let c = create(&ws, &session, "C", PlaceType::Village).await;

    let err = assert_place_succession(
        &ws,
        &session,
        &a,
        PlaceSuccessionInput {
            from_human_ids: vec![b.clone()],
            to_human_ids: vec![c.clone()],
            kind: SuccessionKind::Merged,
            date: None,
        },
        MutationMeta::default(),
    )
    .await;
    assert!(matches!(
        err,
        Err(AppError::PlaceDomain(PlaceError::SuccessionAnchorMismatch(_)))
    ));
}

#[tokio::test]
async fn empty_succession_endpoints_are_a_domain_error() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let a = create(&ws, &session, "A", PlaceType::Village).await;

    let err = assert_place_succession(
        &ws,
        &session,
        &a,
        PlaceSuccessionInput {
            from_human_ids: vec![a.clone()],
            to_human_ids: Vec::new(),
            kind: SuccessionKind::Renamed,
            date: None,
        },
        MutationMeta::default(),
    )
    .await;
    assert!(matches!(
        err,
        Err(AppError::PlaceDomain(PlaceError::EmptySuccessionEndpoints))
    ));
}
