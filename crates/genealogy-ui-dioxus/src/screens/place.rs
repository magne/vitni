use genealogy_app::PlaceType;
// The place row view-models the prelude doesn't re-export; they seed the per-row Name / enclosing edits.
use genealogy_ui::{PlaceHierarchyVm, PlaceNameVm};

use super::prelude::*;

/// The create-mode place record: an uncommitted [`PlaceDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Save commits the whole place; Cancel discards. Save is
/// blocked while the coordinate pair is half-filled or unparseable (§7).
#[component]
pub fn PlaceCreateRecord() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::PlaceDraft>();
    let mut draft = record.draft;
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::PlaceDraft, ProvenanceDraft)| {
        let Some(request) = draft.to_request() else {
            return;
        };
        let label = request.name.clone().unwrap_or_default();
        let services = services.clone();
        spawn(async move {
            match commit_place_change_set(services, request, prov).await {
                Ok(id) => nav.commit_draft(RecordRef {
                    category: Category::Places,
                    human_id: id.clone(),
                    label: if label.is_empty() { id } else { label },
                }),
                Err(message) => nav.notify(message),
            }
        });
    });
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| nav.cancel_draft(Category::Places) }
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
    create_record_frame(
        &loc.place_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {place_record_fields(loc, record)}
            Input {
                label: loc.field_label("name"),
                name: "place-name".to_owned(),
                value: draft().name.clone(),
                oninput: move |event: FormEvent| draft.write().name = event.value(),
            }
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The place's scalar record fields (id · type · latitude · longitude · code), read-first: read boxes
/// in view mode, inputs with per-field reset in edit mode (`record-editing.html` §2/§3). The primary
/// name is not a scalar here — on an existing place it is the Names collection, and the create pane
/// adds its own Name field. Latitude/longitude flag an invalid pair inline (§7). A pure fn (the edit
/// state's signals passed in) so the create pane and SSR tests render it without `AppCtx`.
pub fn place_record_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::PlaceDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let types = place_type_choices();
    let options: Vec<SelectChoice> = types
        .iter()
        .enumerate()
        .map(|(index, place_type)| SelectChoice {
            value: index.to_string(),
            label: loc.place_type_label(place_type),
        })
        .collect();
    let index_of = |place_type: &PlaceType| {
        place_type_choices()
            .iter()
            .position(|t| t == place_type)
            .unwrap_or(0)
            .to_string()
    };
    let current = draft();
    let committed = seed.read().clone();
    rsx! {
        Card { title: loc.field_label("place"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "place-id".to_owned(),
                    editing,
                    value: current.human_id.clone(),
                    original: committed.human_id.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("id")),
                    mono: true,
                    hint: Some(loc.field_human_id_hint()),
                    oninput: move |value: String| draft.write().human_id = value,
                    onreset: move |()| {
                        let value = seed.read().human_id.clone();
                        draft.write().human_id = value;
                    },
                }
                DraftSelect {
                    label: loc.field_label("type"),
                    name: "place-type".to_owned(),
                    editing,
                    value: index_of(&current.place_type),
                    original: index_of(&committed.place_type),
                    reset_label: loc.action_reset_field(&loc.field_label("type")),
                    options,
                    onchange: move |value: String| {
                        let types = place_type_choices();
                        if let Some(place_type) = value.parse::<usize>().ok().and_then(|index| types.get(index).cloned()) {
                            draft.write().place_type = place_type;
                        }
                    },
                    onreset: move |()| {
                        let value = seed.read().place_type.clone();
                        draft.write().place_type = value;
                    },
                }
                {place_coordinate_fields(loc, editing, draft, seed)}
                DraftText {
                    label: loc.field_label("code"),
                    name: "place-code".to_owned(),
                    editing,
                    value: current.code.clone(),
                    original: committed.code.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("code")),
                    mono: true,
                    oninput: move |value: String| draft.write().code = value,
                    onreset: move |()| {
                        let value = seed.read().code.clone();
                        draft.write().code = value;
                    },
                }
            }
        }
    }
}

/// The place's latitude/longitude record fields, each flagging an invalid or half-filled pair inline
/// (`record-editing.html` §7). Split out of [`place_record_fields`] to keep that fn within its line
/// budget.
fn place_coordinate_fields(
    loc: &Localizer,
    editing: bool,
    mut draft: Signal<genealogy_ui::PlaceDraft>,
    seed: Signal<genealogy_ui::PlaceDraft>,
) -> Element {
    let current = draft();
    let committed = seed.read().clone();
    let coordinate_error = loc.place_coordinate_invalid();
    rsx! {
        DraftText {
            label: loc.field_label("latitude"),
            name: "place-latitude".to_owned(),
            editing,
            value: current.latitude.clone(),
            original: committed.latitude.clone(),
            reset_label: loc.action_reset_field(&loc.field_label("latitude")),
            error: current.latitude_invalid().then(|| coordinate_error.clone()),
            oninput: move |value: String| draft.write().latitude = value,
            onreset: move |()| {
                let value = seed.read().latitude.clone();
                draft.write().latitude = value;
            },
        }
        DraftText {
            label: loc.field_label("longitude"),
            name: "place-longitude".to_owned(),
            editing,
            value: current.longitude.clone(),
            original: committed.longitude.clone(),
            reset_label: loc.action_reset_field(&loc.field_label("longitude")),
            error: current.longitude_invalid().then_some(coordinate_error),
            oninput: move |value: String| draft.write().longitude = value,
            onreset: move |()| {
                let value = seed.read().longitude.clone();
                draft.write().longitude = value;
            },
        }
    }
}

/// Which place collection-row edit form (if any) the side panel is showing. The place's own scalar
/// record (id · type · coordinates · code) is edited in place via the sticky header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceEditForm {
    /// Assert a name — `None` adds a new one, `Some(row)` edits (supersedes) an existing one.
    Name(Option<PlaceNameVm>),
    /// Assert an enclosing place — `None` adds a new link, `Some(row)` edits (supersedes) an existing
    /// one (the enclosing place is fixed; the correction updates its provenance).
    Enclosing(Option<PlaceHierarchyVm>),
    /// Attach a citation by `human_id`.
    Citation,
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected place: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn PlaceDetailPane(human_id: String) -> Element {
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
    let editing = use_signal(|| None::<PlaceEditForm>);
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
        async move { load_screen(services, Intent::ShowPlace { human_id }).await }
    });

    // The shared whole-record edit state, seeded from the loaded place (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) => genealogy_ui::PlaceDraft::from_detail(detail),
        _ => genealogy_ui::PlaceDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::PlaceDraft>(&seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the place's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::Places,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (PlaceEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_place_edit(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    // A per-row Retract/Detach opens the shared retract panel; confirming dispatches an
    // `UndoAssertion` carrying the typed rationale (the retract note stays in History — ADR 0004 §2).
    let on_retract = use_callback(move |target: (String, String, bool)| {
        retract_reason.set(String::new());
        retract.set(Some(target));
    });
    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: PlaceEditForm| editing_for_open.set(Some(form)));
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
            let edit = PlaceEdit::UndoAssertion { human_id, assertion_id };
            match save_place_edit(services, edit, prov).await {
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::PlaceDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_place_edit).await;
            finish_record_save(effective, Category::Places, &current, record_nav, reload, toast, &saved);
        });
    });

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            PlaceEdit::UndoAssertion {
                human_id: undo_human.clone(),
                assertion_id,
            },
            ProvenanceDraft::default(),
        ));
    });
    use_record_undo(nav, undo_busy, undo_history, undo_notice, on_undo);

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) => place_detail(
            &state,
            detail,
            PlacePane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            },
            PlaceCallbacks {
                on_submit,
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
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::DataQuality(_)
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

/// The signals a place's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit state.
#[derive(Clone, Copy)]
struct PlacePane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<PlaceEditForm>>,
    /// The whole-record (id · type · coordinates · code) edit state.
    record: RecordEditState<genealogy_ui::PlaceDraft>,
    /// The row being retracted/detached, if the retract panel is open: `(assertion_id, label, detach)`.
    retract: Signal<Option<(String, String, bool)>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a place's detail wires in: one-command collection edits, the whole-record
/// save (the scalar edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct PlaceCallbacks {
    /// Commits one [`PlaceEdit`] command (a collection row).
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::PlaceDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<PlaceEditForm>,
}

/// Renders a loaded place's detail container: header (with the sticky-header record Edit/Cancel/Save),
/// the tab strip, the active tab, and the collection-row side panel.
fn place_detail(
    state: &AppState,
    detail: &PlaceDetail,
    pane: PlacePane,
    callbacks: PlaceCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let PlacePane {
        active,
        side_edit: editing,
        record,
        retract,
        retract_reason,
    } = pane;
    let on_submit = callbacks.on_submit;
    let on_retract = callbacks.on_retract;
    let on_retract_confirm = callbacks.on_retract_confirm;
    let on_edit_open = callbacks.on_edit_open;
    let tabs = place_tabs(detail, loc);
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
        DetailContainer {
            title: detail.title.clone(),
            id_label: Some(detail.human_id.clone()),
            avatar: "📍".to_owned(),
            extras: place_restriction_toggles(loc, detail, on_submit, human_id),
            actions: record_head_actions(&labels, record, rsx! {}, callbacks.on_record_save),
            tabs: tab_items,
            active,
            {place_tab_content(state, detail, active_id, editing, record, on_submit, on_retract, on_edit_open, human_id)}
        }
        {place_edit_panel(state, editing, on_submit, human_id)}
        {place_retract_panel(loc, retract, retract_reason, on_retract_confirm)}
    }
}

/// Renders the shared Retract/Detach side panel when a place collection row's action is armed. Reads
/// the armed `(assertion_id, label, detach)` and binds the rationale input; confirming dispatches
/// `UndoAssertion`. Closed (rendered empty) when nothing is armed. Never renders the target's
/// `AssertionId`.
fn place_retract_panel(
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
            loc.action_title("detach-citation"),
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

/// The interactive privacy-restriction toggles for a place (the mockup `resn-set`).
fn place_restriction_toggles(
    loc: &Localizer,
    detail: &PlaceDetail,
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
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
                on_submit.call((PlaceEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one place detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn place_tab_content(
    state: &AppState,
    detail: &PlaceDetail,
    tab_id: &str,
    mut editing: Signal<Option<PlaceEditForm>>,
    record: RecordEditState<genealogy_ui::PlaceDraft>,
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<PlaceEditForm>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "names" => rsx! {
            div { class: "section-note", "{loc.place_names_note()}" }
            div { class: "tab-actions",
                Button { label: loc.action_label("add-name"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Name(None))) }
            }
            {place_names_table(loc, detail, on_edit_open, on_retract)}
        },
        "hierarchy" => rsx! {
            div { class: "section-note", "{loc.place_hierarchy_note()}" }
            div { class: "tab-actions",
                Button { label: loc.action_label("add-enclosing"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Enclosing(None))) }
            }
            {place_hierarchy_table(loc, detail, on_edit_open, on_retract)}
        },
        "citations" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-citation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Citation)) }
            }
            {place_citations_table(loc, &detail.citations, on_retract)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Media)) }
            }
            {family_media_gallery(loc, &detail.media, Some(on_retract))}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Note)) }
            }
            {id_list(loc, &detail.notes, Some(on_retract))}
        },
        "tags" => place_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => place_history_tab(loc, detail, on_submit, human_id),
        _ => place_overview(loc, detail, record),
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): the place's scalar record (id · type ·
/// coordinates · code) as read boxes plus an "Enclosed by" card. Entering edit mode (via the
/// sticky-header Edit) swaps the record fields to inputs and, while dirty, shows the provenance block;
/// the enclosing card is hidden in edit mode. The coordinate provenance popover shows in view mode.
pub fn place_overview(
    loc: &Localizer,
    detail: &PlaceDetail,
    record: RecordEditState<genealogy_ui::PlaceDraft>,
) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.place_overview_note()}" }
            {place_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        };
    }
    rsx! {
        div { class: "section-note", "{loc.place_overview_note()}" }
        div { class: "grid-2",
            {place_record_fields(loc, record)}
            Card { title: loc.tab_label("hierarchy"),
                if let Some(enclosing) = detail.hierarchy.first() {
                    div { class: "stack",
                        div { class: "fact-row",
                            span { class: "grow", "{enclosing.name}" }
                            if let Some(date) = enclosing.date.clone() {
                                span { class: "muted", "{date}" }
                            }
                        }
                    }
                } else {
                    EmptyState { message: loc.tab_empty() }
                }
            }
        }
    }
}

/// The Names tab: a row per asserted name with language, date, surety, and source columns, plus a
/// per-row Edit (supersedes via [`PlaceEdit::AddName`]) and Retract (retracts the name assertion — it
/// stays in History).
pub fn place_names_table(
    loc: &Localizer,
    detail: &PlaceDetail,
    onedit: Callback<PlaceEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if detail.names.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("name"),
                loc.field_label("language"),
                loc.field_label("date"),
                loc.field_label("confidence"),
                loc.field_label("source"),
                String::new(),
            ],
            for name in detail.names.iter() {
                tr {
                    td { b { "{name.text}" } }
                    td { class: "muted", {name.language.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { {name.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { ConfidenceBadge { level: name.confidence, label: name.confidence_label.clone() } }
                    td { {source_cue(loc, name.source_count)} }
                    {row_actions_cell(
                        loc,
                        &name.text,
                        Some((PlaceEditForm::Name(Some(name.clone())), None)), None,
                        Some(RowRetract { assertion_id: name.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
        }
    }
}

/// The Hierarchy tab: a breadcrumb of the jurisdiction chain plus a level-by-level table, each row
/// carrying a per-row Edit (supersedes via [`PlaceEdit::AddEnclosing`]) and Retract (retracts the
/// enclosing-by assertion — it stays in History).
pub fn place_hierarchy_table(
    loc: &Localizer,
    detail: &PlaceDetail,
    onedit: Callback<PlaceEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if detail.hierarchy.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "breadcrumb", style: "margin-bottom:16px",
            b { "{detail.title}" }
            for enclosing in detail.hierarchy.iter() {
                span { class: "sep", "›" }
                span { "{enclosing.name}" }
            }
        }
        Table {
            headers: vec![
                loc.field_label("name"),
                loc.field_label("attribute-type"),
                loc.field_label("date"),
                loc.field_label("confidence"),
                String::new(),
            ],
            for enclosing in detail.hierarchy.iter() {
                tr {
                    td { "{enclosing.name}" }
                    td {
                        if let Some(type_label) = enclosing.type_label.clone() {
                            Chip { label: type_label }
                        } else {
                            span { class: "muted", "—" }
                        }
                    }
                    td { class: "muted", {enclosing.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { ConfidenceBadge { level: enclosing.confidence, label: enclosing.confidence_label.clone() } }
                    {row_actions_cell(
                        loc,
                        &enclosing.name,
                        Some((PlaceEditForm::Enclosing(Some(enclosing.clone())), None)), None,
                        Some(RowRetract { assertion_id: enclosing.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
        }
    }
}

/// The place Citations tab: each backing citation's source, page, surety, and Evidence Explained
/// axes, plus a per-row Detach (retracts the attach assertion — it stays in History). A citation with
/// no attach `AssertionId` (shown as evidence, not an attachment) renders no Detach.
pub fn place_citations_table(
    loc: &Localizer,
    citations: &[CitationRefVm],
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("source"),
                loc.field_label("page"),
                loc.field_label("confidence"),
                loc.field_label("evidence"),
                String::new(),
            ],
            for citation in citations.iter() {
                tr {
                    td {
                        if let Some(source_id) = &citation.source_id {
                            RecordLink {
                                category: Category::Sources,
                                human_id: source_id.clone(),
                                label: citation.source.clone().unwrap_or_else(|| source_id.clone()),
                            }
                        } else {
                            {citation.source.clone().unwrap_or_else(|| citation.human_id.clone())}
                        }
                    }
                    td { class: "muted", {citation.page.clone().unwrap_or_else(|| "—".to_owned())} }
                    td {
                        if let (Some(level), Some(label)) = (citation.confidence, citation.confidence_label.clone()) {
                            ConfidenceBadge { level, label }
                        } else {
                            span { class: "muted", "—" }
                        }
                    }
                    td { class: "wrap",
                        for chip in citation.evidence_axes.iter() {
                            EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                        }
                    }
                    {row_actions_cell::<PlaceEditForm>(
                        loc,
                        &citation.human_id,
                        None, None,
                        citation.assertion_id.clone().map(|id| RowRetract { assertion_id: id, button_label: "detach", title: "detach-citation", detach: true }),
                        None,
                        onretract)}
                }
            }
        }
    }
}

/// The place Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn place_tags_panel(
    loc: &Localizer,
    detail: &PlaceDetail,
    mut editing: Signal<Option<PlaceEditForm>>,
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call((PlaceEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The place History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn place_history_tab(
    loc: &Localizer,
    detail: &PlaceDetail,
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
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
                on_submit.call((PlaceEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The place editing side panel: renders the form for the open [`PlaceEditForm`], or nothing.
fn place_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<PlaceEditForm>>,
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match &form {
        PlaceEditForm::Name(None) => loc.action_label("add-name"),
        PlaceEditForm::Name(Some(_)) => loc.panel_title("edit-name"),
        PlaceEditForm::Enclosing(None) => loc.action_label("add-enclosing"),
        PlaceEditForm::Enclosing(Some(_)) => loc.panel_title("edit-enclosing"),
        PlaceEditForm::Citation => loc.action_label("attach-citation"),
        PlaceEditForm::Media => loc.action_label("attach-media"),
        PlaceEditForm::Note => loc.action_label("attach-note"),
        PlaceEditForm::Tag => loc.action_label("add-tag"),
    };
    let human_id = human_id.to_owned();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| editing.set(None),
            footer: rsx! {},
            {match form {
                PlaceEditForm::Name(seed) => rsx! { PlaceNameForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Enclosing(seed) => rsx! { PlaceEnclosingForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Citation => rsx! { PlaceLinkForm { human_id, field: "citation".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Media => rsx! { PlaceLinkForm { human_id, field: "media".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Note => rsx! { PlaceLinkForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Tag => rsx! { PlaceTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The place name form → [`PlaceEdit::AddName`]. `seed: None` adds a new name (a free-text place-name
/// string, not a record link); `Some(row)` edits an existing name — the text input is pre-filled and
/// the provenance draft's `supersedes` is seeded with the row's assertion id so Save supersedes
/// (replaces) rather than appends (ADR 0004 §2). The scalar code is edited in the record, not here.
#[component]
fn PlaceNameForm(
    human_id: String,
    seed: Option<PlaceNameVm>,
    onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut value = use_signal(|| seed.as_ref().map(|row| row.text.clone()).unwrap_or_default());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_label("save");
    rsx! {
        Input {
            label: loc.field_label("name"),
            name: "name".to_owned(),
            value: value(),
            oninput: move |event: FormEvent| value.set(event.value()),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let value = value();
                if value.trim().is_empty() {
                    return;
                }
                onsubmit.call((PlaceEdit::AddName { human_id: human_id.clone(), text: value }, prov()));
            },
        }
    }
}

/// The place enclosing-place form → [`PlaceEdit::AddEnclosing`]. `seed: None` adds a new enclosing-by
/// link over an existing-place picker; `Some(row)` edits an existing one — the enclosing place is fixed
/// (shown as a link), the correction updates its provenance, and the draft's `supersedes` is seeded with
/// the row's assertion id so Save supersedes rather than appends (ADR 0004 §2).
#[component]
fn PlaceEnclosingForm(
    human_id: String,
    seed: Option<PlaceHierarchyVm>,
    onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    // Edit mode fixes the enclosing place (only the provenance changes); add mode offers a picker.
    let fixed = seed.as_ref().map(|row| (row.human_id.clone(), row.name.clone()));
    let picker = use_existing_picker(
        services,
        Category::Places,
        loc.field_label("place"),
        "enclosing".to_owned(),
        loc.picker_entity(Category::Places),
        Vec::new(),
    );
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let picker_for_save = picker.clone();
    let fixed_for_save = fixed.as_ref().map(|(id, _)| id.clone());
    let onsave = use_callback(move |()| {
        let Some(enclosing_id) = fixed_for_save.clone().or_else(|| picker_selection_id(&picker_for_save)) else {
            return;
        };
        onsubmit.call((
            PlaceEdit::AddEnclosing {
                human_id: human_id.clone(),
                enclosing_id,
            },
            prov(),
        ));
    });
    if let Some((id, name)) = &fixed {
        rsx! {
            div { class: "field",
                label { "{loc.field_label(\"place\")}" }
                RecordLink { category: Category::Places, human_id: id.clone(), label: name.clone() }
            }
            {provenance_block(loc, prov)}
            Button {
                label: loc.action_label("save"),
                variant: ButtonVariant::Primary,
                onclick: move |_| onsave.call(()),
            }
        }
    } else {
        attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
    }
}

/// A place collection link form over an existing-only picker (an attached citation/media/note) → the
/// matching [`PlaceEdit`] attach variant.
#[component]
fn PlaceLinkForm(human_id: String, field: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let (label, category) = match field.as_str() {
        "citation" => (loc.field_label("citation"), Category::Citations),
        "note" => (loc.field_label("note"), Category::Notes),
        _ => (loc.field_label("media"), Category::Media),
    };
    let picker = use_existing_picker(
        services,
        category,
        label,
        field.clone(),
        loc.picker_entity(category),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let picker_for_save = picker.clone();
    let onsave = use_callback(move |()| {
        let Some(id) = picker_selection_id(&picker_for_save) else {
            return;
        };
        let edit = match field.as_str() {
            "citation" => PlaceEdit::AttachCitation {
                human_id: human_id.clone(),
                citation_id: id,
            },
            "note" => PlaceEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
            _ => PlaceEdit::AttachMedia {
                human_id: human_id.clone(),
                media_id: id,
            },
        };
        onsubmit.call((edit, prov()));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}

/// The place "Add tag" form: a picker of existing tags by name → [`PlaceEdit::Tag`].
#[component]
fn PlaceTagForm(human_id: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((PlaceEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

/// The place types offered by the type picker.
fn place_type_choices() -> [PlaceType; 9] {
    [
        PlaceType::Country,
        PlaceType::County,
        PlaceType::Municipality,
        PlaceType::Parish,
        PlaceType::City,
        PlaceType::Town,
        PlaceType::Village,
        PlaceType::Farm,
        PlaceType::Building,
    ]
}
