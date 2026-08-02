//! SSR assertions for the Place Map tab's pure pieces (Phase 9 map editor, ADR 0024/0025/0026): the
//! "Geometry over time" table (rows, empty state, retract) and its row-scoped labels. The interactive
//! `PlaceMapEditor` component itself (draw tools, the live `MapLibre` canvas, the save-geometry card)
//! needs `AppCtx`/`Services` and cannot be exercised by an SSR test — see the PR report for what needs
//! manual GUI verification, mirroring the Geography tool's own test split (`geography_screen.rs`).

use dioxus::prelude::*;
use genealogy_ui::{ConfidenceLevel, Localizer, MarkerShapeVm, PlaceGeometryVm};
use genealogy_ui_dioxus::screens::{effective_date_choice, place_geometry_table};

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

fn onedit() -> Callback<PlaceGeometryVm> {
    use_callback(|_: PlaceGeometryVm| {})
}

#[component]
fn TableWithBothKinds() -> Element {
    place_geometry_table(&loc(), &[point_geometry(), polygon_geometry()], onretract(), onedit())
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

#[test]
fn each_row_carries_an_edit_button_to_load_it_back_into_the_draw_state() {
    let html = render(TableWithBothKinds);
    let edit_count = html.matches(">Edit<").count();
    assert_eq!(edit_count, 2, "both rows carry an Edit button:\n{html}");
    assert!(
        html.contains("Edit vertices on the map"),
        "the Edit button's title explains what it does:\n{html}"
    );
}

#[component]
fn EmptyTable() -> Element {
    place_geometry_table(&loc(), &[], onretract(), onedit())
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

#[component]
fn EffectiveDateUndated() -> Element {
    effective_date_choice(&loc(), 1900, false, |_: String| {})
}

#[component]
fn EffectiveDateDated() -> Element {
    effective_date_choice(&loc(), 1900, true, |_: String| {})
}

#[test]
fn the_save_form_offers_both_effective_date_choices_with_undated_preselected() {
    let html = render(EffectiveDateUndated);
    assert!(
        html.contains(r#"role="radiogroup""#) && html.matches(r#"role="radio""#).count() == 2,
        "the dated/undated choice is a two-option radiogroup:\n{html}"
    );
    assert!(
        html.contains("Undated — applies to every year"),
        "the undated option says it resolves at every year:\n{html}"
    );
    assert!(
        html.contains("As of 1900"),
        "the dated option is labelled with the slider year:\n{html}"
    );
    assert!(html.contains("Effective date"), "the group is labelled:\n{html}");
    let undated_first = html
        .find("Undated")
        .expect("the undated option renders before the dated one");
    let checked = html
        .find(r#"aria-checked="true""#)
        .expect("exactly one option is checked");
    assert!(
        checked < undated_first,
        "the undated option is the preselected default — a saved shape must resolve at every \
         year unless the operator opts into dating it:\n{html}"
    );
}

#[test]
fn picking_the_dated_choice_moves_the_checked_state_and_the_tab_stop() {
    let html = render(EffectiveDateDated);
    let dated = html.find("As of 1900").expect("the dated option renders");
    let checked = html
        .find(r#"aria-checked="true""#)
        .expect("exactly one option is checked");
    assert!(checked < dated, "the dated option is the checked one:\n{html}");
    assert_eq!(
        html.matches(r#"aria-checked="true""#).count(),
        1,
        "a single-choice group checks exactly one option:\n{html}"
    );
    assert_eq!(
        html.matches(r#"tabindex="0""#).count(),
        1,
        "the roving tab stop follows the selection:\n{html}"
    );
}
