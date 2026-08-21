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
            onclick: move |_| {
                region.set(None);
                onclear.call(());
            },
        }
    }
}

/// The drag-to-draw layer over the frame plus the region rectangle it draws. Renders nothing for a
/// look-only viewer, so a plain preview has no invisible capture layer over its image.
fn crop_overlay(
    active: bool,
    mut region: Signal<Option<Rect>>,
    mut drag_start: Signal<Option<(f64, f64)>>,
    frame_size: Signal<(f64, f64)>,
    frame_el: Signal<Option<MountedEvent>>,
) -> Element {
    if !active {
        return rsx! {};
    }
    rsx! {
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
    let frame_size = use_signal(|| (0.0_f64, 0.0_f64));
    // The crop frame's mounted element, kept so the drag can re-measure it: the frame shrink-wraps the
    // image, which lays out *after* `onmounted` fires, so the initial measure is stale/zero.
    let mut frame_el = use_signal(|| None::<MountedEvent>);

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
                    {crop_overlay(crop.is_some(), region, drag_start, frame_size, frame_el)}
                }
            }
        }
    }
}
