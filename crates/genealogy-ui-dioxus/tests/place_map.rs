//! SSR assertions for the Place Map tab's pure pieces (Phase 9 map editor, ADR 0024/0025/0026): the
//! "Geometry over time" table (rows, empty state, retract) and its row-scoped labels. The interactive
//! `PlaceMapEditor` component itself (draw tools, the live `MapLibre` canvas, the save-geometry card)
//! needs `AppCtx`/`Services` and cannot be exercised by an SSR test — see the PR report for what needs
//! manual GUI verification, mirroring the Geography tool's own test split (`geography_screen.rs`).

use dioxus::prelude::*;
use genealogy_ui::{ConfidenceLevel, Localizer, MarkerShapeVm, PlaceGeometryVm};
use genealogy_ui_dioxus::screens::place_geometry_table;

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn point_geometry() -> PlaceGeometryVm {
    PlaceGeometryVm {
        shape: MarkerShapeVm::Point {
            lat: 40.7128,
            lon: -74.006,
        },
        kind_label: "Point".to_owned(),
        year: None,
        date: None,
        confidence: Some(ConfidenceLevel::High),
        confidence_label: "High".to_owned(),
        source_count: 1,
        assertion_id: "0190-geometry-assert-1".to_owned(),
    }
}

fn polygon_geometry() -> PlaceGeometryVm {
    PlaceGeometryVm {
        shape: MarkerShapeVm::Polygon {
            exterior: vec![(40.70, -74.02), (40.72, -74.02), (40.72, -74.00)],
            holes: Vec::new(),
        },
        kind_label: "Polygon".to_owned(),
        year: Some(1898),
        date: Some("from 1898".to_owned()),
        confidence: Some(ConfidenceLevel::Normal),
        confidence_label: "Normal".to_owned(),
        source_count: 0,
        assertion_id: "0190-geometry-assert-2".to_owned(),
    }
}

fn onretract() -> Callback<(String, String, bool)> {
    use_callback(|_: (String, String, bool)| {})
}

#[component]
fn TableWithBothKinds() -> Element {
    place_geometry_table(&loc(), &[point_geometry(), polygon_geometry()], onretract())
}

#[test]
fn a_point_and_a_polygon_row_each_show_their_kind_date_and_confidence() {
    let html = render(TableWithBothKinds);
    assert!(
        html.contains("Point") && html.contains("Polygon"),
        "both kind chips show:\n{html}"
    );
    assert!(
        html.contains("40.7128"),
        "the point's decimal-degree detail shows:\n{html}"
    );
    assert!(
        html.contains("3 vertices"),
        "the polygon's pluralized vertex count shows:\n{html}"
    );
    assert!(
        html.contains("from 1898"),
        "the polygon's dated-effective caption shows:\n{html}"
    );
    assert!(
        html.contains(r#"data-level="high""#) && html.contains(r#"data-level="normal""#),
        "both confidence badges render:\n{html}"
    );
}

#[test]
fn an_undated_row_shows_an_em_dash_for_its_date() {
    let html = render(TableWithBothKinds);
    assert!(
        html.contains("—"),
        "the undated point row falls back to an em dash:\n{html}"
    );
}

#[test]
fn the_assertion_id_never_leaks_into_the_table_markup() {
    let html = render(TableWithBothKinds);
    for assertion_id in ["0190-geometry-assert-1", "0190-geometry-assert-2"] {
        assert!(
            !html.contains(assertion_id),
            "assertion id {assertion_id:?} must never be rendered:\n{html}"
        );
    }
}

#[test]
fn each_row_carries_a_retract_button() {
    let html = render(TableWithBothKinds);
    let retract_count = html.matches(">Retract<").count();
    assert_eq!(retract_count, 2, "both rows carry a Retract button:\n{html}");
}

#[component]
fn EmptyTable() -> Element {
    place_geometry_table(&loc(), &[], onretract())
}

#[test]
fn no_geometries_yet_shows_the_empty_state() {
    let html = render(EmptyTable);
    assert!(
        html.contains("No geometry yet"),
        "the empty-state heading is shown:\n{html}"
    );
    assert!(
        html.contains("Point") && html.contains("Polygon"),
        "the empty-state help points at the draw tools:\n{html}"
    );
    assert!(!html.contains(">Retract<"), "no rows, so no retract buttons:\n{html}");
}
