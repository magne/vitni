//! SSR assertions for the form-input behavior cores and the composed field (the "global keys fire
//! inside text controls" consolidation): render each primitive and inspect the HTML, the same
//! render-and-inspect pattern as `components.rs`.

use dioxus::prelude::*;
use genealogy_ui_dioxus::components::{SelectChoice, SelectInput, TextField, TextInput, TextInputKind};

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn text_input_gallery() -> Element {
    rsx! {
        TextInput {
            value: Some("hi".to_owned()),
            aria_label: "Given",
            role: "searchbox",
            style: "width:90px",
            oninput: move |_| {},
        }
        TextInput { value: Some("7".to_owned()), kind: TextInputKind::Number, invalid: true }
        TextInput { multiline: true, value: Some("body".to_owned()), rows: "5" }
    }
}

#[test]
fn text_input_renders_the_native_control_and_passes_attributes_through() {
    let html = render(text_input_gallery);
    assert!(html.contains(r#"type="text""#), "default text type:\n{html}");
    assert!(html.contains(r#"aria-label="Given""#), "aria passthrough:\n{html}");
    assert!(html.contains(r#"role="searchbox""#), "role passthrough:\n{html}");
    assert!(html.contains(r#"type="number""#), "number kind:\n{html}");
    assert!(html.contains("<textarea"), "multiline renders a textarea:\n{html}");
}

#[test]
fn text_input_invalid_sets_aria_and_class() {
    let html = render(text_input_gallery);
    assert!(html.contains(r#"aria-invalid="true""#), "aria-invalid:\n{html}");
    assert!(html.contains(r#"class="in invalid""#), "invalid class:\n{html}");
}

fn select_gallery() -> Element {
    rsx! {
        SelectInput {
            aria_label: "Sex",
            selected: "female".to_owned(),
            options: vec![
                SelectChoice { value: "male".to_owned(), label: "male".to_owned() },
                SelectChoice { value: "female".to_owned(), label: "female".to_owned() },
            ],
            onchange: move |_| {},
        }
    }
}

#[test]
fn select_input_renders_a_select_with_the_selected_option() {
    let html = render(select_gallery);
    assert!(html.contains("<select"), "renders a select:\n{html}");
    assert!(html.contains(r#"aria-label="Sex""#), "aria passthrough:\n{html}");
    assert!(
        html.contains(r#"value="female" selected"#),
        "the current option is selected:\n{html}"
    );
}

fn text_field_full() -> Element {
    rsx! {
        TextField {
            label: "Name".to_owned(),
            name: "tag-name".to_owned(),
            value: "Smith".to_owned(),
            invalid: true,
            error: Some("Name is required.".to_owned()),
            hint: Some("A short label.".to_owned()),
            modified: true,
            reset_label: Some("Reset Name".to_owned()),
            oninput: move |_| {},
            onreset: move |()| {},
        }
    }
}

#[test]
fn text_field_associates_the_label_and_carries_error_hint_and_revert() {
    let html = render(text_field_full);
    assert!(html.contains(r#"for="tag-name""#), "label association:\n{html}");
    assert!(html.contains(r#"id="tag-name""#), "input id:\n{html}");
    assert!(html.contains(r#"aria-invalid="true""#), "aria-invalid:\n{html}");
    assert!(html.contains(r#"class="field-error""#), "field-error present:\n{html}");
    assert!(html.contains("Name is required."), "error text:\n{html}");
    assert!(html.contains(r#"class="field-hint""#), "field-hint present:\n{html}");
    assert!(html.contains(r#"aria-label="Reset Name""#), "revert control:\n{html}");
}

fn text_field_stepper() -> Element {
    rsx! {
        TextField {
            label: "Priority".to_owned(),
            name: "tag-priority".to_owned(),
            value: "5".to_owned(),
            container_class: "number-stepper".to_owned(),
            input_class: "stepper-value".to_owned(),
            inputmode: "numeric".to_owned(),
            oninput: move |_| {},
            div { class: "stepper-arrows", button { r#type: "button", "▲" } }
        }
    }
}

#[test]
fn text_field_renders_the_adornment_slot_and_container_class() {
    let html = render(text_field_stepper);
    assert!(html.contains(r#"class="number-stepper""#), "container class:\n{html}");
    assert!(html.contains(r#"class="stepper-value""#), "input class:\n{html}");
    assert!(
        html.contains(r#"inputmode="numeric""#),
        "inputmode passthrough:\n{html}"
    );
    assert!(
        html.contains(r#"class="stepper-arrows""#),
        "adornment slot renders:\n{html}"
    );
}

#[test]
fn text_field_without_reset_hides_the_revert_control() {
    fn view() -> Element {
        rsx! {
            TextField {
                label: "Name".to_owned(),
                name: "tag-name".to_owned(),
                value: "Smith".to_owned(),
                modified: true,
                oninput: move |_| {},
            }
        }
    }
    let html = render(view);
    assert!(!html.contains("↺"), "no revert control without onreset:\n{html}");
}
