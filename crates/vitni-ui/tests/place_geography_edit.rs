//! Geography map-edit integration test (ADR 0025 §2): a map edit dispatches `PlaceEdit::AssertGeometry`
//! through the **same** `dispatch_place_edit` path as a typed-field edit, so it produces the identical
//! audited `GeometryAsserted` event with the operator's provenance — there is no separate "map write"
//! path. Runs against a real on-disk workspace over `vitni-app`'s public surface only (mirrors
//! `dispatch_provenance.rs`).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::str::FromStr;
use uuid::Uuid;
use vitni_app::{
    Agent, AgentId, AgentKind, AppDefaults, ChangeLogEntry, Confidence, GeoCoordinates, Microdegrees, NewPlace,
    OperatorConfig, PlaceGeometry, PlaceType, Provenance, Session, Workspace, WorkspaceDefaults, change_log_for_place,
    create_place, show_place, show_place_as_of, year_only_date,
};
use vitni_ui::{ConfidenceLevel, PlaceEdit, ProvenanceDraft, dispatch_place_edit};

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

async fn setup() -> (Workspace, Session, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, session(), dir)
}

fn point(lat: &str, lon: &str) -> GeoCoordinates {
    GeoCoordinates {
        latitude: Microdegrees::from_str(lat).expect("lat"),
        longitude: Microdegrees::from_str(lon).expect("lon"),
    }
}

/// The entry that carries the draft's (unique) rationale.
fn entry_with_rationale<'a>(log: &'a [ChangeLogEntry], rationale: &str) -> &'a ChangeLogEntry {
    log.iter()
        .find(|entry| entry.rationale.as_deref() == Some(rationale))
        .expect("the dispatched mutation is logged with its rationale")
}

#[tokio::test]
async fn a_map_click_to_drop_a_point_lands_a_geometry_asserted_event_with_provenance() {
    let (ws, session, _dir) = setup().await;
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

    let draft = ProvenanceDraft {
        rationale: "  Traced from the 1886 amt map  ".to_owned(),
        confidence: Some(ConfidenceLevel::Normal),
        ..ProvenanceDraft::default()
    };

    dispatch_place_edit(
        &ws,
        &session,
        &PlaceEdit::AssertGeometry {
            human_id: human_id.clone(),
            geometry: PlaceGeometry::Point(point("61.5", "9.0")),
            year: None,
        },
        &draft,
    )
    .await
    .expect("dispatch AssertGeometry");

    let log = change_log_for_place(&ws, &human_id).await.expect("log");
    let entry = entry_with_rationale(&log, "Traced from the 1886 amt map");
    assert_eq!(
        entry.event_type, "GeometryAsserted",
        "the map click carries the same event a typed edit would"
    );
    assert_eq!(entry.confidence, Some(Confidence::Normal), "confidence threads through");

    let summary = show_place(&ws, &human_id).await.expect("show").expect("place");
    let resolved = summary.resolved_geometry.expect("a resolved geometry");
    assert_eq!(resolved.geometry, PlaceGeometry::Point(point("61.5", "9.0")));
}

/// A map edit made while the time slider is at a chosen year asserts a **dated** geometry — the same
/// ADR 0026 §1 resolution rule then picks it (or not) depending on the query year, exactly like a
/// typed dated assertion would.
#[tokio::test]
async fn a_dated_map_edit_resolves_only_from_its_year_onward() {
    let (ws, session, _dir) = setup().await;
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

    dispatch_place_edit(
        &ws,
        &session,
        &PlaceEdit::AssertGeometry {
            human_id: human_id.clone(),
            geometry: PlaceGeometry::Point(point("61.9", "8.8")),
            year: Some(1900),
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("dispatch dated AssertGeometry");

    let before = show_place_as_of(&ws, &human_id, year_only_date(1850))
        .await
        .expect("show")
        .expect("place");
    assert!(before.resolved_geometry.is_none(), "1850 predates the dated assertion");

    let after = show_place_as_of(&ws, &human_id, year_only_date(1950))
        .await
        .expect("show")
        .expect("place");
    assert_eq!(
        after.resolved_geometry.expect("resolved").geometry,
        PlaceGeometry::Point(point("61.9", "8.8"))
    );
}
