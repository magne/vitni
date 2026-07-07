use genealogy_app::PlaceType;

use super::prelude::*;
use crate::screens::RecordDetail;

/// The place master-detail: a searchable list on the left, the selected place's detail on the right.
#[component]
pub fn PlaceScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Places.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().place_list_empty();
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
        if *nav.pending_create.read() == Some(Category::Places) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowPlaceList).await }
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
                        category: Category::Places,
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
    let on_created = use_callback(move |(id, label): (String, String)| {
        creating.set(false);
        nav.open_record(RecordRef {
            category: Category::Places,
            human_id: id.clone(),
            label: if label.is_empty() { id } else { label },
        });
    });
    let detail = if creating() {
        rsx! {
            PlaceCreateRecord {
                oncreated: move |created| on_created.call(created),
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

/// The create-mode place record: an uncommitted [`PlaceDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Save commits the whole place; Cancel discards. Save is
/// blocked while the coordinate pair is half-filled or unparseable (§7).
#[component]
fn PlaceCreateRecord(
    oncreated: EventHandler<(String, String)>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
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
                Ok(id) => oncreated.call((id, label)),
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
    rsx! {
        {create_record_header(&loc.place_new_title(), &loc.record_draft_badge(), actions)}
        {place_record_fields(loc, record)}
        Input {
            label: loc.field_label("name"),
            name: "place-name".to_owned(),
            value: draft().name.clone(),
            oninput: move |event: FormEvent| draft.write().name = event.value(),
        }
        {record_edit_provenance(loc, record)}
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceEditForm {
    /// Add a name by text.
    Name,
    /// Add an enclosing place by `human_id`.
    Enclosing,
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
            },
            PlaceCallbacks {
                on_submit,
                on_record_save,
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
}

/// The two commit callbacks a place's detail wires in: one-command collection edits and the
/// whole-record save (the scalar edit via `edits_against`).
#[derive(Clone, Copy)]
struct PlaceCallbacks {
    /// Commits one [`PlaceEdit`] command (a collection row).
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::PlaceDraft, ProvenanceDraft)>,
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
    } = pane;
    let on_submit = callbacks.on_submit;
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
            {place_tab_content(state, detail, active_id, editing, record, on_submit, human_id)}
        }
        {place_edit_panel(state, editing, on_submit, human_id)}
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

/// The content of one place detail tab, with its contextual add affordances.
fn place_tab_content(
    state: &AppState,
    detail: &PlaceDetail,
    tab_id: &str,
    mut editing: Signal<Option<PlaceEditForm>>,
    record: RecordEditState<genealogy_ui::PlaceDraft>,
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "names" => rsx! {
            div { class: "section-note", "{loc.place_names_note()}" }
            div { class: "tab-actions",
                Button { label: loc.action_label("add-name"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Name)) }
            }
            {place_names_table(loc, detail)}
        },
        "hierarchy" => rsx! {
            div { class: "section-note", "{loc.place_hierarchy_note()}" }
            div { class: "tab-actions",
                Button { label: loc.action_label("add-enclosing"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Enclosing)) }
            }
            {place_hierarchy_table(loc, detail)}
        },
        "citations" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-citation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Citation)) }
            }
            {citation_table(loc, &detail.citations)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Media)) }
            }
            {family_media_gallery(loc, &detail.media)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(PlaceEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
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

/// The Names tab: a row per asserted name with language, date, surety, and source columns.
pub fn place_names_table(loc: &Localizer, detail: &PlaceDetail) -> Element {
    if detail.names.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("name"),
                loc.field_label("language"),
                loc.field_label("date"),
                loc.field_label("surety"),
                loc.field_label("source"),
            ],
            for name in detail.names.iter() {
                tr {
                    td { b { "{name.text}" } }
                    td { class: "muted", {name.language.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { {name.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { ConfidenceBadge { level: name.confidence, label: name.confidence_label.clone() } }
                    td { {source_cue(loc, name.source_count)} }
                }
            }
        }
    }
}

/// The Hierarchy tab: a breadcrumb of the jurisdiction chain plus a level-by-level table.
pub fn place_hierarchy_table(loc: &Localizer, detail: &PlaceDetail) -> Element {
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
                loc.field_label("surety"),
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
    let title = match form {
        PlaceEditForm::Name => loc.action_label("add-name"),
        PlaceEditForm::Enclosing => loc.action_label("add-enclosing"),
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
                PlaceEditForm::Name => rsx! { PlaceNameForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Enclosing => rsx! { PlaceLinkForm { human_id, field: "enclosing".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Citation => rsx! { PlaceLinkForm { human_id, field: "citation".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Media => rsx! { PlaceLinkForm { human_id, field: "media".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Note => rsx! { PlaceLinkForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Tag => rsx! { PlaceTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The place "Add name" form: a free-text place-name string (not a record link) → [`PlaceEdit::AddName`].
/// The scalar code is edited in the record, not here.
#[component]
fn PlaceNameForm(human_id: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut value = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("name"), name: "name".to_owned(), oninput: move |event: FormEvent| value.set(event.value()) }
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

/// A place collection link form over an existing-only picker (an enclosing place, or an attached
/// citation/media/note) → the matching [`PlaceEdit`] variant.
#[component]
fn PlaceLinkForm(human_id: String, field: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let (label, category) = match field.as_str() {
        "enclosing" => (loc.field_label("place"), Category::Places),
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
            "enclosing" => PlaceEdit::AddEnclosing {
                human_id: human_id.clone(),
                enclosing_id: id,
            },
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
