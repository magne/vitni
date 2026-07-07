//! SSR assertions for the Person create pane (Phase 5 PR27): create renders inline in the detail
//! pane on the shared record form. The scalar identity fields render via the pure `person_record_fields`
//! fn over a create edit state, and Cancel/Save live in the sticky `create_record_header` — both SSR-
//! testable without `AppCtx`.

use dioxus::prelude::*;
use genealogy_ui::{Localizer, PersonDraft, ProvenanceDraft};
use genealogy_ui_dioxus::screens::{RecordEditState, create_record_header, person_record_fields};

fn create_pane_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    // A create edit state (edit mode from the start), with the given name pre-filled to prove the
    // fields are live inputs rather than read text.
    let record = RecordEditState::<PersonDraft> {
        editing: use_signal(|| true),
        seed: use_signal(PersonDraft::new),
        draft: use_signal(|| PersonDraft {
            given: "Ada".to_owned(),
            ..PersonDraft::new()
        }),
        prov: use_signal(ProvenanceDraft::default),
    };
    rsx! {
        {create_record_header(&loc.person_new_title(), &loc.record_draft_badge(), rsx! {})}
        {person_record_fields(&loc, record)}
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
