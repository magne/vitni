//! SSR assertions for the read-only Place Map tab (Phase 6 map MVP): with a coordinate the tab
//! renders the map container (a stable hook carrying lat/lon) and the OpenStreetMap attribution;
//! without one it renders the empty state. The coordinate assertion id never leaks into the markup.

use dioxus::prelude::*;
use genealogy_ui::{ConfidenceLevel, Localizer, MapPointVm, PlaceDetail};
use genealogy_ui_dioxus::screens::place_map;

/// A located place: a city with High-confidence coordinates and a backing coordinate citation whose
/// assertion id must never surface on the Map tab.
fn located() -> PlaceDetail {
    PlaceDetail {
        coordinates: Some("40.7128,-74.006".to_owned()),
        map_point: Some(MapPointVm {
            lat: 40.7128,
            lon: -74.006,
            label: "New York".to_owned(),
        }),
        coordinates_confidence: Some(ConfidenceLevel::High),
        coordinates_confidence_label: Some("High".to_owned()),
        ..bare()
    }
}

/// A place with no coordinate — the empty-state case.
fn unlocated() -> PlaceDetail {
    bare()
}

/// A minimal place detail with everything empty (the fields the Map tab does not read).
fn bare() -> PlaceDetail {
    PlaceDetail {
        human_id: "P0090".to_owned(),
        id: "place-id".to_owned(),
        title: "Nordland".to_owned(),
        place_type: None,
        type_label: None,
        coordinates: None,
        map_point: None,
        coordinates_confidence: None,
        coordinates_confidence_label: None,
        coordinate_citations: Vec::new(),
        code: None,
        code_confidence: None,
        code_confidence_label: None,
        code_citations: Vec::new(),
        names: Vec::new(),
        hierarchy: Vec::new(),
        predecessors: Vec::new(),
        successors: Vec::new(),
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn located_view() -> Element {
    place_map(&loc(), &located())
}

/// A located place whose coordinate is backed by a citation carrying an assertion id — used to prove
/// the id never surfaces on the Map tab.
fn located_with_citation() -> PlaceDetail {
    PlaceDetail {
        coordinate_citations: vec![genealogy_ui::CitationRefVm {
            human_id: "C0009".to_owned(),
            source: Some("GeoNames".to_owned()),
            source_id: Some("S0007".to_owned()),
            page: None,
            backs_count: 0,
            confidence: Some(ConfidenceLevel::High),
            confidence_label: Some("High".to_owned()),
            evidence_axes: Vec::new(),
            asserted_by: None,
            assertion_id: Some("0190-coordinate-assert-1".to_owned()),
        }],
        ..located()
    }
}

fn located_with_citation_view() -> Element {
    place_map(&loc(), &located_with_citation())
}

fn unlocated_view() -> Element {
    place_map(&loc(), &unlocated())
}

#[test]
fn a_located_place_renders_the_map_container_with_a_lat_lon_hook() {
    let html = render(located_view);
    assert!(
        html.contains(r#"id="place-map""#),
        "the Leaflet mount container is present:\n{html}"
    );
    assert!(
        html.contains(r#"data-lat="40.7128""#) && html.contains(r#"data-lon="-74.006""#),
        "the container carries the parsed lat/lon as a stable hook:\n{html}"
    );
    assert!(
        html.contains(r#"role="img""#),
        "the map surface exposes an image role for assistive tech:\n{html}"
    );
}

#[test]
fn a_located_place_shows_the_openstreetmap_attribution() {
    let html = render(located_view);
    assert!(
        html.contains("© OpenStreetMap contributors"),
        "the required OSM attribution is shown verbatim:\n{html}"
    );
}

#[test]
fn a_place_without_a_coordinate_shows_the_empty_state() {
    let html = render(unlocated_view);
    assert!(
        html.contains("No coordinates yet"),
        "the empty-state heading is shown:\n{html}"
    );
    assert!(
        html.contains("Add a latitude"),
        "the empty-state helper text is shown:\n{html}"
    );
    assert!(
        !html.contains(r#"id="place-map""#),
        "no map container is mounted without a coordinate:\n{html}"
    );
}

#[test]
fn the_coordinate_assertion_id_never_leaks_into_the_map_markup() {
    let html = render(located_with_citation_view);
    assert!(
        !html.contains("0190-coordinate-assert-1"),
        "the coordinate assertion id must never render on the Map tab:\n{html}"
    );
}
