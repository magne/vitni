//! `MapLibre` GL JS machinery shared by the Geography tool (`screens::geography`, ADR 0025) and the
//! Place screen's own Map tab (`screens::place`, Phase 9 per-place geometry editor, ADR 0024/0026):
//! the draw-tool/draft state, the `GeoJSON` conversions, the mount/update scripts (parameterized by
//! DOM container id so two mounts can coexist), the generic map surface component, and the
//! assert-geometry save form. Both screens dispatch through the identical audited
//! [`PlaceEdit::AssertGeometry`] path; only the container id/center/zoom and the caller's own
//! toolbar/rail differ.

use genealogy_app::{GeoCoordinates, Microdegrees, PlaceGeometry};
use genealogy_ui::{EventPinVm, MarkerShapeVm, PlaceMarkerVm};
use serde_json::{Value, json};
use std::str::FromStr;

use super::prelude::*;

/// The default map view when nothing else pins a center (Oslo, a reasonable Norwegian default
/// matching every other Norway-flavoured example in this codebase's docs/fixtures).
pub const DEFAULT_CENTER: (f64, f64) = (59.9139, 10.7522);

/// The active draw tool on a map surface (the mockup's toolbar). Only [`Self::Point`] and
/// [`Self::Polygon`] switch the map container to a crosshair cursor (the `is-capturing` CSS class) —
/// [`Self::Pan`] leaves `MapLibre`'s own pan/zoom cursor alone. Neither variant blocks pointer events;
/// every click always reaches `MapLibre`'s own `map.on('click', …)` listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTool {
    /// No editing; `MapLibre`'s native pan/zoom.
    Pan,
    /// Click to drop or move a single point.
    Point,
    /// Click to append a polygon vertex; the caller's "Finish polygon" commits the ring.
    Polygon,
}

/// Arms `next` on a map surface's toolbar. [`Signal::set`] has no equality check of its own, so
/// clicking the tool that is already active would otherwise re-render the whole screen for nothing.
pub fn select_tool(mut tool: Signal<DrawTool>, next: DrawTool) {
    if tool() != next {
        tool.set(next);
    }
}

/// The in-progress shape being drawn, before it is committed to a [`PlaceGeometry`] and handed to a
/// save form.
#[derive(Debug, Clone, PartialEq)]
pub enum MapDraft {
    /// Nothing drawn yet.
    Empty,
    /// A single dropped point, in `(lat, lon)`.
    Point((f64, f64)),
    /// The polygon's vertices so far, in click order, each `(lat, lon)`.
    Polygon(Vec<(f64, f64)>),
}

/// Builds a [`GeoCoordinates`] from decimal degrees (the click-stream/GeoJSON boundary), rounding to
/// the microdegree precision the domain type stores.
#[must_use]
pub fn geo_point(lat: f64, lon: f64) -> GeoCoordinates {
    GeoCoordinates {
        latitude: Microdegrees::from_str(&format!("{lat:.6}")).unwrap_or(Microdegrees::from_microdegrees(0)),
        longitude: Microdegrees::from_str(&format!("{lon:.6}")).unwrap_or(Microdegrees::from_microdegrees(0)),
    }
}

/// The generic `MapLibre` mount surface (draw-tool crosshair cursor + attribution placeholder), shared
/// by the Geography tool (whole-atlas view) and the Place screen's per-place Map tab. `container_id`
/// distinguishes the two DOM mounts (each is its own `MapLibre` instance) so both can coexist if ever
/// shown together.
#[must_use = "renders the map surface; drop it and nothing is shown"]
pub fn map_surface(
    container_id: &'static str,
    aria_label: String,
    tool: Signal<DrawTool>,
    on_map_click: impl FnMut(f64, f64) + Clone + 'static,
    center: (f64, f64),
    zoom: f64,
) -> Element {
    let capturing = !matches!(tool(), DrawTool::Pan);
    rsx! {
        div {
            class: "map-surface",
            role: "img",
            aria_label,
            div {
                id: container_id,
                class: if capturing { "map-container is-capturing" } else { "map-container" },
                style: "position:absolute;inset:0",
                "data-armed": if capturing { "true" } else { "false" },
                onmounted: move |_| mount_maplibre(container_id, center, zoom, on_map_click.clone()),
            }
            div { class: "map-attr", "" }
        }
    }
}

/// One message the mounted map sends back over its single `dioxus.send` channel. Both emitters live
/// inside the init script's own `if (el && !el.__geoMap …)` guard, so they cannot diverge from the map
/// or from each other.
///
/// This is an **ephemeral webview transport**, not the ADR 0002/0004 event encoding: nothing here is
/// ever persisted, so its shape carries no compatibility obligation and an unrecognized payload is
/// simply dropped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapMessage {
    /// The operator clicked the canvas, in `(lat, lon)` decimal degrees.
    Click {
        /// Latitude in decimal degrees.
        lat: f64,
        /// Longitude in decimal degrees.
        lon: f64,
    },
    /// The camera settled at this zoom level (`zoomend`, plus one measurement at `load`).
    Zoom(f64),
}

/// Parses one `dioxus.send` payload into a [`MapMessage`], or `None` for anything this build does not
/// recognize. Hand-parsed off [`Value`] rather than derived — this crate has `serde_json` but no
/// `serde`, and a two-variant transport does not justify the dependency.
#[must_use]
pub fn parse_map_message(payload: &str) -> Option<MapMessage> {
    let value: Value = serde_json::from_str(payload).ok()?;
    if let Some(click) = value.get("click").and_then(Value::as_array) {
        let [lng, lat] = click.as_slice() else { return None };
        // MapLibre reports `[lng, lat]`; every view-model here is `(lat, lon)`.
        return Some(MapMessage::Click {
            lat: lat.as_f64()?,
            lon: lng.as_f64()?,
        });
    }
    if let Some(zoom) = value.get("zoom") {
        return Some(MapMessage::Zoom(zoom.as_f64()?));
    }
    None
}

/// Mounts `MapLibre` on `container_id` (a no-op under SSR, where there is no webview to run the
/// script) and arms the persistent message listener, streaming every click as a tagged payload over
/// `dioxus.send`, read in a loop for the surface's lifetime — not a one-shot eval per click, so the
/// map stays interactive without a Rust round trip blocking each gesture.
pub fn mount_maplibre(container_id: &str, center: (f64, f64), zoom: f64, mut on_click: impl FnMut(f64, f64) + 'static) {
    let mut listener = document::eval(&maplibre_init_script(container_id, center, zoom));
    spawn(async move {
        while let Ok(payload) = listener.recv::<String>().await {
            match parse_map_message(&payload) {
                Some(MapMessage::Click { lat, lon }) => on_click(lat, lon),
                Some(MapMessage::Zoom(_)) | None => {}
            }
        }
    });
}

/// The stroke every circle layer draws around its fill, so a marker stays legible over a dark tile and
/// over another marker beneath it (per the "markers too small to see" fix).
const CIRCLE_STROKE_COLOR: &str = "#ffffff";

/// `(zoom, radius-in-px)` stops for a place marker or a draft point: a fixed radius is either an
/// invisible dot at atlas zoom or a blob at street zoom, so the radius ramps with zoom instead.
const POINT_RADIUS_STOPS: [(u32, u32); 4] = [(0, 7), (8, 9), (14, 12), (20, 16)];

/// The same ramp for event pins, a step smaller throughout — they are plotted over the place markers.
const EVENT_RADIUS_STOPS: [(u32, u32); 4] = [(0, 5), (8, 6), (14, 8), (20, 11)];

/// The `paint` block for one circle layer: `color` fill, a `circle-radius` interpolated linearly across
/// `radius_stops`, and a `stroke_width`-wide [`CIRCLE_STROKE_COLOR`] outline. Interpolated into
/// [`maplibre_init_script`] as JSON, which is valid JS object syntax.
fn circle_paint(color: &str, radius_stops: [(u32, u32); 4], stroke_width: u32) -> Value {
    let mut radius: Vec<Value> = vec![json!("interpolate"), json!(["linear"]), json!(["zoom"])];
    for (zoom, pixels) in radius_stops {
        radius.push(json!(zoom));
        radius.push(json!(pixels));
    }
    json!({
        "circle-color": color,
        "circle-radius": radius,
        "circle-stroke-width": stroke_width,
        "circle-stroke-color": CIRCLE_STROKE_COLOR,
    })
}

/// The `MapLibre` bootstrap script for one container: creates the map (guarded against a re-render
/// remount), adds the marker/event/draft `GeoJSON` sources + layers once loaded, applies any data
/// already stashed by [`push_map_data`]/[`push_map_draft`] before the sources existed (`el.__geoPending`
/// — otherwise a push that races this async `load` event is silently dropped, and nothing ever
/// re-applies it, per the "Place map shows no marker" bug), and arms the click listener. Source/layer
/// ids are scoped to each map instance, so two containers on the page never collide.
///
/// It also arms a repaint observer, which is what keeps the canvas visible across a layout change
/// (#252). Arming a draw tool inserts a Finish/Clear row under the map, shrinking `.map-surface`;
/// `MapLibre`'s own `trackResize` observer does re-measure and resize the canvas correctly, but under
/// `WebKitGTK` the frame it draws never reaches the compositor, so the map went blank until the next
/// camera move. Forcing one more `redraw()` on the animation frame after the resize does composite.
/// `redraw()` changes no layout, so the observer cannot re-trigger itself.
fn maplibre_init_script(container_id: &str, center: (f64, f64), zoom: f64) -> String {
    format!(
        r"
        const el = document.getElementById('{container_id}');
        if (el && !el.__geoMap && window.maplibregl) {{
            const map = new maplibregl.Map({{
                container: el,
                style: {{ version: 8, sources: {{}}, layers: [] }},
                center: [{lon}, {lat}],
                zoom: {zoom},
                attributionControl: false,
            }});
            el.__geoMap = map;
            map.on('load', () => {{
                map.addSource('geo-tiles', {{ type: 'raster', tiles: ['https://tile.openstreetmap.org/{{z}}/{{x}}/{{y}}.png'], tileSize: 256 }});
                map.addLayer({{ id: 'geo-tile-layer', type: 'raster', source: 'geo-tiles' }});
                map.addSource('geo-markers', {{ type: 'geojson', data: {{ type: 'FeatureCollection', features: [] }} }});
                map.addLayer({{ id: 'geo-marker-fill', type: 'fill', source: 'geo-markers', filter: ['==', ['geometry-type'], 'Polygon'], paint: {{ 'fill-color': '#5db3ff', 'fill-opacity': 0.25 }} }});
                map.addLayer({{ id: 'geo-marker-line', type: 'line', source: 'geo-markers', filter: ['==', ['geometry-type'], 'Polygon'], paint: {{ 'line-color': '#5db3ff', 'line-width': 2 }} }});
                map.addLayer({{
                    id: 'geo-marker-point', type: 'circle', source: 'geo-markers', filter: ['==', ['geometry-type'], 'Point'],
                    paint: {marker_paint},
                }});
                map.addSource('geo-events', {{ type: 'geojson', data: {{ type: 'FeatureCollection', features: [] }} }});
                map.addLayer({{
                    id: 'geo-event-point', type: 'circle', source: 'geo-events',
                    paint: {event_paint},
                }});
                map.addSource('geo-draft', {{ type: 'geojson', data: {{ type: 'FeatureCollection', features: [] }} }});
                map.addLayer({{ id: 'geo-draft-fill', type: 'fill', source: 'geo-draft', filter: ['==', ['geometry-type'], 'Polygon'], paint: {{ 'fill-color': '#ff5d5d', 'fill-opacity': 0.2 }} }});
                map.addLayer({{ id: 'geo-draft-line', type: 'line', source: 'geo-draft', paint: {{ 'line-color': '#ff5d5d', 'line-width': 2 }} }});
                map.addLayer({{
                    id: 'geo-draft-point', type: 'circle', source: 'geo-draft', filter: ['==', ['geometry-type'], 'Point'],
                    paint: {draft_paint},
                }});
                const pending = el.__geoPending;
                if (pending) {{
                    if (pending.markers) map.getSource('geo-markers').setData(pending.markers);
                    if (pending.events) map.getSource('geo-events').setData(pending.events);
                    if (pending.draft) map.getSource('geo-draft').setData(pending.draft);
                }}
            }});
            map.on('click', (e) => {{ dioxus.send(JSON.stringify({{ click: [e.lngLat.lng, e.lngLat.lat] }})); }});
            new ResizeObserver(() => {{ requestAnimationFrame(() => el.__geoMap && el.__geoMap.redraw()); }}).observe(el);
        }}
        ",
        lat = center.0,
        lon = center.1,
        marker_paint = circle_paint("#5db3ff", POINT_RADIUS_STOPS, 2),
        event_paint = circle_paint("#ffb020", EVENT_RADIUS_STOPS, 1),
        draft_paint = circle_paint("#ff5d5d", POINT_RADIUS_STOPS, 2),
    )
}

/// Pushes marker/event `GeoJSON` to the running map's sources. Always stashes the data on
/// `el.__geoPending` first, then applies it immediately if the sources already exist — so a push that
/// races the map's own async `load` event (the mount effect firing before `load` adds the sources, per
/// the "Place map shows no marker" bug) is not lost: the init script's `load` handler re-applies
/// whatever is stashed once the sources exist.
pub fn push_map_data(container_id: &str, markers_json: &Value, events_json: &Value) {
    run_map_script(&push_data_script(container_id, markers_json, events_json));
}

/// The marker/event push script: the stash on `el.__geoPending` comes first and is unconditional, so it
/// happens whether or not the map has finished loading; only the immediate `setData` is guarded.
fn push_data_script(container_id: &str, markers_json: &Value, events_json: &Value) -> String {
    format!(
        r"
        const el = document.getElementById('{container_id}');
        if (el) {{
            el.__geoPending = Object.assign({{}}, el.__geoPending, {{ markers: {markers_json}, events: {events_json} }});
            const map = el.__geoMap;
            if (map) {{
                const markers = map.getSource('geo-markers');
                if (markers) markers.setData({markers_json});
                const events = map.getSource('geo-events');
                if (events) events.setData({events_json});
            }}
        }}
        ",
    )
}

/// Pushes the in-progress draft overlay (a dropped point or the polygon vertices so far) to the
/// running map, stashed/applied the same load-race-proof way as [`push_map_data`].
pub fn push_map_draft(container_id: &str, draft: &MapDraft) {
    run_map_script(&push_draft_script(container_id, &draft_geojson(draft)));
}

/// The draft push script, stashing before it consults the map for the same reason [`push_data_script`]
/// does.
fn push_draft_script(container_id: &str, geojson: &Value) -> String {
    format!(
        r"
        const el = document.getElementById('{container_id}');
        if (el) {{
            el.__geoPending = Object.assign({{}}, el.__geoPending, {{ draft: {geojson} }});
            const map = el.__geoMap;
            if (map) {{
                const draft = map.getSource('geo-draft');
                if (draft) draft.setData({geojson});
            }}
        }}
        ",
    )
}

/// Runs a fire-and-forget script against a mounted map (a no-op under SSR).
fn run_map_script(script: &str) {
    let mut eval = document::eval(script);
    spawn(async move {
        let _ = eval.recv::<()>().await;
    });
}

/// Converts place markers to a `GeoJSON` `FeatureCollection`, `[lon, lat]` order.
#[must_use]
pub fn markers_geojson(markers: &[PlaceMarkerVm]) -> Value {
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

/// Converts event pins to a `GeoJSON` `FeatureCollection` of points.
#[must_use]
pub fn events_geojson(events: &[EventPinVm]) -> Value {
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

/// An empty `GeoJSON` `FeatureCollection` (a draft with nothing drawn yet, or a Map tab with no
/// event pins to plot).
#[must_use]
pub fn empty_feature_collection() -> Value {
    json!({ "type": "FeatureCollection", "features": [] })
}

/// Converts a stored [`PlaceGeometry`] shape back to a [`MapDraft`] (the geometry-over-time table's
/// per-row "Edit" loading a saved assertion's vertices back into the draw state, ADR 0024/0026): a
/// polygon's holes are dropped — the draw tools have no hole-editing affordance, so re-saving after an
/// edit would drop them anyway (`PlaceMapEditor`'s own `on_finish_polygon` never builds one either).
#[must_use]
pub fn shape_to_draft(shape: &MarkerShapeVm) -> MapDraft {
    match shape {
        MarkerShapeVm::Point { lat, lon } => MapDraft::Point((*lat, *lon)),
        MarkerShapeVm::Polygon { exterior, .. } => MapDraft::Polygon(exterior.clone()),
    }
}

/// A shape's `(lat, lon)` bounding box, for [`fit_bounds`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    min_lat: f64,
    max_lat: f64,
    min_lon: f64,
    max_lon: f64,
}

/// Every vertex a shape carries: a point's own coordinate, or a polygon's exterior ring plus holes.
fn shape_vertices(shape: &MarkerShapeVm) -> Vec<(f64, f64)> {
    match shape {
        MarkerShapeVm::Point { lat, lon } => vec![(*lat, *lon)],
        MarkerShapeVm::Polygon { exterior, holes } => {
            let mut vertices = exterior.clone();
            for hole in holes {
                vertices.extend(hole.iter().copied());
            }
            vertices
        }
    }
}

/// The combined bounding box of every vertex across `shapes`, or `None` if `shapes` is empty.
fn combined_bounds(shapes: &[MarkerShapeVm]) -> Option<Bounds> {
    let mut bounds: Option<Bounds> = None;
    for shape in shapes {
        for (lat, lon) in shape_vertices(shape) {
            bounds = Some(match bounds {
                None => Bounds {
                    min_lat: lat,
                    max_lat: lat,
                    min_lon: lon,
                    max_lon: lon,
                },
                Some(mut current) => {
                    current.min_lat = current.min_lat.min(lat);
                    current.max_lat = current.max_lat.max(lat);
                    current.min_lon = current.min_lon.min(lon);
                    current.max_lon = current.max_lon.max(lon);
                    current
                }
            });
        }
    }
    bounds
}

/// Zooms/pans the map at `container_id` to fit the combined bounding box of `shapes` (a no-op under
/// SSR, or when `shapes` is empty) — the "⤢ Fit" toolbar button on both the Place Map tab (its own
/// single resolved shape) and the Geography atlas (every currently filtered marker's shape).
pub fn fit_bounds(container_id: &str, shapes: &[MarkerShapeVm]) {
    let Some(bounds) = combined_bounds(shapes) else {
        return;
    };
    let script = format!(
        r"
        const map = document.getElementById('{container_id}')?.__geoMap;
        if (map) {{
            map.fitBounds([[{min_lon}, {min_lat}], [{max_lon}, {max_lat}]], {{ padding: 40, maxZoom: 15, duration: 300 }});
        }}
        ",
        min_lon = bounds.min_lon,
        min_lat = bounds.min_lat,
        max_lon = bounds.max_lon,
        max_lat = bounds.max_lat,
    );
    run_map_script(&script);
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
/// a rendering concern only; a saved [`PlaceGeometry::Polygon`] never duplicates the closing point.
#[expect(
    clippy::float_cmp,
    reason = "exact identity check: comparing a ring's last point to its literal first element, not two independently computed floats"
)]
pub fn closed_ring(points: &[(f64, f64)]) -> Vec<[f64; 2]> {
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
#[must_use]
pub fn draft_geojson(draft: &MapDraft) -> Value {
    match draft {
        MapDraft::Empty => empty_feature_collection(),
        MapDraft::Point((lat, lon)) => json!({
            "type": "FeatureCollection",
            "features": [{ "type": "Feature", "geometry": { "type": "Point", "coordinates": [lon, lat] }, "properties": {} }],
        }),
        MapDraft::Polygon(vertices) if vertices.len() >= 3 => json!({
            "type": "FeatureCollection",
            "features": [{ "type": "Feature", "geometry": { "type": "Polygon", "coordinates": [closed_ring(vertices)] }, "properties": {} }],
        }),
        MapDraft::Polygon(vertices) => json!({
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "geometry": { "type": "LineString", "coordinates": vertices.iter().map(|&(lat, lon)| [lon, lat]).collect::<Vec<_>>() },
                "properties": {},
            }],
        }),
    }
}

/// The `RadioGroup` choice id of an undated (primary) assertion — the default.
const EFFECTIVE_UNDATED: &str = "undated";

/// The `RadioGroup` choice id of an assertion dated to the caller's slider year.
const EFFECTIVE_DATED: &str = "dated";

/// The year an assert-geometry save stamps: the slider year when the operator picked a dated
/// assertion, `None` (undated/primary) otherwise. An undated assertion is ADR 0026 §1's fallback, so
/// it resolves at every year — which is why it is the default and a saved shape never vanishes.
#[must_use]
pub fn save_year(dated: bool, slider_year: i32) -> Option<i32> {
    dated.then_some(slider_year)
}

/// The assert-geometry form (ADR 0025 §2): the drafted shape, the dated/undated effective-date
/// choice, and the standard reason/confidence provenance block, dispatched via
/// [`PlaceEdit::AssertGeometry`] — the same audited path a typed-field edit uses. Shared by the
/// Geography tool's "assert onto selected place" panel and the Place screen's own Map-tab
/// save-geometry card (Phase 9).
///
/// `slider_year` is only the year the *dated* choice would stamp; the caller passes its own time
/// slider's year and cannot express a dating policy of its own, so both panels stamp dates the same
/// way (#257 — the Place tab used to hardcode its slider year while Geography hardcoded undated,
/// and a point saved from one vanished from the other).
#[component]
pub fn GeometrySaveForm(
    human_id: String,
    geometry: PlaceGeometry,
    slider_year: i32,
    onsaved: EventHandler<()>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let prov = use_signal(ProvenanceDraft::default);
    let mut dated = use_signal(|| false);
    rsx! {
        div { class: "faint", style: "font-size:var(--fs-xs)", "{loc.place_map_scope_note()}" }
        {effective_date_choice(loc, slider_year, dated(), move |picked: String| dated.set(picked == EFFECTIVE_DATED))}
        {provenance_block(loc, prov)}
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let edit = PlaceEdit::AssertGeometry {
                    human_id: human_id.clone(),
                    geometry: geometry.clone(),
                    year: save_year(dated(), slider_year),
                };
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

/// The effective-date choice: both options visible, undated first and preselected. Split out of
/// [`GeometrySaveForm`] (which needs `AppCtx`) so an SSR test can render it over a plain localizer.
#[must_use = "renders the effective-date choice; drop it and the form loses its dating control"]
pub fn effective_date_choice(
    loc: &Localizer,
    slider_year: i32,
    dated: bool,
    mut onselect: impl FnMut(String) + 'static,
) -> Element {
    let choices = vec![
        RadioChoice {
            id: EFFECTIVE_UNDATED.to_owned(),
            label: loc.place_geometry_effective_undated(),
        },
        RadioChoice {
            id: EFFECTIVE_DATED.to_owned(),
            label: loc.place_geometry_effective_dated(slider_year),
        },
    ];
    let selected = if dated { EFFECTIVE_DATED } else { EFFECTIVE_UNDATED };
    rsx! {
        div { class: "field",
            label { class: "lbl", "{loc.place_geometry_effective_label()}" }
            RadioGroup {
                group_label: loc.place_geometry_effective_label(),
                choices,
                selected: selected.to_owned(),
                onselect: move |id: String| onselect(id),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MapDraft, MapMessage, closed_ring, combined_bounds, draft_geojson, empty_feature_collection, events_geojson,
        maplibre_init_script, markers_geojson, parse_map_message, push_data_script, push_draft_script, save_year,
        shape_to_draft,
    };
    use genealogy_ui::{EventPinVm, MarkerShapeVm, PlaceMarkerVm};
    use serde_json::{Value, json};

    /// The paint each circle layer is expected to emit, written out here independently of the code that
    /// builds it: a zoom-interpolated `circle-radius` (a fixed radius leaves a marker an invisible dot
    /// at atlas zoom, per the "markers too small to see" fix) and a white stroke around the fill.
    fn expected_circle_paints() -> [(&'static str, Value); 3] {
        [
            (
                "geo-marker-point",
                json!({
                    "circle-color": "#5db3ff",
                    "circle-radius": ["interpolate", ["linear"], ["zoom"], 0, 7, 8, 9, 14, 12, 20, 16],
                    "circle-stroke-width": 2,
                    "circle-stroke-color": "#ffffff",
                }),
            ),
            (
                "geo-event-point",
                json!({
                    "circle-color": "#ffb020",
                    "circle-radius": ["interpolate", ["linear"], ["zoom"], 0, 5, 8, 6, 14, 8, 20, 11],
                    "circle-stroke-width": 1,
                    "circle-stroke-color": "#ffffff",
                }),
            ),
            (
                "geo-draft-point",
                json!({
                    "circle-color": "#ff5d5d",
                    "circle-radius": ["interpolate", ["linear"], ["zoom"], 0, 7, 8, 9, 14, 12, 20, 16],
                    "circle-stroke-width": 2,
                    "circle-stroke-color": "#ffffff",
                }),
            ),
        ]
    }

    /// `GeoJSON`/`MapLibre` order coordinates `[lng, lat]` while every view-model here is `(lat, lon)`,
    /// so the swap on the way in is the easiest thing in this module to get backwards.
    #[test]
    fn a_click_payload_swaps_the_webviews_lng_lat_order_into_lat_lon() {
        assert_eq!(
            parse_map_message(r#"{"click":[10.7,59.9]}"#),
            Some(MapMessage::Click { lat: 59.9, lon: 10.7 })
        );
    }

    #[test]
    fn a_zoom_payload_carries_the_measured_level() {
        assert_eq!(parse_map_message(r#"{"zoom":14.23}"#), Some(MapMessage::Zoom(14.23)));
    }

    #[test]
    fn an_untagged_or_unknown_payload_is_ignored_rather_than_guessed_at() {
        for payload in [
            "[]",
            "[10.7,59.9]",
            r#"{"bearing":90}"#,
            r#"{"click":[10.7]}"#,
            r#"{"zoom":"14.2"}"#,
            "not json at all",
        ] {
            assert_eq!(parse_map_message(payload), None, "{payload} names no known message");
        }
    }

    /// The channel carries more than one kind of message, so the click emitter must tag itself —
    /// an untagged `[lng, lat]` array is exactly what [`parse_map_message`] now refuses.
    #[test]
    fn the_click_emitter_tags_its_payload_so_the_channel_can_carry_more_than_clicks() {
        let script = maplibre_init_script("geo-map", (59.9, 10.7), 5.0);
        assert!(
            script.contains("dioxus.send(JSON.stringify({ click: [e.lngLat.lng, e.lngLat.lat] }))"),
            "the click rides the shared channel under its own key:\n{script}"
        );
    }

    #[test]
    fn every_circle_layer_paints_a_zoom_interpolated_radius_with_a_white_stroke() {
        let script = maplibre_init_script("geo-map", (59.9, 10.7), 5.0);
        for (layer, paint) in expected_circle_paints() {
            assert!(
                script.contains(&format!("id: '{layer}', type: 'circle'")),
                "the {layer} circle layer is still added:\n{script}"
            );
            assert!(
                script.contains(&paint.to_string()),
                "{layer} paints {paint} — a scalar radius or a missing stroke is a regression:\n{script}"
            );
        }
    }

    /// `MapLibre`'s own `trackResize` observer resizes the canvas correctly when the surface shrinks,
    /// but under `WebKitGTK` the frame it draws never composites and the map goes blank until the next
    /// camera move (#252). One more `redraw()` on the animation frame after the resize does composite.
    #[test]
    fn a_container_resize_forces_a_repaint_so_arming_a_draw_tool_cannot_blank_the_map() {
        let script = maplibre_init_script("geo-map", (59.9, 10.7), 5.0);
        let observer = script
            .find("new ResizeObserver(")
            .expect("the container is watched for the layout changes a draw tool causes");
        assert!(
            script[observer..].contains("requestAnimationFrame(() => el.__geoMap && el.__geoMap.redraw())"),
            "the repaint is deferred to the next animation frame, after MapLibre's own resize has \
             run — redrawing inside the observer callback is the frame that does not composite:\n{script}"
        );
        assert!(
            script[observer..].contains(").observe(el)"),
            "the observer watches the map's own container:\n{script}"
        );
    }

    #[test]
    fn pushing_marker_and_event_data_stashes_it_before_the_map_is_consulted() {
        let markers = markers_geojson(&[]);
        let events = events_geojson(&[]);
        let script = push_data_script("geo-map", &markers, &events);
        let stash = script
            .find("el.__geoPending = Object.assign({}, el.__geoPending, ")
            .expect("the push stashes the data");
        assert!(
            script[stash..].contains(&format!("{{ markers: {markers}, events: {events} }}")),
            "both collections are stashed verbatim:\n{script}"
        );
        let map = script.find("el.__geoMap").expect("the push looks for a running map");
        assert!(
            stash < map,
            "the stash happens unconditionally, before the map is known to exist — a push that races \
             the map's async load must survive it:\n{script}"
        );
    }

    #[test]
    fn pushing_a_draft_stashes_it_before_the_map_is_consulted() {
        let draft = MapDraft::Point((59.9, 10.7));
        let geojson = draft_geojson(&draft);
        let script = push_draft_script("geo-map", &geojson);
        let stash = script
            .find("el.__geoPending = Object.assign({}, el.__geoPending, ")
            .expect("the push stashes the draft");
        assert!(
            script[stash..].contains(&format!("{{ draft: {geojson} }}")),
            "the draft is stashed verbatim:\n{script}"
        );
        let map = script.find("el.__geoMap").expect("the push looks for a running map");
        assert!(stash < map, "the stash happens before the map is consulted:\n{script}");
    }

    #[test]
    fn the_init_scripts_load_handler_reapplies_whatever_was_stashed() {
        let script = maplibre_init_script("geo-map", (59.9, 10.7), 5.0);
        let load = script.find("map.on('load'").expect("the load handler");
        let last_source = script
            .rfind("map.addSource('geo-draft'")
            .expect("the draft source is added");
        let reapply = script
            .find("const pending = el.__geoPending;")
            .expect("the load handler reads what was stashed — without this a push that raced load is lost");
        assert!(
            load < reapply && last_source < reapply,
            "the re-apply runs inside the load handler, after every source exists:\n{script}"
        );
        for (key, source) in [
            ("markers", "geo-markers"),
            ("events", "geo-events"),
            ("draft", "geo-draft"),
        ] {
            assert!(
                script[reapply..].contains(&format!(
                    "if (pending.{key}) map.getSource('{source}').setData(pending.{key});"
                )),
                "stashed {key} are re-applied to '{source}':\n{script}"
            );
        }
    }

    #[test]
    fn a_point_shape_becomes_a_geojson_point_in_lon_lat_order() {
        let markers = vec![PlaceMarkerVm {
            human_id: "P0001".to_owned(),
            id: "place-1".to_owned(),
            name: "Oslo".to_owned(),
            type_label: None,
            shape: MarkerShapeVm::Point { lat: 59.9, lon: 10.7 },
        }];
        let geojson = markers_geojson(&markers);
        assert_eq!(
            geojson["features"][0]["geometry"],
            serde_json::json!({ "type": "Point", "coordinates": [10.7, 59.9] })
        );
    }

    #[test]
    fn a_polygon_shape_closes_its_exterior_ring_for_rendering() {
        let markers = vec![PlaceMarkerVm {
            human_id: "P0002".to_owned(),
            id: "place-2".to_owned(),
            name: "Old County".to_owned(),
            type_label: None,
            shape: MarkerShapeVm::Polygon {
                exterior: vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)],
                holes: Vec::new(),
            },
        }];
        let geojson = markers_geojson(&markers);
        let ring = geojson["features"][0]["geometry"]["coordinates"][0]
            .as_array()
            .expect("a ring");
        assert_eq!(ring.len(), 4, "the ring is closed (first point repeated last)");
        assert_eq!(ring[0], ring[3]);
    }

    #[test]
    fn an_already_closed_ring_is_not_duplicated_again() {
        let ring = closed_ring(&[(60.0, 5.0), (61.0, 5.0), (61.0, 6.0), (60.0, 5.0)]);
        assert_eq!(ring.len(), 4);
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
        let geojson = draft_geojson(&MapDraft::Empty);
        assert_eq!(geojson, empty_feature_collection());
    }

    #[test]
    fn a_two_vertex_polygon_draft_previews_as_a_line() {
        let geojson = draft_geojson(&MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0)]));
        assert_eq!(geojson["features"][0]["geometry"]["type"], "LineString");
    }

    #[test]
    fn a_three_vertex_polygon_draft_previews_as_a_closed_polygon() {
        let geojson = draft_geojson(&MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)]));
        assert_eq!(geojson["features"][0]["geometry"]["type"], "Polygon");
    }

    #[test]
    fn a_point_shape_converts_to_a_point_draft() {
        let shape = MarkerShapeVm::Point { lat: 59.9, lon: 10.7 };
        assert_eq!(shape_to_draft(&shape), MapDraft::Point((59.9, 10.7)));
    }

    #[test]
    fn a_polygon_shape_converts_to_a_polygon_draft_dropping_its_holes() {
        let shape = MarkerShapeVm::Polygon {
            exterior: vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)],
            holes: vec![vec![(60.3, 5.3)]],
        };
        assert_eq!(
            shape_to_draft(&shape),
            MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)])
        );
    }

    #[test]
    fn the_default_undated_choice_stamps_no_year_so_the_shape_resolves_at_every_year() {
        assert_eq!(save_year(false, 1900), None);
    }

    #[test]
    fn the_dated_choice_stamps_the_slider_year() {
        assert_eq!(save_year(true, 1900), Some(1900));
        assert_eq!(save_year(true, 1600), Some(1600));
    }

    #[test]
    fn no_shapes_have_no_combined_bounds() {
        assert!(combined_bounds(&[]).is_none());
    }

    #[test]
    fn a_single_points_bounds_are_that_point_on_every_edge() {
        let shape = MarkerShapeVm::Point { lat: 59.9, lon: 10.7 };
        let bounds = combined_bounds(std::slice::from_ref(&shape)).expect("bounds");
        assert!((bounds.min_lat - 59.9).abs() < 1e-9);
        assert!((bounds.max_lat - 59.9).abs() < 1e-9);
        assert!((bounds.min_lon - 10.7).abs() < 1e-9);
        assert!((bounds.max_lon - 10.7).abs() < 1e-9);
    }

    #[test]
    fn combined_bounds_spans_every_shapes_vertices() {
        let shapes = vec![
            MarkerShapeVm::Point { lat: 59.9, lon: 10.7 },
            MarkerShapeVm::Polygon {
                exterior: vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)],
                holes: Vec::new(),
            },
        ];
        let bounds = combined_bounds(&shapes).expect("bounds");
        assert!((bounds.min_lat - 59.9).abs() < 1e-9);
        assert!((bounds.max_lat - 61.0).abs() < 1e-9);
        assert!((bounds.min_lon - 5.0).abs() < 1e-9);
        assert!((bounds.max_lon - 10.7).abs() < 1e-9);
    }
}
