use super::prelude::*;
use crate::screens::RecordDetail;
// The family-create view-model types the prelude doesn't re-export (the partner draft + its new-person
// fields). `PartnerInput` is the genealogy-ui view-model twin, not genealogy-app's.
use genealogy_ui::{FamilyChildVm, NewPersonFields, PartnerInput};

/// The selectable child-parent relationships offered when adding a child (the standard set; a custom
/// relationship is not entered from the UI).
fn relationship_choices() -> [ChildParentRelationship; 6] {
    [
        ChildParentRelationship::Birth,
        ChildParentRelationship::Adopted,
        ChildParentRelationship::Foster,
        ChildParentRelationship::Step,
        ChildParentRelationship::Sealed,
        ChildParentRelationship::Unknown,
    ]
}

/// The family master-detail: a searchable list on the left, the selected family's detail (overview,
/// children with per-partner relationships, events, media, notes, tags, history) on the right.
#[component]
pub fn FamilyScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Families.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().family_list_empty();
    let dismiss_label = state.data_loc().action_label("dismiss");
    let list_chrome = ListChrome {
        list_label: entity.clone(),
        filter_placeholder: chrome.list_filter(&entity),
        empty,
    };
    let mut nav = use_context::<NavState>();
    let mut selected = use_signal(|| None::<String>);
    let mut creating = use_signal(|| false);
    let mut toast = use_signal(|| None::<String>);
    use_effect(move || selected.set(nav.active_record_ref().map(|record| record.human_id)));
    // The top-bar `New` sets `pending_create`; open the draft here (nothing is created until Save).
    use_effect(move || {
        if *nav.pending_create.read() == Some(Category::Families) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowFamilyList).await }
    });
    let list_pane = match &*list.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::List(rows))) => rsx! {
            ListPane {
                rows: rows.clone(),
                query,
                selected,
                chrome: list_chrome.clone(),
                onselect: move |row: RowVm| {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::Families,
                        human_id: row.id,
                        label: row.title,
                    });
                },
            }
        },
        Some(ScreenData::Loaded(
            IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::SourceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Pedigree(_)
            | IntentOutcome::Relationship(_)
            | IntentOutcome::DuplicateCandidates(_)
            | IntentOutcome::MergeCompare(_),
        )) => rsx! {},
    };
    let on_created = use_callback(move |id: String| {
        creating.set(false);
        nav.open_record(RecordRef {
            category: Category::Families,
            human_id: id.clone(),
            label: id,
        });
    });
    let detail = if creating() {
        rsx! {
            FamilyCreateRecord {
                oncreated: move |id| on_created.call(id),
                oncancel: move |()| creating.set(false),
                onerror: move |message| toast.set(Some(message)),
            }
        }
    } else {
        rsx! { RecordDetail {} }
    };
    rsx! {
        MasterDetail { list: list_pane, detail }
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// The create-mode family record: an uncommitted [`FamilyDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). The editable human id plus a People find-or-create picker
/// (pick an existing partner, or "+ New person" to create one inline); Save commits the whole family
/// (≥1 partner required); Cancel discards.
#[component]
fn FamilyCreateRecord(
    oncreated: EventHandler<String>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::FamilyDraft>();
    let mut draft = record.draft;
    // The People picker adds one partner at a time: a pick appends an existing-partner chip and resets
    // the search; "+ New person" opens the pending new-partner draft card. Options load once per form.
    let partner_state = use_signal(genealogy_ui::PickerState::default);
    let pending_new = use_signal(|| None::<NewPersonFields>);
    let partner_services = services.clone();
    let partner_rows = use_resource(move || {
        let services = partner_services.clone();
        async move { load_picker_rows(services, Category::People).await }
    });
    let mut partner_state_reset = partner_state;
    let partner_onpick = use_callback(move |selection: PickerSelection| {
        draft.write().add_partner(selection);
        partner_state_reset.write().clear();
    });
    let partner_onclear = use_callback(move |()| partner_state_reset.write().clear());
    let mut pending_new_open = pending_new;
    let partner_onnew = use_callback(move |_query: String| pending_new_open.set(Some(NewPersonFields::default())));
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::FamilyDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let services = services.clone();
        spawn(async move {
            match commit_family_change_set(services, request, prov).await {
                Ok(id) => oncreated.call(id),
                Err(message) => onerror.call(message),
            }
        });
    });
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| oncancel.call(()) }
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| {
                if record.can_save() {
                    on_save.call((record.draft.read().clone(), record.prov.read().clone()));
                }
            },
        }
    };
    let excluded: Vec<String> = draft()
        .partners
        .iter()
        .filter_map(|partner| match partner {
            PartnerInput::Existing(selection) => Some(selection.human_id.clone()),
            PartnerInput::New(_) => None,
        })
        .collect();
    let partner_picker = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("partner"),
            name: "family-partner".to_owned(),
            entity_label: loc.picker_entity(Category::People),
            allow_new: true,
        },
        state: partner_state,
        options: picker_options(partner_rows.read_unchecked().as_ref()),
        exclude: excluded,
        callbacks: PickerCallbacks {
            onpick: partner_onpick,
            onclear: partner_onclear,
            onnew: partner_onnew,
        },
    };
    create_record_frame(
        &loc.family_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {family_create_fields(loc, draft, pending_new, &partner_picker)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The family's only editable scalar — its user-facing id — read-first (`record-editing.html` §2/§3):
/// a read box in view mode, an input with per-field reset in edit mode. A pure fn (the edit state's
/// signals passed in) so the SSR tests render it without `AppCtx`.
pub fn family_record_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::FamilyDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    rsx! {
        Card { title: loc.section_label("partners"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "family-id".to_owned(),
                    editing,
                    value: draft().human_id.clone(),
                    original: seed.read().human_id.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("id")),
                    mono: true,
                    hint: Some(loc.field_human_id_hint()),
                    oninput: move |value: String| draft.write().human_id = value,
                    onreset: move |()| {
                        let value = seed.read().human_id.clone();
                        draft.write().human_id = value;
                    },
                }
            }
        }
    }
}

/// The family create form's field rows (`family.html`): the editable human id, the partner chips
/// (removable), and the partner entry — a People picker, the pending new-partner draft card, or the
/// two-partner cap note. A pure fn (no `AppCtx`) so SSR tests can render it directly; `pending_new`
/// holds the in-progress "+ New person" fields and `partner` is the configured People picker.
pub fn family_create_fields(
    loc: &Localizer,
    draft: Signal<genealogy_ui::FamilyDraft>,
    pending_new: Signal<Option<NewPersonFields>>,
    partner: &RecordPicker,
) -> Element {
    let partners = draft().partners.clone();
    let at_capacity = partners.len() >= 2;
    let has_partner = !partners.is_empty();
    rsx! {
        Card { title: loc.section_label("partners"),
            div { class: "stack",
                {family_create_id_field(loc, draft)}
            }
            {family_partner_chips(loc, draft, &partners)}
            {family_partner_entry(loc, draft, pending_new, partner, at_capacity)}
            if !has_partner {
                div { class: "field",
                    span { class: "field-error", "{loc.family_partners_required()}" }
                }
            }
        }
    }
}

/// The editable human-id field of the family create form (edit mode always; blank ⇒ generated).
fn family_create_id_field(loc: &Localizer, mut draft: Signal<genealogy_ui::FamilyDraft>) -> Element {
    rsx! {
        DraftText {
            label: loc.field_label("id"),
            name: "human-id".to_owned(),
            editing: true,
            value: draft().human_id.clone(),
            original: String::new(),
            reset_label: loc.action_reset_field(&loc.field_label("id")),
            mono: true,
            hint: Some(loc.field_human_id_hint()),
            oninput: move |value: String| draft.write().human_id = value,
            onreset: move |()| draft.write().human_id = String::new(),
        }
    }
}

/// The added-partner chips: an existing partner shows its title + mono id; a new partner shows its
/// name + a "draft" badge. Each chip removes its own entry by index.
fn family_partner_chips(
    loc: &Localizer,
    draft: Signal<genealogy_ui::FamilyDraft>,
    partners: &[PartnerInput],
) -> Element {
    if partners.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "wrap", style: "margin-bottom:8px",
            for (index , partner) in partners.iter().enumerate() {
                {family_partner_chip(loc, draft, index, partner)}
            }
        }
    }
}

/// One partner chip (existing or new) with a labelled remove control.
fn family_partner_chip(
    loc: &Localizer,
    mut draft: Signal<genealogy_ui::FamilyDraft>,
    index: usize,
    partner: &PartnerInput,
) -> Element {
    let dismiss = loc.action_label("dismiss");
    let remove = rsx! {
        button {
            r#type: "button",
            class: "chip-x",
            aria_label: dismiss,
            onclick: move |_| draft.write().remove_partner(index),
            "×"
        }
    };
    match partner {
        PartnerInput::Existing(selection) => {
            let title = selection.title.clone();
            let id = selection.human_id.clone();
            rsx! {
                span { key: "{index}", class: "chip",
                    span { class: "val", "{title}" }
                    span { class: "row-id", "{id}" }
                    {remove}
                }
            }
        }
        PartnerInput::New(fields) => {
            let name = new_partner_name(fields);
            rsx! {
                span { key: "{index}", class: "chip",
                    "{name}"
                    span { class: "badge draft", "{loc.draft_card_badge()}" }
                    {remove}
                }
            }
        }
    }
}

/// The partner entry control: the People picker while adding, the pending new-partner draft card once
/// "+ New person" is chosen, or the cap note once both partners are set.
fn family_partner_entry(
    loc: &Localizer,
    draft: Signal<genealogy_ui::FamilyDraft>,
    mut pending_new: Signal<Option<NewPersonFields>>,
    partner: &RecordPicker,
    at_capacity: bool,
) -> Element {
    if at_capacity {
        return rsx! {
            div { class: "field",
                span { class: "muted", "{loc.family_partners_full()}" }
            }
        };
    }
    if pending_new().is_some() {
        let title = loc.person_new_title();
        let body = family_new_partner_body(loc, pending_new, draft);
        return draft_card(
            &title,
            &loc.draft_card_badge(),
            loc.draft_card_discard(&title),
            Callback::new(move |()| pending_new.set(None)),
            body,
        );
    }
    record_picker(loc, partner)
}

/// The inline new-partner fields inside the draft card: a given-name and surname input bound to the
/// pending buffer, plus an add action that commits the named partner to the draft and closes the card.
fn family_new_partner_body(
    loc: &Localizer,
    mut pending_new: Signal<Option<NewPersonFields>>,
    mut draft: Signal<genealogy_ui::FamilyDraft>,
) -> Element {
    let fields = pending_new().unwrap_or_default();
    let can_add = !(fields.given.trim().is_empty() && fields.surname.trim().is_empty());
    rsx! {
        Input {
            label: loc.field_label("given"),
            name: "new-partner-given".to_owned(),
            value: fields.given.clone(),
            oninput: move |event: FormEvent| {
                if let Some(fields) = pending_new.write().as_mut() {
                    fields.given = event.value();
                }
            },
        }
        Input {
            label: loc.field_label("surname"),
            name: "new-partner-surname".to_owned(),
            value: fields.surname.clone(),
            oninput: move |event: FormEvent| {
                if let Some(fields) = pending_new.write().as_mut() {
                    fields.surname = event.value();
                }
            },
        }
        Button {
            label: loc.action_label("add-partner"),
            variant: ButtonVariant::Primary,
            disabled: !can_add,
            onclick: move |_| {
                if let Some(fields) = pending_new() {
                    draft.write().add_new_partner(fields);
                    pending_new.set(None);
                }
            },
        }
    }
}

/// A new partner's display name from its inline fields (given + surname, blanks dropped).
fn new_partner_name(fields: &NewPersonFields) -> String {
    [fields.given.trim(), fields.surname.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Which family edit form (if any) the side panel is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FamilyEditForm {
    /// Add a partner by `human_id`.
    Partner,
    /// Assert a child with per-partner relationships — `None` adds a new one, `Some(row)` edits
    /// (supersedes) an existing one.
    Child(Option<FamilyChildVm>),
    /// Link an existing event by `human_id`.
    Event,
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected family: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn FamilyDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let nav = use_context::<NavState>();
    let mut label_nav = nav;
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<FamilyEditForm>);
    let mut retract = use_signal(|| None::<(String, String, bool)>);
    let mut retract_reason = use_signal(String::new);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowFamily { human_id }).await }
    });

    // The shared whole-record edit state, seeded from the loaded family (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::FamilyDetail(detail))) => genealogy_ui::FamilyDraft::from_detail(detail),
        _ => genealogy_ui::FamilyDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::FamilyDraft>(&seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the family's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::FamilyDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::Families,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (FamilyEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_family_edit(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    let batch_services = services.clone();
    let batch_saved = saved_label.clone();
    let mut editing_for_batch = editing;
    let on_submit_batch = use_callback(move |edits: Vec<(FamilyEdit, ProvenanceDraft)>| {
        let services = batch_services.clone();
        let saved = batch_saved.clone();
        spawn(async move {
            // Apply each per-partner edit in turn (per-link supersede/assert/retract), then reload
            // once. cqrs-es commits per aggregate, so a mid-batch failure leaves earlier links applied.
            let mut outcome = Ok(());
            for (edit, prov) in edits {
                if let Err(message) = save_family_edit(services.clone(), edit, prov).await {
                    outcome = Err(message);
                    break;
                }
            }
            match outcome {
                Ok(()) => {
                    editing_for_batch.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    // A per-row Retract/Remove/Unlink/Detach opens the shared retract panel; confirming dispatches an
    // `UndoAssertion` carrying the typed rationale (the retract note stays in History — ADR 0004 §2).
    let on_retract = use_callback(move |target: (String, String, bool)| {
        retract_reason.set(String::new());
        retract.set(Some(target));
    });
    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: FamilyEditForm| editing_for_open.set(Some(form)));
    let retract_services = state.services().clone();
    let retract_human = human_id.clone();
    let retract_saved = saved_label.clone();
    let on_retract_confirm = use_callback(move |()| {
        let Some((assertion_id, _, _)) = retract() else {
            return;
        };
        let services = retract_services.clone();
        let human_id = retract_human.clone();
        let saved = retract_saved.clone();
        let prov = ProvenanceDraft {
            rationale: retract_reason(),
            ..ProvenanceDraft::default()
        };
        spawn(async move {
            let edit = FamilyEdit::UndoAssertion { human_id, assertion_id };
            match save_family_edit(services, edit, prov).await {
                Ok(_) => {
                    retract.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    let record_services = services.clone();
    let record_nav = nav;
    let current_id = human_id.clone();
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::FamilyDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_family_edit).await;
            finish_record_save(
                effective,
                Category::Families,
                &current,
                record_nav,
                reload,
                toast,
                &saved,
            );
        });
    });

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::FamilyDetail(detail))) => family_detail(
            &state,
            detail,
            FamilyPane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            },
            FamilyCallbacks {
                on_submit,
                on_submit_batch,
                on_record_save,
                on_retract,
                on_retract_confirm,
                on_edit_open,
            },
            &human_id,
        ),
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::SourceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Pedigree(_)
            | IntentOutcome::Relationship(_)
            | IntentOutcome::DuplicateCandidates(_)
            | IntentOutcome::MergeCompare(_),
        )) => rsx! {},
    };

    rsx! {
        {body}
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// Renders a loaded family's detail container: header (title, restriction toggles), the tab strip,
/// the active tab's content, and the editing side panel.
/// The signals a family's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit state.
#[derive(Clone, Copy)]
struct FamilyPane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<FamilyEditForm>>,
    /// The whole-record (id-only) edit state.
    record: RecordEditState<genealogy_ui::FamilyDraft>,
    /// The row being retracted/detached, if the retract panel is open: `(assertion_id, label, detach)`.
    retract: Signal<Option<(String, String, bool)>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a family's detail wires in: one-command collection edits, the whole-record
/// save (the id edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct FamilyCallbacks {
    /// Commits one [`FamilyEdit`] command (a collection row).
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    /// Commits a batch of [`FamilyEdit`] commands sequentially, each with its own provenance — the
    /// child edit form's per-partner relationship diff (ADR 0021). Reloads once when the batch lands.
    on_submit_batch: Callback<Vec<(FamilyEdit, ProvenanceDraft)>>,
    /// Commits the buffered id edit as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::FamilyDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<FamilyEditForm>,
}

fn family_detail(
    state: &AppState,
    detail: &FamilyDetail,
    pane: FamilyPane,
    callbacks: FamilyCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let FamilyPane {
        active,
        side_edit: editing,
        record,
        retract,
        retract_reason,
    } = pane;
    let on_submit = callbacks.on_submit;
    let on_submit_batch = callbacks.on_submit_batch;
    let on_record_save = callbacks.on_record_save;
    let on_retract = callbacks.on_retract;
    let on_retract_confirm = callbacks.on_retract_confirm;
    let on_edit_open = callbacks.on_edit_open;
    let tabs = family_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_id = tabs.get(active()).map_or("overview", |tab| tab.id);
    let labels = RecordActionLabels::resolve(loc);
    rsx! {
        div { class: "record-pane", tabindex: "-1", onkeydown: move |event| record_keydown(&event, record, on_record_save),
            DetailContainer {
                title: detail.title.clone(),
                id_label: Some(detail.human_id.clone()),
                avatar: "👪".to_owned(),
                extras: family_restriction_toggles(loc, detail, on_submit, human_id),
                actions: record_head_actions(&labels, record, rsx! {}, on_record_save),
                tabs: tab_items,
                active,
                {family_tab_content(state, detail, active_id, editing, record, on_submit, on_retract, on_edit_open, human_id)}
            }
            {family_edit_panel(state, detail, editing, on_submit, on_submit_batch, human_id)}
            {family_retract_panel(loc, retract, retract_reason, on_retract_confirm)}
        }
    }
}

/// Renders the shared Retract/Detach side panel when a family collection row's action is armed. Reads
/// the armed `(assertion_id, label, detach)` and binds the rationale input; confirming dispatches
/// `UndoAssertion`. Closed (rendered empty) when nothing is armed. Never renders the target's
/// `AssertionId`.
fn family_retract_panel(
    loc: &Localizer,
    mut retract: Signal<Option<(String, String, bool)>>,
    reason: Signal<String>,
    on_confirm: Callback<()>,
) -> Element {
    let Some((_, label, detach)) = retract() else {
        return rsx! {};
    };
    let (title_id, button_id, note, accessible) = if detach {
        (
            "detach",
            "detach",
            loc.action_title("detach-media"),
            loc.action_detach_row(&label),
        )
    } else {
        ("retract", "retract", loc.retract_note(), loc.action_retract_row(&label))
    };
    rsx! {
        SidePanel {
            title: loc.panel_title(title_id),
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| retract.set(None),
            footer: rsx! {},
            {retract_panel(loc, &loc.panel_title(title_id), &label, accessible, &note, loc.action_label(button_id), reason, on_confirm)}
        }
    }
}

/// The interactive privacy-restriction toggles for a family (the mockup `resn-set`).
fn family_restriction_toggles(
    loc: &Localizer,
    detail: &FamilyDetail,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let selected: Vec<RestrictionKind> = detail.restrictions.clone();
    let choices: Vec<RestrictionChoice> = RestrictionKind::all()
        .into_iter()
        .map(|kind| RestrictionChoice {
            kind,
            label: loc.restriction_label(kind),
        })
        .collect();
    let human_id = human_id.to_owned();
    rsx! {
        RestrictionSet {
            choices,
            selected: selected.clone(),
            ontoggle: move |kind: RestrictionKind| {
                let mut next = selected.clone();
                if let Some(position) = next.iter().position(|&k| k == kind) {
                    next.remove(position);
                } else {
                    next.push(kind);
                }
                on_submit.call((FamilyEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one family detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn family_tab_content(
    state: &AppState,
    detail: &FamilyDetail,
    tab_id: &str,
    mut editing: Signal<Option<FamilyEditForm>>,
    record: RecordEditState<genealogy_ui::FamilyDraft>,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<FamilyEditForm>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "children" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-child"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Child(None))) }
            }
            {family_children_table(loc, detail, on_edit_open, on_retract)}
        },
        "events" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("link-event"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Event)) }
            }
            {family_events_table(loc, &detail.events, on_retract)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Media)) }
            }
            {family_media_gallery(loc, &detail.media, Some(on_retract))}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Note)) }
            }
            {id_list(loc, &detail.notes, Some(on_retract))}
        },
        "tags" => family_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => family_history_tab(loc, detail, on_submit, human_id),
        _ => family_overview(loc, detail, editing, record, on_retract),
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): the neutral-roles note, the Partners and
/// Marriage cards. Entering edit mode (via the sticky-header Edit) swaps in the family's only scalar —
/// its id — and, while dirty, the provenance block.
pub fn family_overview(
    loc: &Localizer,
    detail: &FamilyDetail,
    mut editing: Signal<Option<FamilyEditForm>>,
    record: RecordEditState<genealogy_ui::FamilyDraft>,
    on_retract: Callback<(String, String, bool)>,
) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.family_overview_note()}" }
            {family_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        };
    }
    rsx! {
        div { class: "section-note", "{loc.family_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("partners"),
                div { class: "tab-actions",
                    Button { label: loc.action_label("add-partner"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(FamilyEditForm::Partner)) }
                }
                if detail.partners.is_empty() {
                    EmptyState { message: loc.tab_empty() }
                } else {
                    div { class: "stack",
                        for partner in detail.partners.iter() {
                            div { class: "fact-row",
                                span { class: "grow", "{partner.name}" }
                                if let Some(vitals) = partner.vitals.clone() {
                                    span { class: "muted", "{vitals}" }
                                }
                                {provenance_cue(loc, loc.provenance_title_claim(&partner.name), &partner.citations)}
                                {
                                    let assertion_id = partner.assertion_id.clone();
                                    let name = partner.name.clone();
                                    rsx! {
                                        Button {
                                            label: loc.action_label("remove"),
                                            variant: ButtonVariant::Ghost,
                                            small: true,
                                            title: loc.action_title("remove-partner"),
                                            aria_label: loc.action_remove_row(&partner.name),
                                            onclick: move |_| on_retract.call((assertion_id.clone(), name.clone(), false)),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Card { title: loc.section_label("marriage"),
                if let Some(marriage) = detail.marriage.as_ref() {
                    div { class: "stack",
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:64px;margin:0", "{loc.field_label(\"date\")}" }
                            span { class: "grow", {marriage.date.clone().unwrap_or_else(|| "—".to_owned())} }
                            ConfidenceBadge { level: marriage.confidence, label: marriage.confidence_label.clone() }
                            {provenance_cue(loc, loc.provenance_title_claim(&marriage.type_label), &marriage.citations)}
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:64px;margin:0", "{loc.field_label(\"place\")}" }
                            span { class: "grow", {marriage.place.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:64px;margin:0", "{loc.field_label(\"attribute-type\")}" }
                            span { class: "grow", Chip { label: marriage.type_label.clone() } }
                        }
                    }
                } else {
                    EmptyState { message: loc.tab_empty() }
                }
            }
        }
    }
}

/// The Children tab: a row per child with a relationship column per family partner, plus surety and
/// source columns (the per-partner relationship model — GEDCOM `_FREL`/`_MREL`).
pub fn family_children_table(
    loc: &Localizer,
    detail: &FamilyDetail,
    onedit: Callback<FamilyEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if detail.children.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    let mut headers = vec![loc.field_label("child"), loc.field_label("born")];
    for partner in &detail.partners {
        headers.push(partner.name.clone());
    }
    headers.push(loc.field_label("surety"));
    headers.push(loc.field_label("source"));
    headers.push(String::new());
    let partner_ids: Vec<String> = detail.partners.iter().map(|partner| partner.human_id.clone()).collect();
    rsx! {
        Table { headers,
            for child in detail.children.iter() {
                tr {
                    td { "{child.name}" }
                    td { class: "muted", {child.born.clone().unwrap_or_else(|| "—".to_owned())} }
                    for partner_id in partner_ids.iter() {
                        td {
                            {
                                match child.relationships.iter().find(|link| &link.partner_human_id == partner_id) {
                                    Some(link) => rsx! { Chip { label: link.label.clone() } },
                                    None => rsx! { span { class: "muted", "—" } },
                                }
                            }
                        }
                    }
                    td { ConfidenceBadge { level: child.confidence, label: child.confidence_label.clone() } }
                    td { {source_cue(loc, child.source_count)} }
                    {row_actions_cell(
                        loc,
                        &child.name,
                        Some((FamilyEditForm::Child(Some(child.clone())), None)), None,
                        Some(RowRetract { assertion_id: child.assertion_id.clone(), button_label: "remove", title: "remove-child", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
        }
    }
}

/// The Events tab: a row per linked family event with its kind, date, place, surety, and source.
pub fn family_events_table(
    loc: &Localizer,
    events: &[FamilyEventVm],
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if events.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.tab_label("events"),
                loc.field_label("date"),
                loc.field_label("place"),
                loc.field_label("surety"),
                loc.field_label("source"),
                String::new(),
            ],
            for event in events.iter() {
                tr {
                    td { "{event.type_label}" }
                    td { class: "muted", {event.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { {event.place.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { ConfidenceBadge { level: event.confidence, label: event.confidence_label.clone() } }
                    td { {source_cue(loc, event.source_count)} }
                    {row_actions_cell::<FamilyEditForm>(
                        loc,
                        &event.type_label,
                        None, None,
                        Some(RowRetract { assertion_id: event.assertion_id.clone(), button_label: "unlink", title: "unlink-event", detach: false }),
                        None,
                        onretract)}
                }
            }
        }
    }
}

/// The Tags tab: each applied tag as a colour-dot chip (name + colour, never the id) with a remove
/// control, plus an "Add tag" affordance.
pub fn family_tags_panel(
    loc: &Localizer,
    detail: &FamilyDetail,
    mut editing: Signal<Option<FamilyEditForm>>,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(FamilyEditForm::Tag)) }
        }
        if detail.tags.is_empty() {
            EmptyState { message: loc.tab_empty() }
        } else {
            div { class: "wrap",
                for tag in detail.tags.iter() {
                    {
                        let tag_id = tag.id.clone();
                        let human_id = human_id.clone();
                        let remove_label = loc.action_label("remove-tag");
                        rsx! {
                            span { class: "fact-row",
                                Chip { label: tag.name.clone(), dot_color: tag.color.clone() }
                                Button {
                                    label: remove_label,
                                    variant: ButtonVariant::Ghost,
                                    small: true,
                                    onclick: move |_| on_submit.call((FamilyEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn family_history_tab(
    loc: &Localizer,
    detail: &FamilyDetail,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    if detail.history.is_empty() {
        return rsx! { EmptyState { symbol: "🕓".to_owned(), message: loc.history_empty() } };
    }
    let undo_text = loc.history_undo_short();
    let entries: Vec<HistoryEntry> = detail
        .history
        .iter()
        .map(|entry| HistoryEntry {
            when: entry.when.clone(),
            what: entry.what.clone(),
            who: entry.who.clone(),
            why: entry.why.clone(),
            assertion_id: entry.assertion_id.clone(),
            can_undo: entry.can_undo,
            undo_text: undo_text.clone(),
            undo_label: loc.history_undo_label(&entry.what),
        })
        .collect();
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "section-note", "{loc.history_note()}" }
        HistoryTimeline {
            entries,
            onundo: move |assertion_id: String| {
                on_submit.call((FamilyEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The family editing side panel: renders the form for the open [`FamilyEditForm`], or nothing.
fn family_edit_panel(
    state: &AppState,
    detail: &FamilyDetail,
    mut editing: Signal<Option<FamilyEditForm>>,
    on_submit: Callback<(FamilyEdit, ProvenanceDraft)>,
    on_submit_batch: Callback<Vec<(FamilyEdit, ProvenanceDraft)>>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match &form {
        FamilyEditForm::Partner => loc.action_label("add-partner"),
        FamilyEditForm::Child(None) => loc.action_label("add-child"),
        FamilyEditForm::Child(Some(_)) => loc.panel_title("edit-child"),
        FamilyEditForm::Event => loc.action_label("link-event"),
        FamilyEditForm::Media => loc.action_label("attach-media"),
        FamilyEditForm::Note => loc.action_label("attach-note"),
        FamilyEditForm::Tag => loc.action_label("add-tag"),
    };
    let human_id = human_id.to_owned();
    let partners: Vec<(String, String)> = detail
        .partners
        .iter()
        .map(|partner| (partner.human_id.clone(), partner.name.clone()))
        .collect();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| editing.set(None),
            footer: rsx! {},
            {match form {
                FamilyEditForm::Partner => rsx! { FamilyAddPartnerForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                FamilyEditForm::Child(seed) => rsx! { FamilyAddChildForm { human_id, partners, seed, onsubmit: move |edits| on_submit_batch.call(edits) } },
                FamilyEditForm::Event => rsx! { FamilyLinkEventForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                FamilyEditForm::Media => rsx! { FamilyAttachForm { human_id, is_note: false, onsubmit: move |edit| on_submit.call(edit) } },
                FamilyEditForm::Note => rsx! { FamilyAttachForm { human_id, is_note: true, onsubmit: move |edit| on_submit.call(edit) } },
                FamilyEditForm::Tag => rsx! { FamilyTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Add partner" form: a person `human_id` → [`FamilyEdit::AddPartner`].
#[component]
fn FamilyAddPartnerForm(human_id: String, onsubmit: EventHandler<(FamilyEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let picker = use_existing_picker(
        services,
        Category::People,
        loc.field_label("partner"),
        "partner".to_owned(),
        loc.picker_entity(Category::People),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let picker_for_save = picker.clone();
    let onsave = use_callback(move |()| {
        let Some(person_id) = picker_selection_id(&picker_for_save) else {
            return;
        };
        onsubmit.call((
            FamilyEdit::AddPartner {
                human_id: human_id.clone(),
                person_id,
            },
            prov(),
        ));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}

/// The child form. `seed: None` adds a new child (a People picker + one relationship select per
/// family partner) → a single [`FamilyEdit::AddChild`] the app fans out (ADR 0021). `Some(row)` edits
/// an existing child — the child is fixed (shown as a link), the per-partner selects are pre-filled
/// (with a "no relationship" choice), and Save diffs each partner: a changed link supersedes that
/// link's assertion, a new one asserts plainly, and a cleared one retracts it — each its own command
/// (ADR 0004 §2), dispatched as a batch.
#[component]
fn FamilyAddChildForm(
    human_id: String,
    partners: Vec<(String, String)>,
    seed: Option<FamilyChildVm>,
    onsubmit: EventHandler<Vec<(FamilyEdit, ProvenanceDraft)>>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let edit_mode = seed.is_some();
    // The selectable relationships; edit mode prepends a "no relationship" choice (index 0) so a
    // partner's link can be cleared. `None` = no relationship for that partner.
    let mut choices: Vec<Option<ChildParentRelationship>> = Vec::new();
    if edit_mode {
        choices.push(None);
    }
    choices.extend(relationship_choices().into_iter().map(Some));
    let options: Vec<SelectChoice> = choices
        .iter()
        .enumerate()
        .map(|(position, choice)| SelectChoice {
            value: position.to_string(),
            label: choice
                .as_ref()
                .map_or_else(|| loc.relationship_none(), |kind| loc.relationship_label(kind)),
        })
        .collect();
    // Edit mode fixes the child (only the per-partner relationships change); add mode offers a picker.
    let fixed_child = seed.as_ref().map(|s| s.human_id.clone());
    let exclude: Vec<String> = partners.iter().map(|(id, _)| id.clone()).collect();
    let picker = use_existing_picker(
        services,
        Category::People,
        loc.field_label("child"),
        "child".to_owned(),
        loc.picker_entity(Category::People),
        exclude,
    );
    // Each partner's existing link (kind + assertion id) from the seed row — the diff baseline.
    let seed_links: Vec<Option<(ChildParentRelationship, String)>> = partners
        .iter()
        .map(|(partner_id, _)| {
            seed.as_ref().and_then(|row| {
                row.relationships
                    .iter()
                    .find(|link| &link.partner_human_id == partner_id)
                    .map(|link| (link.kind.clone(), link.assertion_id.clone()))
            })
        })
        .collect();
    // Seed each partner's select: its existing link's index into `choices`, else 0 (Birth in add
    // mode, "no relationship" in edit mode).
    let seed_indices: Vec<usize> = seed_links
        .iter()
        .map(|link| {
            link.as_ref()
                .and_then(|(kind, _)| choices.iter().position(|choice| choice.as_ref() == Some(kind)))
                .unwrap_or(0)
        })
        .collect();
    let mut selections = use_signal({
        let seed_indices = seed_indices.clone();
        move || seed_indices
    });
    let prov = use_signal(ProvenanceDraft::default);
    let extra = rsx! {
        for (index , (_ , name)) in partners.iter().enumerate() {
            Select {
                label: name.clone(),
                name: "rel-{index}".to_owned(),
                value: Some(seed_indices.get(index).copied().unwrap_or(0).to_string()),
                options: options.clone(),
                onchange: move |event: FormEvent| {
                    let value = event.value().parse::<usize>().unwrap_or(0);
                    selections.with_mut(|slots| {
                        if let Some(slot) = slots.get_mut(index) {
                            *slot = value;
                        }
                    });
                },
            }
        }
    };
    let partners_for_submit = partners.clone();
    let picker_for_save = picker.clone();
    let fixed_for_save = fixed_child.clone();
    let choices_for_save = choices.clone();
    let onsave = use_callback(move |()| {
        let Some(person_id) = fixed_for_save.clone().or_else(|| picker_selection_id(&picker_for_save)) else {
            return;
        };
        let chosen = selections();
        let selected = |index: usize| -> Option<ChildParentRelationship> {
            choices_for_save
                .get(chosen.get(index).copied().unwrap_or(0))
                .cloned()
                .flatten()
        };
        let batch = if edit_mode {
            child_relationship_edits(
                &human_id,
                &person_id,
                &partners_for_submit,
                &seed_links,
                &prov(),
                selected,
            )
        } else {
            let relationships: Vec<(String, ChildParentRelationship)> = partners_for_submit
                .iter()
                .enumerate()
                .map(|(index, (partner_id, _))| {
                    (
                        partner_id.clone(),
                        selected(index).unwrap_or(ChildParentRelationship::Birth),
                    )
                })
                .collect();
            vec![(
                FamilyEdit::AddChild {
                    human_id: human_id.clone(),
                    person_id,
                    relationships,
                },
                prov(),
            )]
        };
        onsubmit.call(batch);
    });
    if let Some(child) = &fixed_child {
        rsx! {
            div { class: "field",
                label { "{loc.field_label(\"child\")}" }
                RecordLink { category: Category::People, human_id: child.clone(), label: child.clone() }
            }
            {extra}
            {provenance_block(loc, prov)}
            Button {
                label: loc.action_label("save"),
                variant: ButtonVariant::Primary,
                onclick: move |_| onsave.call(()),
            }
        }
    } else {
        attach_picker_form(loc, &picker, extra, prov, onsave)
    }
}

/// Diffs the edit form's per-partner relationship selections against the child's existing links,
/// producing one [`FamilyEdit`] per change (ADR 0021): a changed link supersedes its assertion, a new
/// one asserts plainly, a cleared one retracts it. Unchanged partners produce nothing.
fn child_relationship_edits(
    human_id: &str,
    person_id: &str,
    partners: &[(String, String)],
    seed_links: &[Option<(ChildParentRelationship, String)>],
    base: &ProvenanceDraft,
    selected: impl Fn(usize) -> Option<ChildParentRelationship>,
) -> Vec<(FamilyEdit, ProvenanceDraft)> {
    let mut batch = Vec::new();
    for (index, (partner_id, _)) in partners.iter().enumerate() {
        let seed_link = seed_links.get(index).cloned().flatten();
        match (seed_link, selected(index)) {
            (None, None) => {}
            (Some((seed_kind, _)), Some(kind)) if kind == seed_kind => {}
            (seed, Some(kind)) => {
                let mut prov = base.clone();
                prov.supersedes = seed.map(|(_, link_id)| link_id);
                batch.push((
                    FamilyEdit::AssertChildRelationship {
                        human_id: human_id.to_owned(),
                        person_id: person_id.to_owned(),
                        partner_id: partner_id.clone(),
                        relationship: kind,
                    },
                    prov,
                ));
            }
            (Some((_, link_id)), None) => {
                batch.push((
                    FamilyEdit::UndoAssertion {
                        human_id: human_id.to_owned(),
                        assertion_id: link_id,
                    },
                    base.clone(),
                ));
            }
        }
    }
    batch
}

/// The "Link family event" form: an event `human_id` → [`FamilyEdit::LinkFamilyEvent`].
#[component]
fn FamilyLinkEventForm(human_id: String, onsubmit: EventHandler<(FamilyEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let picker = use_existing_picker(
        services,
        Category::Events,
        loc.tab_label("events"),
        "event".to_owned(),
        loc.picker_entity(Category::Events),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let picker_for_save = picker.clone();
    let onsave = use_callback(move |()| {
        let Some(event_id) = picker_selection_id(&picker_for_save) else {
            return;
        };
        onsubmit.call((
            FamilyEdit::LinkFamilyEvent {
                human_id: human_id.clone(),
                event_id,
            },
            prov(),
        ));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}

/// The "Attach media/note by id" form → [`FamilyEdit::AttachMedia`]/[`FamilyEdit::AttachNote`].
#[component]
fn FamilyAttachForm(human_id: String, is_note: bool, onsubmit: EventHandler<(FamilyEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let (field, category) = if is_note {
        ("note", Category::Notes)
    } else {
        ("media", Category::Media)
    };
    let picker = use_existing_picker(
        services,
        category,
        loc.field_label(field),
        field.to_owned(),
        loc.picker_entity(category),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let picker_for_save = picker.clone();
    let onsave = use_callback(move |()| {
        let Some(id) = picker_selection_id(&picker_for_save) else {
            return;
        };
        let edit = if is_note {
            FamilyEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            }
        } else {
            FamilyEdit::AttachMedia {
                human_id: human_id.clone(),
                media_id: id,
            }
        };
        onsubmit.call((edit, prov()));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}

/// The "Add tag" form: a picker of existing tags by name (the tag id is the option value, never
/// shown) → [`FamilyEdit::Tag`].
#[component]
fn FamilyTagForm(human_id: String, onsubmit: EventHandler<(FamilyEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let save_label = loc.action_label("save");
    let field_label = loc.field_label("tag");
    let tags = use_resource(move || {
        let services = services.clone();
        async move { load_tags(services).await }
    });
    let mut chosen = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    match &*tags.read_unchecked() {
        None => rsx! { p { class: "loading", "{loc.tab_empty()}" } },
        Some(Err(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(Ok(list)) => {
            let options: Vec<SelectChoice> = list
                .iter()
                .filter_map(|tag| {
                    tag.name.clone().map(|name| SelectChoice {
                        value: tag.id.clone(),
                        label: name,
                    })
                })
                .collect();
            let first = options.first().map(|choice| choice.value.clone()).unwrap_or_default();
            if chosen().is_empty() {
                chosen.set(first.clone());
            }
            rsx! {
                Select {
                    label: field_label,
                    name: "tag".to_owned(),
                    value: Some(first),
                    options,
                    onchange: move |event: FormEvent| chosen.set(event.value()),
                }
                {provenance_block(loc, prov)}
                Button {
                    label: save_label,
                    variant: ButtonVariant::Primary,
                    onclick: move |_| {
                        let tag_id = chosen();
                        if tag_id.is_empty() {
                            return;
                        }
                        onsubmit.call((FamilyEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}
