//! `show_geography`/`show_place` integration tests: a place with only a scalar `AssertCoordinates`
//! point (no dedicated ADR 0024 geometry assertion — the common case for a GEDCOM-imported or
//! manually geocoded place) must still show up as a marker, not silently vanish from the map. A
//! place's own `show_place` summary also carries the events that occurred there (the Place Map tab's
//! event pins), scoped to just that one place.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use genealogy_app::{
    AppDefaults, EventType, GeoCoordinates, Microdegrees, MutationMeta, NewEvent, NewPlace, OperatorConfig,
    PlaceGeometry, PlaceType, Provenance, Session, Workspace, WorkspaceDefaults, assert_place_coordinates,
    assert_place_geometry, create_event, create_place, link_place, show_geography, show_place,
};
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
use std::str::FromStr;
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

fn point(lat: &str, lon: &str) -> GeoCoordinates {
    GeoCoordinates {
        latitude: Microdegrees::from_str(lat).expect("lat"),
        longitude: Microdegrees::from_str(lon).expect("lon"),
    }
}

#[tokio::test]
async fn a_place_with_only_scalar_coordinates_still_becomes_a_marker() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::Farm,
            name: Some("Nordgarden".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");

    assert_place_coordinates(&ws, &session, &human_id, point("61.5", "9.0"), MutationMeta::default())
        .await
        .expect("assert coordinates");

    let summary = show_geography(&ws, None).await.expect("show_geography");
    let marker = summary
        .markers
        .iter()
        .find(|marker| marker.human_id == human_id)
        .expect("the place with only scalar coordinates is still a marker");
    assert_eq!(marker.geometry, PlaceGeometry::Point(point("61.5", "9.0")));
}

#[tokio::test]
async fn an_explicit_geometry_assertion_wins_over_the_scalar_coordinate_fallback() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::Parish,
            name: Some("Vågå".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");

    assert_place_coordinates(&ws, &session, &human_id, point("61.0", "8.0"), MutationMeta::default())
        .await
        .expect("assert coordinates");
    assert_place_geometry(
        &ws,
        &session,
        &human_id,
        PlaceGeometry::Point(point("61.9", "8.8")),
        None,
        MutationMeta::default(),
    )
    .await
    .expect("assert geometry");

    let summary = show_geography(&ws, None).await.expect("show_geography");
    let marker = summary
        .markers
        .iter()
        .find(|marker| marker.human_id == human_id)
        .expect("a marker");
    assert_eq!(
        marker.geometry,
        PlaceGeometry::Point(point("61.9", "8.8")),
        "the drawn geometry, not the scalar coordinate, is shown"
    );
}

#[tokio::test]
async fn a_place_with_neither_geometry_nor_coordinates_is_not_a_marker() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::County,
            name: Some("Nordland".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");

    let summary = show_geography(&ws, None).await.expect("show_geography");
    assert!(
        !summary.markers.iter().any(|marker| marker.human_id == human_id),
        "an unlocated place is not plotted"
    );
}

#[tokio::test]
async fn show_place_carries_only_the_events_that_occurred_at_that_place() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let here = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::Farm,
            name: Some("Nordgarden".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");
    assert_place_coordinates(&ws, &session, &here, point("61.5", "9.0"), MutationMeta::default())
        .await
        .expect("assert coordinates");

    let elsewhere = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::Farm,
            name: Some("Sørgarden".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");
    assert_place_coordinates(&ws, &session, &elsewhere, point("60.0", "7.0"), MutationMeta::default())
        .await
        .expect("assert coordinates");

    let birth = create_event(
        &ws,
        &session,
        NewEvent {
            human_id: None,
            event_type: EventType::Birth,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("event");
    link_place(&ws, &session, &birth, &here, MutationMeta::default())
        .await
        .expect("link place");

    let marriage = create_event(
        &ws,
        &session,
        NewEvent {
            human_id: None,
            event_type: EventType::Marriage,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("event");
    link_place(&ws, &session, &marriage, &elsewhere, MutationMeta::default())
        .await
        .expect("link place");

    let summary = show_place(&ws, &here).await.expect("show_place").expect("found");
    assert_eq!(summary.events.len(), 1, "only the event at this place is pinned");
    assert_eq!(summary.events[0].human_id, birth);
    assert_eq!(
        summary.events[0].point,
        point("61.5", "9.0"),
        "pinned at this place's own point"
    );
}

#[tokio::test]
async fn show_place_has_no_events_when_none_occurred_there() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::Farm,
            name: Some("Nordgarden".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");
    assert_place_coordinates(&ws, &session, &human_id, point("61.5", "9.0"), MutationMeta::default())
        .await
        .expect("assert coordinates");

    let summary = show_place(&ws, &human_id).await.expect("show_place").expect("found");
    assert!(summary.events.is_empty());
}
