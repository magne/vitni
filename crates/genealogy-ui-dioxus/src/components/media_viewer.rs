//! The media viewer overlay with the interactive crop tool (ADR 0017 §GUI; `media.html`).
//!
//! Shows an image at a chosen zoom inside a scrollable, shrink-wrapped frame, with a percent-based
//! crop overlay drawn by dragging. Because the geometry is percentages of the frame, the region stays
//! valid at any zoom. The drag math is the framework-free, unit-tested
//! [`rect_from_drag`](genealogy_ui::rect_from_drag); the pointer handlers here are thin closures over
//! it. Committing a region (Set region) or clearing it supersedes the owning aggregate's media
//! reference (the caller wires `onset`/`onclear` to a `SetMediaRegion` intent).

use dioxus::prelude::*;
use genealogy_app::Rect;
use genealogy_ui::{MediaRefVm, rect_css, rect_from_drag};

use crate::components::{Button, ButtonVariant};

/// The zoom steps offered by the viewer toolbar: fit-to-width, then fixed percentages that overflow
/// the scroll container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zoom {
    /// Scale the image down to fit the frame width.
    Fit,
    /// 100% of the frame width.
    P100,
    /// 150% of the frame width.
    P150,
    /// 200% of the frame width.
    P200,
}

impl Zoom {
    /// The inline `style` that sizes the image for this zoom.
    const fn image_style(self) -> &'static str {
        match self {
            Zoom::Fit => "max-width:100%;height:auto",
            Zoom::P100 => "width:100%;height:auto",
            Zoom::P150 => "width:150%;height:auto",
            Zoom::P200 => "width:200%;height:auto",
        }
    }
}

/// The already-localized labels the viewer renders (built by the caller from the `Localizer`).
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
    /// The readout prefix (e.g. "region").
    pub region: String,
    /// The readout shown when no region is set.
    pub no_region: String,
    /// The "Set region" action label.
    pub set_region: String,
    /// The "Clear region" action label.
    pub clear_region: String,
    /// The "Close" action label.
    pub close: String,
}

/// Reads the crop frame's on-screen box into `frame_size` (a no-op under SSR, where `get_client_rect`
/// returns `MountedError::NotSupported`). Called at mount and again at drag start, since the frame
/// shrink-wraps an image that lays out after mount.
fn measure_frame(node: MountedEvent, mut frame_size: Signal<(f64, f64)>) {
    spawn(async move {
        if let Ok(rect) = node.get_client_rect().await {
            frame_size.set((rect.size.width, rect.size.height));
        }
    });
}

/// The percent readout for a region (e.g. `region · 22,18 → 61,44 %`), or the no-region text.
fn readout(labels: &MediaViewerLabels, region: Option<Rect>) -> String {
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

/// The media viewer overlay: zoom controls, the image, a drag-to-draw crop overlay, a live percent
/// readout, and the Set / Clear / Close actions. Controlled by the caller, which owns whether it is
/// shown and handles the committed region.
#[component]
pub fn MediaViewer(
    /// The media reference being viewed (its image source + existing crop).
    item: MediaRefVm,
    /// The already-localized labels.
    labels: MediaViewerLabels,
    /// Fired when the operator commits a region (Set region).
    onset: EventHandler<Rect>,
    /// Fired when the operator clears the region (Clear region).
    onclear: EventHandler<()>,
    /// Fired when the operator closes the viewer.
    onclose: EventHandler<()>,
) -> Element {
    let mut zoom = use_signal(|| Zoom::Fit);
    let mut region = use_signal(|| item.crop);
    let mut drag_start = use_signal(|| None::<(f64, f64)>);
    let frame_size = use_signal(|| (0.0_f64, 0.0_f64));
    // The crop frame's mounted element, kept so the drag can re-measure it: the frame shrink-wraps the
    // image, which lays out *after* `onmounted` fires, so the initial measure is stale/zero.
    let mut frame_el = use_signal(|| None::<MountedEvent>);

    let src = item.src();
    let is_image = item.is_image();
    let current = region();

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
                span { class: "mv-readout", "{readout(&labels, current)}" }
                span { class: "spacer" }
                Button {
                    label: labels.set_region.clone(),
                    small: true,
                    disabled: current.is_none(),
                    onclick: move |_| {
                        if let Some(rect) = region() {
                            onset.call(rect);
                        }
                    },
                }
                Button {
                    label: labels.clear_region.clone(),
                    variant: ButtonVariant::Ghost,
                    small: true,
                    onclick: move |_| {
                        region.set(None);
                        onclear.call(());
                    },
                }
                Button {
                    label: labels.close.clone(),
                    variant: ButtonVariant::Ghost,
                    small: true,
                    onclick: move |_| onclose.call(()),
                }
            }
            div { class: "mv-canvas",
                div {
                    class: "crop-frame img-frame",
                    style: "display:inline-block;position:relative",
                    onmounted: move |event: MountedEvent| {
                        frame_el.set(Some(event.clone()));
                        measure_frame(event, frame_size);
                    },
                    if is_image {
                        if let Some(src) = src.clone() {
                            img { class: "media-full", src: "{src}", style: zoom().image_style() }
                        }
                    } else {
                        div { class: "img-glyph", aria_hidden: "true", "🗎" }
                    }
                    div {
                        class: "crop-capture",
                        style: "position:absolute;inset:0",
                        onpointerdown: move |event: PointerEvent| {
                            // Re-measure at drag time: by now the image has laid out, so the frame's
                            // box is its true size (the `onmounted` measure ran before load).
                            if let Some(frame) = frame_el() {
                                measure_frame(frame, frame_size);
                            }
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
                    if let Some(rect) = current {
                        div { class: "crop-rect", style: rect_css(&rect) }
                    }
                }
            }
        }
    }
}
