//! SSR a11y assertions for the `RadioGroup` primitive (Phase 5 PR35): a single-choice
//! `role="radiogroup"` with an accessible name over `role="radio"` options, exactly one of which is
//! `aria-checked="true"`. Unlike the multi-select `RestrictionSet`, it carries no `aria-pressed`, and
//! it implements the WAI-ARIA roving-tabindex contract the old inline theme picker lacked: the
//! checked radio is the single tab stop (`tabindex="0"`), the rest are `tabindex="-1"`.

use dioxus::prelude::*;
use vitni_ui_dioxus::components::{RadioChoice, RadioGroup};

fn choices() -> Vec<RadioChoice> {
    vec![
        RadioChoice {
            id: "light".to_owned(),
            label: "Light".to_owned(),
        },
        RadioChoice {
            id: "dark".to_owned(),
            label: "Dark".to_owned(),
        },
        RadioChoice {
            id: "system".to_owned(),
            label: "System".to_owned(),
        },
    ]
}

fn group() -> Element {
    rsx! {
        RadioGroup {
            group_label: "Theme".to_owned(),
            choices: choices(),
            selected: "dark".to_owned(),
            onselect: |_: String| {},
        }
    }
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn radiogroup_is_a_named_radiogroup_of_radios() {
    let html = render(group);
    assert!(
        html.contains(r#"role="radiogroup""#),
        "the control is a radiogroup:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Theme""#),
        "the group carries its accessible name:\n{html}"
    );
    assert_eq!(
        html.matches(r#"role="radio""#).count(),
        3,
        "one radio per choice:\n{html}"
    );
    assert!(
        html.contains(r#"class="resn""#),
        "reuses the resn design-system class:\n{html}"
    );
}

#[test]
fn exactly_one_radio_is_checked() {
    let html = render(group);
    assert_eq!(
        html.matches(r#"aria-checked="true""#).count(),
        1,
        "a single-choice group checks exactly one radio:\n{html}"
    );
    assert_eq!(
        html.matches(r#"aria-checked="false""#).count(),
        2,
        "the other two radios are unchecked:\n{html}"
    );
}

#[test]
fn radiogroup_carries_no_aria_pressed() {
    let html = render(group);
    assert!(
        !html.contains("aria-pressed"),
        "a radiogroup is single-choice, never the RestrictionSet's aria-pressed:\n{html}"
    );
}

#[test]
fn the_checked_radio_is_the_single_roving_tab_stop() {
    let html = render(group);
    assert_eq!(
        html.matches(r#"tabindex="0""#).count(),
        1,
        "exactly the checked radio is the tab stop:\n{html}"
    );
    assert_eq!(
        html.matches(r#"tabindex="-1""#).count(),
        2,
        "the non-selected radios are removed from the tab order:\n{html}"
    );
    // The tab stop is the checked radio: the "0" tabindex sits on the same button as the single
    // aria-checked="true".
    let checked_at = html.find(r#"aria-checked="true""#).expect("a checked radio");
    let stop_at = html.find(r#"tabindex="0""#).expect("a tab stop");
    let button_start = html[..checked_at].rfind("<button").expect("checked radio's button");
    let button_end = html[button_start..].find('>').expect("button open-tag end") + button_start;
    assert!(
        (button_start..button_end).contains(&stop_at),
        "the tab stop is on the checked radio's button:\n{html}"
    );
}
