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
/// [`Self::Polygon`] make the click-capture overlay intercept pointer events — [`Self::Pan`] lets
/// them fall through to `MapLibre`'s own pan/zoom gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTool {
    /// No editing; `MapLibre`'s native pan/zoom.
    Pan,
    /// Click to drop or move a single point.
    Point,
    /// Click to append a polygon vertex; the caller's "Finish polygon" commits the ring.
    Polygon,
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

/// The generic `MapLibre` mount surface (draw-tool pointer-capture overlay + attribution placeholder),
/// shared by the Geography tool (whole-atlas view) and the Place screen's per-place Map tab.
/// `container_id` distinguishes the two DOM mounts (each is its own `MapLibre` instance) so both can
/// coexist if ever shown together.
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
                class: "map-container",
                style: "position:absolute;inset:0",
                onmounted: move |_| mount_maplibre(container_id, center, zoom, on_map_click.clone()),
            }
            if capturing {
                div { class: "geo-capture", style: "position:absolute;inset:0", "data-armed": "true" }
            }
            div { class: "map-attr", "" }
        }
    }
}

/// Mounts `MapLibre` on `container_id` (a no-op under SSR, where there is no webview to run the
/// script) and arms the persistent click listener, streaming every click as a `[lng, lat]` payload
/// over `dioxus.send`, read in a loop for the surface's lifetime — not a one-shot eval per click, so
/// the map stays interactive without a Rust round trip blocking each gesture.
pub fn mount_maplibre(container_id: &str, center: (f64, f64), zoom: f64, mut on_click: impl FnMut(f64, f64) + 'static) {
    let mut listener = document::eval(&maplibre_init_script(container_id, center, zoom));
    spawn(async move {
        while let Ok(payload) = listener.recv::<String>().await {
            if let Ok(click) = serde_json::from_str::<[f64; 2]>(&payload) {
                on_click(click[1], click[0]);
            }
        }
    });
}

/// The `MapLibre` bootstrap script for one container: creates the map (guarded against a re-render
/// remount), adds the marker/event/draft `GeoJSON` sources + layers once loaded, and arms the click
/// listener. Source/layer ids are scoped to each map instance, so two containers on the page never
/// collide.
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
        lat = center.0,
        lon = center.1,
    )
}

/// Pushes marker/event `GeoJSON` to the running map's sources, guarded so a reload that races the
/// map's own async `load` event simply skips (the next data/effect re-run catches up).
pub fn push_map_data(container_id: &str, markers_json: &Value, events_json: &Value) {
    let script = format!(
        r"
        const map = document.getElementById('{container_id}')?.__geoMap;
        if (map) {{
            const markers = map.getSource('geo-markers');
            if (markers) markers.setData({markers_json});
            const events = map.getSource('geo-events');
            if (events) events.setData({events_json});
        }}
        ",
    );
    run_map_script(&script);
}

/// Pushes the in-progress draft overlay (a dropped point or the polygon vertices so far) to the
/// running map, guarded the same way as [`push_map_data`].
pub fn push_map_draft(container_id: &str, draft: &MapDraft) {
    let geojson = draft_geojson(draft);
    let script = format!(
        r"
        const map = document.getElementById('{container_id}')?.__geoMap;
        if (map) {{
            const draft = map.getSource('geo-draft');
            if (draft) draft.setData({geojson});
        }}
        ",
    );
    run_map_script(&script);
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

/// The assert-geometry form (ADR 0025 §2): the drafted shape plus the standard reason/confidence
/// provenance block, dispatched via [`PlaceEdit::AssertGeometry`] — the same audited path a
/// typed-field edit uses. Shared by the Geography tool's "assert onto selected place" panel and the
/// Place screen's own Map-tab save-geometry card (Phase 9); only `year` differs (each screen binds
/// its own time slider — `None` for an undated/primary assertion).
#[component]
pub fn GeometrySaveForm(
    human_id: String,
    geometry: PlaceGeometry,
    year: Option<i32>,
    onsaved: EventHandler<()>,
) -> Element {
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
                let edit = PlaceEdit::AssertGeometry { human_id: human_id.clone(), geometry: geometry.clone(), year };
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

#[cfg(test)]
mod tests {
    use super::{
        MapDraft, closed_ring, combined_bounds, draft_geojson, empty_feature_collection, events_geojson,
        markers_geojson, shape_to_draft,
    };
    use genealogy_ui::{EventPinVm, MarkerShapeVm, PlaceMarkerVm};

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
