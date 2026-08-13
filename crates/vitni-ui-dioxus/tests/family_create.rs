//! SSR assertions for the Family create pane (Phase 5 PR28): the editable human id, the People
//! find-or-create picker (rows + "+ New person"), the pending new-partner draft card, the added-partner
//! chips (existing = title + id; new = name + draft badge), the two-partner cap note, and Save gated on
//! at least one partner (disabled even when the id is dirty). Every builder is a pure fn over signals,
//! so it renders under SSR without an `AppCtx`.

use dioxus::prelude::*;
use vitni_ui::{FamilyDraft, Localizer, NewPersonFields, PickerSelection, PickerState, ProvenanceDraft, RowVm};
use vitni_ui_dioxus::components::{Button, ButtonVariant, PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{RecordEditState, create_record_header, family_create_fields, record_edit_provenance};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn row(id: &str, title: &str) -> RowVm {
    RowVm {
        id: id.to_owned(),
        title: title.to_owned(),
        ..RowVm::default()
    }
}

fn people() -> PickerOptions {
    PickerOptions::Ready(vec![row("I0001", "Ada Lovelace"), row("I0002", "Charles Babbage")])
}

fn noop_callbacks() -> PickerCallbacks {
    PickerCallbacks {
        onpick: Callback::new(|_: PickerSelection| {}),
        onclear: Callback::new(|()| {}),
        onnew: Callback::new(|_: String| {}),
    }
}

fn partner_picker(state: Signal<PickerState>) -> RecordPicker {
    RecordPicker {
        config: PickerConfig {
            label: "Partner".to_owned(),
            name: "family-partner".to_owned(),
            entity_label: "person".to_owned(),
            allow_new: true,
        },
        state,
        options: people(),
        exclude: Vec::new(),
        callbacks: noop_callbacks(),
    }
}

/// Renders the create fields for `draft` with the picker in `state` and the pending new-partner buffer.
fn fields(draft: FamilyDraft, state: PickerState, pending: Option<NewPersonFields>) -> Element {
    let loc = loc();
    let draft = use_signal(move || draft);
    let pending_new = use_signal(move || pending);
    let picker_state = use_signal(move || state);
    family_create_fields(&loc, draft, pending_new, &partner_picker(picker_state))
}

/// Renders the whole create pane (header + fields + provenance) so Save gating can be asserted.
fn pane(draft: FamilyDraft) -> Element {
    let loc = loc();
    let record = RecordEditState::<FamilyDraft> {
        editing: use_signal(|| true),
        seed: use_signal(FamilyDraft::new),
        draft: use_signal(move || draft),
        prov: use_signal(ProvenanceDraft::default),
    };
    let pending_new = use_signal(|| None::<NewPersonFields>);
    let picker_state = use_signal(PickerState::default);
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.family_new_title(), &loc.record_draft_badge(), actions)}
        {family_create_fields(&loc, record.draft, pending_new, &partner_picker(picker_state))}
        {record_edit_provenance(&loc, record)}
    }
}

fn empty_pane_view() -> Element {
    pane(FamilyDraft::new())
}

fn id_typed_pane_view() -> Element {
    pane(FamilyDraft {
        human_id: "F0042".to_owned(),
        ..FamilyDraft::new()
    })
}

fn open_picker_view() -> Element {
    fields(
        FamilyDraft::new(),
        PickerState {
            query: String::new(),
            open: true,
            selection: None,
        },
        None,
    )
}

fn new_partner_view() -> Element {
    fields(
        FamilyDraft::new(),
        PickerState::default(),
        Some(NewPersonFields::default()),
    )
}

fn existing_chip_view() -> Element {
    let mut draft = FamilyDraft::new();
    draft.add_partner(PickerSelection {
        human_id: "I0001".to_owned(),
        title: "Ada Lovelace".to_owned(),
    });
    fields(draft, PickerState::default(), None)
}

fn new_chip_view() -> Element {
    let mut draft = FamilyDraft::new();
    draft.add_new_partner(NewPersonFields {
        given: "Grace".to_owned(),
        surname: "Hopper".to_owned(),
    });
    fields(draft, PickerState::default(), None)
}

fn cap_view() -> Element {
    let mut draft = FamilyDraft::new();
    draft.add_partner(PickerSelection {
        human_id: "I0001".to_owned(),
        title: "Ada Lovelace".to_owned(),
    });
    draft.add_partner(PickerSelection {
        human_id: "I0002".to_owned(),
        title: "Charles Babbage".to_owned(),
    });
    fields(draft, PickerState::default(), None)
}

#[test]
fn the_create_pane_has_an_editable_human_id_field() {
    let html = render(empty_pane_view);
    assert!(html.contains(r#"id="human-id""#), "the human-id input renders:\n{html}");
    assert!(html.contains("New family"), "the draft header renders:\n{html}");
}

#[test]
fn save_is_disabled_at_zero_partners_even_when_the_id_is_dirty() {
    let html = render(id_typed_pane_view);
    assert!(
        html.contains("F0042"),
        "the typed id is present (draft is dirty):\n{html}"
    );
    assert!(
        html.contains("disabled"),
        "Save stays disabled without a partner:\n{html}"
    );
    assert!(
        html.contains("Add at least one partner."),
        "the required hint shows:\n{html}"
    );
}

#[test]
fn the_partner_picker_renders_and_opens_with_rows_and_a_new_person_row() {
    let html = render(open_picker_view);
    assert!(
        html.contains(r#"id="family-partner""#),
        "the picker search input renders:\n{html}"
    );
    assert!(
        html.contains(r#"class="picker-results""#),
        "the open result list renders:\n{html}"
    );
    assert!(
        html.contains("Ada Lovelace") && html.contains("I0001"),
        "a person row shows:\n{html}"
    );
    assert!(
        html.contains(r#"class="picker-new""#) && html.contains("New person"),
        "the + New person row shows:\n{html}"
    );
}

#[test]
fn choosing_new_person_opens_a_named_draft_card_with_name_inputs() {
    let html = render(new_partner_view);
    assert!(
        html.contains(r#"class="draft-card""#),
        "the new-partner draft card renders:\n{html}"
    );
    assert!(
        html.contains("New person"),
        "the card is named for what it creates:\n{html}"
    );
    for needle in [r#"id="new-partner-given""#, r#"id="new-partner-surname""#] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn an_existing_partner_chip_shows_the_title_and_id() {
    let html = render(existing_chip_view);
    assert!(html.contains(r#"class="chip""#), "the chip renders:\n{html}");
    assert!(
        html.contains("Ada Lovelace") && html.contains("I0001"),
        "the title + id show:\n{html}"
    );
}

#[test]
fn a_new_partner_chip_shows_the_name_and_a_draft_badge() {
    let html = render(new_chip_view);
    assert!(html.contains("Grace Hopper"), "the new partner's name shows:\n{html}");
    assert!(
        html.contains(r#"class="badge draft""#),
        "the draft badge marks the uncommitted partner:\n{html}"
    );
}

#[test]
fn the_cap_state_replaces_the_picker_at_two_partners() {
    let html = render(cap_view);
    assert!(
        html.contains("Both partners added."),
        "the cap note shows at two partners:\n{html}"
    );
    assert!(
        !html.contains(r#"id="family-partner""#),
        "no picker search input at the cap:\n{html}"
    );
}
