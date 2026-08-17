//! SSR assertions for the shared correction side panel (PR29, `record-editing.html` §8): the
//! panel shows the row or tag being acted on, the "stays in History" note, a rationale-only input, and
//! a Danger confirm button whose accessible name carries the label. It never renders the target
//! assertion's UUID nor a tag's — the same discipline as the tag-id SSR tests (data-model §9,
//! ADR 0004 §2).
//!
//! The three subjects the panel serves are covered here: a Retract, a Detach, and (issue #315) an
//! Untag, which is the reason an untag is no longer a click that commits with no operator reason.

use dioxus::prelude::*;
use vitni_ui::ActionLabel;
use vitni_ui::Localizer;
use vitni_ui_dioxus::screens::{RetractSubject, RetractTarget, retract_panel, retract_side_panel};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

/// Renders the panel for a Birth fact row, seeded with an assertion id that must NOT appear in HTML.
fn panel() -> Element {
    let loc = loc();
    let rationale = use_signal(String::new);
    let onconfirm = use_callback(|()| {});
    retract_panel(
        &loc,
        &loc.panel_title("retract"),
        "Birth",
        loc.action_retract_row("Birth"),
        &loc.retract_note(),
        loc.action_button(ActionLabel::Retract),
        rationale,
        onconfirm,
    )
}

fn render() -> String {
    let mut vdom = VirtualDom::new(panel);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn retract_panel_shows_note_reason_input_and_danger_button() {
    let html = render();
    assert!(html.contains("Retract assertion"), "the panel title renders:\n{html}");
    assert!(
        html.contains("recorded in History"),
        "the stays-in-History note renders:\n{html}"
    );
    assert!(
        html.contains(r#"id="retract-reason""#),
        "a rationale input renders:\n{html}"
    );
    assert!(
        html.contains("btn danger"),
        "the confirm button is a Danger button:\n{html}"
    );
    // The Danger button's accessible name carries the row label (a bare "Retract" is not enough).
    assert!(
        html.contains(r#"aria-label="Retract Birth""#),
        "the confirm button names the row:\n{html}"
    );
}

#[test]
fn retract_panel_never_renders_an_assertion_uuid() {
    // A realistic UUID v7 that a caller might hold as the row's assertion id — it must stay off-screen.
    let assertion = "0190a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b";
    let html = render();
    assert!(
        !html.contains(assertion),
        "the panel must not render the target assertion's UUID:\n{html}"
    );
}

/// A realistic UUID v7 for the armed subject — an assertion id or a tag id. Neither may reach the DOM.
const SUBJECT_UUID: &str = "0190a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b";

/// Renders the shared side panel with `subject` armed for `label`, as a pane's own `retract` signal
/// would hold it. `detach_note_id` is the pane's Detach note, as at the real call sites.
fn side_panel(subject: RetractSubject, label: &str, detach_note_id: &str) -> Element {
    let loc = loc();
    let target = RetractTarget {
        subject,
        label: label.to_owned(),
    };
    let retract = use_signal(|| Some(target));
    let reason = use_signal(String::new);
    let onconfirm = use_callback(|()| {});
    retract_side_panel(&loc, retract, reason, onconfirm, detach_note_id)
}

fn untag_side_panel() -> Element {
    side_panel(
        RetractSubject::Tag {
            tag_id: SUBJECT_UUID.to_owned(),
        },
        "Direct ancestor",
        "detach-citation",
    )
}

fn retract_side_panel_armed() -> Element {
    side_panel(
        RetractSubject::Assertion {
            assertion_id: SUBJECT_UUID.to_owned(),
            detach: false,
        },
        "Birth",
        "detach-citation",
    )
}

fn detach_side_panel() -> Element {
    side_panel(
        RetractSubject::Assertion {
            assertion_id: SUBJECT_UUID.to_owned(),
            detach: true,
        },
        "Trinity Church baptisms",
        "detach-citation",
    )
}

fn render_component(component: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(component);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn an_armed_untag_names_the_tag_and_takes_a_reason() {
    let html = render_component(untag_side_panel);
    assert!(html.contains("Remove tag"), "the untag panel title renders:\n{html}");
    assert!(
        html.contains("Direct ancestor"),
        "the panel names the tag it will remove:\n{html}"
    );
    assert!(
        html.contains("The tag removal is recorded in History; nothing is deleted."),
        "the untag panel carries its own stays-in-History note:\n{html}"
    );
    assert!(
        html.contains(r#"id="retract-reason""#),
        "an untag takes an operator rationale — issue #315:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Remove tag Direct ancestor""#),
        "the confirm button names the tag:\n{html}"
    );
    // The panel must not fall through to the Retract wording — an untag retracts no assertion.
    assert!(
        !html.contains("Retract assertion"),
        "an untag is not a retraction:\n{html}"
    );
}

#[test]
fn an_armed_untag_never_renders_the_tag_uuid() {
    let html = render_component(untag_side_panel);
    assert!(
        !html.contains(SUBJECT_UUID),
        "the tag's UUID must stay off-screen (data-model §9):\n{html}"
    );
}

#[test]
fn an_armed_assertion_keeps_the_retract_and_detach_wording() {
    let retract = render_component(retract_side_panel_armed);
    assert!(
        retract.contains("Retract assertion"),
        "the retract title is unchanged:\n{retract}"
    );
    assert!(
        retract.contains("The retraction is recorded in History; nothing is deleted."),
        "the retract note is unchanged:\n{retract}"
    );
    assert!(
        retract.contains(r#"aria-label="Retract Birth""#),
        "the retract confirm still names the row:\n{retract}"
    );

    let detach = render_component(detach_side_panel);
    assert!(detach.contains("Detach"), "the detach title is unchanged:\n{detach}");
    assert!(
        detach.contains("Detach this citation — the detachment is recorded in History"),
        "the per-screen detach note still reaches the panel:\n{detach}"
    );
    assert!(
        detach.contains(r#"aria-label="Detach Trinity Church baptisms""#),
        "the detach confirm still names the row:\n{detach}"
    );
    assert!(
        !detach.contains(SUBJECT_UUID),
        "the assertion UUID must stay off-screen:\n{detach}"
    );
}
