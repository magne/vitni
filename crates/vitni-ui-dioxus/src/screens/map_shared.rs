//! `MapLibre` GL JS machinery shared by the Geography tool (`screens::geography`, ADR 0025) and the
//! Place screen's own Map tab (`screens::place`, Phase 9 per-place geometry editor, ADR 0024/0026):
//! the draw-tool/draft state, the `GeoJSON` conversions, the mount/update scripts (parameterized by
//! DOM container id so two mounts can coexist), the generic map surface component, and the
//! assert-geometry save form. Both screens dispatch through the identical audited
//! [`PlaceEdit::AssertGeometry`] path; only the container id/center/zoom and the caller's own
//! toolbar/rail differ.

use serde_json::{Value, json};
use std::str::FromStr;
use vitni_app::{GeoCoordinates, MapBasemap, MapSource, Microdegrees, PlaceGeometry};
use vitni_ui::{EventPinVm, MarkerShapeVm, PlaceMarkerVm, ZOOM_RANGE, clamp_zoom};

use super::prelude::*;
use crate::i18n::Chrome;
use crate::shell::ChromeCtx;

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

/// Which draft-action row a map screen offers for the armed tool and the current draft. Keyed off
/// `tool`, not `draft`, for `Polygon` — the Finish/Clear row is the polygon tool's own affordance and
/// stays offered even over an `Empty` draft (today's silent Finish no-op); only `Point` additionally
/// needs a draft of the matching shape before it offers anything at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftActions {
    /// No draft-action row: `Pan`, or `Point` with nothing (yet) dropped.
    None,
    /// `Point` armed, with a point on the canvas to confirm.
    ConfirmPoint,
    /// `Polygon` armed.
    FinishPolygon,
}

/// Why [`draft_geometry`] refused to build a geometry. `Nothing` is the tool's own action row not
/// being offered at all (or offered over a draft that cannot back it) — a silent no-op, matching
/// today's behaviour. `TooFewVertices` is a polygon short of the 3 vertices a ring needs — this one
/// earns the caller's toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftRefusal {
    /// No committable draft under the armed tool; refuse silently.
    Nothing,
    /// A polygon draft with fewer than 3 vertices.
    TooFewVertices,
}

/// Which draft-action row `tool` and `draft` together offer (`draft_actions_row`'s own input).
#[must_use]
pub fn draft_actions(tool: DrawTool, draft: &MapDraft) -> DraftActions {
    match tool {
        DrawTool::Pan => DraftActions::None,
        DrawTool::Point => match draft {
            MapDraft::Point(_) => DraftActions::ConfirmPoint,
            MapDraft::Empty | MapDraft::Polygon(_) => DraftActions::None,
        },
        DrawTool::Polygon => DraftActions::FinishPolygon,
    }
}

/// The geometry the armed tool's own action row would commit right now, or why it cannot. Keyed off
/// the same `(tool, draft)` pair as [`draft_actions`], so a `Point` draft left over from switching
/// tools can never be committed by "Finish polygon", and vice versa.
///
/// # Errors
///
/// Returns [`DraftRefusal::Nothing`] when the armed tool has no committable draft (`Pan`, or a tool
/// paired with a draft of the wrong shape), and [`DraftRefusal::TooFewVertices`] for a polygon short of
/// 3 vertices.
pub fn draft_geometry(tool: DrawTool, draft: &MapDraft) -> Result<PlaceGeometry, DraftRefusal> {
    match tool {
        DrawTool::Pan => Err(DraftRefusal::Nothing),
        DrawTool::Point => match draft {
            MapDraft::Point((lat, lon)) => Ok(PlaceGeometry::Point(geo_point(*lat, *lon))),
            MapDraft::Empty | MapDraft::Polygon(_) => Err(DraftRefusal::Nothing),
        },
        DrawTool::Polygon => match draft {
            MapDraft::Polygon(vertices) if vertices.len() >= 3 => Ok(PlaceGeometry::Polygon {
                exterior: vertices.iter().map(|&(lat, lon)| geo_point(lat, lon)).collect(),
                holes: Vec::new(),
            }),
            MapDraft::Polygon(_) => Err(DraftRefusal::TooFewVertices),
            MapDraft::Empty | MapDraft::Point(_) => Err(DraftRefusal::Nothing),
        },
    }
}

/// The settled camera a `moveend` reports (ADR 0033): the zoom plus the visible bounds, in decimal
/// degrees. The Google Map Tiles adapter's per-viewport attribution refresh is the one consumer today
/// (`geography.rs`'s `on_moved`); every other provider kind simply ignores it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovedCamera {
    /// The settled zoom level.
    pub zoom: f64,
    /// The visible viewport's northern edge, in decimal degrees.
    pub north: f64,
    /// The visible viewport's southern edge, in decimal degrees.
    pub south: f64,
    /// The visible viewport's eastern edge, in decimal degrees.
    pub east: f64,
    /// The visible viewport's western edge, in decimal degrees.
    pub west: f64,
}

/// [`map_surface`]'s mount parameters, bundled so the function stays within the house limit of 5
/// positional parameters instead of growing an `#[expect(clippy::too_many_arguments)]` every time the
/// surface gains one more caller-supplied piece of state (`docs/issues.md`'s "one struct for the
/// surface's mount parameters" bullet). [`mount_maplibre`] borrows the same struct — its own mount
/// script only ever needs a subset of these fields.
pub struct MapSurface {
    /// The mount `div`'s DOM id; distinguishes the two possible mounts (Geography's whole-atlas view,
    /// the Place screen's per-place Map tab) so both could coexist if ever shown together.
    pub container_id: &'static str,
    /// The surface's accessible label.
    pub aria_label: String,
    /// The active draw tool, shared with the caller's own toolbar.
    pub tool: Signal<DrawTool>,
    /// The in-progress drawn shape, shared with the caller's own toolbar/save form.
    pub draft: Signal<MapDraft>,
    /// The map's opening center, in decimal degrees.
    pub center: (f64, f64),
    /// Both the zoom level the map opens at and where the mounted map reports every settled camera
    /// back to — the caller renders it with [`MapZoomReadout`]. Read only via [`Signal::peek`] at
    /// mount, never `.read()`: a subscribed surface re-renders on a zoom gesture, and a re-rendered
    /// surface remounts, which rebuilds the map.
    pub zoom: Signal<f64>,
    /// The tiles/style the map fetches at mount (ADR 0033); a later provider switch goes through
    /// [`apply_map_source`] instead of a remount.
    pub source: MapSource,
    /// The reactive credit drawn over the map (#254) — seeded from `source.attribution`, but updated
    /// independently of it afterwards (a provider switch, or the Google adapter's live per-viewport
    /// refresh, ADR 0033). `MapLibre`'s own `AttributionControl` stays disabled and this static
    /// overlay carries the text instead (`docs/research/geography-rendering.md`); omitted entirely
    /// when empty — an empty bordered box is not a credit.
    pub attribution: Signal<String>,
    /// Every string a `MapLibre` control renders, localized by the caller (ADR 0003).
    pub labels: MapControlLabels,
    /// Called with every settled camera (ADR 0033) — the Geography toolbar's own Google
    /// viewport-attribution refresh; the Place Map tab, which needs no such refresh, passes a no-op.
    pub on_moved: EventHandler<MovedCamera>,
}

/// The generic `MapLibre` mount surface (draw-tool crosshair cursor + the tile source's credit), shared
/// by the Geography tool (whole-atlas view) and the Place screen's per-place Map tab — see
/// [`MapSurface`]'s field docs for what each part of `surface` does.
///
/// `surface.tool` and `surface.draft` are the caller's own draw state, passed rather than a click
/// callback: every gesture on the canvas (a click appending a vertex, a handle drag moving one)
/// resolves against the same pair, so both screens get identical behaviour from one implementation
/// instead of two copies of the same closure.
#[must_use = "renders the map surface; drop it and nothing is shown"]
pub fn map_surface(surface: MapSurface) -> Element {
    // Copied out before `surface` moves into `onmounted` below: the reactive handles (`Signal`,
    // `EventHandler`) are `Copy`, so this is a second cheap handle onto the same slot, not a borrow
    // that would conflict with the move.
    let container_id = surface.container_id;
    let tool = surface.tool;
    let attribution = surface.attribution;
    let aria_label = surface.aria_label.clone();
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
                onmounted: move |_| {
                    mount_maplibre(&surface);
                },
            }
            if !attribution().is_empty() {
                div { class: "map-attr", "{attribution}" }
            }
        }
    }
}

/// The Pan/Point/Polygon toggle row, identical on both map screens (the Geography toolbar and the
/// Place Map tab's own toolbar).
#[must_use = "renders the draw-tool buttons; drop it and the toolbar loses them"]
pub fn draw_tool_buttons(chrome: &Chrome, tool: Signal<DrawTool>) -> Element {
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
    rsx! {
        {tool_button(DrawTool::Pan, chrome.geography_tool_pan())}
        {tool_button(DrawTool::Point, chrome.geography_tool_point())}
        {tool_button(DrawTool::Polygon, chrome.geography_tool_polygon())}
    }
}

/// The draft-action row under the map surface: nothing for [`DraftActions::None`], else a primary
/// button (labelled for the action) beside a ghost "Clear", shared by both map screens (the missing
/// row for the Point tool was #282(a) — `GeoPanel::CreateHere` was dead code with no button to reach
/// it).
#[must_use = "renders the draft-action row; drop it and neither button shows"]
pub fn draft_actions_row(
    chrome: &Chrome,
    actions: DraftActions,
    on_commit: EventHandler<()>,
    on_clear: EventHandler<()>,
) -> Element {
    let label = match actions {
        DraftActions::None => return rsx! {},
        DraftActions::ConfirmPoint => chrome.place_map_confirm_point(),
        DraftActions::FinishPolygon => chrome.geography_finish_polygon(),
    };
    rsx! {
        div { class: "wrap", style: "gap:8px",
            Button { label, small: true, variant: ButtonVariant::Primary, onclick: move |_| on_commit.call(()) }
            Button { label: chrome.geography_clear_draft(), small: true, variant: ButtonVariant::Ghost, onclick: move |_| on_clear.call(()) }
        }
    }
}

/// Re-pushes the in-progress draft overlay to `container_id`'s map whenever it changes — the
/// `use_effect` both map screens ran as an identical copy.
pub fn use_draft_push(container_id: &'static str, draft: Signal<MapDraft>) {
    use_effect(move || push_map_draft(container_id, &draft()));
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
    /// A draft vertex handle was dragged and released at this point (#259). Reported once per gesture,
    /// on release — the webview rubber-bands the shape itself while the pointer moves.
    VertexMoved {
        /// The vertex's index into the draft's own (unclosed) vertex list.
        index: usize,
        /// The vertex's new latitude in decimal degrees.
        lat: f64,
        /// The vertex's new longitude in decimal degrees.
        lon: f64,
    },
    /// The camera settled (`moveend`, ADR 0033) — the zoom plus the visible bounds. The Google
    /// viewport-attribution refresh is the one consumer; every other provider kind ignores it.
    Moved(MovedCamera),
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
    if let Some(vertex) = value.get("vertex").and_then(Value::as_array) {
        let [index, lng, lat] = vertex.as_slice() else {
            return None;
        };
        // `as_u64` is what rejects a negative or fractional index; nothing here uses an `as` cast.
        return Some(MapMessage::VertexMoved {
            index: usize::try_from(index.as_u64()?).ok()?,
            lat: lat.as_f64()?,
            lon: lng.as_f64()?,
        });
    }
    if let Some(zoom) = value.get("zoom") {
        return Some(MapMessage::Zoom(zoom.as_f64()?));
    }
    if let Some(moved) = value.get("moved") {
        return Some(MapMessage::Moved(MovedCamera {
            zoom: moved.get("zoom")?.as_f64()?,
            north: moved.get("north")?.as_f64()?,
            south: moved.get("south")?.as_f64()?,
            east: moved.get("east")?.as_f64()?,
            west: moved.get("west")?.as_f64()?,
        }));
    }
    None
}

/// The draft with the vertex at `index` moved to `(lat, lon)`. An index the draft does not have leaves
/// it unchanged — the index arrives from the webview, so it is untrusted input rather than a bug to
/// fail on. Returns a new draft; the caller writes it back to its own signal.
#[must_use]
pub fn move_vertex(draft: &MapDraft, index: usize, lat: f64, lon: f64) -> MapDraft {
    let mut moved = draft.clone();
    match &mut moved {
        MapDraft::Empty => {}
        MapDraft::Point(point) => {
            if index == 0 {
                *point = (lat, lon);
            }
        }
        MapDraft::Polygon(vertices) => {
            if let Some(vertex) = vertices.get_mut(index) {
                *vertex = (lat, lon);
            }
        }
    }
    moved
}

/// Applies a canvas click to the caller's draft, gated by the armed tool: `Pan` ignores it, `Point`
/// drops (or re-drops) the single point, `Polygon` appends a vertex. Shared by both map screens, which
/// previously carried identical copies of this closure.
///
/// Reads and writes the same signal, so the read has to finish before the write: `draft()` returns an
/// owned clone and its guard is dropped at the end of that statement. Holding a read guard across a
/// [`Signal::set`] on the same signal panics at runtime, and no lint catches it.
pub fn apply_map_click(tool: Signal<DrawTool>, mut draft: Signal<MapDraft>, lat: f64, lon: f64) {
    match tool() {
        DrawTool::Pan => {}
        DrawTool::Point => draft.set(MapDraft::Point((lat, lon))),
        DrawTool::Polygon => {
            let mut vertices = match draft() {
                MapDraft::Polygon(vertices) => vertices,
                MapDraft::Empty | MapDraft::Point(_) => Vec::new(),
            };
            vertices.push((lat, lon));
            draft.set(MapDraft::Polygon(vertices));
        }
    }
}

/// Applies a released vertex drag to the caller's draft (#259). Clones out of the `peek()` borrow before
/// writing, for the same reason [`apply_map_click`] documents.
pub fn apply_vertex_move(mut draft: Signal<MapDraft>, index: usize, lat: f64, lon: f64) {
    let current = draft.peek().clone();
    draft.set(move_vertex(&current, index, lat, lon));
}

/// Mounts `MapLibre` on `surface.container_id` (a no-op under SSR, where there is no webview to run
/// the script) and arms the persistent message listener, streaming every gesture as a tagged payload
/// over `dioxus.send`, read in a loop for the surface's lifetime — not a one-shot eval per click, so
/// the map stays interactive without a Rust round trip blocking each gesture. `surface.on_moved` is
/// called with every settled camera (`MapMessage::Moved`, ADR 0033); [`MapSurface`]'s field docs cover
/// its use. Borrows [`MapSurface`] rather than its own parameter list — only `container_id`/`center`/
/// `zoom`/`labels`/`source.basemap`/`tool`/`draft`/`on_moved` are read; `aria_label` and `attribution`
/// are this function's own no-ops.
pub fn mount_maplibre(surface: &MapSurface) {
    let script = maplibre_init_script(
        surface.container_id,
        surface.center,
        *surface.zoom.peek(),
        &surface.labels,
        &surface.source.basemap,
    );
    let zoom = surface.zoom;
    let tool = surface.tool;
    let draft = surface.draft;
    let on_moved = surface.on_moved;
    let mut listener = document::eval(&script);
    spawn(async move {
        while let Ok(payload) = listener.recv::<String>().await {
            match parse_map_message(&payload) {
                Some(MapMessage::Click { lat, lon }) => apply_map_click(tool, draft, lat, lon),
                Some(MapMessage::Zoom(level)) => set_zoom(zoom, level),
                Some(MapMessage::VertexMoved { index, lat, lon }) => apply_vertex_move(draft, index, lat, lon),
                Some(MapMessage::Moved(camera)) => on_moved.call(camera),
                None => {}
            }
        }
    });
}

/// The toolbar's live zoom readout. Its own `#[component]` on purpose: it subscribes to `zoom` in its
/// own scope, so a zoom gesture repaints these ~15 characters instead of re-rendering the surface —
/// and a re-rendered surface remounts, which rebuilds the map.
#[component]
pub fn MapZoomReadout(zoom: Signal<f64>) -> Element {
    let chrome = use_context::<ChromeCtx>();
    let level = format_zoom(zoom());
    rsx! {
        span {
            class: "map-zoom-readout",
            aria_label: chrome.0.geography_zoom_aria(&level),
            "{chrome.0.geography_zoom_readout(&level)}"
        }
    }
}

/// How far the camera must move before the readout is worth re-rendering. `MapLibre` reports a settled
/// zoom as a float, so this is an epsilon compare rather than an identity one (`clippy::float_cmp` is
/// live) — and z14.2000001 renders as `z14.2` either way, so there would be nothing to repaint.
const ZOOM_READOUT_EPSILON: f64 = 0.05;

/// Whether a newly measured zoom is a different *reading* than `current`. Clamps **first**, so a level
/// outside [`ZOOM_RANGE`] compares as the bound it is pinned to instead of as a fresh value.
#[must_use]
pub fn zoom_changed(current: f64, next: f64) -> bool {
    (clamp_zoom(current) - clamp_zoom(next)).abs() >= ZOOM_READOUT_EPSILON
}

/// Stores a newly measured zoom, ignoring a reading that would render the same. `.peek()` so writing
/// the signal never subscribes the mount closure that owns it — a subscribed surface re-renders on a
/// zoom gesture, and a re-rendered surface remounts, which rebuilds the map.
pub fn set_zoom(mut zoom: Signal<f64>, level: f64) {
    if zoom_changed(*zoom.peek(), level) {
        zoom.set(clamp_zoom(level));
    }
}

/// The zoom level as the readout shows it: one decimal, formatted Rust-side like the Place screen's
/// `{lat:.4}` coordinate readout — no Fluent message in this repo interpolates a float, so the
/// readout's own message takes this string.
#[must_use]
pub fn format_zoom(zoom: f64) -> String {
    format!("{:.1}", clamp_zoom(zoom))
}

/// Every string a `MapLibre` control renders, localized by this app (ADR 0003) rather than by
/// `MapLibre`'s own bundled i18n — its built-in defaults *are* its own i18n, which the ADR forbids.
/// Handed to the map through the constructor's `locale` option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapControlLabels {
    /// The zoom-in button's tooltip/`aria-label`.
    pub zoom_in: String,
    /// The zoom-out button's tooltip/`aria-label`.
    pub zoom_out: String,
    /// The scale bar's metre unit.
    pub meters: String,
    /// The scale bar's kilometre unit.
    pub kilometers: String,
}

impl MapControlLabels {
    /// Resolves every control string from the app's own chrome catalogue.
    #[must_use]
    pub fn from_chrome(chrome: &Chrome) -> Self {
        Self {
            zoom_in: chrome.geography_zoom_in(),
            zoom_out: chrome.geography_zoom_out(),
            meters: chrome.geography_scale_meters(),
            kilometers: chrome.geography_scale_kilometers(),
        }
    }
}

/// The `locale` table for the constructor: exactly the keys the controls this map adds will look up.
/// `NavigationControl.ResetBearing` is deliberately absent — `showCompass: false`, so no control ever
/// reads it, and an unused Fluent message is what `cargo xtask i18n-check` warns about. Interpolated
/// as JSON (like [`circle_paint`]), which is both valid JS object syntax and escaping-safe for
/// translated text.
fn control_locale(labels: &MapControlLabels) -> Value {
    json!({
        "NavigationControl.ZoomIn": labels.zoom_in,
        "NavigationControl.ZoomOut": labels.zoom_out,
        "ScaleControl.Meters": labels.meters,
        "ScaleControl.Kilometers": labels.kilometers,
    })
}

/// The controls added once the map has loaded, plus the opening zoom measurement: the zoom buttons
/// (a pointer-free way to change zoom, top-left where `place.html`'s `.map-zoom` stand-in draws them)
/// and a metric scale bar (bottom-left, clear of the attribution). Interpolated into
/// [`maplibre_init_script`]'s `load` handler rather than written inline there, so that function stays
/// inside the line cap. The zoom emit is last, so the first readout is a measurement of the loaded
/// camera rather than the seed value the script was built with.
const MAP_CONTROLS_SCRIPT: &str = "
                map.addControl(new maplibregl.NavigationControl({ showCompass: false }), 'top-left');
                map.addControl(new maplibregl.ScaleControl({ unit: 'metric' }), 'bottom-left');
                dioxus.send(JSON.stringify({ zoom: map.getZoom() }));";

/// Drag-to-move for the draft vertex handles (#259), armed inside [`maplibre_init_script`]'s own
/// single-map guard so a re-render cannot register a second copy of any of it. Interpolated as a value,
/// so its braces need no `format!` escaping.
///
/// The gesture rubber-bands entirely in the webview and reports once on release, which is the same
/// choice this module makes for `zoomend` over `zoom`: a `dioxus.send` per frame would round-trip a
/// whole drag through Rust and re-render (and therefore remount) the surface on every one.
///
/// Three details are load-bearing:
///
/// - `properties.vertex` is compared against `undefined`/`null`, never tested for truthiness — vertex
///   `0` is a real handle and a falsy value.
/// - `e.preventDefault()` on the press is what suppresses `dragPan`; without it a grab also pans the
///   basemap under the handle.
/// - the release listens on **`window`**, not the map: the map's own `mouseup` only fires over the
///   canvas, so releasing outside it would leave the drag armed.
const MAP_VERTEX_DRAG_SCRIPT: &str = "
            const vertexAt = (point) => {
                if (!map.getLayer('geo-draft-point')) return null;
                for (const hit of map.queryRenderedFeatures(point, { layers: ['geo-draft-point'] })) {
                    const index = hit.properties ? hit.properties.vertex : undefined;
                    if (index !== undefined && index !== null) return index;
                }
                return null;
            };
            const handleAt = (point) => vertexAt(point) !== null;
            const moveDraftVertex = (data, index, lng, lat) => {
                const features = data.features || [];
                for (const feature of features) {
                    if (feature.properties && feature.properties.vertex === index) {
                        feature.geometry.coordinates = [lng, lat];
                    }
                }
                const shape = features[0] && features[0].geometry;
                if (!shape) return;
                if (shape.type === 'LineString' && shape.coordinates[index]) {
                    shape.coordinates[index] = [lng, lat];
                }
                if (shape.type === 'Polygon') {
                    const ring = shape.coordinates[0] || [];
                    if (!ring[index]) return;
                    ring[index] = [lng, lat];
                    if (index === 0) ring[ring.length - 1] = [lng, lat];
                }
            };
            map.on('mousedown', (e) => {
                if (el.dataset.armed !== 'true') return;
                const index = vertexAt(e.point);
                if (index === null) return;
                el.__geoDrag = { index: index, moved: false, lng: e.lngLat.lng, lat: e.lngLat.lat };
                e.preventDefault();
            });
            map.on('mousemove', (e) => {
                const drag = el.__geoDrag;
                if (!drag) return;
                drag.moved = true;
                drag.lng = e.lngLat.lng;
                drag.lat = e.lngLat.lat;
                const draft = el.__geoPending && el.__geoPending.draft;
                if (!draft) return;
                moveDraftVertex(draft, drag.index, drag.lng, drag.lat);
                const source = map.getSource('geo-draft');
                if (source) source.setData(draft);
            });
            window.addEventListener('mouseup', () => {
                const drag = el.__geoDrag;
                el.__geoDrag = null;
                if (drag && drag.moved) dioxus.send(JSON.stringify({ vertex: [drag.index, drag.lng, drag.lat] }));
            });
            map.on('mouseenter', 'geo-draft-point', () => { map.getCanvas().style.cursor = 'grab'; });
            map.on('mouseleave', 'geo-draft-point', () => { map.getCanvas().style.cursor = ''; });";

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

/// The `el.__geoBasemap` descriptor the init script and [`apply_map_source_script`] both hand to
/// `__geoInstall`: a raster source's URL/tile size/overzoom ceiling, or just its kind tag for a style
/// (a whole style document declares its own basemap layers — `__geoInstall` adds nothing for it).
fn basemap_descriptor(basemap: &MapBasemap) -> Value {
    match basemap {
        MapBasemap::Raster {
            tile_url,
            tile_size,
            max_zoom,
        } => json!({ "kind": "raster", "tileUrl": tile_url, "tileSize": tile_size, "maxZoom": max_zoom }),
        MapBasemap::Style { .. } => json!({ "kind": "style" }),
    }
}

/// The map constructor's/`setStyle`'s own `style` value for `basemap`: a blank style-8 document for a
/// raster basemap (`__geoInstall` adds its raster source once the map has loaded), or the style URL
/// itself for a `MapLibre` style.
fn initial_style_value(basemap: &MapBasemap) -> Value {
    match basemap {
        MapBasemap::Raster { .. } => json!({ "version": 8, "sources": {}, "layers": [] }),
        MapBasemap::Style { style_url } => json!(style_url),
    }
}

/// The `MapLibre` bootstrap script for one container: creates the map (guarded against a re-render
/// remount), defines `el.__geoInstall` — the marker/event/draft `GeoJSON` sources + layers, plus the
/// current basemap's own source when it is a raster one — and calls it once the map has loaded.
/// [`apply_map_source_script`] later re-runs the same `__geoInstall` after a provider switch, so the
/// overlay layers are defined exactly once regardless of how many times the basemap changes.
/// `__geoInstall` also applies any data already stashed by [`push_map_data`]/[`push_map_draft`] before
/// the sources existed (`el.__geoPending` — otherwise a push that races this async `load` event is
/// silently dropped, and nothing ever re-applies it, per the "Place map shows no marker" bug). Source/
/// layer ids are scoped to each map instance, so two containers on the page never collide.
///
/// It also arms a repaint observer, which is what keeps the canvas visible across a layout change
/// (#252). Arming a draw tool inserts a Finish/Clear row under the map, shrinking `.map-surface`;
/// `MapLibre`'s own `trackResize` observer does re-measure and resize the canvas correctly, but under
/// `WebKitGTK` the frame it draws never reaches the compositor, so the map went blank until the next
/// camera move. Forcing one more `redraw()` on the animation frame after the resize does composite.
/// `redraw()` changes no layout, so the observer cannot re-trigger itself.
fn maplibre_init_script(
    container_id: &str,
    center: (f64, f64),
    zoom: f64,
    labels: &MapControlLabels,
    basemap: &MapBasemap,
) -> String {
    format!(
        r"
        const el = document.getElementById('{container_id}');
        if (el && !el.__geoMap && window.maplibregl) {{
            el.__geoBasemap = {basemap_json};
            const map = new maplibregl.Map({{
                container: el,
                style: {initial_style},
                center: [{lon}, {lat}],
                zoom: {zoom},
                minZoom: {min_zoom},
                maxZoom: {max_zoom},
                locale: {locale},
                attributionControl: false,
            }});
            el.__geoMap = map;
            map.on('load', () => {{
                el.__geoInstall(map);{controls}
            }});
            el.__geoInstall = function(map) {{
                const basemap = el.__geoBasemap;
                if (basemap && basemap.kind === 'raster') {{
                    map.addSource('geo-tiles', {{ type: 'raster', tiles: [basemap.tileUrl], tileSize: basemap.tileSize, maxzoom: basemap.maxZoom }});
                    map.addLayer({{ id: 'geo-tile-layer', type: 'raster', source: 'geo-tiles' }});
                }}
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
                map.addLayer({{ id: 'geo-draft-line', type: 'line', source: 'geo-draft', filter: ['!=', ['geometry-type'], 'Point'], paint: {{ 'line-color': '#ff5d5d', 'line-width': 2 }} }});
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
            }};{vertex_drag}
            map.on('click', (e) => {{ if (!handleAt(e.point)) dioxus.send(JSON.stringify({{ click: [e.lngLat.lng, e.lngLat.lat] }})); }});
            map.on('zoomend', () => {{ dioxus.send(JSON.stringify({{ zoom: map.getZoom() }})); }});
            map.on('moveend', () => {{
                const b = map.getBounds();
                dioxus.send(JSON.stringify({{ moved: {{ zoom: map.getZoom(), north: b.getNorth(), south: b.getSouth(), east: b.getEast(), west: b.getWest() }} }}));
            }});
            new ResizeObserver(() => {{ requestAnimationFrame(() => el.__geoMap && el.__geoMap.redraw()); }}).observe(el);
        }}
        ",
        lat = center.0,
        lon = center.1,
        min_zoom = ZOOM_RANGE.0,
        max_zoom = ZOOM_RANGE.1,
        locale = control_locale(labels),
        basemap_json = basemap_descriptor(basemap),
        initial_style = initial_style_value(basemap),
        marker_paint = circle_paint("#5db3ff", POINT_RADIUS_STOPS, 2),
        event_paint = circle_paint("#ffb020", EVENT_RADIUS_STOPS, 1),
        draft_paint = circle_paint("#ff5d5d", POINT_RADIUS_STOPS, 2),
        controls = MAP_CONTROLS_SCRIPT,
        vertex_drag = MAP_VERTEX_DRAG_SCRIPT,
    )
}

/// Switches the running map at `container_id` to `source`'s basemap (ADR 0033) without a remount:
/// re-points `el.__geoBasemap`, then `setStyle`s (`diff: false`, so the reload is unconditional and
/// `style.load` always fires) and re-runs `__geoInstall` once the new style has loaded, followed by a
/// forced `redraw()` — the same `WebKitGTK` compositor reason [`push_draft_script`]'s doc comment
/// documents. A no-op under SSR, or if the map has not mounted yet.
pub fn apply_map_source(container_id: &str, source: &MapSource) {
    run_map_script(&apply_map_source_script(container_id, &source.basemap));
}

/// The provider-switch script: re-point `el.__geoBasemap` at the new basemap, `setStyle` to match, and
/// re-run the one `__geoInstall` the init script defined.
fn apply_map_source_script(container_id: &str, basemap: &MapBasemap) -> String {
    format!(
        r"
        const el = document.getElementById('{container_id}');
        const map = el && el.__geoMap;
        if (map) {{
            el.__geoBasemap = {basemap_json};
            map.setStyle({initial_style}, {{ diff: false }});
            map.once('style.load', () => {{
                el.__geoInstall(map);
                map.redraw();
            }});
        }}
        ",
        basemap_json = basemap_descriptor(basemap),
        initial_style = initial_style_value(basemap),
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
///
/// The trailing `redraw()` is a #252-shaped compositor bug, not a data bug (#282b): `setData` on a
/// source schedules `MapLibre`'s own render, but that scheduling assumes something is already driving
/// the browser's animation-frame loop. A canvas gesture (drag, wheel, a click `MapLibre` itself
/// handles) keeps that loop alive incidentally; a draft push from an external button (Clear, Finish)
/// reaches the map only through this `document::eval`, with no such gesture behind it, and under
/// `WebKitGTK`'s software-GL path the scheduled frame then never reaches the compositor — the ring
/// stayed on screen after Clear, indefinitely, not just for one frame. Forcing `redraw()` here, exactly
/// as the resize observer already does for the #252 case, composites the just-applied `setData`
/// immediately instead of waiting on a frame that never arrives on its own.
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
                map.redraw();
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
    run_map_script(&fit_bounds_script(
        container_id,
        bounds.min_lat,
        bounds.max_lat,
        bounds.min_lon,
        bounds.max_lon,
    ));
}

/// How far in Fit will go. A framing choice rather than a camera bound: fitting a single point has no
/// extent of its own, so without a ceiling `fitBounds` slams to the camera's maximum. Sits inside
/// [`ZOOM_RANGE`], which bounds every other way the camera moves.
const FIT_MAX_ZOOM: f64 = 15.0;

/// The `fitBounds` script for one `(lat, lon)` box (the [`push_data_script`]/[`push_draft_script`]
/// precedent: the script text is its own testable function).
fn fit_bounds_script(container_id: &str, min_lat: f64, max_lat: f64, min_lon: f64, max_lon: f64) -> String {
    format!(
        r"
        const map = document.getElementById('{container_id}')?.__geoMap;
        if (map) {{
            map.fitBounds([[{min_lon}, {min_lat}], [{max_lon}, {max_lat}]], {{ padding: 40, maxZoom: {FIT_MAX_ZOOM}, duration: 300 }});
        }}
        ",
    )
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

/// One draggable draft vertex: a `Point` feature tagged with its own index into [`MapDraft`]'s
/// unclosed vertex list. The tag is what the drag script's hit test reads back to know which vertex it
/// grabbed, so it must stay a number (`0` is a real index, and the script therefore never tests it for
/// truthiness).
fn vertex_feature(index: usize, lat: f64, lon: f64) -> Value {
    json!({
        "type": "Feature",
        "geometry": { "type": "Point", "coordinates": [lon, lat] },
        "properties": { "vertex": index },
    })
}

/// The polygon draft's own shape feature: a closed ring once it has at least 3 vertices, else the open
/// line through whatever has been clicked so far. Carries no `vertex` property, so a hit test can only
/// ever land on a handle.
fn polygon_shape_feature(vertices: &[(f64, f64)]) -> Value {
    let geometry = if vertices.len() >= 3 {
        json!({ "type": "Polygon", "coordinates": [closed_ring(vertices)] })
    } else {
        let line: Vec<[f64; 2]> = vertices.iter().map(|&(lat, lon)| [lon, lat]).collect();
        json!({ "type": "LineString", "coordinates": line })
    };
    json!({ "type": "Feature", "geometry": geometry, "properties": {} })
}

/// Converts the in-progress draft to a `GeoJSON` `FeatureCollection`: the shape feature first (so the
/// existing index-0 assertions and the drag script's ring rewrite both still address it), then one
/// [`vertex_feature`] handle per vertex the operator placed (#259 — before this a ring drew as fill plus
/// outline with no corners, and a one-vertex draft drew nothing at all).
///
/// A point draft's single feature *is* its handle rather than gaining a second coincident one: two
/// features under one `queryRenderedFeatures` hit test have no defined order between them.
#[must_use]
pub fn draft_geojson(draft: &MapDraft) -> Value {
    match draft {
        MapDraft::Empty => empty_feature_collection(),
        MapDraft::Point((lat, lon)) => json!({
            "type": "FeatureCollection",
            "features": [vertex_feature(0, *lat, *lon)],
        }),
        MapDraft::Polygon(vertices) => {
            let mut features = vec![polygon_shape_feature(vertices)];
            for (index, &(lat, lon)) in vertices.iter().enumerate() {
                features.push(vertex_feature(index, lat, lon));
            }
            json!({ "type": "FeatureCollection", "features": features })
        }
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
        DraftActions, DraftRefusal, DrawTool, FIT_MAX_ZOOM, MapControlLabels, MapDraft, MapMessage, MovedCamera,
        apply_map_source_script, basemap_descriptor, closed_ring, combined_bounds, draft_actions, draft_geojson,
        draft_geometry, empty_feature_collection, events_geojson, fit_bounds_script, format_zoom, geo_point,
        initial_style_value, maplibre_init_script, markers_geojson, move_vertex, parse_map_message, push_data_script,
        push_draft_script, save_year, shape_to_draft, zoom_changed,
    };
    use serde_json::{Value, json};
    use vitni_app::{MapBasemap, PlaceGeometry};
    use vitni_ui::{EventPinVm, MarkerShapeVm, PlaceMarkerVm, ZOOM_RANGE};

    /// Stand-in control labels: in the app every one of these is a Fluent lookup (ADR 0003), so the
    /// tests assert the supplied text reaches the script rather than any particular wording.
    fn labels() -> MapControlLabels {
        MapControlLabels {
            zoom_in: "Zoom in".to_owned(),
            zoom_out: "Zoom out".to_owned(),
            meters: "m".to_owned(),
            kilometers: "km".to_owned(),
        }
    }

    /// A tile URL that is deliberately *not* the built-in OSM one, so an assertion can tell "the
    /// resolved basemap reached the script" apart from "the old hardcoded literal is still there".
    const OTHER_TILE_URL: &str = "https://tiles.example/{z}/{x}/{y}.png";

    /// A raster basemap over [`OTHER_TILE_URL`], for tests that need one resolved descriptor.
    fn raster_basemap() -> MapBasemap {
        MapBasemap::Raster {
            tile_url: OTHER_TILE_URL.to_owned(),
            tile_size: 256,
            max_zoom: 19,
        }
    }

    /// A vector style basemap, for the tests distinguishing it from a raster one.
    fn style_basemap() -> MapBasemap {
        MapBasemap::Style {
            style_url: "https://tiles.example/style.json".to_owned(),
        }
    }

    fn init_script() -> String {
        maplibre_init_script("geo-map", (59.9, 10.7), 5.0, &labels(), &raster_basemap())
    }

    /// The resolved raster URL/size/ceiling reach the script two ways: stashed on `el.__geoBasemap`
    /// (so a later switch can re-point it), and read back off it by the `addSource` call — never
    /// baked in as separate literals that could drift apart from each other.
    #[test]
    fn a_raster_basemap_reaches_the_tile_source_with_its_resolved_url_size_and_ceiling() {
        let script = init_script();
        assert!(
            script.contains(&format!("el.__geoBasemap = {};", basemap_descriptor(&raster_basemap()))),
            "the resolved descriptor is stashed verbatim:\n{script}"
        );
        let source = script
            .find("map.addSource('geo-tiles'")
            .expect("the tile source is added");
        let end = script[source..].find(");").expect("the addSource call ends") + source;
        for fragment in [
            "tiles: [basemap.tileUrl]",
            "tileSize: basemap.tileSize",
            "maxzoom: basemap.maxZoom",
        ] {
            assert!(
                script[source..end].contains(fragment),
                "the raster source reads {fragment} off the resolved descriptor, not a literal:\n{}",
                &script[source..end]
            );
        }
    }

    /// A `MapLibre` style is a whole document that already declares its own basemap layers.
    /// `__geoInstall`'s raster branch is shared, JS-text-identical code re-run after every basemap
    /// switch (not a per-kind script), so it stays in the text regardless of which basemap built the
    /// script — what must hold is that it is runtime-gated by `el.__geoBasemap.kind`, so a style
    /// basemap's own descriptor (`kind: 'style'`) never lets it run and paint a second, unrelated
    /// basemap over the style's own.
    #[test]
    fn a_style_basemap_sets_the_style_url_and_guards_the_raster_source_behind_its_own_kind_check() {
        let script = maplibre_init_script("geo-map", (59.9, 10.7), 5.0, &labels(), &style_basemap());
        assert!(
            script.contains("style: \"https://tiles.example/style.json\""),
            "the constructor's style is set to the resolved style URL:\n{script}"
        );
        assert!(
            script.contains(&format!("el.__geoBasemap = {};", basemap_descriptor(&style_basemap()))),
            "the style's own descriptor (kind: 'style', no tile fields) is what __geoInstall reads at runtime:\n{script}"
        );
        let guard = script
            .find("if (basemap && basemap.kind === 'raster')")
            .expect("the raster source is runtime-gated by kind");
        let raster_source = script
            .find("map.addSource('geo-tiles'")
            .expect("__geoInstall's raster branch is still defined — it is reused by every basemap kind");
        assert!(
            guard < raster_source,
            "the raster addSource sits behind the kind guard:\n{script}"
        );
    }

    /// A raster basemap's constructor style is the blank style-8 document `__geoInstall` builds on —
    /// its own raster source is added once the map has loaded, not baked into the initial style.
    #[test]
    fn a_raster_basemap_starts_from_a_blank_style() {
        let script = init_script();
        let map = script.find("new maplibregl.Map(").expect("the map is constructed");
        assert!(
            script[map..].contains(r#"style: {"layers":[],"sources":{},"version":8}"#),
            "a raster basemap opens on a blank style-8 document:\n{script}"
        );
    }

    /// [`apply_map_source_script`] is the provider-switch path (ADR 0033): it must re-point
    /// `el.__geoBasemap`, call `setStyle` unconditionally (`diff: false`, or a switch back to an
    /// identical-looking style could no-op), and re-run the one `__geoInstall` the init script defined
    /// — never a second copy of the overlay-layer logic.
    #[test]
    fn the_switch_script_repoints_the_basemap_and_reinstalls_the_overlay_layers() {
        let script = apply_map_source_script("geo-map", &raster_basemap());
        assert!(
            script.contains(&format!("el.__geoBasemap = {};", basemap_descriptor(&raster_basemap()))),
            "the basemap descriptor is re-pointed at the new provider:\n{script}"
        );
        assert!(
            script.contains(&format!(
                "map.setStyle({}, {{ diff: false }});",
                initial_style_value(&raster_basemap())
            )),
            "setStyle reloads unconditionally so style.load always fires:\n{script}"
        );
        let set_style = script.find("map.setStyle(").expect("setStyle is called");
        let reinstall = script
            .find("el.__geoInstall(map);")
            .expect("the overlay layers are re-installed");
        assert!(
            set_style < reinstall,
            "the re-install runs after the style swap:\n{script}"
        );
        assert!(
            script.contains("map.redraw();"),
            "a forced redraw composites the swap under WebKitGTK's software-GL path:\n{script}"
        );
        assert!(
            !script.contains("addLayer"),
            "the switch script has no overlay-layer logic of its own — it only re-runs __geoInstall:\n{script}"
        );
    }

    /// The camera-settled message a Google viewport-attribution refresh needs (ADR 0033) — ignored by
    /// every other provider kind, but still parsed the same way regardless of which is active.
    #[test]
    fn a_moved_payload_carries_the_settled_camera() {
        assert_eq!(
            parse_map_message(r#"{"moved":{"zoom":6.5,"north":61.0,"south":58.0,"east":12.0,"west":9.0}}"#),
            Some(MapMessage::Moved(MovedCamera {
                zoom: 6.5,
                north: 61.0,
                south: 58.0,
                east: 12.0,
                west: 9.0,
            }))
        );
    }

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

    /// The dragged handle reports the same `[lng, lat]` swap a click does, with the index it addresses
    /// in front. Index `0` is the case to watch: it is a real vertex, not an absent one.
    #[test]
    fn a_vertex_payload_carries_its_index_and_swaps_the_webviews_lng_lat_order() {
        assert_eq!(
            parse_map_message(r#"{"vertex":[2,10.7,59.9]}"#),
            Some(MapMessage::VertexMoved {
                index: 2,
                lat: 59.9,
                lon: 10.7
            })
        );
        assert_eq!(
            parse_map_message(r#"{"vertex":[0,10.7,59.9]}"#),
            Some(MapMessage::VertexMoved {
                index: 0,
                lat: 59.9,
                lon: 10.7
            })
        );
    }

    #[test]
    fn an_untagged_or_unknown_payload_is_ignored_rather_than_guessed_at() {
        for payload in [
            "[]",
            "[10.7,59.9]",
            r#"{"bearing":90}"#,
            r#"{"click":[10.7]}"#,
            r#"{"zoom":"14.2"}"#,
            r#"{"vertex":[0,10.7]}"#,
            r#"{"vertex":["0",10.7,59.9]}"#,
            r#"{"vertex":[-1,10.7,59.9]}"#,
            r#"{"vertex":[1.5,10.7,59.9]}"#,
            "not json at all",
        ] {
            assert_eq!(parse_map_message(payload), None, "{payload} names no known message");
        }
    }

    /// A drag rewrites the grabbed vertex and nothing else — the neighbours keep their coordinates, so
    /// the ring's shape only changes at the corner that was moved.
    #[test]
    fn moving_a_vertex_rewrites_only_the_one_it_addresses() {
        let draft = MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)]);
        assert_eq!(
            move_vertex(&draft, 1, 62.5, 4.5),
            MapDraft::Polygon(vec![(60.0, 5.0), (62.5, 4.5), (61.0, 6.0)])
        );
    }

    #[test]
    fn moving_a_point_drafts_only_vertex_repositions_it() {
        let draft = MapDraft::Point((59.9, 10.7));
        assert_eq!(move_vertex(&draft, 0, 60.1, 11.2), MapDraft::Point((60.1, 11.2)));
    }

    /// The index arrives from the webview, so it is untrusted input: an index the draft does not have
    /// leaves the draft alone rather than growing it or failing.
    #[test]
    fn a_vertex_index_past_the_last_one_leaves_the_draft_unchanged() {
        let draft = MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0)]);
        assert_eq!(move_vertex(&draft, 7, 62.5, 4.5), draft);
        let point = MapDraft::Point((59.9, 10.7));
        assert_eq!(move_vertex(&point, 1, 62.5, 4.5), point);
    }

    #[test]
    fn an_empty_draft_has_no_vertex_to_move() {
        assert_eq!(move_vertex(&MapDraft::Empty, 0, 62.5, 4.5), MapDraft::Empty);
    }

    /// Grabbing a handle has to suppress `dragPan`, or the press both grabs the vertex and pans the
    /// whole basemap under it. `preventDefault` on `MapLibre`'s own mouse event is what does that.
    #[test]
    fn pressing_a_handle_suppresses_the_maps_own_drag_pan() {
        let script = init_script();
        let press = script.find("map.on('mousedown'").expect("the press is listened for");
        let end = script[press..]
            .find("map.on('mousemove'")
            .expect("the move handler follows")
            + press;
        assert!(
            script[press..end].contains("e.preventDefault()"),
            "the press cancels MapLibre's own drag behaviour:\n{}",
            &script[press..end]
        );
    }

    /// Pan is still pan: the same `data-armed` attribute the surface renders for the crosshair cursor
    /// gates the grab, so dragging over a handle with no tool armed pans the map as it always did.
    #[test]
    fn a_handle_is_grabbable_only_while_a_draw_tool_is_armed() {
        let script = init_script();
        let press = script.find("map.on('mousedown'").expect("the press is listened for");
        let end = script[press..]
            .find("map.on('mousemove'")
            .expect("the move handler follows")
            + press;
        assert!(
            script[press..end].contains("el.dataset.armed !== 'true'"),
            "an unarmed surface never grabs a handle:\n{}",
            &script[press..end]
        );
    }

    /// The handle rubber-bands in the webview and commits once on release — the same reasoning as this
    /// module's `zoomend`-not-`zoom` choice: a `dioxus.send` per `mousemove` would round-trip the whole
    /// gesture through Rust and re-render the surface ~20 times.
    #[test]
    fn a_drag_reports_once_on_release_rather_than_once_per_frame() {
        let script = init_script();
        let move_start = script.find("map.on('mousemove'").expect("the move handler exists");
        let release = script
            .find("window.addEventListener('mouseup'")
            .expect("the gesture ends on a window listener, so releasing off-canvas still ends it");
        assert!(
            !script[move_start..release].contains("dioxus.send"),
            "the move only repaints the webview's own copy of the draft:\n{}",
            &script[move_start..release]
        );
        assert!(
            script[release..].contains("dioxus.send(JSON.stringify({ vertex: [drag.index, drag.lng, drag.lat] }))"),
            "the release reports the moved vertex under its own key:\n{script}"
        );
        assert!(
            !script.contains("map.on('mouseup'"),
            "the map's own mouseup only fires over the canvas, which would leave a drag armed:\n{script}"
        );
    }

    /// A short drag still fires the map's own `click`, so the click emitter has to refuse a click that
    /// landed on a handle — otherwise pressing a corner would append a vertex on top of moving it.
    #[test]
    fn a_click_on_a_handle_grabs_it_instead_of_appending_a_vertex() {
        let script = init_script();
        assert!(
            script.contains(
                "if (!handleAt(e.point)) dioxus.send(JSON.stringify({ click: [e.lngLat.lng, e.lngLat.lat] }))"
            ),
            "the click emitter is guarded by the same hit test the grab uses:\n{script}"
        );
    }

    /// A re-render remounts the surface and re-runs this script; the `__geoMap` guard is what stops a
    /// second map being built, and the drag listeners have to sit inside it or they stack up.
    #[test]
    fn the_drag_listeners_are_armed_inside_the_single_map_guard() {
        let script = init_script();
        let guard = script
            .find("if (el && !el.__geoMap && window.maplibregl)")
            .expect("the single-map guard");
        let press = script.find("map.on('mousedown'").expect("the press is listened for");
        let release = script
            .find("window.addEventListener('mouseup'")
            .expect("the release is listened for");
        assert!(
            guard < press && guard < release,
            "both drag listeners are armed inside the guard:\n{script}"
        );
    }

    /// `place.html` draws its vertex specimens with `cursor: grab`; the canvas-drawn handles say the
    /// same thing by swapping the canvas cursor while the pointer is over one.
    #[test]
    fn hovering_a_handle_says_it_can_be_grabbed() {
        let script = init_script();
        for event in ["mouseenter", "mouseleave"] {
            assert!(
                script.contains(&format!("map.on('{event}', 'geo-draft-point'")),
                "the {event} cursor swap is scoped to the handle layer:\n{script}"
            );
        }
        assert!(
            script.contains("map.getCanvas().style.cursor = 'grab'"),
            "hovering a handle shows the grab cursor:\n{script}"
        );
    }

    /// The channel carries more than one kind of message, so the click emitter must tag itself —
    /// an untagged `[lng, lat]` array is exactly what [`parse_map_message`] now refuses.
    #[test]
    fn the_click_emitter_tags_its_payload_so_the_channel_can_carry_more_than_clicks() {
        let script = init_script();
        assert!(
            script.contains("dioxus.send(JSON.stringify({ click: [e.lngLat.lng, e.lngLat.lat] }))"),
            "the click rides the shared channel under its own key:\n{script}"
        );
    }

    /// One decimal, because `MapLibre` zoom is fractional: an integer readout would sit on the same
    /// number through most of a wheel gesture and read as stuck.
    #[test]
    fn the_readout_shows_one_decimal_so_a_gesture_is_visible_without_being_noisy() {
        assert_eq!(format_zoom(14.234), "14.2");
        assert_eq!(format_zoom(4.0), "4.0");
        assert_eq!(format_zoom(18.96), "19.0");
    }

    #[test]
    fn a_zoom_that_renders_identically_is_not_a_change() {
        assert!(!zoom_changed(14.2, 14.201), "the readout would print z14.2 either way");
        assert!(zoom_changed(14.2, 14.3), "one tenth of a level is a visible change");
    }

    /// Pins the clamp-then-compare order: z25 is not reachable, so it compares as the ceiling it is
    /// pinned to. Comparing first would let an out-of-range reading re-render the readout forever.
    #[test]
    fn an_out_of_range_reading_compares_as_the_bound_it_clamps_to() {
        assert!(!zoom_changed(ZOOM_RANGE.1, 25.0));
        assert!(!zoom_changed(ZOOM_RANGE.0, -8.0));
    }

    #[test]
    fn the_camera_is_bounded_to_the_zooms_the_tiles_exist_at() {
        let script = init_script();
        let map = script.find("new maplibregl.Map(").expect("the map is constructed");
        for bound in [
            format!("minZoom: {}", ZOOM_RANGE.0),
            format!("maxZoom: {}", ZOOM_RANGE.1),
        ] {
            assert!(
                script[map..].contains(&bound),
                "the constructor carries `{bound}`:\n{script}"
            );
        }
    }

    #[test]
    fn the_navigation_control_is_added_once_the_map_has_loaded() {
        let script = init_script();
        let load = script.find("map.on('load'").expect("the load handler");
        let control = script
            .find("new maplibregl.NavigationControl(")
            .expect("a pointer-free way to change zoom exists at all");
        assert!(
            load < control,
            "the control is added inside the load handler, not against a map with no style yet:\n{script}"
        );
        assert!(
            script[control..].contains("{ showCompass: false }"),
            "only the zoom buttons: there is no bearing/pitch gesture on this map to reset:\n{script}"
        );
        assert!(
            script[control..].contains("'top-left'"),
            "the zoom buttons sit top-left, where place.html's `.map-zoom` stand-in puts them:\n{script}"
        );
    }

    #[test]
    fn a_metric_scale_bar_is_added_bottom_left() {
        let script = init_script();
        let control = script
            .find("new maplibregl.ScaleControl(")
            .expect("a scale bar says what the zoom level means on the ground");
        assert!(
            script[control..].contains("{ unit: 'metric' }"),
            "metric only — an imperial unit is out of scope:\n{script}"
        );
        assert!(
            script[control..].contains("'bottom-left'"),
            "the scale bar sits bottom-left, clear of the attribution:\n{script}"
        );
    }

    /// ADR 0003 in executable form: `MapLibre`'s own control text is its own i18n, so every string it
    /// renders comes from this app's Fluent catalogue through the `locale` option instead.
    #[test]
    fn the_controls_take_their_text_from_the_apps_own_catalogue() {
        let script = init_script();
        let locale = script.find("locale: ").expect("the constructor carries a locale table");
        for (key, text) in [
            ("NavigationControl.ZoomIn", "Zoom in"),
            ("NavigationControl.ZoomOut", "Zoom out"),
            ("ScaleControl.Meters", "m"),
            ("ScaleControl.Kilometers", "km"),
        ] {
            assert!(
                script[locale..].contains(&format!(r#""{key}":"{text}""#)),
                "{key} is localized by the app, not by MapLibre:\n{script}"
            );
        }
        assert!(
            !script.contains("ResetBearing"),
            "no key for a control this map never adds — i18n-check warns on an unused message:\n{script}"
        );
    }

    /// `zoomend`, not `zoom`: `zoom` fires per animation frame, so one gesture would be ~20 round
    /// trips and ~20 re-renders (the #252 class of `WebKitGTK` problem).
    #[test]
    fn the_zoom_emitter_reports_a_settled_camera_and_an_opening_measurement() {
        let script = init_script();
        assert!(
            !script.contains("map.on('zoom',"),
            "a per-frame `zoom` listener would round-trip the whole gesture:\n{script}"
        );
        let emitter = script
            .find("map.on('zoomend'")
            .expect("the settled camera is reported back");
        assert!(
            script[emitter..].contains("dioxus.send(JSON.stringify({ zoom: map.getZoom() }))"),
            "the settled level rides the shared channel under its own key:\n{script}"
        );
        let load = script.find("map.on('load'").expect("the load handler");
        let opening = script
            .find("dioxus.send(JSON.stringify({ zoom: map.getZoom() }))")
            .expect("an emit exists");
        assert!(
            load < opening && opening < emitter,
            "the first readout is a measurement taken inside the load handler, not the seed value:\n{script}"
        );
    }

    /// Fit's own ceiling is a framing choice (don't slam to street level for a single point), so it
    /// has to sit inside the camera's range rather than fight it.
    #[test]
    fn fit_stops_inside_the_cameras_own_range() {
        assert!(
            ZOOM_RANGE.0 <= FIT_MAX_ZOOM && FIT_MAX_ZOOM <= ZOOM_RANGE.1,
            "Fit's ceiling {FIT_MAX_ZOOM} is a zoom the camera allows"
        );
        let script = fit_bounds_script("geo-map", 59.0, 60.0, 5.0, 6.0);
        assert!(
            script.contains(&format!("maxZoom: {FIT_MAX_ZOOM}")),
            "Fit passes its own ceiling, named rather than inlined:\n{script}"
        );
    }

    #[test]
    fn every_circle_layer_paints_a_zoom_interpolated_radius_with_a_white_stroke() {
        let script = init_script();
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
        let script = init_script();
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

    /// #282(b): Clear correctly emptied `MapDraft` and pushed the empty `FeatureCollection` to the
    /// `geo-draft` source, yet the ring stayed on screen — the same #252-shaped compositor gap, this
    /// time hit by a draft push with no canvas gesture behind it to keep the render loop alive. Forcing
    /// a `redraw()` right after `setData` is what makes the push actually reach the screen.
    #[test]
    fn a_draft_push_forces_a_redraw_so_the_canvas_repaints_with_no_gesture_behind_it() {
        let geojson = draft_geojson(&MapDraft::Empty);
        let script = push_draft_script("geo-map", &geojson);
        let set_data = script.find("draft.setData(").expect("the source is still updated");
        assert!(
            script[set_data..].contains("map.redraw()"),
            "setData is followed by a forced redraw, or the update never composites:\n{script}"
        );
    }

    #[test]
    fn the_init_scripts_load_handler_reapplies_whatever_was_stashed() {
        let script = init_script();
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

    /// #259: a ring used to render as fill + outline with no corners, because the only feature in the
    /// collection was the shape itself and the draft's circle layer filters to `Point`. Each vertex now
    /// gets its own `Point` feature tagged with its index — that tag is what a drag hit-test reads.
    #[test]
    fn every_polygon_vertex_gets_its_own_indexed_handle_feature() {
        let vertices = vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)];
        let geojson = draft_geojson(&MapDraft::Polygon(vertices.clone()));
        let features = geojson["features"].as_array().expect("a feature collection");
        assert_eq!(
            features.len(),
            vertices.len() + 1,
            "one shape feature plus one handle per vertex:\n{geojson}"
        );
        for (index, &(lat, lon)) in vertices.iter().enumerate() {
            let handle = &features[index + 1];
            assert_eq!(
                handle["properties"]["vertex"],
                json!(index),
                "handle {index} names its own position in the ring:\n{geojson}"
            );
            assert_eq!(
                handle["geometry"],
                json!({ "type": "Point", "coordinates": [lon, lat] }),
                "handle {index} sits on its vertex, in GeoJSON lon/lat order:\n{geojson}"
            );
        }
    }

    /// The shape feature stays at index 0: the existing geometry-type assertions address it that way,
    /// and the drag script rewrites `features[0]`'s coordinates as the ring it belongs to.
    #[test]
    fn the_shape_feature_stays_first_ahead_of_the_handles() {
        for (vertices, geometry) in [
            (vec![(60.0, 5.0), (61.0, 5.0)], "LineString"),
            (vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)], "Polygon"),
        ] {
            let geojson = draft_geojson(&MapDraft::Polygon(vertices));
            assert_eq!(geojson["features"][0]["geometry"]["type"], geometry);
            assert_eq!(
                geojson["features"][0]["properties"],
                json!({}),
                "the shape carries no vertex tag, so a hit test can never grab it:\n{geojson}"
            );
        }
    }

    /// One feature, not two: a point draft's own feature *is* its handle. A second coincident feature
    /// would put two hits under one `queryRenderedFeatures` call, and which one comes back first is
    /// not something this code gets to decide.
    #[test]
    fn a_point_draft_is_its_own_handle_rather_than_a_second_coincident_feature() {
        let geojson = draft_geojson(&MapDraft::Point((59.9, 10.7)));
        let features = geojson["features"].as_array().expect("a feature collection");
        assert_eq!(features.len(), 1, "exactly one feature under the hit test:\n{geojson}");
        assert_eq!(features[0]["properties"]["vertex"], json!(0));
        assert_eq!(
            features[0]["geometry"],
            json!({ "type": "Point", "coordinates": [10.7, 59.9] })
        );
    }

    /// The first click of a polygon used to paint nothing at all: a 1-coordinate `LineString` has no
    /// segment to stroke and the point layer filtered it out. Its handle is now the visible feedback
    /// that the click landed.
    #[test]
    fn a_single_vertex_polygon_draft_still_draws_one_handle() {
        let geojson = draft_geojson(&MapDraft::Polygon(vec![(60.0, 5.0)]));
        let features = geojson["features"].as_array().expect("a feature collection");
        assert_eq!(features.len(), 2, "the shape feature plus one handle:\n{geojson}");
        assert_eq!(features[1]["properties"]["vertex"], json!(0));
    }

    /// Index `0` has to survive as the number `0`, not as `false`/absent: the drag script compares it
    /// against `undefined`/`null` precisely because it cannot test it for truthiness.
    #[test]
    fn the_first_handles_index_round_trips_as_the_number_zero() {
        let geojson = draft_geojson(&MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)]));
        let encoded = geojson.to_string();
        let decoded: Value = serde_json::from_str(&encoded).expect("the collection round trips");
        assert_eq!(decoded["features"][1]["properties"]["vertex"], json!(0));
        assert!(
            encoded.contains(r#""vertex":0"#),
            "the tag is encoded as a number:\n{encoded}"
        );
    }

    /// A ring's closing point is `closed_ring`'s rendering concern, not a vertex the operator placed —
    /// so it gets no handle, and handle indices address `MapDraft`'s own unclosed list.
    #[test]
    fn the_rings_duplicated_closing_point_gets_no_handle_of_its_own() {
        let geojson = draft_geojson(&MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)]));
        let ring = geojson["features"][0]["geometry"]["coordinates"][0]
            .as_array()
            .expect("a closed ring");
        assert_eq!(ring.len(), 4, "the rendered ring repeats its first point");
        assert_eq!(
            geojson["features"].as_array().map(Vec::len),
            Some(4),
            "but only three handles exist:\n{geojson}"
        );
    }

    /// The draft outline is a `line` layer over the same source, so without a filter it would try to
    /// stroke every handle `Point` too — and each vertex added would re-stroke the whole ring.
    #[test]
    fn the_draft_outline_layer_ignores_the_handle_features() {
        let script = init_script();
        let layer = script
            .find("id: 'geo-draft-line'")
            .expect("the draft outline layer is added");
        let end = script[layer..].find("});").expect("the addLayer call ends") + layer;
        assert!(
            script[layer..end].contains("filter: ['!=', ['geometry-type'], 'Point']"),
            "the outline skips the handles:\n{}",
            &script[layer..end]
        );
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

    /// Pan offers no draft-action row regardless of what is drawn — there is no tool to commit it.
    #[test]
    fn pan_offers_no_row_for_any_draft() {
        for draft in [
            MapDraft::Empty,
            MapDraft::Point((59.9, 10.7)),
            MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)]),
        ] {
            assert_eq!(draft_actions(DrawTool::Pan, &draft), DraftActions::None);
            assert_eq!(draft_geometry(DrawTool::Pan, &draft), Err(DraftRefusal::Nothing));
        }
    }

    #[test]
    fn point_with_nothing_dropped_yet_offers_no_row() {
        assert_eq!(draft_actions(DrawTool::Point, &MapDraft::Empty), DraftActions::None);
    }

    #[test]
    fn point_with_a_dropped_point_offers_confirm_and_commits_it() {
        let draft = MapDraft::Point((59.9, 10.7));
        assert_eq!(draft_actions(DrawTool::Point, &draft), DraftActions::ConfirmPoint);
        assert_eq!(
            draft_geometry(DrawTool::Point, &draft),
            Ok(PlaceGeometry::Point(geo_point(59.9, 10.7)))
        );
    }

    /// A polygon left over from switching tools cannot be confirmed by the Point tool's own row — it
    /// does not even offer one.
    #[test]
    fn point_over_a_stale_polygon_draft_offers_no_row() {
        let draft = MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)]);
        assert_eq!(draft_actions(DrawTool::Point, &draft), DraftActions::None);
        assert_eq!(draft_geometry(DrawTool::Point, &draft), Err(DraftRefusal::Nothing));
    }

    /// The polygon tool's Finish/Clear row is offered unconditionally — even with nothing drawn yet,
    /// matching today's silent no-op rather than a refusal toast.
    #[test]
    fn polygon_over_an_empty_draft_offers_the_row_but_refuses_silently() {
        assert_eq!(
            draft_actions(DrawTool::Polygon, &MapDraft::Empty),
            DraftActions::FinishPolygon
        );
        assert_eq!(
            draft_geometry(DrawTool::Polygon, &MapDraft::Empty),
            Err(DraftRefusal::Nothing)
        );
    }

    #[test]
    fn polygon_with_fewer_than_three_vertices_earns_the_toast() {
        let draft = MapDraft::Polygon(vec![(60.0, 5.0), (61.0, 5.0)]);
        assert_eq!(draft_actions(DrawTool::Polygon, &draft), DraftActions::FinishPolygon);
        assert_eq!(
            draft_geometry(DrawTool::Polygon, &draft),
            Err(DraftRefusal::TooFewVertices)
        );
    }

    #[test]
    fn polygon_with_three_or_more_vertices_commits_in_draft_order() {
        let vertices = vec![(60.0, 5.0), (61.0, 5.0), (61.0, 6.0)];
        let draft = MapDraft::Polygon(vertices.clone());
        assert_eq!(
            draft_geometry(DrawTool::Polygon, &draft),
            Ok(PlaceGeometry::Polygon {
                exterior: vertices.iter().map(|&(lat, lon)| geo_point(lat, lon)).collect(),
                holes: Vec::new(),
            })
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
