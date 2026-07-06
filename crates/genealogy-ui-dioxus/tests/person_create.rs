//! SSR assertions for the Person create pane (Phase 5 PR26): create now renders inline in the detail
//! pane (the `creating` signal), not a side panel. The preferred-name text inputs render via the pure
//! `person_create_fields` fn (the name-type/sex selects, citation, and tags stay in the AppCtx-bound
//! `PersonRecordForm`), so the create pane's inputs are SSR-testable without `AppCtx`.

use dioxus::prelude::*;
use genealogy_ui::Localizer;
use genealogy_ui_dioxus::screens::{create_record_header, person_create_fields};

fn create_pane_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let prefix = use_signal(String::new);
    let given = use_signal(|| "Ada".to_owned());
    let nickname = use_signal(String::new);
    let surname_prefix = use_signal(String::new);
    let surname = use_signal(String::new);
    let suffix = use_signal(String::new);
    rsx! {
        {create_record_header(&loc.person_new_title(), &loc.record_draft_badge())}
        {person_create_fields(&loc, prefix, given, nickname, surname_prefix, surname, suffix)}
    }
}

#[test]
fn create_pane_shows_the_draft_header_and_name_inputs() {
    let mut vdom = VirtualDom::new(create_pane_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in [
        "New person",
        "draft · not saved",
        r#"id="given""#,
        r#"id="surname""#,
        r#"id="prefix""#,
        r#"id="suffix""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    // The given-name input is seeded, proving the fields are live inputs (not read text).
    assert!(
        html.contains(r#"value="Ada""#),
        "the given-name input carries its value:\n{html}"
    );
}
