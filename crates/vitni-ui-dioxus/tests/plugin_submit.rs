//! SSR assertions for the plugin-UI submission outcome (Phase 5 PR37, ADR 0022 §2): a successful
//! submission with a message is announced as `role="status"`, while a validation failure or a
//! technical error is a `role="alert"`. Pure render-and-inspect over the exported
//! [`submit_outcome_view`] — no window, no workspace, no plugin host.

use std::cell::RefCell;

use dioxus::prelude::*;
use vitni_ui::SubmitResult;
use vitni_ui_dioxus::screens::submit_outcome_view;

thread_local! {
    static OUTCOME: RefCell<Option<SubmitResult>> = const { RefCell::new(None) };
    static ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn root() -> Element {
    let outcome = OUTCOME.with(|cell| cell.borrow().clone());
    let error = ERROR.with(|cell| cell.borrow().clone());
    submit_outcome_view(outcome.as_ref(), error.as_deref())
}

fn render(outcome: Option<SubmitResult>, error: Option<String>) -> String {
    OUTCOME.with(|cell| *cell.borrow_mut() = outcome);
    ERROR.with(|cell| *cell.borrow_mut() = error);
    let mut vdom = VirtualDom::new(root);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn a_success_message_renders_as_a_status() {
    let html = render(
        Some(SubmitResult::Success {
            message: Some("Note saved".to_owned()),
            panel: None,
        }),
        None,
    );
    assert!(
        html.contains(r#"role="status""#),
        "a success is announced politely:\n{html}"
    );
    assert!(html.contains("Note saved"), "the resolved message renders:\n{html}");
}

#[test]
fn a_validation_failure_renders_as_an_alert() {
    let html = render(
        Some(SubmitResult::Failure {
            message: "A title is required".to_owned(),
        }),
        None,
    );
    assert!(
        html.contains(r#"role="alert""#),
        "a validation failure is an alert:\n{html}"
    );
    assert!(
        html.contains("A title is required"),
        "the failure message renders:\n{html}"
    );
}

#[test]
fn a_technical_error_renders_as_an_alert() {
    let html = render(None, Some("the plugin panicked".to_owned()));
    assert!(
        html.contains(r#"role="alert""#),
        "a technical error is an alert:\n{html}"
    );
    assert!(
        html.contains("the plugin panicked"),
        "the error message renders:\n{html}"
    );
}

#[test]
fn a_bare_success_with_no_message_renders_nothing() {
    let html = render(
        Some(SubmitResult::Success {
            message: None,
            panel: None,
        }),
        None,
    );
    assert!(!html.contains("role="), "no message means no announcement:\n{html}");
}
