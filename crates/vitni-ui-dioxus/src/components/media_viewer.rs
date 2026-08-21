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
use vitni_ui::{MediaRefVm, rect_css, rect_from_drag};

use crate::components::{Button, ButtonVariant};

/// The zoom steps offered by the viewer toolbar: fit-to-width, then fixed percentages of the canvas
/// width that overflow it and make it scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zoom {
    /// Show the image at its natural size, shrunk if it is wider than the canvas.
    Fit,
    /// 100% of the canvas width.
    P100,
    /// 150% of the canvas width.
    P150,
    /// 200% of the canvas width.
    P200,
}

impl Zoom {
    /// The frame's full class list for this zoom.
    ///
    /// The zoom sizes the **frame**, not the image: the frame is the crop overlay's coordinate space,
    /// so percent geometry is only zoom-invariant (ADR 0017 §GUI) while the frame stays exactly the
    /// image's box. `zoom-fit` shrink-wraps the image's natural size, capped at the canvas; every other
    /// step is a definite percentage of `.mv-canvas`'s content box, which is what makes the canvas
    /// scroll instead of the frame clipping.
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
mod zoom_geometry_tests {
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

/// The drag-to-draw layer over the frame plus the region rectangle it draws. Renders nothing for a
/// look-only viewer, so a plain preview has no invisible capture layer over its image.
///
/// `frame_size` is the frame's observed box (see the `onresize` on the frame itself), never measured
/// from here: a measurement started at `onpointerdown` resolves *after* the `onpointermove` that
/// follows it, so the first move of a drag would read the size the frame had before it was ever
/// measured — `(0.0, 0.0)`, which [`rect_from_drag`] rejects.
fn crop_overlay(
    active: bool,
    mut region: Signal<Option<Rect>>,
    mut drag_start: Signal<Option<(f64, f64)>>,
    frame_size: Signal<(f64, f64)>,
) -> Element {
    if !active {
        return rsx! {};
    }
    rsx! {
        div {
            class: "crop-capture",
            style: "position:absolute;inset:0",
            onpointerdown: move |event: PointerEvent| {
                let point = event.element_coordinates();
                drag_start.set(Some((point.x, point.y)));
            },
            onpointermove: move |event: PointerEvent| {
                let Some(start) = drag_start() else { return };
                let point = event.element_coordinates();
                if let Some(rect) = rect_from_drag(start, (point.x, point.y), frame_size()) {
                    region.set(Some(rect));
                }
            },
            onpointerup: move |_| drag_start.set(None),
        }
        if let Some(rect) = region() {
            div { class: "crop-rect", style: rect_css(&rect) }
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
    let drag_start = use_signal(|| None::<(f64, f64)>);
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
                    {crop_overlay(crop.is_some(), region, drag_start, frame_size)}
                }
            }
        }
    }
}
