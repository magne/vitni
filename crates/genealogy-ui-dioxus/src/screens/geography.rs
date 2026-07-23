//! The Geography tool (ADR 0025): place/event markers on a `MapLibre` GL JS map, a time slider that
//! resolves names/boundaries/jurisdiction as of a chosen year (ADR 0026 §1), in-map geometry editing
//! audited through the same [`PlaceEdit::AssertGeometry`] path as a typed-field edit, and a pluggable
//! tile/style provider (a client-scope `[map]` config descriptor, ADR 0025 §3).
//!
//! The map is a JS canvas the Dioxus renderer cannot draw into directly, so the interaction seam is:
//! [`init_geography_map`] mounts `MapLibre` once (`onmounted`) and arms a persistent `map.on('click', …)`
//! listener that streams `[lng, lat]` payloads back over `document::eval`'s `dioxus.send`/`recv` channel
//! (the same seam `watch_scroll_close` uses in `components/record_picker.rs`) — not a one-shot round
//! trip per click. A `use_effect` pushes updated marker/event/draft `GeoJSON` to the running map whenever
//! the loaded data, resolved year, or in-progress draft changes.
//!
//! Interactive canvas behavior (pan/zoom, the actual click-to-place feel, polygon vertex rendering)
//! cannot be exercised by an SSR test — the map container/attribution/empty-state/provider-select DOM
//! and the pure GeoJSON/geometry assembly are; see the module's test coverage and the PR report for the
//! items needing manual GUI verification.

use genealogy_app::{
    ConfigStore, FileConfigStore, GeoCoordinates, MapConfig, MapProvider, Microdegrees, PlaceGeometry, PlaceType,
};
use genealogy_ui::{EventPinVm, GeographyVm, MarkerShapeVm, PlaceMarkerVm, TIME_SLIDER_RANGE, clamp_slider_year};
use serde_json::{Value, json};
use std::str::FromStr;

use super::prelude::*;
use crate::i18n::Chrome;
use crate::services::Services;

/// The mount id of the map's container `div`, referenced by the init/update JS.
const MAP_CONTAINER_ID: &str = "geography-map";

/// The default view when no place has a resolved geometry yet (Oslo, a reasonable Norwegian default
/// matching every other Norway-flavoured example in this codebase's docs/fixtures).
const DEFAULT_CENTER: (f64, f64) = (59.9139, 10.7522);

/// The mockup's initial time-slider year (`geography.html`).
const DEFAULT_YEAR: i32 = 1900;

/// The active draw tool on the map surface (the mockup's toolbar). Only [`Self::Point`] and
/// [`Self::Polygon`] make the click-capture overlay intercept pointer events — [`Self::Pan`] lets
/// them fall through to `MapLibre`'s own pan/zoom gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTool {
    /// No editing; `MapLibre`'s native pan/zoom.
    Pan,
    /// Click to drop or move a single point.
    Point,
    /// Click to append a polygon vertex; [`GeographyScreen`]'s "Finish polygon" commits the ring.
    Polygon,
}

/// The in-progress shape being drawn, before it is committed to a [`PlaceGeometry`] and handed to the
/// edit/create panel.
#[derive(Debug, Clone, PartialEq)]
enum Draft {
    /// Nothing drawn yet.
    Empty,
    /// A single dropped point, in `(lat, lon)`.
    Point((f64, f64)),
    /// The polygon's vertices so far, in click order, each `(lat, lon)`.
    Polygon(Vec<(f64, f64)>),
}

/// Which side panel (if any) the screen shows: creating a new place at a clicked point, or asserting
/// a geometry onto the currently rail-selected place.
#[derive(Debug, Clone, PartialEq)]
enum GeoPanel {
    /// No panel open.
    None,
    /// A quick-create form for a new place at this point (Point tool, no rail selection).
    CreateHere {
        /// The clicked point.
        point: (f64, f64),
    },
    /// The picked geometry is ready to assert onto an existing (rail-selected) place.
    AssertOnSelected {
        /// The target place's `human_id`.
        human_id: String,
        /// The target place's display name (for the panel title).
        name: String,
        /// The geometry to assert.
        geometry: PlaceGeometry,
    },
}

/// The Geography tool screen.
#[component]
pub fn GeographyScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let coordinate_invalid = state.data_loc().place_coordinate_invalid();
    let chrome = use_context::<ChromeCtx>();
    let loading = state.chrome().loading();

    let year = use_signal(|| DEFAULT_YEAR);
    let tool = use_signal(|| DrawTool::Pan);
    let selected = use_signal(|| None::<(String, String)>);
    let mut draft = use_signal(|| Draft::Empty);
    let panel = use_signal(|| GeoPanel::None);
    let mut toast = use_signal(|| None::<String>);
    let reload = use_signal(|| 0_u32);
    let filter = use_signal(String::new);

    let data_services = services.clone();
    let data = use_resource(move || {
        let services = data_services.clone();
        let year = year();
        let _ = reload();
        async move { load_screen(services, Intent::ShowGeography { year: Some(year) }).await }
    });

    let provider_dir = services.dir.clone();
    let provider = use_memo(move || map_config(&provider_dir).resolved_provider());

    // The clicked-point stream (armed once at mount) turns into a draft point/vertex, gated by the
    // active tool; `on_map_click` owns that decision so the mount closure stays a thin trigger.
    let on_click_tool = tool;
    let mut on_click_draft = draft;
    let on_map_click = move |lat: f64, lon: f64| match on_click_tool() {
        DrawTool::Pan => {}
        DrawTool::Point => on_click_draft.set(Draft::Point((lat, lon))),
        DrawTool::Polygon => {
            let mut vertices = match on_click_draft() {
                Draft::Polygon(vertices) => vertices,
                _ => Vec::new(),
            };
            vertices.push((lat, lon));
            on_click_draft.set(Draft::Polygon(vertices));
        }
    };

    // Re-push marker/event GeoJSON whenever the loaded data, the provider, or the name filter changes
    // (the typed search box hides non-matching markers on the map, not just the rail).
    use_effect(move || {
        let query = filter();
        if let Some(ScreenData::Loaded(IntentOutcome::Geography(vm))) = &*data.read() {
            update_geography_data(vm, &query);
        }
    });
    // Re-push the in-progress draft overlay whenever it changes.
    use_effect(move || update_geography_draft(&draft()));

    let vm = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::Geography(vm))) => Some((**vm).clone()),
        _ => None,
    };
    let marker_count = vm.as_ref().map_or(0, |vm| vm.markers.len());
    let event_count = vm.as_ref().map_or(0, |vm| vm.events.len());

    let on_finish_polygon = move |_| {
        let Draft::Polygon(vertices) = draft() else { return };
        if vertices.len() < 3 {
            toast.set(Some(coordinate_invalid.clone()));
            return;
        }
        let geometry = PlaceGeometry::Polygon {
            exterior: vertices.iter().map(|&(lat, lon)| geo_point(lat, lon)).collect(),
            holes: Vec::new(),
        };
        open_geometry_panel(selected, panel, geometry);
    };
    let on_clear_draft = move |_| draft.set(Draft::Empty);

    let saved_label = state.data_loc().action_label("saved");
    rsx! {
        div { style: "display:flex;flex-direction:column;height:100%;min-height:0;gap:var(--sp-3)",
            h1 { class: "sr-only", "{chrome.0.rail_label(\"nav-geography\")}" }
            {geography_toolbar(&chrome.0, &services, provider, tool, marker_count, event_count, filter)}
            div { class: "geo", style: "flex:1;min-height:0",
                {geography_rail(&chrome.0, vm.as_ref(), selected, filter)}
                div { class: "geo-main",
                    if data.read_unchecked().is_none() {
                        p { class: "loading", "{loading}" }
                    } else if marker_count == 0 && event_count == 0 {
                        {geography_empty_state(&chrome.0)}
                    } else {
                        {geography_map_surface(&chrome.0, marker_count, event_count, tool, on_map_click)}
                    }
                    if matches!(tool(), DrawTool::Polygon) {
                        div { class: "wrap", style: "gap:8px",
                            Button { label: chrome.0.geography_finish_polygon(), small: true, variant: ButtonVariant::Primary, onclick: on_finish_polygon }
                            Button { label: chrome.0.geography_clear_draft(), small: true, variant: ButtonVariant::Ghost, onclick: on_clear_draft }
                        }
                    }
                    {geography_time_slider(&chrome.0, year)}
                }
            }
        }
        {geo_edit_panel(&chrome.0, panel, reload, toast, &saved_label)}
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: state.data_loc().action_label("dismiss"),
            onaction: move |_| toast.set(None),
        }
    }
}

/// Opens the geometry panel for the rail-selected place, or (no selection) stashes the point for the
/// quick-create form — only reachable from the Point-tool path (a polygon draft always targets an
/// existing selected place; polygon-drawn creation is deferred, see the PR report).
fn open_geometry_panel(
    selected: Signal<Option<(String, String)>>,
    mut panel: Signal<GeoPanel>,
    geometry: PlaceGeometry,
) {
    if let Some((human_id, name)) = selected() {
        panel.set(GeoPanel::AssertOnSelected {
            human_id,
            name,
            geometry,
        });
        return;
    }
    let PlaceGeometry::Point(point) = &geometry else { return };
    panel.set(GeoPanel::CreateHere {
        point: (point.latitude.to_degrees(), point.longitude.to_degrees()),
    });
}

/// The top toolbar: search placeholder (geocoding is a Phase 8+ follow-up, ADR 0025 §4 — the field is
/// present for parity with the mockup but inert), marker/event counts, and the provider select.
fn geography_toolbar(
    chrome: &Chrome,
    services: &Services,
    provider: Memo<MapProvider>,
    mut tool: Signal<DrawTool>,
    marker_count: usize,
    event_count: usize,
    mut filter: Signal<String>,
) -> Element {
    let tool_button = |this: DrawTool, label: String| {
        let active = tool() == this;
        rsx! {
            Button {
                label,
                small: true,
                variant: if active { ButtonVariant::Primary } else { ButtonVariant::Default },
                onclick: move |_| tool.set(this),
            }
        }
    };
    rsx! {
        div { class: "geo-toolbar",
            TextInput {
                style: "width:220px",
                placeholder: chrome.geography_search_placeholder(),
                value: filter(),
                oninput: move |event: FormEvent| filter.set(event.value()),
            }
            Chip { label: format!("{marker_count}") }
            Chip { label: format!("{event_count}") }
            span { class: "spacer" }
            {tool_button(DrawTool::Pan, chrome.geography_tool_pan())}
            {tool_button(DrawTool::Point, chrome.geography_tool_point())}
            {tool_button(DrawTool::Polygon, chrome.geography_tool_polygon())}
            {geography_provider_select(chrome, services, provider)}
        }
    }
}

/// The provider select: a kind picker that immediately persists the built-in defaults for
/// OSM/MapLibre-demo choices. Google and a custom `MapLibre` style need key/URL entry the mockup shows
/// as a picker-only affordance; wiring that full sub-form is deferred (see the PR report) — selecting
/// them here keeps the current provider and surfaces a toast explaining why.
fn geography_provider_select(chrome: &Chrome, services: &Services, provider: Memo<MapProvider>) -> Element {
    let dir = services.dir.clone();
    let current = provider_kind(&provider());
    let options = vec![
        SelectChoice {
            value: "osm-raster".to_owned(),
            label: chrome.geography_provider_kind_label("osm-raster"),
        },
        SelectChoice {
            value: "maplibre-style".to_owned(),
            label: chrome.geography_provider_kind_label("maplibre-style"),
        },
        SelectChoice {
            value: "google".to_owned(),
            label: chrome.geography_provider_kind_label("google"),
        },
    ];
    rsx! {
        Select {
            label: chrome.geography_provider_label(),
            name: "geography-provider".to_owned(),
            value: Some(current),
            options,
            onchange: move |event: FormEvent| {
                if event.value() == "osm-raster" {
                    let store = FileConfigStore::for_workspace(dir.clone());
                    let _ = store.store_map_config(&MapConfig { provider: Some(MapProvider::default_osm()), net_allowlist: Vec::new() });
                }
                // MapLibre-style / Google need a style URL or API key the compact toolbar select has
                // no room to collect; picking them here is a no-op until that sub-form lands.
            },
        }
    }
}

/// The place rail: every marker matching the toolbar's name filter, selectable as the in-map editor's
/// target (`record-editing.html`'s row-select precedent, simplified to a plain list since Geography is
/// not a record master-detail). An empty/whitespace-only filter shows every marker.
pub fn geography_rail(
    chrome: &Chrome,
    vm: Option<&GeographyVm>,
    mut selected: Signal<Option<(String, String)>>,
    filter: Signal<String>,
) -> Element {
    let query = filter();
    let markers: Vec<PlaceMarkerVm> = vm
        .map(|vm| filtered_markers(&vm.markers, &query).into_iter().cloned().collect())
        .unwrap_or_default();
    rsx! {
        aside { class: "geo-rail", role: "listbox", aria_label: chrome.geography_rail_label(),
            div { class: "list-rows",
                for marker in markers {
                    {
                        let is_selected = selected.read().as_ref().is_some_and(|(id, _)| *id == marker.id);
                        let target = (marker.human_id.clone(), marker.name.clone());
                        rsx! {
                            div {
                                class: if is_selected { "row sel" } else { "row" },
                                role: "option",
                                aria_selected: if is_selected { "true" } else { "false" },
                                onclick: move |_| selected.set(Some(target.clone())),
                                div { class: "avatar", "📍" }
                                div { class: "row-main",
                                    div { class: "row-title", "{marker.name}" }
                                    if let Some(type_label) = &marker.type_label {
                                        div { class: "row-sub", "{type_label}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The empty state: no place has a resolved geometry to plot yet.
pub fn geography_empty_state(chrome: &Chrome) -> Element {
    rsx! {
        div { class: "card map-card",
            div { class: "map-empty",
                div {
                    div { class: "map-empty-glyph", "🗺" }
                    div { class: "map-empty-heading", "{chrome.geography_empty_heading()}" }
                    div { class: "faint", "{chrome.geography_empty_help()}" }
                }
            }
        }
    }
}

/// The map surface: the `MapLibre` mount container (attribution rendered as a static overlay, matching
/// the Phase-6 Leaflet MVP) plus the pointer-capture overlay that is only "hot" while a draw tool
/// (not Pan) is active, so Pan still gets `MapLibre`'s own drag-to-pan gesture.
pub fn geography_map_surface(
    chrome: &Chrome,
    marker_count: usize,
    event_count: usize,
    tool: Signal<DrawTool>,
    on_map_click: impl FnMut(f64, f64) + Clone + 'static,
) -> Element {
    let capturing = !matches!(tool(), DrawTool::Pan);
    let aria = chrome.geography_map_aria(marker_count, event_count);
    rsx! {
        div {
            class: "map-surface",
            role: "img",
            aria_label: aria,
            div {
                id: MAP_CONTAINER_ID,
                class: "map-container",
                style: "position:absolute;inset:0",
                onmounted: move |_| init_geography_map(on_map_click.clone()),
            }
            if capturing {
                div {
                    class: "geo-capture",
                    style: "position:absolute;inset:0",
                    "data-armed": "true",
                }
            }
            div { class: "map-attr", "{provider_attribution_placeholder()}" }
        }
    }
}

/// A placeholder attribution shown before the map's own provider descriptor has painted its first
/// frame (the real string is set by [`update_geography_data`]'s script, since it must match whatever
/// provider is active — the static Rust-rendered node here is only the layout anchor, mirroring the
/// Phase-6 Leaflet MVP's `.map-attr` overlay).
fn provider_attribution_placeholder() -> &'static str {
    ""
}

/// The time slider: a year `<input type=range>` over [`TIME_SLIDER_RANGE`], captioned with the
/// resolved year (ADR 0026 §1).
pub fn geography_time_slider(chrome: &Chrome, mut year: Signal<i32>) -> Element {
    rsx! {
        div { class: "card", style: "padding:10px 14px",
            div { class: "time-slider",
                span { class: "muted", "{chrome.geography_time_slider_label()}" }
                TextInput {
                    kind: TextInputKind::Range,
                    style: "flex:1",
                    min: TIME_SLIDER_RANGE.0.to_string(),
                    max: TIME_SLIDER_RANGE.1.to_string(),
                    value: year().to_string(),
                    aria_label: chrome.geography_time_slider_label(),
                    oninput: move |event: FormEvent| {
                        if let Ok(value) = event.value().parse::<i32>() {
                            year.set(clamp_slider_year(value));
                        }
                    },
                }
                span { class: "time-year", "{year}" }
            }
            div { class: "faint", style: "font-size:var(--fs-xs);margin-top:4px", "{chrome.geography_time_caption(year())}" }
        }
    }
}

/// The geometry side panel: either the quick-create form (a new place at the clicked point) or the
/// assert-onto-selected form (the drafted geometry, plus the standard provenance block), both
/// dispatching through the audited change-set/`PlaceEdit` path.
fn geo_edit_panel(
    chrome: &Chrome,
    mut panel: Signal<GeoPanel>,
    mut reload: Signal<u32>,
    mut toast: Signal<Option<String>>,
    saved_label: &str,
) -> Element {
    let current = panel();
    if current == GeoPanel::None {
        return rsx! {};
    }
    let title = match &current {
        GeoPanel::CreateHere { .. } => chrome.geography_create_here(),
        GeoPanel::AssertOnSelected { name, .. } => format!("{} — {name}", chrome.geography_edit_geometry()),
        GeoPanel::None => String::new(),
    };
    let saved = saved_label.to_owned();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: chrome.rail_label("nav-preferences"),
            onclose: move |_| panel.set(GeoPanel::None),
            footer: rsx! {},
            {match current {
                GeoPanel::CreateHere { point } => rsx! {
                    GeographyCreateForm {
                        point,
                        onsaved: move |()| { panel.set(GeoPanel::None); reload += 1; toast.set(Some(saved.clone())); },
                    }
                },
                GeoPanel::AssertOnSelected { human_id, geometry, .. } => rsx! {
                    GeographyGeometryForm {
                        human_id,
                        geometry,
                        onsaved: move |()| { panel.set(GeoPanel::None); reload += 1; toast.set(Some(saved.clone())); },
                    }
                },
                GeoPanel::None => rsx! {},
            }}
        }
    }
}

/// The "New place here" quick-create form: name + type over the clicked point, committed via the
/// existing place change-set (the point rides in [`genealogy_ui::PlaceChangeSetRequest::coordinates`]
/// — the plain-point case needs no separate `AssertGeometry`, ADR 0024 §1's `Point` = the scalar
/// coordinate case).
#[component]
fn GeographyCreateForm(point: (f64, f64), onsaved: EventHandler<()>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let mut name = use_signal(String::new);
    let place_types = [
        PlaceType::Farm,
        PlaceType::Village,
        PlaceType::Town,
        PlaceType::City,
        PlaceType::Parish,
        PlaceType::Municipality,
        PlaceType::County,
        PlaceType::Country,
        PlaceType::Building,
    ];
    let mut place_type = use_signal(|| PlaceType::Farm);
    let type_options: Vec<SelectChoice> = place_types
        .iter()
        .enumerate()
        .map(|(index, kind)| SelectChoice {
            value: index.to_string(),
            label: loc.place_type_label(kind),
        })
        .collect();
    let prov = use_signal(ProvenanceDraft::default);
    let (lat, lon) = point;
    rsx! {
        Input { label: loc.field_label("name"), name: "geo-new-name".to_owned(), value: name(), oninput: move |event: FormEvent| name.set(event.value()) }
        Select {
            label: loc.field_label("type"),
            name: "geo-new-type".to_owned(),
            value: Some("0".to_owned()),
            options: type_options,
            onchange: move |event: FormEvent| {
                if let Some(kind) = event.value().parse::<usize>().ok().and_then(|index| place_types.get(index).cloned()) {
                    place_type.set(kind);
                }
            },
        }
        {provenance_block(loc, prov)}
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let request = genealogy_ui::PlaceChangeSetRequest {
                    human_id: None,
                    place_type: place_type(),
                    name: (!name().trim().is_empty()).then(|| name().trim().to_owned()),
                    coordinates: Some(geo_point(lat, lon)),
                    code: None,
                };
                let services = services.clone();
                let prov = prov();
                let onsaved = onsaved;
                spawn(async move {
                    if commit_place_change_set(services, request, prov).await.is_ok() {
                        onsaved.call(());
                    }
                });
            },
        }
    }
}

/// The assert-geometry form for an existing (rail-selected) place: the drafted shape plus the
/// standard reason/confidence provenance block, dispatched via [`PlaceEdit::AssertGeometry`] — the
/// same audited path a typed-field edit uses (ADR 0025 §2). The year is left undated (`None`) here;
/// dating a map edit to the active time-slider year is the natural next increment (see the PR report).
#[component]
fn GeographyGeometryForm(human_id: String, geometry: PlaceGeometry, onsaved: EventHandler<()>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let prov = use_signal(ProvenanceDraft::default);
    rsx! {
        div { class: "faint", style: "font-size:var(--fs-xs)", "{loc.place_map_scope_note()}" }
        {provenance_block(loc, prov)}
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let edit = PlaceEdit::AssertGeometry { human_id: human_id.clone(), geometry: geometry.clone(), year: None };
                let services = services.clone();
                let prov = prov();
                let onsaved = onsaved;
                spawn(async move {
                    if save_place_edit(services, edit, prov).await.is_ok() {
                        onsaved.call(());
                    }
                });
            },
        }
    }
}

/// Reads the configured `[map]` provider from the workspace's config store, falling back to the
/// built-in OSM default on any read error (mirrors `services::ai_config`'s fallback pattern).
fn map_config(dir: &std::path::Path) -> MapConfig {
    match FileConfigStore::for_workspace(dir.to_path_buf()).load_map_config() {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "could not read the map config; using the built-in default");
            MapConfig::default()
        }
    }
}

/// The provider's stable kind token, for the select's current value.
fn provider_kind(provider: &MapProvider) -> String {
    match provider {
        MapProvider::OsmRaster { .. } => "osm-raster",
        MapProvider::MaplibreStyle { .. } => "maplibre-style",
        MapProvider::Google { .. } => "google",
    }
    .to_owned()
}

/// Builds a [`GeoCoordinates`] from decimal degrees (the click-stream/GeoJSON boundary), rounding to
/// the microdegree precision the domain type stores.
fn geo_point(lat: f64, lon: f64) -> GeoCoordinates {
    GeoCoordinates {
        latitude: Microdegrees::from_str(&format!("{lat:.6}")).unwrap_or(Microdegrees::from_microdegrees(0)),
        longitude: Microdegrees::from_str(&format!("{lon:.6}")).unwrap_or(Microdegrees::from_microdegrees(0)),
    }
}

/// Mounts `MapLibre` on the `geography-map` container (a no-op under SSR, where there is no webview to
/// run the script) and arms the persistent click listener. The listener streams every click as a
/// `[lng, lat]` JSON payload over `dioxus.send`, read in a loop for the screen's lifetime (the same
/// seam `record_picker.rs`'s `watch_scroll_close` uses) — not a one-shot eval per click, so the map
/// stays interactive without a Rust round trip blocking each gesture.
fn init_geography_map(mut on_click: impl FnMut(f64, f64) + 'static) {
    let mut listener = document::eval(&geography_init_script());
    spawn(async move {
        while let Ok(payload) = listener.recv::<String>().await {
            if let Ok(click) = serde_json::from_str::<[f64; 2]>(&payload) {
                on_click(click[1], click[0]);
            }
        }
    });
}

/// The `MapLibre` bootstrap script: creates the map (guarded against a re-render remount), adds the
/// marker/event/draft `GeoJSON` sources + layers once loaded, and arms the click listener.
fn geography_init_script() -> String {
    format!(
        r"
        const el = document.getElementById('{MAP_CONTAINER_ID}');
        if (el && !el.__geoMap && window.maplibregl) {{
            const map = new maplibregl.Map({{
                container: el,
                style: {{ version: 8, sources: {{}}, layers: [] }},
                center: [{lon}, {lat}],
                zoom: 4,
                attributionControl: false,
            }});
            el.__geoMap = map;
            map.on('load', () => {{
                map.addSource('geo-tiles', {{ type: 'raster', tiles: ['https://tile.openstreetmap.org/{{z}}/{{x}}/{{y}}.png'], tileSize: 256 }});
                map.addLayer({{ id: 'geo-tile-layer', type: 'raster', source: 'geo-tiles' }});
                map.addSource('geo-markers', {{ type: 'geojson', data: {{ type: 'FeatureCollection', features: [] }} }});
                map.addLayer({{ id: 'geo-marker-fill', type: 'fill', source: 'geo-markers', filter: ['==', ['geometry-type'], 'Polygon'], paint: {{ 'fill-color': '#5db3ff', 'fill-opacity': 0.25 }} }});
                map.addLayer({{ id: 'geo-marker-line', type: 'line', source: 'geo-markers', filter: ['==', ['geometry-type'], 'Polygon'], paint: {{ 'line-color': '#5db3ff', 'line-width': 2 }} }});
                map.addLayer({{ id: 'geo-marker-point', type: 'circle', source: 'geo-markers', filter: ['==', ['geometry-type'], 'Point'], paint: {{ 'circle-color': '#5db3ff', 'circle-radius': 6 }} }});
                map.addSource('geo-events', {{ type: 'geojson', data: {{ type: 'FeatureCollection', features: [] }} }});
                map.addLayer({{ id: 'geo-event-point', type: 'circle', source: 'geo-events', paint: {{ 'circle-color': '#ffb020', 'circle-radius': 4 }} }});
                map.addSource('geo-draft', {{ type: 'geojson', data: {{ type: 'FeatureCollection', features: [] }} }});
                map.addLayer({{ id: 'geo-draft-fill', type: 'fill', source: 'geo-draft', filter: ['==', ['geometry-type'], 'Polygon'], paint: {{ 'fill-color': '#ff5d5d', 'fill-opacity': 0.2 }} }});
                map.addLayer({{ id: 'geo-draft-line', type: 'line', source: 'geo-draft', paint: {{ 'line-color': '#ff5d5d', 'line-width': 2 }} }});
                map.addLayer({{ id: 'geo-draft-point', type: 'circle', source: 'geo-draft', filter: ['==', ['geometry-type'], 'Point'], paint: {{ 'circle-color': '#ff5d5d', 'circle-radius': 6 }} }});
            }});
            map.on('click', (e) => {{ dioxus.send(JSON.stringify([e.lngLat.lng, e.lngLat.lat])); }});
        }}
        ",
        lat = DEFAULT_CENTER.0,
        lon = DEFAULT_CENTER.1,
    )
}

/// The markers matching a name filter (case-insensitive substring; an empty/whitespace-only query
/// matches everything) — shared by the rail listbox and the map's pushed marker `GeoJSON` so a typed
/// search hides the same places in both (the Geography toolbar's search box).
fn filtered_markers<'a>(markers: &'a [PlaceMarkerVm], query: &str) -> Vec<&'a PlaceMarkerVm> {
    let query = query.trim().to_lowercase();
    markers
        .iter()
        .filter(|marker| query.is_empty() || marker.name.to_lowercase().contains(&query))
        .collect()
}

/// Pushes the loaded markers/event pins to the running map's `GeoJSON` sources, guarded so a reload
/// that races the map's own async `load` event simply skips (the next data/effect re-run catches up).
/// `query` filters the pushed markers the same way [`geography_rail`] does, so a typed search hides the
/// same places on the map as in the rail.
fn update_geography_data(vm: &GeographyVm, query: &str) {
    let matching: Vec<PlaceMarkerVm> = filtered_markers(&vm.markers, query).into_iter().cloned().collect();
    let markers_json = markers_geojson(&matching);
    let events_json = events_geojson(&vm.events);
    let script = format!(
        r"
        const map = document.getElementById('{MAP_CONTAINER_ID}')?.__geoMap;
        if (map) {{
            const markers = map.getSource('geo-markers');
            if (markers) markers.setData({markers_json});
            const events = map.getSource('geo-events');
            if (events) events.setData({events_json});
        }}
        ",
    );
    run_geography_script(&script);
}

/// Pushes the in-progress draft overlay (a dropped point or the polygon vertices so far) to the
/// running map, guarded the same way as [`update_geography_data`].
fn update_geography_draft(draft: &Draft) {
    let geojson = draft_geojson(draft);
    let script = format!(
        r"
        const map = document.getElementById('{MAP_CONTAINER_ID}')?.__geoMap;
        if (map) {{
            const draft = map.getSource('geo-draft');
            if (draft) draft.setData({geojson});
        }}
        ",
    );
    run_geography_script(&script);
}

/// Runs a fire-and-forget script against the mounted map (a no-op under SSR).
fn run_geography_script(script: &str) {
    let mut eval = document::eval(script);
    spawn(async move {
        let _ = eval.recv::<()>().await;
    });
}

/// Converts the resolved place markers to a `GeoJSON` `FeatureCollection`, `[lon, lat]` order.
fn markers_geojson(markers: &[PlaceMarkerVm]) -> Value {
    let features: Vec<Value> = markers
        .iter()
        .map(|marker| {
            json!({
                "type": "Feature",
                "geometry": shape_geojson(&marker.shape),
                "properties": { "id": marker.id, "human_id": marker.human_id, "name": marker.name },
            })
        })
        .collect();
    json!({ "type": "FeatureCollection", "features": features })
}

/// Converts the resolved event pins to a `GeoJSON` `FeatureCollection` of points.
fn events_geojson(events: &[EventPinVm]) -> Value {
    let features: Vec<Value> = events
        .iter()
        .map(|pin| {
            json!({
                "type": "Feature",
                "geometry": { "type": "Point", "coordinates": [pin.lon, pin.lat] },
                "properties": { "id": pin.id, "human_id": pin.human_id, "label": pin.label },
            })
        })
        .collect();
    json!({ "type": "FeatureCollection", "features": features })
}

fn shape_geojson(shape: &MarkerShapeVm) -> Value {
    match shape {
        MarkerShapeVm::Point { lat, lon } => json!({ "type": "Point", "coordinates": [lon, lat] }),
        MarkerShapeVm::Polygon { exterior, holes } => {
            let mut rings: Vec<Vec<[f64; 2]>> = vec![closed_ring(exterior)];
            rings.extend(holes.iter().map(|hole| closed_ring(hole)));
            json!({ "type": "Polygon", "coordinates": rings })
        }
    }
}

/// A ring's `[lon, lat]` points, closed (first point repeated last) for correct WebGL fill rendering —
/// a rendering concern only; the saved [`PlaceGeometry::Polygon`] never duplicates the closing point.
#[expect(
    clippy::float_cmp,
    reason = "exact identity check: comparing a ring's last point to its literal first element, not two independently computed floats"
)]
fn closed_ring(points: &[(f64, f64)]) -> Vec<[f64; 2]> {
    let mut ring: Vec<[f64; 2]> = points.iter().map(|&(lat, lon)| [lon, lat]).collect();
    if let (Some(&first), Some(&last)) = (ring.first(), ring.last())
        && first != last
    {
        ring.push(first);
    }
    ring
}

/// Converts the in-progress draft to a `GeoJSON` `FeatureCollection` (empty, a point, or a polygon
/// preview drawn as a closed ring once it has at least 3 vertices, else an open line).
fn draft_geojson(draft: &Draft) -> Value {
    match draft {
        Draft::Empty => json!({ "type": "FeatureCollection", "features": [] }),
        Draft::Point((lat, lon)) => json!({
            "type": "FeatureCollection",
            "features": [{ "type": "Feature", "geometry": { "type": "Point", "coordinates": [lon, lat] }, "properties": {} }],
        }),
        Draft::Polygon(vertices) if vertices.len() >= 3 => json!({
            "type": "FeatureCollection",
            "features": [{ "type": "Feature", "geometry": { "type": "Polygon", "coordinates": [closed_ring(vertices)] }, "properties": {} }],
        }),
        Draft::Polygon(vertices) => json!({
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "geometry": { "type": "LineString", "coordinates": vertices.iter().map(|&(lat, lon)| [lon, lat]).collect::<Vec<_>>() },
                "properties": {},
            }],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Draft, closed_ring, draft_geojson, events_geojson, filtered_markers, markers_geojson, provider_kind,
        shape_geojson,
    };
    use genealogy_app::MapProvider;
    use genealogy_ui::{EventPinVm, MarkerShapeVm, PlaceMarkerVm};

    #[test]
    fn a_point_shape_becomes_a_geojson_point_in_lon_lat_order() {
        let shape = MarkerShapeVm::Point { lat: 59.9, lon: 10.7 };
        assert_eq!(
            shape_geojson(&shape),
            serde_json::json!({ "type": "Point", "coordinates": [10.7, 59.9] })
        );
    }

    #[test]
    fn a_polygon_shape_closes_its_exterior_ring_for_rendering() {
        let shape = MarkerShapeVm::Polygon {
            exterior: vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)],
            holes: Vec::new(),
        };
        let geojson = shape_geojson(&shape);
        let ring = geojson["coordinates"][0].as_array().expect("a ring");
        assert_eq!(ring.len(), 4, "the ring is closed (first point repeated last)");
        assert_eq!(ring[0], ring[3]);
    }

    #[test]
    fn an_already_closed_ring_is_not_duplicated_again() {
        let ring = closed_ring(&[(60.0, 5.0), (61.0, 5.0), (61.0, 6.0), (60.0, 5.0)]);
        assert_eq!(ring.len(), 4);
    }

    #[test]
    fn markers_geojson_wraps_each_marker_as_a_feature() {
        let markers = vec![PlaceMarkerVm {
            human_id: "P0001".to_owned(),
            id: "id-1".to_owned(),
            name: "Oslo".to_owned(),
            type_label: None,
            shape: MarkerShapeVm::Point { lat: 59.9, lon: 10.7 },
        }];
        let geojson = markers_geojson(&markers);
        assert_eq!(geojson["features"].as_array().expect("features").len(), 1);
        assert_eq!(geojson["features"][0]["properties"]["human_id"], "P0001");
    }

    #[test]
    fn events_geojson_wraps_each_pin_as_a_point_feature() {
        let events = vec![EventPinVm {
            human_id: "E0001".to_owned(),
            id: "id-1".to_owned(),
            label: "Birth".to_owned(),
            date: None,
            place_human_id: "P0001".to_owned(),
            lat: 59.9,
            lon: 10.7,
        }];
        let geojson = events_geojson(&events);
        assert_eq!(
            geojson["features"][0]["geometry"]["coordinates"],
            serde_json::json!([10.7, 59.9])
        );
    }

    #[test]
    fn an_empty_draft_has_no_features() {
        let geojson = draft_geojson(&Draft::Empty);
        assert!(geojson["features"].as_array().expect("features").is_empty());
    }

    #[test]
    fn a_two_vertex_polygon_draft_previews_as_a_line() {
        let geojson = draft_geojson(&Draft::Polygon(vec![(60.0, 5.0), (61.0, 5.0)]));
        assert_eq!(geojson["features"][0]["geometry"]["type"], "LineString");
    }

    #[test]
    fn a_three_vertex_polygon_draft_previews_as_a_closed_polygon() {
        let geojson = draft_geojson(&Draft::Polygon(vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)]));
        assert_eq!(geojson["features"][0]["geometry"]["type"], "Polygon");
    }

    #[test]
    fn provider_kind_tokens_round_trip_the_select_value() {
        assert_eq!(provider_kind(&MapProvider::default_osm()), "osm-raster");
        assert_eq!(
            provider_kind(&MapProvider::Google {
                api_key_env: "K".to_owned(),
                attribution: "A".to_owned()
            }),
            "google"
        );
    }

    fn markers() -> Vec<PlaceMarkerVm> {
        vec![
            PlaceMarkerVm {
                human_id: "P0001".to_owned(),
                id: "place-1".to_owned(),
                name: "Oslo".to_owned(),
                type_label: None,
                shape: MarkerShapeVm::Point { lat: 59.9, lon: 10.7 },
            },
            PlaceMarkerVm {
                human_id: "P0002".to_owned(),
                id: "place-2".to_owned(),
                name: "Nordland".to_owned(),
                type_label: None,
                shape: MarkerShapeVm::Point { lat: 67.0, lon: 15.0 },
            },
        ]
    }

    #[test]
    fn a_blank_filter_matches_every_marker() {
        assert_eq!(filtered_markers(&markers(), "").len(), 2);
        assert_eq!(filtered_markers(&markers(), "   ").len(), 2);
    }

    #[test]
    fn the_filter_matches_a_case_insensitive_substring_of_the_name() {
        let all = markers();
        let matches = filtered_markers(&all, "osl");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "Oslo");
        assert_eq!(filtered_markers(&all, "OSLO").len(), 1);
    }

    #[test]
    fn a_non_matching_filter_yields_no_markers() {
        assert!(filtered_markers(&markers(), "Bergen").is_empty());
    }
}
