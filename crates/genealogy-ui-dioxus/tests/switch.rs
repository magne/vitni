//! SSR a11y assertions for the Switch primitive (Phase 5 PR35): a single `role="switch"` control
//! whose `aria-checked` reflects the `checked` prop, whose accessible name comes from the label, and
//! whose visible text carries the state (never colour alone). The single ARIA state attribute is
//! `aria-checked` — a switch never also carries `aria-pressed`. Pure render-and-inspect, the same
//! pattern as `plugin_manager.rs`.

use dioxus::prelude::*;
use genealogy_ui_dioxus::components::Switch;

fn checked_switch() -> Element {
    rsx! {
        Switch {
            checked: true,
            label: "gedcom-import enabled".to_owned(),
            state_text: "On".to_owned(),
            ontoggle: |_| {},
        }
    }
}

fn unchecked_switch() -> Element {
    rsx! {
        Switch {
            checked: false,
            label: "ui-panel enabled".to_owned(),
            state_text: "Off".to_owned(),
            ontoggle: |_| {},
        }
    }
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn switch_is_a_role_switch_button_with_the_checked_state() {
    let html = render(checked_switch);
    assert!(html.contains(r#"role="switch""#), "the control is a switch:\n{html}");
    assert!(
        html.contains(r#"type="button""#),
        "a real button, not a submit:\n{html}"
    );
    assert!(
        html.contains(r#"class="switch""#),
        "reuses the switch design-system class:\n{html}"
    );
    assert!(
        html.contains(r#"aria-checked="true""#),
        "the checked prop drives aria-checked:\n{html}"
    );
}

#[test]
fn switch_reflects_the_unchecked_state() {
    let html = render(unchecked_switch);
    assert!(
        html.contains(r#"aria-checked="false""#),
        "an unchecked switch reports aria-checked=false:\n{html}"
    );
}

#[test]
fn switch_takes_its_accessible_name_from_the_label() {
    let html = render(checked_switch);
    assert!(
        html.contains(r#"aria-label="gedcom-import enabled""#),
        "the label prop is the accessible name:\n{html}"
    );
}

#[test]
fn switch_state_is_visible_text_not_colour_alone() {
    let on_html = render(checked_switch);
    assert!(
        on_html.contains(">On<"),
        "the on state renders as visible text:\n{on_html}"
    );
    let off_html = render(unchecked_switch);
    assert!(
        off_html.contains(">Off<"),
        "the off state renders as visible text:\n{off_html}"
    );
}

#[test]
fn switch_carries_a_single_aria_state_never_aria_pressed() {
    let html = render(checked_switch);
    assert!(
        !html.contains("aria-pressed"),
        "a switch's only state attribute is aria-checked:\n{html}"
    );
}
