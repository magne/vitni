//! SSR assertions for the Phase 8 media GUI (ADR 0017 §GUI): the gallery card (real thumbnail vs
//! glyph fallback by MIME, caption, crop outline), the media viewer in both shapes (with crop tools:
//! image, zoom controls, percent readout, crop rectangle, Set/Clear region actions — and without them,
//! zoom and Close alone), and the media save dialog (fields + live path preview). Rendered to a string
//! over `vitni-ui`'s framework-free view-models.

use dioxus::prelude::*;
use vitni_app::Rect;
use vitni_ui::{Localizer, MediaRefVm, MediaSaveDraft};
use vitni_ui_dioxus::components::{MediaCropTools, MediaSaveDialog, MediaSaveLabels, MediaViewer};
use vitni_ui_dioxus::screens::{media_crop_labels, media_gallery, media_viewer_labels};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// An attached image with a caption and an existing crop region.
fn image_ref() -> MediaRefVm {
    MediaRefVm {
        human_id: "O0050".to_owned(),
        assertion_id: "attach-1".to_owned(),
        caption: Some("John Smith · portrait".to_owned()),
        crop: Some(Rect {
            left: 30,
            top: 14,
            width: 40,
            height: 46,
        }),
        path: Some("portraits/john.jpg".to_owned()),
        mime: Some("image/jpeg".to_owned()),
    }
}

/// An attached non-image (a PDF), which falls back to a glyph placeholder.
fn document_ref() -> MediaRefVm {
    MediaRefVm {
        human_id: "O0051".to_owned(),
        assertion_id: "attach-2".to_owned(),
        caption: Some("1880 census".to_owned()),
        crop: None,
        path: Some("scans/census.pdf".to_owned()),
        mime: Some("application/pdf".to_owned()),
    }
}

/// An attached image as the store actually records one: the stored path carries the `media/` prefix and
/// no MIME was ever recorded (the CLI cannot set one) — both of #301's live causes at once.
fn stored_image_ref() -> MediaRefVm {
    MediaRefVm {
        human_id: "O0052".to_owned(),
        assertion_id: "attach-3".to_owned(),
        caption: Some("Ada · portrait".to_owned()),
        crop: None,
        path: Some("media/portraits/ada.jpg".to_owned()),
        mime: None,
    }
}

/// An attached image whose stored name carries `æøå` and a space — what `slugify` and the plugin host's
/// `sanitize_component` actually produce, and what the operator's own library is full of.
fn nordic_image_ref() -> MediaRefVm {
    MediaRefVm {
        human_id: "O0053".to_owned(),
        assertion_id: "attach-4".to_owned(),
        caption: Some("Asbjørn · folketelling".to_owned()),
        crop: None,
        path: Some("media/02_folketelling/1920 greipstad_bergstøl-asbjørn.jpg".to_owned()),
        mime: None,
    }
}

fn nordic_gallery_view() -> Element {
    let loc = loc();
    let media = vec![nordic_image_ref()];
    rsx! {
        {media_gallery(&loc, &media, None, None)}
    }
}

fn nordic_viewer_view() -> Element {
    let loc = loc();
    rsx! {
        MediaViewer {
            item: nordic_image_ref(),
            labels: media_viewer_labels(&loc),
            crop: Some(MediaCropTools {
                labels: media_crop_labels(&loc),
                onset: EventHandler::new(|_: Rect| {}),
                onclear: EventHandler::new(|()| {}),
            }),
            onclose: |()| {},
        }
    }
}

#[test]
fn a_nordic_or_spaced_filename_is_served_percent_encoded_in_the_gallery_and_the_viewer() {
    let expected = "src=\"/media/02_folketelling/1920%20greipstad_bergst%C3%B8l-asbj%C3%B8rn.jpg\"";
    for (name, html) in [
        ("gallery", render(nordic_gallery_view)),
        ("viewer", render(nordic_viewer_view)),
    ] {
        assert!(html.contains(expected), "{name} img src is percent-encoded: {html}");
        assert!(
            !html.contains("bergstøl"),
            "{name} sends no raw non-ASCII byte to the asset handler: {html}"
        );
    }
}

fn gallery_view() -> Element {
    let loc = loc();
    let media = vec![image_ref(), document_ref()];
    rsx! {
        {media_gallery(&loc, &media, None, None)}
    }
}

fn stored_gallery_view() -> Element {
    let loc = loc();
    let media = vec![stored_image_ref()];
    rsx! {
        {media_gallery(&loc, &media, None, None)}
    }
}

#[test]
fn a_gallery_thumbnail_of_a_stored_image_needs_no_recorded_mime_and_gets_one_prefix() {
    let html = render(stored_gallery_view);
    assert!(
        html.contains("src=\"/media/portraits/ada.jpg\""),
        "the stored `media/` prefix is not doubled (#301 cause 2): {html}"
    );
    assert!(
        !html.contains("/media/media/"),
        "the prefix is added exactly once: {html}"
    );
    assert!(
        !html.contains("🗎"),
        "the extension classifies the image with no recorded MIME (#301 cause 1): {html}"
    );
}

#[test]
fn gallery_renders_thumbnail_caption_and_crop_outline() {
    let html = render(gallery_view);
    // The image gets a real <img> served by the asset handler.
    assert!(
        html.contains("src=\"/media/portraits/john.jpg\""),
        "image thumbnail src: {html}"
    );
    assert!(html.contains("loading=\"lazy\""), "thumbnails are lazy: {html}");
    // The existing crop renders as a dashed outline with percent geometry.
    assert!(html.contains("crop-outline"), "crop outline present: {html}");
    assert!(
        html.contains("left:30%;top:14%;width:40%;height:46%"),
        "crop geometry: {html}"
    );
    // Captions render.
    assert!(html.contains("John Smith · portrait"), "image caption: {html}");
    assert!(html.contains("1880 census"), "document caption: {html}");
    // The non-image falls back to the glyph, not an <img>.
    assert!(
        !html.contains("scans/census.pdf"),
        "a non-image is not rendered as an img: {html}"
    );
    assert!(html.contains("🗎"), "document glyph fallback: {html}");
}

fn viewer_view() -> Element {
    let loc = loc();
    rsx! {
        MediaViewer {
            item: image_ref(),
            labels: media_viewer_labels(&loc),
            crop: Some(MediaCropTools {
                labels: media_crop_labels(&loc),
                onset: EventHandler::new(|_: Rect| {}),
                onclear: EventHandler::new(|()| {}),
            }),
            onclose: |()| {},
        }
    }
}

/// The crop viewer over an image that carries no region yet — the state every media reference starts
/// in, and the one both region actions have to be inert in.
fn viewer_view_no_region() -> Element {
    let loc = loc();
    let item = MediaRefVm {
        crop: None,
        ..image_ref()
    };
    rsx! {
        MediaViewer {
            item,
            labels: media_viewer_labels(&loc),
            crop: Some(MediaCropTools {
                labels: media_crop_labels(&loc),
                onset: EventHandler::new(|_: Rect| {}),
                onclear: EventHandler::new(|()| {}),
            }),
            onclose: |()| {},
        }
    }
}

/// The viewer without crop tools — the Media record's own preview dialog, which is only for looking.
fn viewer_view_zoom_only() -> Element {
    let loc = loc();
    rsx! {
        MediaViewer {
            item: image_ref(),
            labels: media_viewer_labels(&loc),
            crop: None,
            onclose: |()| {},
        }
    }
}

#[test]
fn viewer_renders_image_zoom_controls_readout_and_actions() {
    let html = render(viewer_view);
    assert!(
        html.contains("src=\"/media/portraits/john.jpg\""),
        "viewer image: {html}"
    );
    // Zoom controls.
    assert!(html.contains("Fit"), "fit control: {html}");
    assert!(
        html.contains("100%") && html.contains("150%") && html.contains("200%"),
        "zoom steps: {html}"
    );
    // The percent readout reflects the existing crop (30,14 → 70,60).
    assert!(html.contains("30,14 → 70,60"), "region readout: {html}");
    // The crop rectangle overlay.
    assert!(html.contains("crop-rect"), "crop rectangle: {html}");
    assert!(
        html.contains("left:30%;top:14%;width:40%;height:46%"),
        "crop geometry: {html}"
    );
    // The region actions.
    assert!(html.contains("Set region"), "set region action: {html}");
    assert!(html.contains("Clear region"), "clear region action: {html}");
}

/// The `<button …>` opening tag carrying `label`, so a test can read the attributes on it rather than
/// searching the whole document for a `disabled` that may belong to any other control.
fn button_tag(html: &str, label: &str) -> String {
    let Some(end) = html.find(&format!(">{label}<")) else {
        return String::new();
    };
    let Some(start) = html[..end].rfind("<button") else {
        return String::new();
    };
    html[start..end].to_owned()
}

#[test]
fn both_region_actions_are_inert_until_there_is_a_region_to_act_on() {
    // Clear had no `disabled:` at all, so pressing it with nothing selected called `onclear` — and the
    // caller's `SetMediaRegion(None)` wrote an event for a no-op change.
    let html = render(viewer_view_no_region);
    for label in ["Set region", "Clear region"] {
        assert!(
            button_tag(&html, label).contains("disabled"),
            "{label:?} is disabled with no region set: {html}"
        );
    }
}

#[test]
fn both_region_actions_are_live_once_a_region_exists() {
    let html = render(viewer_view);
    for label in ["Set region", "Clear region"] {
        let tag = button_tag(&html, label);
        assert!(!tag.is_empty(), "{label:?} renders as a button at all: {html}");
        assert!(
            !tag.contains("disabled"),
            "{label:?} is live once a region is set: {html}"
        );
    }
}

#[test]
fn a_viewer_without_crop_tools_keeps_zoom_and_close_and_drops_every_region_control() {
    let html = render(viewer_view_zoom_only);
    assert!(
        html.contains("src=\"/media/portraits/john.jpg\""),
        "the image still renders: {html}"
    );
    for needle in ["Fit", "100%", "150%", "200%", "Close"] {
        assert!(html.contains(needle), "expected {needle:?} in: {html}");
    }
    for needle in ["mv-readout", "crop-rect", "crop-capture", "Set region", "Clear region"] {
        assert!(
            !html.contains(needle),
            "a look-only viewer renders no {needle:?}: {html}"
        );
    }
}

fn save_labels() -> MediaSaveLabels {
    MediaSaveLabels {
        title: "Add file to the media library".to_owned(),
        choose_category: "Choose a category".to_owned(),
        category: "Category".to_owned(),
        subfolder: "Subfolder".to_owned(),
        filename: "Filename".to_owned(),
        path_preview: "Target path".to_owned(),
        save: "Add to library".to_owned(),
        cancel: "Cancel".to_owned(),
        dismiss: "Dismiss".to_owned(),
    }
}

fn save_dialog_view() -> Element {
    let draft = use_signal(|| MediaSaveDraft {
        category: "05_personbilder".to_owned(),
        subfolder: String::new(),
        filename: "1900_bergen_ada.jpg".to_owned(),
    });
    rsx! {
        MediaSaveDialog {
            open: true,
            labels: save_labels(),
            categories: vec!["01_kirkebok".to_owned(), "05_personbilder".to_owned()],
            draft,
            onsave: |_: String| {},
            oncancel: |()| {},
        }
    }
}

#[test]
fn save_dialog_renders_fields_and_live_path_preview() {
    let html = render(save_dialog_view);
    assert!(html.contains("Add file to the media library"), "dialog title: {html}");
    assert!(
        html.contains("Category") && html.contains("Filename"),
        "field labels: {html}"
    );
    // The live path preview joins the draft parts under the media root, skipping the blank subfolder.
    assert!(
        html.contains("media/05_personbilder/1900_bergen_ada.jpg"),
        "path preview: {html}"
    );
    assert!(html.contains("Add to library"), "save action: {html}");
}
