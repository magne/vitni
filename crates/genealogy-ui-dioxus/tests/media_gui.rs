//! SSR assertions for the Phase 8 media GUI (ADR 0017 §GUI): the gallery card (real thumbnail vs
//! glyph fallback by MIME, caption, crop outline), the media viewer overlay (image, zoom controls,
//! percent readout, crop rectangle, Set/Clear region actions), and the media save dialog (fields +
//! live path preview). Rendered to a string over `genealogy-ui`'s framework-free view-models.

use dioxus::prelude::*;
use genealogy_app::Rect;
use genealogy_ui::{Localizer, MediaRefVm, MediaSaveDraft};
use genealogy_ui_dioxus::components::{MediaSaveDialog, MediaSaveLabels, MediaViewer};
use genealogy_ui_dioxus::screens::{media_gallery, media_viewer_labels};

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

fn gallery_view() -> Element {
    let loc = loc();
    let media = vec![image_ref(), document_ref()];
    rsx! {
        {media_gallery(&loc, &media, None, None)}
    }
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
            onset: |_: Rect| {},
            onclear: |()| {},
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
