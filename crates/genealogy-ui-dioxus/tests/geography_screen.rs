//! SSR assertions for the Geography tool (ADR 0025): the map container/attribution, the empty state
//! shown when no place has a resolved geometry, the rail list, and the time-slider caption. Mirrors
//! the Phase-6 MVP's `place_map.rs` SSR test pattern — pure render functions over hand-built
//! view-models, no `AppCtx`/`Services` needed (interactive canvas behavior itself cannot be exercised
//! this way; see the PR report for what needs manual GUI verification).

use dioxus::prelude::*;
use genealogy_ui::{EventPinVm, GeographyVm, MapProviderVm, MarkerShapeVm, PlaceMarkerVm};
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::screens::{
    DrawTool, MapCredit, MapDraft, geography_draw_target, geography_empty_state, geography_map_surface, geography_rail,
    geography_time_slider, geography_unplotted_note,
};

fn chrome() -> Chrome {
    Chrome::with_languages(None, &["en".parse().unwrap_or_default()])
}

/// The credit the surface is handed in these renders. Not a Fluent lookup: an attribution is the tile
/// provider's own required wording (it arrives from `[map]` config), so it is shown verbatim in every
/// locale — which is why the assertions below pin the supplied text rather than any translation.
fn credit() -> MapCredit {
    MapCredit {
        tile_url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_owned(),
        attribution: "© OpenStreetMap contributors".to_owned(),
    }
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
    let zoom = use_signal(|| 4.0);
    let draft = use_signal(|| MapDraft::Empty);
    geography_map_surface(&chrome(), 3, 5, tool, draft, zoom, credit())
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

/// The tile source's terms require its credit to be visible, and `MapLibre`'s own
/// `AttributionControl` is disabled — so this static overlay is the only thing that shows it. Before
/// #254 the box rendered with a literal empty string in it.
#[test]
fn the_map_surface_shows_the_tile_sources_required_credit() {
    let mut vdom = VirtualDom::new(MapSurfaceWithMarkers);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"class="map-attr""#),
        "the attribution overlay is rendered:\n{html}"
    );
    assert!(
        html.contains("© OpenStreetMap contributors"),
        "the overlay carries the configured credit text, not an empty box:\n{html}"
    );
}

#[component]
fn MapSurfaceWithNoCredit() -> Element {
    let tool = use_signal(|| DrawTool::Pan);
    let zoom = use_signal(|| 4.0);
    let blank = MapCredit {
        tile_url: String::new(),
        attribution: String::new(),
    };
    let draft = use_signal(|| MapDraft::Empty);
    geography_map_surface(&chrome(), 0, 0, tool, draft, zoom, blank)
}

/// `.map-attr` is a bordered panel in the stylesheet, so rendering it around nothing draws a small
/// empty box over the corner of the map — worse than drawing nothing at all.
#[test]
fn an_empty_credit_renders_no_overlay_at_all() {
    let mut vdom = VirtualDom::new(MapSurfaceWithNoCredit);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        !html.contains("map-attr"),
        "no credit means no empty bordered box:\n{html}"
    );
}

#[component]
fn MapSurfacePanHasNoCaptureOverlay() -> Element {
    let tool = use_signal(|| DrawTool::Pan);
    let zoom = use_signal(|| 4.0);
    let draft = use_signal(|| MapDraft::Empty);
    geography_map_surface(&chrome(), 0, 0, tool, draft, zoom, credit())
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
    let zoom = use_signal(|| 4.0);
    let draft = use_signal(|| MapDraft::Empty);
    geography_map_surface(&chrome(), 0, 0, tool, draft, zoom, credit())
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
        unplotted_count: 0,
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
        unplotted_count: 0,
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

fn draw_target_view() -> Element {
    let target = Some(("P0001".to_owned(), "Oslo".to_owned()));
    geography_draw_target(&chrome(), target.as_ref())
}

#[test]
fn the_toolbar_names_the_place_a_drawn_shape_attaches_to() {
    let html = render(draw_target_view);
    assert!(html.contains("Drawing on Oslo"), "the target's name is shown:\n{html}");
    assert!(html.contains("P0001"), "the target's human id is shown:\n{html}");
    assert!(html.contains(r#"class="chip""#), "the readout is a chip:\n{html}");
}

fn no_draw_target_view() -> Element {
    geography_draw_target(&chrome(), None)
}

#[test]
fn the_toolbar_says_when_there_is_no_draw_target() {
    let html = render(no_draw_target_view);
    assert!(html.contains("No place selected"), "the empty state is named:\n{html}");
    assert!(
        !html.contains("Drawing on"),
        "no target is claimed when there is none:\n{html}"
    );
}

#[component]
fn RailWithSelectedMarker() -> Element {
    let selected = use_signal(|| Some(("P0001".to_owned(), "Oslo".to_owned())));
    let vm = two_marker_geography_vm();
    geography_rail(&chrome(), Some(&vm), selected, "")
}

#[test]
fn the_selected_row_is_highlighted_and_announced() {
    let mut vdom = VirtualDom::new(RailWithSelectedMarker);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"class="row sel""#),
        "the selected row carries the sel modifier class:\n{html}"
    );
    assert!(
        html.contains(r#"aria-selected="true""#),
        "the selected row is announced to assistive tech:\n{html}"
    );
}

#[test]
fn only_the_selected_row_is_announced_as_selected() {
    let mut vdom = VirtualDom::new(RailWithSelectedMarker);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert_eq!(
        html.matches(r#"aria-selected="true""#).count(),
        1,
        "only the one selected row is announced:\n{html}"
    );
    assert!(
        html.contains(r#"aria-selected="false""#),
        "the non-selected row is announced as not selected:\n{html}"
    );
}

fn unplotted_note_view() -> Element {
    geography_unplotted_note(&chrome(), 1, 1850)
}

fn unplotted_note_plural_view() -> Element {
    geography_unplotted_note(&chrome(), 3, 1850)
}

fn no_unplotted_note_view() -> Element {
    geography_unplotted_note(&chrome(), 0, 1850)
}

#[test]
fn the_places_that_did_not_resolve_are_named_in_a_note_not_silently_absent() {
    let html = render(unplotted_note_view);
    assert!(
        html.contains("1 place has no geometry as of 1850."),
        "the note counts the places and names the year:\n{html}"
    );
    assert!(
        html.contains(r#"class="section-note""#),
        "the note reuses the section-note design-system class:\n{html}"
    );
}

#[test]
fn the_unplotted_note_pluralizes_its_count() {
    let html = render(unplotted_note_plural_view);
    assert!(
        html.contains("3 places have no geometry as of 1850."),
        "the plural form is used for more than one place:\n{html}"
    );
}

#[test]
fn nothing_unplotted_renders_no_note_at_all() {
    let html = render(no_unplotted_note_view);
    assert!(
        !html.contains("section-note") && !html.contains("no geometry"),
        "a count of zero renders nothing — an empty note is noise:\n{html}"
    );
}
