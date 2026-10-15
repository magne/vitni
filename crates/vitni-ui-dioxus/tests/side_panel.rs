//! SSR assertions for the shared `SidePanel`/`Modal` overlay containers (viewport-overflow fix):
//! both render a click-away scrim alongside their regions (a panel's head/body, a modal's
//! head/body/foot), both bracket their content with the focus-trap guards, and both render nothing
//! while closed. The viewport-bounded scroll (absolute positioning against `.detail`/`.overlay`,
//! `.sp-body`/`.m-body` scrolling, the pinned head, the sticky `.sp-actions` row, the modal's pinned
//! foot), the slide-in motion, and the focus movement the guards drive are CSS/webview
//! behaviour and not SSR-observable — verified by running the app, same constraint as the floating
//! record-picker's measured position in `record_picker.rs`.

use dioxus::prelude::*;
use vitni_ui::Localizer;
use vitni_ui_dioxus::components::{Modal, SidePanel, SidePanelActions};
use vitni_ui_dioxus::screens::edit_side_panel;

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn open_side_panel_view() -> Element {
    let onclose = use_callback(|(): ()| {});
    rsx! {
        SidePanel {
            title: "Edit name".to_owned(),
            open: true,
            close_label: "Close".to_owned(),
            onclose,
            div { "body" }
        }
    }
}

fn closed_side_panel_view() -> Element {
    let onclose = use_callback(|(): ()| {});
    rsx! {
        SidePanel {
            title: "Edit name".to_owned(),
            open: false,
            close_label: "Close".to_owned(),
            onclose,
            div { "body" }
        }
    }
}

fn open_modal_view() -> Element {
    let onclose = use_callback(|(): ()| {});
    rsx! {
        Modal {
            title: "Confirm".to_owned(),
            open: true,
            close_label: "Dismiss".to_owned(),
            onclose,
            footer: rsx! { button { "OK" } },
            div { "body" }
        }
    }
}

fn closed_modal_view() -> Element {
    let onclose = use_callback(|(): ()| {});
    rsx! {
        Modal {
            title: "Confirm".to_owned(),
            open: false,
            close_label: "Dismiss".to_owned(),
            onclose,
            footer: rsx! { button { "OK" } },
            div { "body" }
        }
    }
}

/// A modal whose only control is a disabled button — the shape the close confirm takes when the
/// parked edit cannot be saved. The guards still have to bracket it, or `Tab` escapes a dialog with
/// nothing to move between.
fn modal_with_one_disabled_control() -> Element {
    let onclose = use_callback(|(): ()| {});
    rsx! {
        Modal {
            title: "Confirm".to_owned(),
            open: true,
            close_label: "Dismiss".to_owned(),
            onclose,
            footer: rsx! { button { disabled: true, "Save" } },
            p { "nothing to save" }
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
    assert!(
        !html.contains("sp-foot"),
        "no footer: a form's Save sits in its own sticky actions row (#663):\n{html}"
    );
}

#[test]
fn an_open_side_panel_brackets_its_content_with_focus_guards() {
    let html = render(open_side_panel_view);
    assert!(
        html.contains(r#"data-focus-trap="true""#),
        "the panel root is the trap the guards wrap focus inside:\n{html}"
    );
    let guards = html.matches("data-focus-guard").count();
    assert_eq!(guards, 2, "one guard before the content and one after:\n{html}");
    let leading = html.find("focus-guard").unwrap_or_default();
    let head = html.find("sp-head").unwrap_or_default();
    let trailing = html.rfind("focus-guard").unwrap_or_default();
    let body = html.find("sp-body").unwrap_or_default();
    assert!(leading < head, "the leading guard precedes the head:\n{html}");
    assert!(body < trailing, "the trailing guard follows the body:\n{html}");
    let scrim = html.find("sidepanel-scrim").unwrap_or_default();
    assert!(
        scrim < leading,
        "the scrim stays outside the trap, so tabbing cannot land on it:\n{html}"
    );
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
fn an_open_modal_renders_a_labelled_click_away_scrim_inside_its_layer() {
    let html = render(open_modal_view);
    assert!(
        html.contains(r#"class="overlay""#),
        "the shared overlay layer positions the dialog over the app:\n{html}"
    );
    assert!(
        html.contains(r#"class="modal-scrim""#),
        "the click-away scrim renders while open:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Dismiss""#),
        "the scrim carries the caller's already-localized name:\n{html}"
    );
}

#[test]
fn an_open_modal_brackets_its_content_with_focus_guards() {
    let html = render(open_modal_view);
    assert!(
        html.contains(r#"data-focus-trap="true""#),
        "the dialog root is the trap the guards wrap focus inside:\n{html}"
    );
    let guards = html.matches("data-focus-guard").count();
    assert_eq!(guards, 2, "one guard before the content and one after:\n{html}");
    let leading = html.find("focus-guard").unwrap_or_default();
    let head = html.find("m-head").unwrap_or_default();
    let trailing = html.rfind("focus-guard").unwrap_or_default();
    let foot = html.find("m-foot").unwrap_or_default();
    assert!(leading < head, "the leading guard precedes the head:\n{html}");
    assert!(foot < trailing, "the trailing guard follows the footer:\n{html}");
}

#[test]
fn a_dialog_with_a_single_disabled_control_is_still_bracketed() {
    let html = render(modal_with_one_disabled_control);
    let guards = html.matches("data-focus-guard").count();
    assert_eq!(guards, 2, "the guards do not depend on the control count:\n{html}");
    assert!(
        html.contains("disabled"),
        "the sole control is the disabled Save:\n{html}"
    );
}

#[test]
fn a_closed_modal_renders_nothing() {
    let html = render(closed_modal_view);
    assert!(!html.contains("modal"), "no modal surface while closed:\n{html}");
    assert!(!html.contains("focus-guard"), "no guards while closed:\n{html}");
}

fn side_panel_with_actions_view() -> Element {
    let onclose = use_callback(|(): ()| {});
    rsx! {
        SidePanel {
            title: "Edit name".to_owned(),
            open: true,
            close_label: "Close".to_owned(),
            onclose,
            div { "body" }
            SidePanelActions { button { "Save" } }
        }
    }
}

/// The shared actions row (#663) sits inside the scrolling body, after the fields — it is the body's
/// own last child, so `position: sticky; bottom: 0` pins it to the panel's bottom edge while the
/// fields scroll under it.
#[test]
fn a_side_panels_actions_row_is_the_last_thing_in_its_body() {
    let html = render(side_panel_with_actions_view);
    let body = html.find("sp-body").unwrap_or_default();
    let actions = html.find(r#"<div class="sp-actions"><button>Save</button></div>"#);
    assert!(actions.is_some(), "the row wraps the Save button:\n{html}");
    let actions = actions.unwrap_or_default();
    assert!(body < actions, "the row sits inside the body:\n{html}");
    assert!(
        html[actions..].starts_with(r#"<div class="sp-actions"><button>Save</button></div></div>"#),
        "the row is the body's last child:\n{html}"
    );
}

/// A screen's edit-form enum, as `edit_side_panel` sees it: one variant per panel the screen opens.
#[derive(Clone, PartialEq)]
enum DemoForm {
    Name(String),
    Tag,
}

fn edit_panel_view(form: Option<DemoForm>) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| form);
    edit_side_panel(
        &loc,
        editing,
        |form| match form {
            DemoForm::Name(_) => "Edit name".to_owned(),
            DemoForm::Tag => "Add tag".to_owned(),
        },
        |form| match form {
            DemoForm::Name(name) => rsx! { p { "editing {name}" } },
            DemoForm::Tag => rsx! { p { "tag picker" } },
        },
    )
}

#[test]
fn an_edit_side_panel_with_no_open_form_renders_nothing() {
    let html = render(|| edit_panel_view(None));
    assert!(html.is_empty(), "no panel while nothing is being edited:\n{html}");
}

#[test]
fn an_edit_side_panel_renders_the_open_forms_title_and_body() {
    let html = render(|| edit_panel_view(Some(DemoForm::Name("Ola".to_owned()))));
    assert!(
        html.contains(r#"aria-label="Edit name""#),
        "the dialog is named by the form's title:\n{html}"
    );
    assert!(html.contains("editing Ola"), "the form's body renders:\n{html}");
    assert!(
        !html.contains("tag picker"),
        "only the open form's body renders:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Cancel""#),
        "the close control reads Cancel:\n{html}"
    );
}

#[test]
fn an_edit_side_panel_picks_the_title_per_form() {
    let html = render(|| edit_panel_view(Some(DemoForm::Tag)));
    assert!(
        html.contains(r#"aria-label="Add tag""#),
        "the Tag form's title:\n{html}"
    );
    assert!(html.contains("tag picker"), "the Tag form's body:\n{html}");
}
