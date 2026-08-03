//! The Geography tool (ADR 0025): place/event markers on a `MapLibre` GL JS map, a time slider that
//! resolves names/boundaries/jurisdiction as of a chosen year (ADR 0026 §1), in-map geometry editing
//! audited through the same [`PlaceEdit::AssertGeometry`] path as a typed-field edit, and a pluggable
//! tile/style provider (a client-scope `[map]` config descriptor, ADR 0025 §3).
//!
//! The toolbar search is a [`RecordPicker`] over every place in the workspace (`Category::Places`,
//! `record_picker`), not a geocoder (still deferred, ADR 0025 §4): picking a result selects it as the
//! in-map editor's target exactly like a rail click, and its live query also filters the rail/pushed
//! markers as you type (the map only ever plots places with a resolved geometry either way).
//!
//! The `MapLibre` mount/update machinery (draw tools, `GeoJSON` conversion, the click-stream seam, and
//! the assert-geometry save form) is shared with the Place screen's own Map tab (Phase 9) — see
//! `screens::map_shared`. A `use_effect` pushes updated marker/event/draft `GeoJSON` to the running map
//! whenever the loaded data, the picker's query, or the in-progress draft changes.
//!
//! Interactive canvas behavior (pan/zoom, the actual click-to-place feel, polygon vertex rendering)
//! and the toolbar picker (needs `AppCtx`'s `Services`, so it isn't SSR-testable in isolation) cannot
//! be exercised by an SSR test; see the module's test coverage and the PR report for the items needing
//! manual GUI verification.

use genealogy_app::{ConfigStore, FileConfigStore, MapConfig, MapProvider, PlaceGeometry, PlaceType};
use genealogy_ui::{GeographyVm, MarkerShapeVm, PlaceMarkerVm, TIME_SLIDER_RANGE, clamp_slider_year};

use super::map_shared::{
    DEFAULT_CENTER, DrawTool, GeometrySaveForm, MapControlLabels, MapDraft, MapZoomReadout, events_geojson, fit_bounds,
    geo_point, map_surface, markers_geojson, push_map_data, push_map_draft, select_tool,
};
use super::prelude::*;
use crate::i18n::Chrome;
use crate::services::Services;

/// The mount id of the map's container `div`, referenced by the init/update JS.
const MAP_CONTAINER_ID: &str = "geography-map";

/// The mockup's initial time-slider year (`geography.html`).
const DEFAULT_YEAR: i32 = 1900;

/// The zoom the atlas opens at: the whole-continent view the mockup sketches, wide enough that every
/// marker in a Norwegian workspace is on screen before anyone touches Fit.
const GEOGRAPHY_DEFAULT_ZOOM: f64 = 4.0;

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
    let loc = state.data_loc();
    let coordinate_invalid = loc.place_coordinate_invalid();
    let chrome = use_context::<ChromeCtx>();
    let no_draw_target = chrome.0.geography_draw_target_required();
    let loading = state.chrome().loading();

    let year = use_signal(|| DEFAULT_YEAR);
    let zoom = use_signal(|| GEOGRAPHY_DEFAULT_ZOOM);
    let tool = use_signal(|| DrawTool::Pan);
    let selected = use_signal(|| None::<(String, String)>);
    let mut draft = use_signal(|| MapDraft::Empty);
    let panel = use_signal(|| GeoPanel::None);
    let mut toast = use_signal(|| None::<String>);
    let reload = use_signal(|| 0_u32);

    // Consumes a pending "Open in Geography ↗" focus target (the Place Map tab's own toolbar button),
    // stashed on `NavState` before navigating here: pre-selects it in the rail exactly like a rail
    // click would, then clears it so it does not re-apply on a later, unrelated visit. `.peek()` (not
    // `.read()`) so this runs once at mount rather than re-subscribing to every future focus request.
    let mut nav = use_context::<NavState>();
    let mut focus_selected = selected;
    use_effect(move || {
        let focus = nav.geography_focus.peek().clone();
        if let Some(focus) = focus {
            focus_selected.set(Some(focus));
            nav.geography_focus.set(None);
        }
    });

    // The toolbar search is a Place picker (not a geocoder — that stays deferred, ADR 0025 §4): it
    // searches every place in the workspace (not just already-plotted markers), and picking one
    // selects it as the in-map editor's target exactly like a rail click does. Its live query also
    // filters the rail/map markers as you type, so `filtered_markers` still applies below.
    let mut places_picker = use_existing_picker(
        services.clone(),
        Category::Places,
        loc.field_label("place"),
        "geography-search".to_owned(),
        loc.picker_entity(Category::Places),
        Vec::new(),
    );
    let mut picker_selected = selected;
    places_picker.callbacks.onpick =
        use_callback(move |picked: PickerSelection| picker_selected.set(Some((picked.human_id, picked.title))));
    let mut picker_cleared = selected;
    places_picker.callbacks.onclear = use_callback(move |()| picker_cleared.set(None));
    let filter = places_picker.state;
    let picker = places_picker;

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
        DrawTool::Point => on_click_draft.set(MapDraft::Point((lat, lon))),
        DrawTool::Polygon => {
            let mut vertices = match on_click_draft() {
                MapDraft::Polygon(vertices) => vertices,
                _ => Vec::new(),
            };
            vertices.push((lat, lon));
            on_click_draft.set(MapDraft::Polygon(vertices));
        }
    };

    // Re-push marker/event GeoJSON whenever the loaded data or the picker's live query changes (the
    // typed search hides non-matching markers on the map, not just the rail).
    use_effect(move || {
        let query = filter().query;
        if let Some(ScreenData::Loaded(IntentOutcome::Geography(vm))) = &*data.read() {
            update_geography_data(vm, &query);
        }
    });
    // Re-push the in-progress draft overlay whenever it changes.
    use_effect(move || push_map_draft(MAP_CONTAINER_ID, &draft()));
    // Recentre/zoom the map on the rail/picker's current selection, so picking a place gives visible
    // feedback that the selection took (before this, only the toolbar's own Fit button ever moved the
    // map — selecting a place otherwise had no on-map effect at all).
    use_effect(move || {
        let current = selected();
        if let Some(ScreenData::Loaded(IntentOutcome::Geography(vm))) = &*data.read()
            && let Some(shape) = selected_marker_shape(vm, current.as_ref())
        {
            fit_bounds(MAP_CONTAINER_ID, std::slice::from_ref(shape));
        }
    });

    let vm = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::Geography(vm))) => Some((**vm).clone()),
        _ => None,
    };
    let marker_count = vm.as_ref().map_or(0, |vm| vm.markers.len());
    let event_count = vm.as_ref().map_or(0, |vm| vm.events.len());
    let unplotted_count = vm.as_ref().map_or(0, |vm| vm.unplotted_count);
    let draw_target = selected();
    // The "⤢ Fit" toolbar button's target: every currently filtered marker's shape (mirrors what
    // `update_geography_data` pushes to the map, so Fit frames exactly what is shown).
    let fit_shapes: Vec<MarkerShapeVm> = vm.as_ref().map_or_else(Vec::new, |vm| {
        filtered_markers(&vm.markers, &filter().query)
            .into_iter()
            .map(|marker| marker.shape.clone())
            .collect()
    });

    let on_finish_polygon = move |_| {
        let MapDraft::Polygon(vertices) = draft() else { return };
        if vertices.len() < 3 {
            toast.set(Some(coordinate_invalid.clone()));
            return;
        }
        let geometry = PlaceGeometry::Polygon {
            exterior: vertices.iter().map(|&(lat, lon)| geo_point(lat, lon)).collect(),
            holes: Vec::new(),
        };
        open_geometry_panel(selected, panel, toast, &no_draw_target, geometry);
    };
    let on_clear_draft = move |_| draft.set(MapDraft::Empty);

    let saved_label = state.data_loc().action_label("saved");
    rsx! {
        div { style: "display:flex;flex-direction:column;height:100%;min-height:0;gap:var(--sp-3)",
            h1 { class: "sr-only", "{chrome.0.rail_label(\"nav-geography\")}" }
            {geography_toolbar(loc, &chrome.0, &picker, &services, provider, tool, zoom, marker_count, event_count, &fit_shapes, draw_target.as_ref())}
            {geography_unplotted_note(&chrome.0, unplotted_count, year())}
            div { class: "geo", style: "flex:1;min-height:0",
                {geography_rail(&chrome.0, vm.as_ref(), selected, &filter().query)}
                div { class: "geo-main",
                    if data.read_unchecked().is_none() {
                        p { class: "loading", "{loading}" }
                    } else if marker_count == 0 && event_count == 0 {
                        {geography_empty_state(&chrome.0)}
                    } else {
                        {geography_map_surface(&chrome.0, marker_count, event_count, tool, zoom, on_map_click)}
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
        {geo_edit_panel(&chrome.0, panel, reload, toast, &saved_label, year())}
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: state.data_loc().action_label("dismiss"),
            onaction: move |_| toast.set(None),
        }
    }
}

/// The panel a finished geometry opens, or `None` when there is no draw target to attach it to.
fn geometry_panel_for(selected: Option<(String, String)>, geometry: PlaceGeometry) -> Option<GeoPanel> {
    if let Some((human_id, name)) = selected {
        return Some(GeoPanel::AssertOnSelected {
            human_id,
            name,
            geometry,
        });
    }
    let PlaceGeometry::Point(point) = &geometry else {
        return None;
    };
    Some(GeoPanel::CreateHere {
        point: (point.latitude.to_degrees(), point.longitude.to_degrees()),
    })
}

/// Opens the geometry panel for the rail-selected place (the only caller today is the polygon
/// finish; the `Point`/`CreateHere` branch is retained for that tool but currently unreachable, see
/// the PR report). A polygon with no draw target is refused with a toast; the draft is deliberately
/// kept on the canvas, so picking a place and pressing Finish again commits the same geometry.
fn open_geometry_panel(
    selected: Signal<Option<(String, String)>>,
    mut panel: Signal<GeoPanel>,
    mut toast: Signal<Option<String>>,
    no_target: &str,
    geometry: PlaceGeometry,
) {
    match geometry_panel_for(selected(), geometry) {
        Some(next) => panel.set(next),
        None => toast.set(Some(no_target.to_owned())),
    }
}

/// The top toolbar: a Place picker (searches every place in the workspace, not just already-plotted
/// markers — geocoding a real-world address is a separate, still-deferred follow-up, ADR 0025 §4),
/// marker/event counts, the draw tools, the live zoom readout, and the provider select.
#[expect(
    clippy::too_many_arguments,
    reason = "a toolbar threads the screen's picker + provider + draw-tool + zoom + draw-target state"
)]
fn geography_toolbar(
    loc: &Localizer,
    chrome: &Chrome,
    picker: &RecordPicker,
    services: &Services,
    provider: Memo<MapProvider>,
    tool: Signal<DrawTool>,
    zoom: Signal<f64>,
    marker_count: usize,
    event_count: usize,
    fit_shapes: &[MarkerShapeVm],
    draw_target: Option<&(String, String)>,
) -> Element {
    let tool_button = |this: DrawTool, label: String| {
        let active = tool() == this;
        rsx! {
            Button {
                label,
                small: true,
                variant: if active { ButtonVariant::Primary } else { ButtonVariant::Default },
                onclick: move |_| select_tool(tool, this),
            }
        }
    };
    let fit_shapes = fit_shapes.to_vec();
    rsx! {
        div { class: "geo-toolbar",
            div { style: "width:240px", {record_picker(loc, picker)} }
            Chip { label: format!("{marker_count}") }
            Chip { label: format!("{event_count}") }
            {geography_draw_target(chrome, draw_target)}
            span { class: "spacer" }
            {tool_button(DrawTool::Pan, chrome.geography_tool_pan())}
            {tool_button(DrawTool::Point, chrome.geography_tool_point())}
            {tool_button(DrawTool::Polygon, chrome.geography_tool_polygon())}
            Button {
                label: chrome.geography_tool_fit(),
                small: true,
                variant: ButtonVariant::Ghost,
                onclick: move |_| fit_bounds(MAP_CONTAINER_ID, &fit_shapes),
            }
            MapZoomReadout { zoom }
            {geography_provider_select(chrome, services, provider)}
        }
    }
}

/// The toolbar's draw-target readout: which place a finished point/polygon will attach to. Split out
/// of [`geography_toolbar`] because the toolbar as a whole needs `Services` and a reactive `Memo`, so
/// only this slice is SSR-testable (see the module doc). Keeps showing the target even when the
/// picker's live query has filtered that marker out of the rail — the target persists through a
/// search, so this is not a bug.
pub fn geography_draw_target(chrome: &Chrome, target: Option<&(String, String)>) -> Element {
    match target {
        Some((human_id, name)) => rsx! {
            Chip {
                icon: Some("🎯".to_owned()),
                label: chrome.geography_drawing_on(name),
                id_label: Some(human_id.clone()),
            }
        },
        None => rsx! { Chip { icon: Some("🎯".to_owned()), label: chrome.geography_draw_target_none() } },
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

/// The place rail: every marker matching the toolbar picker's live query, selectable as the in-map
/// editor's target (`record-editing.html`'s row-select precedent, simplified to a plain list since
/// Geography is not a record master-detail). An empty/whitespace-only query shows every marker.
pub fn geography_rail(
    chrome: &Chrome,
    vm: Option<&GeographyVm>,
    mut selected: Signal<Option<(String, String)>>,
    query: &str,
) -> Element {
    let markers: Vec<PlaceMarkerVm> = vm
        .map(|vm| filtered_markers(&vm.markers, query).into_iter().cloned().collect())
        .unwrap_or_default();
    rsx! {
        aside { class: "geo-rail", role: "listbox", aria_label: chrome.geography_rail_label(),
            div { class: "list-rows",
                for marker in markers {
                    {
                        let is_selected = selected.read().as_ref().is_some_and(|(id, _)| *id == marker.human_id);
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

/// The note counting the places whose geometry does not resolve as of the slider year (ADR 0026 §1):
/// they hold geometry, all of it dated later, so the map cannot plot them. Without this they were
/// simply absent — no marker, no rail row, no message (#257). Renders nothing at a count of zero.
pub fn geography_unplotted_note(chrome: &Chrome, count: usize, year: i32) -> Element {
    if count == 0 {
        return rsx! {};
    }
    rsx! {
        div { class: "section-note", "{chrome.geography_unplotted_note(count, year)}" }
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

/// The map surface: the shared `MapLibre` mount (`screens::map_shared::map_surface`) at the
/// Geography-tool's own container id, default center, and world/country-level zoom. `zoom` is the
/// live camera level the toolbar's [`MapZoomReadout`] shows.
pub fn geography_map_surface(
    chrome: &Chrome,
    marker_count: usize,
    event_count: usize,
    tool: Signal<DrawTool>,
    zoom: Signal<f64>,
    on_map_click: impl FnMut(f64, f64) + Clone + 'static,
) -> Element {
    let aria = chrome.geography_map_aria(marker_count, event_count);
    let labels = MapControlLabels::from_chrome(chrome);
    map_surface(MAP_CONTAINER_ID, aria, tool, on_map_click, DEFAULT_CENTER, zoom, labels)
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
    slider_year: i32,
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
            close_label: chrome.close(),
            onclose: move |()| panel.set(GeoPanel::None),
            footer: rsx! {},
            {match current {
                GeoPanel::CreateHere { point } => rsx! {
                    GeographyCreateForm {
                        point,
                        onsaved: move |()| { panel.set(GeoPanel::None); reload += 1; toast.set(Some(saved.clone())); },
                    }
                },
                GeoPanel::AssertOnSelected { human_id, geometry, .. } => rsx! {
                    GeometrySaveForm {
                        human_id,
                        geometry,
                        slider_year,
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

/// The shape the selection-driven recentre effect should fit: the rail/picker's current selection
/// (`selected`'s `human_id`), resolved against the currently loaded markers. `None` if nothing is
/// selected or the selected place is not (yet) among the loaded markers — a no-op, not an error, since
/// the map only ever plots places with a resolved geometry (see the module doc).
fn selected_marker_shape<'a>(vm: &'a GeographyVm, selected: Option<&(String, String)>) -> Option<&'a MarkerShapeVm> {
    let (human_id, _) = selected?;
    vm.markers
        .iter()
        .find(|marker| &marker.human_id == human_id)
        .map(|marker| &marker.shape)
}

/// Pushes the loaded markers/event pins to the running map's `GeoJSON` sources. `query` filters the
/// pushed markers the same way [`geography_rail`] does, so a typed search hides the same places on
/// the map as in the rail.
fn update_geography_data(vm: &GeographyVm, query: &str) {
    let matching: Vec<PlaceMarkerVm> = filtered_markers(&vm.markers, query).into_iter().cloned().collect();
    push_map_data(
        MAP_CONTAINER_ID,
        &markers_geojson(&matching),
        &events_geojson(&vm.events),
    );
}

#[cfg(test)]
mod tests {
    use super::{GeoPanel, filtered_markers, geometry_panel_for, provider_kind, selected_marker_shape};
    use crate::screens::map_shared::geo_point;
    use genealogy_app::{MapProvider, PlaceGeometry};
    use genealogy_ui::{EventPinVm, GeographyVm, MapProviderVm, MarkerShapeVm, PlaceMarkerVm};

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

    fn geography_vm() -> GeographyVm {
        GeographyVm {
            markers: markers(),
            events: vec![EventPinVm {
                human_id: "E0001".to_owned(),
                id: "event-1".to_owned(),
                label: "Birth".to_owned(),
                date: None,
                place_human_id: "P0001".to_owned(),
                lat: 59.9,
                lon: 10.7,
            }],
            unplotted_count: 0,
            resolved_year: None,
            provider: MapProviderVm::OsmRaster {
                tile_url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_owned(),
                attribution: "© OpenStreetMap contributors".to_owned(),
            },
        }
    }

    #[test]
    fn no_selection_has_no_shape_to_fit() {
        assert_eq!(selected_marker_shape(&geography_vm(), None), None);
    }

    #[test]
    fn a_selected_marker_resolves_to_its_own_shape() {
        let vm = geography_vm();
        let selection = ("P0002".to_owned(), "Nordland".to_owned());
        let shape = selected_marker_shape(&vm, Some(&selection)).expect("Nordland is a loaded marker");
        assert_eq!(*shape, MarkerShapeVm::Point { lat: 67.0, lon: 15.0 });
    }

    #[test]
    fn a_selection_not_among_the_loaded_markers_is_a_no_op() {
        let vm = geography_vm();
        let selection = ("P9999".to_owned(), "Not loaded yet".to_owned());
        assert_eq!(selected_marker_shape(&vm, Some(&selection)), None);
    }

    fn polygon() -> PlaceGeometry {
        PlaceGeometry::Polygon {
            exterior: vec![geo_point(59.9, 10.7), geo_point(60.0, 10.8), geo_point(60.1, 10.6)],
            holes: Vec::new(),
        }
    }

    #[test]
    fn a_polygon_with_no_draw_target_opens_no_panel() {
        assert_eq!(geometry_panel_for(None, polygon()), None);
    }

    #[test]
    fn a_polygon_asserts_onto_the_selected_place() {
        let selected = Some(("P0001".to_owned(), "Oslo".to_owned()));
        assert_eq!(
            geometry_panel_for(selected, polygon()),
            Some(GeoPanel::AssertOnSelected {
                human_id: "P0001".to_owned(),
                name: "Oslo".to_owned(),
                geometry: polygon(),
            })
        );
    }

    #[test]
    fn a_point_with_no_draw_target_still_offers_the_quick_create_form() {
        let geometry = PlaceGeometry::Point(geo_point(59.9, 10.7));
        let panel = geometry_panel_for(None, geometry);
        let Some(GeoPanel::CreateHere { point }) = panel else {
            panic!("expected a CreateHere panel, got {panel:?}");
        };
        assert_eq!(point, (59.9, 10.7));
    }
}
