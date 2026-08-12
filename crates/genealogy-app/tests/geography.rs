//! `show_geography`/`show_place` integration tests: a place with only a scalar `AssertCoordinates`
//! point (no dedicated ADR 0024 geometry assertion — the common case for a GEDCOM-imported or
//! manually geocoded place) must still show up as a marker, not silently vanish from the map. A
//! place with geometry that resolves to nothing as of the feed's year is *reported* as unplotted
//! rather than dropped in silence. A place's own `show_place` summary also carries the events that
//! occurred there (the Place Map tab's event pins), scoped to just that one place.
//!
//! A marker's label must track the same as-of resolution as its geometry (ADR 0026 §1, issue
//! #232): a place renamed over time should read the name in effect for the slider's year, not
//! always the first-asserted one. The app-layer `create_place` use-case has no way to assert a
//! *dated* name (`place::place_name` hardcodes `date: None`, mirroring the pre-existing gap
//! `place_temporal.rs`'s module docs describe for dated enclosures), so `assert_dated_name` below
//! executes a raw `PlaceCommand::AssertName` through the public `Store` directly — the same
//! workaround pattern, for the same reason.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use genealogy_app::{
    AppDefaults, EventType, GeoCoordinates, Microdegrees, MutationMeta, NewEvent, NewPlace, OperatorConfig,
    PlaceGeometry, PlaceType, Provenance, Session, UnplottedReason, Workspace, WorkspaceDefaults,
    assert_place_coordinates, assert_place_geometry, create_event, create_place, link_place, show_geography,
    show_place, year_only_date,
};
use genealogy_core::date::GenealogicalDate;
use genealogy_core::ids::AgentId;
use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use genealogy_core::place_name::PlaceName;
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

/// Directly asserts a dated name through the public `Store` (see module docs): the app-layer
/// `create_place`/`place_name` path has no way to attach a date to a name.
async fn assert_dated_name(ws: &Workspace, session: &Session, human_id: &str, text: &str, date: GenealogicalDate) {
    let store = ws.store();
    let place_id = store
        .find_place(human_id)
        .await
        .expect("find")
        .expect("place exists")
        .place_id()
        .expect("place id");
    let envelope = PlaceCommandEnvelope {
        meta: session.new_meta(Provenance::default(), Vec::new()),
        command: PlaceCommand::AssertName {
            place_id,
            name: PlaceName {
                text: text.to_owned(),
                language: None,
                date: Some(date),
            },
        },
    };
    store
        .execute_place(&place_id.to_string(), envelope)
        .await
        .expect("assert name");
}

/// Creates a place with a single geometry assertion, dated `year` when given.
async fn place_with_geometry(ws: &Workspace, session: &Session, name: &str, year: Option<i32>) -> String {
    let human_id = create_place(
        ws,
        session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::Parish,
            name: Some(name.to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");
    assert_place_geometry(
        ws,
        session,
        &human_id,
        PlaceGeometry::Point(point("61.9", "8.8")),
        year.map(year_only_date),
        MutationMeta::default(),
    )
    .await
    .expect("assert geometry");
    human_id
}

#[tokio::test]
async fn a_place_whose_only_geometry_postdates_the_feed_is_reported_unplotted() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = place_with_geometry(&ws, &session, "Vågå", Some(1900)).await;

    let summary = show_geography(&ws, Some(1850)).await.expect("show_geography");
    assert!(
        !summary.markers.iter().any(|marker| marker.human_id == human_id),
        "a geometry dated 1900 does not resolve as of 1850"
    );
    let unplotted = summary
        .unplotted
        .iter()
        .find(|place| place.human_id == human_id)
        .expect("the place is reported, not silently dropped");
    assert_eq!(unplotted.name, "Vågå");
    assert_eq!(unplotted.reason, UnplottedReason::DatedLater);
}

#[tokio::test]
async fn the_same_place_is_a_marker_and_not_unplotted_at_its_geometrys_own_year() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = place_with_geometry(&ws, &session, "Vågå", Some(1900)).await;

    let summary = show_geography(&ws, Some(1900)).await.expect("show_geography");
    assert!(summary.markers.iter().any(|marker| marker.human_id == human_id));
    assert!(
        summary.unplotted.is_empty(),
        "a resolved place is not also reported unplotted"
    );
}

#[tokio::test]
async fn an_undated_geometry_plots_at_every_year() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = place_with_geometry(&ws, &session, "Vågå", None).await;

    for year in [1600, 1900, 2000] {
        let summary = show_geography(&ws, Some(year)).await.expect("show_geography");
        assert!(
            summary.markers.iter().any(|marker| marker.human_id == human_id),
            "an undated/primary geometry resolves as of {year}"
        );
        assert!(summary.unplotted.is_empty(), "nothing is unplotted as of {year}");
    }
}

#[tokio::test]
async fn a_scalar_coordinate_place_is_a_marker_at_every_year_and_never_unplotted() {
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

    for year in [1600, 2000] {
        let summary = show_geography(&ws, Some(year)).await.expect("show_geography");
        assert!(
            summary.markers.iter().any(|marker| marker.human_id == human_id),
            "the GEDCOM-import case still plots as of {year}"
        );
        assert!(summary.unplotted.is_empty(), "nothing is unplotted as of {year}");
    }
}

#[tokio::test]
async fn a_place_with_no_geometry_and_no_coordinate_is_reported_unplotted_with_no_geometry() {
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

    let summary = show_geography(&ws, Some(1850)).await.expect("show_geography");
    assert!(!summary.markers.iter().any(|marker| marker.human_id == human_id));
    let unplotted = summary
        .unplotted
        .iter()
        .find(|place| place.human_id == human_id)
        .expect("a place nobody ever located is reported unplotted (#256)");
    assert_eq!(unplotted.reason, UnplottedReason::NoGeometry);
    assert_eq!(unplotted.place_type, Some(PlaceType::County));
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

/// Creates a place named "Oslo", plotted at a scalar coordinate, then renamed "Kristiania" from
/// 1877 and back to "Oslo" from 1925 — the historical rename this issue's bug affects.
async fn renamed_place(ws: &Workspace, session: &Session) -> String {
    let human_id = create_place(
        ws,
        session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::Municipality,
            name: Some("Oslo".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");
    assert_place_coordinates(ws, session, &human_id, point("59.9", "10.7"), MutationMeta::default())
        .await
        .expect("assert coordinates");
    assert_dated_name(ws, session, &human_id, "Kristiania", year_only_date(1877)).await;
    assert_dated_name(ws, session, &human_id, "Oslo", year_only_date(1925)).await;
    human_id
}

#[tokio::test]
async fn a_marker_is_labelled_with_the_name_in_effect_that_year() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = renamed_place(&ws, &session).await;

    let summary = show_geography(&ws, Some(1900)).await.expect("show_geography");
    let marker = summary
        .markers
        .iter()
        .find(|marker| marker.human_id == human_id)
        .expect("a marker");
    assert_eq!(marker.name, "Kristiania");
}

#[tokio::test]
async fn a_marker_labelled_after_a_rename_uses_the_later_name() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = renamed_place(&ws, &session).await;

    let summary = show_geography(&ws, Some(1950)).await.expect("show_geography");
    let marker = summary
        .markers
        .iter()
        .find(|marker| marker.human_id == human_id)
        .expect("a marker");
    assert_eq!(marker.name, "Oslo");
}

#[tokio::test]
async fn a_marker_before_every_dated_name_falls_back_to_the_undated_one() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = renamed_place(&ws, &session).await;

    let summary = show_geography(&ws, Some(1600)).await.expect("show_geography");
    let marker = summary
        .markers
        .iter()
        .find(|marker| marker.human_id == human_id)
        .expect("a marker");
    assert_eq!(
        marker.name, "Oslo",
        "before 1877, the undated (primary) name is the fallback"
    );
}

#[tokio::test]
async fn a_marker_with_no_year_keeps_the_first_asserted_name() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = renamed_place(&ws, &session).await;

    let summary = show_geography(&ws, None).await.expect("show_geography");
    let marker = summary
        .markers
        .iter()
        .find(|marker| marker.human_id == human_id)
        .expect("a marker");
    assert_eq!(
        marker.name, "Oslo",
        "without a slider year, the current/primary resolution is unchanged"
    );
}

#[tokio::test]
async fn an_unplotted_place_is_labelled_with_the_name_in_effect_that_year() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let human_id = place_with_geometry(&ws, &session, "Oslo", Some(1900)).await;
    assert_dated_name(&ws, &session, &human_id, "Kristiania", year_only_date(1877)).await;
    assert_dated_name(&ws, &session, &human_id, "Oslo", year_only_date(1925)).await;

    let summary = show_geography(&ws, Some(1880)).await.expect("show_geography");
    assert!(
        !summary.markers.iter().any(|marker| marker.human_id == human_id),
        "the geometry is dated 1900 and does not resolve as of 1880"
    );
    let unplotted = summary
        .unplotted
        .iter()
        .find(|place| place.human_id == human_id)
        .expect("the place is reported unplotted");
    assert_eq!(unplotted.name, "Kristiania");
}
