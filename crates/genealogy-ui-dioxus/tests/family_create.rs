//! SSR assertions for the Family create pane (Phase 5 PR27): the shared record frame in create mode —
//! a "draft · not saved" header with Cancel/Save in the sticky head — plus the partner chips +
//! add-partner input. Save gated on having at least one partner.

use dioxus::prelude::*;
use genealogy_ui::{FamilyDraft, Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::components::{Button, ButtonVariant};
use genealogy_ui_dioxus::screens::{
    RecordEditState, create_record_header, family_create_fields, record_edit_provenance,
};

fn view(seed: FamilyDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let record = RecordEditState::<FamilyDraft> {
        editing: use_signal(|| true),
        seed: use_signal(FamilyDraft::new),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    };
    let new_partner = use_signal(String::new);
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.family_new_title(), &loc.record_draft_badge(), actions)}
        {family_create_fields(&loc, record.draft, new_partner)}
        {record_edit_provenance(&loc, record)}
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

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn create_pane_shows_the_draft_badge_and_partner_input() {
    let html = render(empty_view);
    for needle in ["New family", "draft · not saved", "Partner", "Add partner"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(html.contains("disabled"), "Save disabled with no partner:\n{html}");
}

#[test]
fn a_partner_chip_enables_save() {
    let html = render(one_partner_view);
    assert!(html.contains("I0001"), "the partner chip shows:\n{html}");
    assert!(
        !html.contains("disabled"),
        "Save enabled once a partner is added:\n{html}"
    );
}
