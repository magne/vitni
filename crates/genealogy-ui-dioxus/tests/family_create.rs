//! SSR assertions for the Family create form (Phase 5 PR26): the draft header, the partner chips +
//! add-partner input, and Save gated on having at least one partner.

use dioxus::prelude::*;
use genealogy_ui::{FamilyDraft, Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::screens::{RecordActions, create_record_header, family_create_fields, provenance_block};

fn view(seed: FamilyDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let draft = use_signal(move || seed);
    let new_partner = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty();
    rsx! {
        {create_record_header(&loc.family_new_title(), &loc.record_draft_badge())}
        {family_create_fields(&loc, draft, new_partner)}
        {provenance_block(&loc, prov)}
        RecordActions {
            save_label: loc.action_label("save"),
            cancel_label: loc.action_label("cancel"),
            can_save,
            onsave: move |()| {},
            oncancel: move |()| {},
        }
    }
}

fn empty_view() -> Element {
    view(FamilyDraft::new())
}

fn one_partner_view() -> Element {
    let mut draft = FamilyDraft::new();
    draft.add_partner("I0001");
    view(draft)
}

#[test]
fn create_pane_shows_the_draft_badge_and_partner_input() {
    let mut vdom = VirtualDom::new(empty_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in ["New family", "draft · not saved", "Partner", "Add partner"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(html.contains("disabled"), "Save disabled with no partner:\n{html}");
}

#[test]
fn a_partner_chip_enables_save() {
    let mut vdom = VirtualDom::new(one_partner_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains("I0001"), "the partner chip shows:\n{html}");
    assert!(
        !html.contains("disabled"),
        "Save enabled once a partner is added:\n{html}"
    );
}
