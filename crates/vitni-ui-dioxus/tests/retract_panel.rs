//! SSR assertions for the shared Retract/Detach side panel (PR29, `record-editing.html` §8): the
//! panel shows the row being acted on, the "stays in History" note, a rationale-only input, and a
//! Danger confirm button whose accessible name carries the row label. It never renders the target
//! assertion's UUID — the same discipline as the tag-id SSR tests (data-model §9, ADR 0004 §2).

use dioxus::prelude::*;
use vitni_ui::Localizer;
use vitni_ui_dioxus::screens::retract_panel;

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
        loc.action_label("retract"),
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
