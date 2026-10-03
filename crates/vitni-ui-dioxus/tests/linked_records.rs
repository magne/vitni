//! SSR assertions for a person's *Linked records* tab and its unlink panel (ADR 0039 §5): each record
//! of the cluster with its origin, source and *Unlink*, the person itself without one.

use dioxus::prelude::*;
use vitni_ui::{LinkedRecordVm, Localizer, OriginVm};
use vitni_ui_dioxus::screens::{linked_tab, unlink_side_panel};
use vitni_ui_dioxus::shell::nav_state::NavState;

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn origin() -> OriginVm {
    OriginVm {
        label: "record pf01 in Digitalarkivet".to_owned(),
        url: Some("https://www.digitalarkivet.no/pf01".to_owned()),
        source: Some("1910 census".to_owned()),
    }
}

fn record(human_id: &str, origin: Option<OriginVm>, via: Option<&str>, root: bool) -> LinkedRecordVm {
    LinkedRecordVm {
        human_id: human_id.to_owned(),
        name: format!("Ola Hansen {human_id}"),
        is_persona: !root,
        evidence_level_label: if root { "Conclusion" } else { "Persona" }.to_owned(),
        origin,
        via: via.map(|id| format!("via {id}")),
        root,
    }
}

fn tab() -> Element {
    use_context_provider(NavState::new);
    let loc = loc();
    let records = vec![
        record("I0001", None, None, true),
        record("I0007", Some(origin()), None, false),
        record("I0008", Some(OriginVm { url: None, ..origin() }), Some("I0007"), false),
    ];
    let onunlink = use_callback(|_: (String, String)| {});
    linked_tab(&loc, &records, onunlink)
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn each_record_shows_its_origin_and_source() {
    let html = render(tab);
    for needle in [
        "Ola Hansen I0007",
        ">Persona<",
        "Conclusion",
        "Linked to",
        "1910 census",
        "via I0007",
        r#"href="https://www.digitalarkivet.no/pf01""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert_eq!(
        html.matches("https://www.digitalarkivet.no/pf01").count(),
        1,
        "a record with no URL form is named, not linked:\n{html}"
    );
}

#[test]
fn every_member_but_not_the_person_itself_can_be_unlinked() {
    let html = render(tab);
    assert!(html.contains(r#"aria-label="Unlink Ola Hansen I0007""#), "{html}");
    assert!(html.contains(r#"aria-label="Unlink Ola Hansen I0008""#), "{html}");
    assert!(!html.contains(r#"aria-label="Unlink Ola Hansen I0001""#), "{html}");
}

fn panel() -> Element {
    let loc = loc();
    let unlinking = use_signal(|| Some(("I0007".to_owned(), "Ola Hansen I0007".to_owned())));
    let reason = use_signal(String::new);
    let onconfirm = use_callback(|()| {});
    unlink_side_panel(&loc, unlinking, reason, onconfirm)
}

#[test]
fn the_unlink_panel_asks_for_a_reason_and_names_the_record() {
    let html = render(panel);
    for needle in [
        "Unlink record",
        "Ola Hansen I0007",
        r#"id="retract-reason""#,
        "becomes its own person again",
        r#"aria-label="Unlink Ola Hansen I0007""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

fn closed_panel() -> Element {
    let loc = loc();
    let unlinking = use_signal(|| None::<(String, String)>);
    let reason = use_signal(String::new);
    let onconfirm = use_callback(|()| {});
    unlink_side_panel(&loc, unlinking, reason, onconfirm)
}

#[test]
fn nothing_renders_until_an_unlink_is_armed() {
    assert_eq!(render(closed_panel), "");
}
