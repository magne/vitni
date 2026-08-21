//! The media viewer, with an optional interactive crop tool (ADR 0017 §GUI; `media.html`).
//!
//! Shows an image at a chosen zoom inside a scrollable, shrink-wrapped frame. The crop half is opt-in
//! ([`MediaCropTools`]): a caller that can record a region gets a percent-based crop overlay drawn by
//! dragging, a live readout and the Set / Clear actions; a caller that is only looking (the Media
//! record's own preview dialog) passes `crop: None` and gets the same image and zoom controls with no
//! region chrome. Because the geometry is percentages of the frame, the region stays valid at any zoom.
//! The drag math is the framework-free, unit-tested [`rect_from_drag`](vitni_ui::rect_from_drag); the
//! pointer handlers here are thin closures over it. Committing a region (Set region) or clearing it
//! supersedes the owning aggregate's media reference (the caller wires `onset`/`onclear` to a
//! `SetMediaRegion` intent).

use dioxus::prelude::*;
use vitni_app::Rect;
use vitni_ui::{CropCorner, MediaRefVm, rect_contains, rect_css, rect_from_drag, rect_moved, rect_resized};

use crate::components::{Button, ButtonVariant};

/// The zoom steps offered by the viewer toolbar: fit-to-width, then multiples of the image's **own**
/// size, which overflow the canvas and make it scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zoom {
    /// Fill the canvas's width, scaling the image either way. The one container-relative step, which is
    /// what fitting means.
    Fit,
    /// The image's own pixels, 1:1.
    P100,
    /// One and a half times the image's own size.
    P150,
    /// Twice the image's own size.
    P200,
}

impl Zoom {
    /// The frame's full class list for this zoom.
    ///
    /// The frame stays exactly the image's box at every step — it shrink-wraps the image — because it is
    /// the crop overlay's coordinate space and percent geometry is only zoom-invariant (ADR 0017 §GUI)
    /// while the two coincide. Above `Fit` the size comes from the image's **intrinsic** pixels (the CSS
    /// `zoom` property, which unlike `transform` affects layout, so the frame grows and `.mv-canvas`
    /// gets something to scroll). A percentage of the containing block would make the same button mean a
    /// different magnification in the preview dialog than on a record's Media tab, which is exactly what
    /// was reported; `Fit` is the one step that is container-relative, by definition.
    ///
    /// A **class**, not an inline `style`. Measured in the real webview: a width set in the frame's
    /// `style` applies when it mounts and is then never updated again — the same `width:2000px` gives a
    /// 2000px frame as the mount-time value and leaves it at the canvas width as an update. So no zoom
    /// button moved anything at all. Its `class` does update; the toolbar's own active-button classes
    /// change in the very same screenshots.
    const fn frame_class(self) -> &'static str {
        match self {
            Zoom::Fit => "crop-frame img-frame mv-frame zoom-fit",
            Zoom::P100 => "crop-frame img-frame mv-frame zoom-100",
            Zoom::P150 => "crop-frame img-frame mv-frame zoom-150",
            Zoom::P200 => "crop-frame img-frame mv-frame zoom-200",
        }
    }
}

#[cfg(test)]
mod geometry_tests {
    use super::Zoom;

    const STEPS: [Zoom; 4] = [Zoom::Fit, Zoom::P100, Zoom::P150, Zoom::P200];

    #[test]
    fn fit_and_one_hundred_percent_are_not_the_same_geometry() {
        // They rendered identically, and every other pair did too: the geometry was an inline `style`
        // that the frame only ever applied at mount, so whichever step was current when the viewer
        // opened was the only one that took.
        assert_ne!(Zoom::Fit.frame_class(), Zoom::P100.frame_class());
    }

    #[test]
    fn every_step_has_its_own_frame_class() {
        for (index, step) in STEPS.into_iter().enumerate() {
            for other in STEPS.into_iter().skip(index + 1) {
                assert_ne!(
                    step.frame_class(),
                    other.frame_class(),
                    "{step:?} and {other:?} size the frame the same way"
                );
            }
        }
    }

    #[test]
    fn the_zoom_is_a_class_and_carries_no_inline_geometry() {
        // The frame's inline `style` is applied at mount and never updated again (measured in the real
        // webview), so a zoom expressed there moved nothing at all. Its class does update.
        for step in STEPS {
            let class = step.frame_class();
            assert!(!class.contains(':'), "{step:?} is a class list, not a style: {class}");
            assert!(
                class.contains("mv-frame"),
                "{step:?} carries the frame base class the zoom modifiers hang off: {class}"
            );
        }
    }

    #[test]
    fn a_keyboard_nudge_is_scaled_from_percent_to_the_frames_pixels() {
        // The move path takes pixels; handing it the percentage directly nudged the region by one pixel,
        // which rounds to no percent at all and made Shift+arrow silently inert.
        assert_eq!(super::percent_step_px((1.0, 0.0), (1200.0, 600.0)), (12.0, 0.0));
        assert_eq!(super::percent_step_px((0.0, -1.0), (1200.0, 600.0)), (0.0, -6.0));
    }

    #[test]
    fn a_grips_corner_resolves_to_its_own_edges_in_pixels() {
        let rect = vitni_app::Rect {
            left: 10,
            top: 20,
            width: 30,
            height: 40,
        };
        // The `se` grip sits at 40%,60% of a 200x200 frame; a +1% nudge takes it to 41%,61%.
        assert_eq!(
            super::corner_point(rect, super::CropCorner::SouthEast, (200.0, 200.0), (1.0, 1.0)),
            (82.0, 122.0)
        );
        // The `nw` grip is the origin, and an unstepped read is the corner itself.
        assert_eq!(
            super::corner_point(rect, super::CropCorner::NorthWest, (200.0, 200.0), (0.0, 0.0)),
            (20.0, 40.0)
        );
    }

    #[test]
    fn every_step_keeps_the_frame_a_crop_frame_and_an_image_frame() {
        // The crop overlay is positioned against `.crop-frame` and `.crop-rect`'s dimming mask needs
        // its `overflow:hidden`; `.img-frame` carries the frame's own chrome. A zoom modifier must not
        // replace either.
        for step in STEPS {
            let class = step.frame_class();
            assert!(class.contains("crop-frame"), "{step:?}: {class}");
            assert!(class.contains("img-frame"), "{step:?}: {class}");
        }
    }
}

/// The already-localized labels every viewer renders (built by the caller from the `Localizer`): the
/// zoom group and Close, which both callers show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaViewerLabels {
    /// The zoom button group's accessible name.
    pub zoom_group: String,
    /// The fit-to-width zoom label.
    pub fit: String,
    /// The 100% zoom label.
    pub zoom_100: String,
    /// The 150% zoom label.
    pub zoom_150: String,
    /// The 200% zoom label.
    pub zoom_200: String,
    /// The "Close" action label.
    pub close: String,
}

/// The already-localized labels only the crop half renders — required exactly when [`MediaCropTools`]
/// is present, so a look-only caller never has to supply a string it cannot show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaCropLabels {
    /// The readout prefix (e.g. "region").
    pub region: String,
    /// The readout shown when no region is set.
    pub no_region: String,
    /// The "Set region" action label.
    pub set_region: String,
    /// The "Clear region" action label.
    pub clear_region: String,
    /// The four corner grips' accessible names, in `nw, ne, sw, se` order.
    pub handles: [String; 4],
    /// The grips' shared tooltip, naming the keyboard gestures.
    pub handle_hint: String,
}

impl MediaCropLabels {
    /// The accessible name of one corner grip.
    fn handle(&self, corner: CropCorner) -> String {
        let index = match corner {
            CropCorner::NorthWest => 0,
            CropCorner::NorthEast => 1,
            CropCorner::SouthWest => 2,
            CropCorner::SouthEast => 3,
        };
        self.handles.get(index).cloned().unwrap_or_default()
    }
}

/// The crop half of the viewer: present when the caller can record a region, absent when the viewer is
/// only for looking (the Media record's own preview). Its absence removes the readout, the Set/Clear
/// actions, the drag-capture layer and the region rectangle — the image and the zoom group are shared.
#[derive(Clone, PartialEq)]
pub struct MediaCropTools {
    /// The crop-only labels.
    pub labels: MediaCropLabels,
    /// Fired when the operator commits a region (Set region).
    pub onset: EventHandler<Rect>,
    /// Fired when the operator clears the region (Clear region).
    pub onclear: EventHandler<()>,
}

/// The percent readout for a region (e.g. `region · 22,18 → 61,44 %`), or the no-region text.
fn readout(labels: &MediaCropLabels, region: Option<Rect>) -> String {
    match region {
        Some(rect) => format!(
            "{} · {},{} → {},{} %",
            labels.region,
            rect.left,
            rect.top,
            u16::from(rect.left) + u16::from(rect.width),
            u16::from(rect.top) + u16::from(rect.height),
        ),
        None => labels.no_region.clone(),
    }
}

/// The crop half's toolbar actions — Set region (enabled once a region is drawn) and Clear region.
/// Renders nothing for a look-only viewer.
fn crop_actions(tools: Option<&MediaCropTools>, mut region: Signal<Option<Rect>>) -> Element {
    let Some(tools) = tools else {
        return rsx! {};
    };
    let onset = tools.onset;
    let onclear = tools.onclear;
    rsx! {
        Button {
            label: tools.labels.set_region.clone(),
            small: true,
            disabled: region().is_none(),
            onclick: move |_| {
                if let Some(rect) = region() {
                    onset.call(rect);
                }
            },
        }
        Button {
            label: tools.labels.clear_region.clone(),
            variant: ButtonVariant::Ghost,
            small: true,
            // Clearing nothing is not a change: `onclear` fires the caller's `SetMediaRegion(None)`,
            // which would write an event asserting the region a record already does not have.
            disabled: region().is_none(),
            onclick: move |_| {
                region.set(None);
                onclear.call(());
            },
        }
    }
}

/// Which gesture a press started. All three end up in the same `region` signal, so the readout, the
/// rectangle and `Set region` need no knowledge of how the region was arrived at.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CropDrag {
    /// Drawing a fresh region: the press landed on empty frame.
    Draw {
        /// Where the press landed, in frame pixels — the region's anchored corner.
        start: (f64, f64),
    },
    /// Moving the whole region: the press landed inside it.
    Move {
        /// Where the press landed, in frame pixels.
        start: (f64, f64),
        /// The region as it was when the press landed, so the move stays a pure translation of it
        /// rather than accumulating each frame's rounding.
        origin: Rect,
    },
    /// Resizing by a corner grip.
    Resize {
        /// The grip being dragged; the opposite corner is the anchor.
        corner: CropCorner,
        /// The region as it was when the grip was grabbed — the resize is absolute against the live
        /// pointer, so applying it to the original each time cannot drift.
        origin: Rect,
    },
}

/// The region the pointer is now describing, or `None` if the gesture cannot produce one.
fn dragged_region(drag: CropDrag, point: (f64, f64), bounds: (f64, f64)) -> Option<Rect> {
    match drag {
        CropDrag::Draw { start } => rect_from_drag(start, point, bounds),
        CropDrag::Move { start, origin } => rect_moved(origin, (point.0 - start.0, point.1 - start.1), bounds),
        CropDrag::Resize { corner, origin } => rect_resized(origin, corner, point, bounds),
    }
}

/// One corner grip of the region: a drag target for the pointer and a focus stop for the keyboard.
///
/// A `<button>` rather than the mockup's `<span>` so the gesture is reachable without a pointer at all —
/// the arrow keys nudge the grip's own corner and `Shift` + arrow moves the whole region, both by one
/// percent, which is [`vitni_ui::rect_resized`]'s and [`vitni_ui::rect_moved`]'s minimum step.
fn crop_handle(corner: CropCorner, labels: &MediaCropLabels, state: CropGrips) -> Element {
    let CropGrips {
        mut region,
        mut drag,
        frame_size,
    } = state;
    let Some(origin) = region() else {
        return rsx! {};
    };
    let class = match corner {
        CropCorner::NorthWest => "crop-handle nw",
        CropCorner::NorthEast => "crop-handle ne",
        CropCorner::SouthWest => "crop-handle sw",
        CropCorner::SouthEast => "crop-handle se",
    };
    rsx! {
        button {
            class,
            r#type: "button",
            aria_label: labels.handle(corner),
            title: labels.handle_hint.clone(),
            onpointerdown: move |_| drag.set(Some(CropDrag::Resize { corner, origin })),
            onpointerup: move |_| drag.set(None),
            onkeydown: move |event: KeyboardEvent| {
                let Some(step) = arrow_step(&event) else { return };
                let Some(current) = region() else { return };
                let bounds = frame_size();
                let next = if event.modifiers().shift() {
                    rect_moved(current, percent_step_px(step, bounds), bounds)
                } else {
                    rect_resized(current, corner, corner_point(current, corner, bounds, step), bounds)
                };
                if let Some(rect) = next {
                    event.prevent_default();
                    region.set(Some(rect));
                }
            },
        }
    }
}

/// One percent, in the direction an arrow key points, or `None` for any other key. One percent because
/// that is the geometry's own resolution — a smaller step rounds away. The unit is *percent*: the resize
/// path adds it to a corner's percentage, and the move path scales it by the frame first.
fn arrow_step(event: &KeyboardEvent) -> Option<(f64, f64)> {
    match event.key() {
        Key::ArrowLeft => Some((-1.0, 0.0)),
        Key::ArrowRight => Some((1.0, 0.0)),
        Key::ArrowUp => Some((0.0, -1.0)),
        Key::ArrowDown => Some((0.0, 1.0)),
        _ => None,
    }
}

/// A percentage step in frame pixels. `rect_moved` takes pixels while [`arrow_step`] speaks percent, and
/// passing the percentage straight through moves the region by a *pixel* — 0.08% of a 1213px frame,
/// which rounds away to no move at all.
fn percent_step_px(step: (f64, f64), bounds: (f64, f64)) -> (f64, f64) {
    (step.0 / 100.0 * bounds.0, step.1 / 100.0 * bounds.1)
}

/// Where `corner` sits in frame pixels, offset by `step` percent — the synthetic pointer position a
/// keyboard nudge resizes to.
fn corner_point(rect: Rect, corner: CropCorner, bounds: (f64, f64), step: (f64, f64)) -> (f64, f64) {
    let (width, height) = bounds;
    let x_pct = match corner {
        CropCorner::NorthWest | CropCorner::SouthWest => f64::from(rect.left),
        CropCorner::NorthEast | CropCorner::SouthEast => f64::from(rect.left.saturating_add(rect.width)),
    };
    let y_pct = match corner {
        CropCorner::NorthWest | CropCorner::NorthEast => f64::from(rect.top),
        CropCorner::SouthWest | CropCorner::SouthEast => f64::from(rect.top.saturating_add(rect.height)),
    };
    ((x_pct + step.0) / 100.0 * width, (y_pct + step.1) / 100.0 * height)
}

/// The signals the corner grips share with the capture layer.
#[derive(Clone, Copy)]
struct CropGrips {
    /// The live region.
    region: Signal<Option<Rect>>,
    /// The gesture in progress, if any.
    drag: Signal<Option<CropDrag>>,
    /// The frame's observed box, in pixels.
    frame_size: Signal<(f64, f64)>,
}

/// The gesture layer over the frame plus the region rectangle and its four corner grips. Renders
/// nothing for a look-only viewer, so a plain preview has no invisible capture layer over its image.
///
/// Three gestures share one capture layer, told apart at `onpointerdown` by
/// [`vitni_ui::rect_contains`]: a press inside the region moves it, a press anywhere else draws a fresh
/// one, and a press on a grip (the only descendant of the `pointer-events:none` rectangle that takes
/// events) resizes it. Hit-testing rather than putting handlers on the rectangle is what keeps drawing a
/// fresh region working over the top of an existing one's interior.
///
/// `frame_size` is the frame's observed box (see the `onresize` on the frame itself), never measured
/// from here: a measurement started at `onpointerdown` resolves *after* the `onpointermove` that
/// follows it, so the first move of a drag would read the size the frame had before it was ever
/// measured — `(0.0, 0.0)`, which the geometry rejects.
fn crop_overlay(tools: Option<&MediaCropTools>, state: CropGrips) -> Element {
    let Some(tools) = tools else {
        return rsx! {};
    };
    let CropGrips {
        mut region,
        mut drag,
        frame_size,
    } = state;
    let labels = tools.labels.clone();
    rsx! {
        div {
            class: "crop-capture",
            style: "position:absolute;inset:0",
            onpointerdown: move |event: PointerEvent| {
                let point = event.element_coordinates();
                let start = (point.x, point.y);
                let inside = region().filter(|rect| rect_contains(*rect, start, frame_size()));
                drag.set(Some(match inside {
                    Some(origin) => CropDrag::Move { start, origin },
                    None => CropDrag::Draw { start },
                }));
            },
            onpointermove: move |event: PointerEvent| {
                let Some(gesture) = drag() else { return };
                let point = event.element_coordinates();
                if let Some(rect) = dragged_region(gesture, (point.x, point.y), frame_size()) {
                    region.set(Some(rect));
                }
            },
            onpointerup: move |_| drag.set(None),
        }
        if let Some(rect) = region() {
            div { class: "crop-rect", style: rect_css(&rect),
                {crop_handle(CropCorner::NorthWest, &labels, state)}
                {crop_handle(CropCorner::NorthEast, &labels, state)}
                {crop_handle(CropCorner::SouthWest, &labels, state)}
                {crop_handle(CropCorner::SouthEast, &labels, state)}
            }
        }
    }
}

/// The media viewer: zoom controls, the image, the Close action, and — when the caller passes
/// [`MediaCropTools`] — a drag-to-draw crop overlay with its live percent readout and Set / Clear
/// actions. Controlled by the caller, which owns whether it is shown and handles the committed region.
#[component]
pub fn MediaViewer(
    /// The media reference being viewed (its image source + existing crop).
    item: MediaRefVm,
    /// The already-localized labels every viewer shows.
    labels: MediaViewerLabels,
    /// The crop half, when the caller can record a region; `None` for a look-only viewer.
    crop: Option<MediaCropTools>,
    /// Fired when the operator closes the viewer.
    onclose: EventHandler<()>,
) -> Element {
    let mut zoom = use_signal(|| Zoom::Fit);
    let region = use_signal(|| item.crop);
    let drag = use_signal(|| None::<CropDrag>);
    let mut frame_size = use_signal(|| (0.0_f64, 0.0_f64));

    let src = item.src();
    let is_image = item.is_image();

    let zoom_button = |this: Zoom, label: &str| {
        let active = zoom() == this;
        let class = if active { "btn sm primary" } else { "btn sm" };
        rsx! {
            button {
                class,
                r#type: "button",
                aria_pressed: if active { "true" } else { "false" },
                onclick: move |_| zoom.set(this),
                "{label}"
            }
        }
    };

    rsx! {
        div { class: "card media-viewer",
            div { class: "mv-toolbar",
                span { class: "mv-zoom", role: "group", aria_label: "{labels.zoom_group}",
                    {zoom_button(Zoom::Fit, &labels.fit)}
                    {zoom_button(Zoom::P100, &labels.zoom_100)}
                    {zoom_button(Zoom::P150, &labels.zoom_150)}
                    {zoom_button(Zoom::P200, &labels.zoom_200)}
                }
                if let Some(tools) = crop.as_ref() {
                    span { class: "mv-readout", "{readout(&tools.labels, region())}" }
                }
                span { class: "spacer" }
                {crop_actions(crop.as_ref(), region)}
                Button {
                    label: labels.close.clone(),
                    variant: ButtonVariant::Ghost,
                    small: true,
                    onclick: move |_| onclose.call(()),
                }
            }
            div { class: "mv-canvas",
                div {
                    class: zoom().frame_class(),
                    // The crop overlay's coordinate space, observed rather than measured: the frame's
                    // box changes when the image lays out and again on every zoom step, and a
                    // `ResizeObserver` reports each change as it happens. `get_client_rect()` could
                    // only be read at a moment someone chose, and the moments available — mount
                    // (before the image has laid out) and drag start (after the first move is
                    // handled) — are both the wrong one.
                    onresize: move |event: ResizeEvent| {
                        if let Ok(size) = event.get_border_box_size() {
                            frame_size.set((size.width, size.height));
                        }
                    },
                    if is_image {
                        if let Some(src) = src.clone() {
                            img { class: "media-full", src: "{src}" }
                        }
                    } else {
                        div { class: "img-glyph", aria_hidden: "true", "🗎" }
                    }
                    {crop_overlay(crop.as_ref(), CropGrips { region, drag, frame_size })}
                }
            }
        }
    }
}
