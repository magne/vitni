//! SSR assertions for the shared `SidePanel`/`Modal` overlay containers (viewport-overflow fix):
//! the side panel renders a click-away scrim alongside its head/body/foot regions, and both
//! containers render nothing while closed. The viewport-bounded scroll (absolute positioning
//! against `.detail`, `.sp-body`/`.m-body` scrolling, the pinned head/foot) is CSS-only and not
//! SSR-observable — verified by running the app, same constraint as the floating record-picker's
//! measured position in `record_picker.rs`.

use dioxus::prelude::*;
use genealogy_ui_dioxus::components::{Modal, SidePanel};

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn open_side_panel_view() -> Element {
    let onclose = use_callback(|_: MouseEvent| {});
    rsx! {
        SidePanel {
            title: "Edit name".to_owned(),
            open: true,
            close_label: "Close".to_owned(),
            onclose,
            footer: rsx! { button { "Save" } },
            div { "body" }
        }
    }
}

fn closed_side_panel_view() -> Element {
    let onclose = use_callback(|_: MouseEvent| {});
    rsx! {
        SidePanel {
            title: "Edit name".to_owned(),
            open: false,
            close_label: "Close".to_owned(),
            onclose,
            footer: rsx! { button { "Save" } },
            div { "body" }
        }
    }
}

fn open_modal_view() -> Element {
    rsx! {
        Modal {
            title: "Confirm".to_owned(),
            open: true,
            footer: rsx! { button { "OK" } },
            div { "body" }
        }
    }
}

fn closed_modal_view() -> Element {
    rsx! {
        Modal {
            title: "Confirm".to_owned(),
            open: false,
            footer: rsx! { button { "OK" } },
            div { "body" }
        }
    }
}

#[test]
fn an_open_side_panel_renders_a_click_away_scrim() {
    let html = render(open_side_panel_view);
    assert!(
        html.contains(r#"class="sidepanel-scrim""#),
        "the click-away scrim renders while open:\n{html}"
    );
    assert!(html.contains("sidepanel"), "the panel surface renders:\n{html}");
    assert!(html.contains("sp-head"), "the panel head renders:\n{html}");
    assert!(html.contains("sp-body"), "the panel body renders:\n{html}");
    assert!(html.contains("sp-foot"), "the panel footer renders:\n{html}");
}

#[test]
fn a_closed_side_panel_renders_nothing() {
    let html = render(closed_side_panel_view);
    assert!(!html.contains("sidepanel"), "no panel surface while closed:\n{html}");
    assert!(!html.contains("sidepanel-scrim"), "no scrim while closed:\n{html}");
}

#[test]
fn an_open_modal_renders_head_body_foot() {
    let html = render(open_modal_view);
    assert!(html.contains("modal"), "the modal surface renders:\n{html}");
    assert!(html.contains("m-head"), "the modal head renders:\n{html}");
    assert!(html.contains("m-body"), "the modal body renders:\n{html}");
    assert!(html.contains("m-foot"), "the modal footer renders:\n{html}");
}

#[test]
fn a_closed_modal_renders_nothing() {
    let html = render(closed_modal_view);
    assert!(!html.contains("modal"), "no modal surface while closed:\n{html}");
}
