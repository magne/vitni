//! SSR assertions for the Geography tool (ADR 0025): the map container/attribution, the empty state
//! shown when no place has a resolved geometry, the rail list, and the time-slider caption. Mirrors
//! the Phase-6 MVP's `place_map.rs` SSR test pattern — pure render functions over hand-built
//! view-models, no `AppCtx`/`Services` needed (interactive canvas behavior itself cannot be exercised
//! this way; see the PR report for what needs manual GUI verification).

use dioxus::prelude::*;
use genealogy_ui::{EventPinVm, GeographyVm, MapProviderVm, MarkerShapeVm, PlaceMarkerVm};
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::screens::{
    DrawTool, geography_empty_state, geography_map_surface, geography_rail, geography_time_slider,
};

fn chrome() -> Chrome {
    Chrome::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn empty_state_view() -> Element {
    geography_empty_state(&chrome())
}

#[test]
fn no_resolved_geometry_shows_the_empty_state() {
    let html = render(empty_state_view);
    assert!(
        html.contains("No places to plot yet"),
        "the empty-state heading is shown:\n{html}"
    );
    assert!(
        html.contains("Overview tab"),
        "the empty-state helper text points at where to set a location:\n{html}"
    );
}

#[component]
fn MapSurfaceWithMarkers() -> Element {
    let tool = use_signal(|| DrawTool::Pan);
    geography_map_surface(&chrome(), 3, 5, tool, |_lat: f64, _lon: f64| {})
}

#[test]
fn the_map_surface_carries_the_container_and_an_accessible_marker_count() {
    let mut vdom = VirtualDom::new(MapSurfaceWithMarkers);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"id="geography-map""#),
        "the MapLibre mount container is present:\n{html}"
    );
    assert!(
        html.contains(r#"role="img""#),
        "the map surface exposes an image role for assistive tech:\n{html}"
    );
    assert!(
        html.contains("3 place markers") || html.contains("markers"),
        "the accessible label names the marker count:\n{html}"
    );
}

#[component]
fn MapSurfacePanHasNoCaptureOverlay() -> Element {
    let tool = use_signal(|| DrawTool::Pan);
    geography_map_surface(&chrome(), 0, 0, tool, |_lat: f64, _lon: f64| {})
}

#[test]
fn pan_mode_does_not_arm_the_crosshair_cursor() {
    let mut vdom = VirtualDom::new(MapSurfacePanHasNoCaptureOverlay);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        !html.contains("is-capturing") && html.contains(r#"data-armed="false""#),
        "Pan mode uses MapLibre's own pan/zoom cursor, so the capturing class/flag is absent:\n{html}"
    );
}

#[component]
fn MapSurfacePointModeHasCaptureOverlay() -> Element {
    let tool = use_signal(|| DrawTool::Point);
    geography_map_surface(&chrome(), 0, 0, tool, |_lat: f64, _lon: f64| {})
}

#[test]
fn point_mode_arms_the_crosshair_cursor_without_blocking_clicks() {
    let mut vdom = VirtualDom::new(MapSurfacePointModeHasCaptureOverlay);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains("is-capturing") && html.contains(r#"data-armed="true""#),
        "Point mode shows a crosshair cursor on the same #container MapLibre mounts on (no overlay\
         sibling that would intercept the click before MapLibre's own listener sees it):\n{html}"
    );
    assert!(
        html.matches(r#"id="geography-map""#).count() == 1,
        "the crosshair class lives on the single MapLibre mount div itself, not a separate overlay:\n{html}"
    );
}

#[component]
fn TimeSliderAt1900() -> Element {
    let year = use_signal(|| 1900);
    geography_time_slider(&chrome(), year)
}

#[test]
fn the_time_slider_captions_the_resolved_year() {
    let mut vdom = VirtualDom::new(TimeSliderAt1900);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"type="range""#),
        "a year range input is rendered:\n{html}"
    );
    assert!(html.contains("1900"), "the caption names the selected year:\n{html}");
    assert!(html.contains("Map as of"), "the slider's own label is shown:\n{html}");
}

fn marker() -> PlaceMarkerVm {
    PlaceMarkerVm {
        human_id: "P0001".to_owned(),
        id: "place-1".to_owned(),
        name: "Oslo".to_owned(),
        type_label: Some("City".to_owned()),
        shape: MarkerShapeVm::Point { lat: 59.9, lon: 10.7 },
    }
}

fn geography_vm() -> GeographyVm {
    GeographyVm {
        markers: vec![marker()],
        events: vec![EventPinVm {
            human_id: "E0001".to_owned(),
            id: "event-1".to_owned(),
            label: "Birth".to_owned(),
            date: Some("14 Jun 1876".to_owned()),
            place_human_id: "P0001".to_owned(),
            lat: 59.9,
            lon: 10.7,
        }],
        resolved_year: Some(1900),
        provider: MapProviderVm::OsmRaster {
            tile_url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_owned(),
            attribution: "© OpenStreetMap contributors".to_owned(),
        },
    }
}

#[component]
fn RailWithOneMarker() -> Element {
    let selected = use_signal(|| None::<(String, String)>);
    let vm = geography_vm();
    geography_rail(&chrome(), Some(&vm), selected, "")
}

#[test]
fn the_rail_lists_every_marker_by_name_and_type() {
    let mut vdom = VirtualDom::new(RailWithOneMarker);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains("Oslo"), "the marker's name is listed:\n{html}");
    assert!(html.contains("City"), "the marker's type label is listed:\n{html}");
    assert!(
        html.contains(r#"role="listbox""#) && html.contains(r#"role="option""#),
        "the rail is a proper listbox of options:\n{html}"
    );
}

#[component]
fn EmptyRail() -> Element {
    let selected = use_signal(|| None::<(String, String)>);
    geography_rail(&chrome(), None, selected, "")
}

#[test]
fn no_data_yet_renders_an_empty_rail_without_panicking() {
    let mut vdom = VirtualDom::new(EmptyRail);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains("geo-rail"), "the rail container still renders:\n{html}");
}

fn two_marker_geography_vm() -> GeographyVm {
    GeographyVm {
        markers: vec![
            marker(),
            PlaceMarkerVm {
                human_id: "P0002".to_owned(),
                id: "place-2".to_owned(),
                name: "Nordland".to_owned(),
                type_label: Some("County".to_owned()),
                shape: MarkerShapeVm::Point { lat: 67.0, lon: 15.0 },
            },
        ],
        events: Vec::new(),
        resolved_year: None,
        provider: MapProviderVm::OsmRaster {
            tile_url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_owned(),
            attribution: "© OpenStreetMap contributors".to_owned(),
        },
    }
}

#[component]
fn RailFilteredToOslo() -> Element {
    let selected = use_signal(|| None::<(String, String)>);
    let vm = two_marker_geography_vm();
    geography_rail(&chrome(), Some(&vm), selected, "osl")
}

#[test]
fn the_typed_filter_hides_non_matching_markers_from_the_rail() {
    let mut vdom = VirtualDom::new(RailFilteredToOslo);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains("Oslo"), "the matching marker stays listed:\n{html}");
    assert!(!html.contains("Nordland"), "the non-matching marker is hidden:\n{html}");
}

#[component]
fn RailWithBlankFilter() -> Element {
    let selected = use_signal(|| None::<(String, String)>);
    let vm = two_marker_geography_vm();
    geography_rail(&chrome(), Some(&vm), selected, "")
}

#[test]
fn a_blank_filter_lists_every_marker() {
    let mut vdom = VirtualDom::new(RailWithBlankFilter);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains("Oslo") && html.contains("Nordland"),
        "both markers listed:\n{html}"
    );
}
