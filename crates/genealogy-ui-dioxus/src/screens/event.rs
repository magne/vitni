use super::prelude::*;
use crate::screens::RecordDetail;
use genealogy_app::EventType;

/// The event master-detail: a searchable list on the left, the selected event's detail on the right.
#[component]
pub fn EventScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Events.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().event_list_empty();
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
        if *nav.pending_create.read() == Some(Category::Events) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowEventList).await }
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
                        category: Category::Events,
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
            category: Category::Events,
            human_id: id.clone(),
            label: if label.is_empty() { id } else { label },
        });
    });
    let detail = if creating() {
        rsx! {
            EventCreateRecord {
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

/// The create-mode event record: an uncommitted [`EventDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). The type is required; a "new place" selection creates a
/// place inline on Save (§6b cascade). Save commits the whole event; Cancel discards.
#[component]
fn EventCreateRecord(
    oncreated: EventHandler<(String, String)>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let draft = use_signal(genealogy_ui::EventDraft::new);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty();
    let on_save = use_callback(move |()| {
        let request = draft().to_request();
        let services = services.clone();
        let prov = prov();
        spawn(async move {
            match commit_event_change_set(services, request, prov).await {
                Ok(id) => oncreated.call((id, String::new())),
                Err(message) => onerror.call(message),
            }
        });
    });
    rsx! {
        {create_record_header(&loc.event_new_title(), &loc.record_draft_badge())}
        {event_create_fields(loc, draft)}
        {provenance_block(loc, prov)}
        RecordActions {
            save_label: loc.action_label("save"),
            cancel_label: loc.action_label("cancel"),
            can_save,
            onsave: move |()| on_save.call(()),
            oncancel: move |()| oncancel.call(()),
        }
    }
}

/// The place types offered for an inline "new place" (a common subset; the model has more).
fn event_place_type_choices() -> [genealogy_app::PlaceType; 5] {
    use genealogy_app::PlaceType;
    [
        PlaceType::City,
        PlaceType::Town,
        PlaceType::Parish,
        PlaceType::Building,
        PlaceType::Country,
    ]
}

/// The event create form's field rows (`event.html` edit specimen, date deferred to PR29): a required
/// Type select, a Place (none / existing / inline new — §6b), and a Description. A pure fn (no
/// `AppCtx`) so SSR tests can render it directly.
pub fn event_create_fields(loc: &Localizer, mut draft: Signal<genealogy_ui::EventDraft>) -> Element {
    use genealogy_ui::EventPlaceKind;
    let event_types = event_type_choices();
    let type_options: Vec<SelectChoice> = event_types
        .iter()
        .enumerate()
        .map(|(index, event_type)| SelectChoice {
            value: index.to_string(),
            label: loc.event_type_label(event_type),
        })
        .collect();
    let type_selected = event_types
        .iter()
        .position(|t| *t == draft().event_type)
        .unwrap_or(0)
        .to_string();
    let place_kinds = [EventPlaceKind::None, EventPlaceKind::Existing, EventPlaceKind::New];
    let place_labels = [
        loc.event_place_none(),
        loc.event_place_existing(),
        loc.event_place_new(),
    ];
    let place_options: Vec<SelectChoice> = place_labels
        .iter()
        .enumerate()
        .map(|(index, label)| SelectChoice {
            value: index.to_string(),
            label: label.clone(),
        })
        .collect();
    let place_selected = place_kinds
        .iter()
        .position(|k| *k == draft().place_kind)
        .unwrap_or(0)
        .to_string();
    let place_types = event_place_type_choices();
    let (new_place_options, new_place_selected) = optional_enum_select(
        loc.record_unset(),
        &place_types,
        Some(&draft().new_place_type),
        |place_type| loc.place_type_label(place_type),
    );
    let kind = draft().place_kind;
    rsx! {
        Card { title: loc.tab_label("overview"),
            div { class: "stack",
                Select {
                    label: loc.field_label("type"),
                    name: "event-type".to_owned(),
                    value: Some(type_selected),
                    options: type_options,
                    onchange: move |event: FormEvent| {
                        let types = event_type_choices();
                        if let Some(event_type) = event.value().parse::<usize>().ok().and_then(|index| types.get(index).cloned()) {
                            draft.write().event_type = event_type;
                        }
                    },
                }
                Select {
                    label: loc.field_label("place"),
                    name: "event-place-kind".to_owned(),
                    value: Some(place_selected),
                    options: place_options,
                    onchange: move |event: FormEvent| {
                        let kinds = [EventPlaceKind::None, EventPlaceKind::Existing, EventPlaceKind::New];
                        if let Some(kind) = event.value().parse::<usize>().ok().and_then(|index| kinds.get(index).copied()) {
                            draft.write().place_kind = kind;
                        }
                    },
                }
                {event_place_subfields(loc, draft, kind, new_place_options, new_place_selected)}
                Input {
                    label: loc.field_label("description"),
                    name: "event-description".to_owned(),
                    value: draft().description.clone(),
                    oninput: move |event: FormEvent| draft.write().description = event.value(),
                }
            }
        }
    }
}

/// The conditional place sub-fields for the event create form: an id input when linking an existing
/// place, or a type select + name input when creating one inline (§6b). Nothing when no place.
fn event_place_subfields(
    loc: &Localizer,
    mut draft: Signal<genealogy_ui::EventDraft>,
    kind: genealogy_ui::EventPlaceKind,
    new_place_options: Vec<SelectChoice>,
    new_place_selected: String,
) -> Element {
    use genealogy_ui::EventPlaceKind;
    rsx! {
        if kind == EventPlaceKind::Existing {
            Input {
                label: loc.field_label("place"),
                name: "event-existing-place".to_owned(),
                value: draft().existing_place.clone(),
                oninput: move |event: FormEvent| draft.write().existing_place = event.value(),
            }
        }
        if kind == EventPlaceKind::New {
            Select {
                label: loc.field_label("type"),
                name: "event-new-place-type".to_owned(),
                value: Some(new_place_selected),
                options: new_place_options,
                onchange: move |event: FormEvent| {
                    let place_types = event_place_type_choices();
                    if let Some(place_type) = event.value().parse::<usize>().ok().and_then(|index| place_types.get(index).cloned()) {
                        draft.write().new_place_type = place_type;
                    }
                },
            }
            Input {
                label: loc.field_label("name"),
                name: "event-new-place-name".to_owned(),
                value: draft().new_place_name.clone(),
                oninput: move |event: FormEvent| draft.write().new_place_name = event.value(),
            }
        }
    }
}

/// Which event edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventEditForm {
    /// Set the event's type.
    Type,
    /// Set the event's date.
    Date,
    /// Set the event's description.
    Description,
    /// Add a participant (person + role).
    Participant,
    /// Attach a citation by `human_id`.
    Citation,
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected event: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn EventDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let mut nav = use_context::<NavState>();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<EventEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowEvent { human_id }).await }
    });

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the event's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::EventDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        nav.set_record_label(
            Category::Events,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (EventEdit, ProvenanceDraft)| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_event_edit(services, edit, prov).await {
                Ok(()) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::EventDetail(detail))) => {
            event_detail(&state, detail, active, editing, on_submit, &human_id)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
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

/// Renders a loaded event's detail container: header (title, type/restriction chips), the tab strip,
/// the active tab's content, and the editing side panel.
fn event_detail(
    state: &AppState,
    detail: &EventDetail,
    active: Signal<usize>,
    editing: Signal<Option<EventEditForm>>,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = event_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_id = tabs.get(active()).map_or("overview", |tab| tab.id);
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            id_label: detail.human_id.clone(),
            avatar: "📅".to_owned(),
            extras: event_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {event_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {event_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for an event (the mockup `resn-set`).
fn event_restriction_toggles(
    loc: &Localizer,
    detail: &EventDetail,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
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
                on_submit.call((EventEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one event detail tab, with its contextual add affordances.
fn event_tab_content(
    state: &AppState,
    detail: &EventDetail,
    tab_id: &str,
    mut editing: Signal<Option<EventEditForm>>,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "participants" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-participant"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Participant)) }
            }
            {event_participants_table(loc, detail)}
        },
        "citations" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-citation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Citation)) }
            }
            {citation_table(loc, &detail.citations)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Media)) }
            }
            {family_media_gallery(loc, &detail.media)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => event_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => event_history_tab(loc, detail, on_submit, human_id),
        _ => event_overview(loc, detail, editing),
    }
}

/// The Overview tab: the structured-date note, the Event card (type/date/place), and a Description card.
pub fn event_overview(loc: &Localizer, detail: &EventDetail, mut editing: Signal<Option<EventEditForm>>) -> Element {
    rsx! {
        div { class: "section-note", "{loc.event_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.tab_label("events"),
                div { class: "tab-actions",
                    Button { label: loc.field_label("type"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(EventEditForm::Type)) }
                    Button { label: loc.field_label("date"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(EventEditForm::Date)) }
                    Button { label: loc.field_label("description"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(EventEditForm::Description)) }
                }
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"attribute-type\")}" }
                        span { class: "grow", Chip { label: detail.type_label.clone() } }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"date\")}" }
                        span { class: "grow", {detail.date.clone().unwrap_or_else(|| "—".to_owned())} }
                        if let (Some(level), Some(label)) = (detail.date_confidence, detail.date_confidence_label.clone()) {
                            ConfidenceBadge { level, label }
                        }
                        {provenance_cue(loc, loc.provenance_title_claim(&loc.field_label("date")), &detail.date_citations)}
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"place\")}" }
                        if let Some(place) = detail.place.as_ref() {
                            span { class: "grow", "{place.name}" }
                            if let (Some(level), Some(label)) = (detail.place_confidence, detail.place_confidence_label.clone()) {
                                ConfidenceBadge { level, label }
                            }
                        } else {
                            span { class: "grow muted", "—" }
                        }
                    }
                }
            }
            Card { title: loc.field_label("value"),
                if let Some(description) = detail.description.clone() {
                    p { "{description}" }
                } else {
                    EmptyState { message: loc.tab_empty() }
                }
            }
        }
    }
}

/// The Participants tab: a row per participant with role, surety, and source columns.
pub fn event_participants_table(loc: &Localizer, detail: &EventDetail) -> Element {
    if detail.participants.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("name"),
                loc.field_label("role"),
                loc.field_label("surety"),
                loc.field_label("source"),
            ],
            for participant in detail.participants.iter() {
                tr {
                    td { "{participant.name}" }
                    td { Chip { label: participant.role_label.clone() } }
                    td { ConfidenceBadge { level: participant.confidence, label: participant.confidence_label.clone() } }
                    td { {source_cue(loc, participant.source_count)} }
                }
            }
        }
    }
}

/// The event Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn event_tags_panel(
    loc: &Localizer,
    detail: &EventDetail,
    mut editing: Signal<Option<EventEditForm>>,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EventEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call((EventEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The event History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn event_history_tab(
    loc: &Localizer,
    detail: &EventDetail,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
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
                on_submit.call((EventEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The event editing side panel: renders the form for the open [`EventEditForm`], or nothing.
fn event_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<EventEditForm>>,
    on_submit: Callback<(EventEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        EventEditForm::Type => loc.field_label("type"),
        EventEditForm::Date => loc.field_label("date"),
        EventEditForm::Description => loc.field_label("description"),
        EventEditForm::Participant => loc.action_label("add-participant"),
        EventEditForm::Citation => loc.action_label("attach-citation"),
        EventEditForm::Media => loc.action_label("attach-media"),
        EventEditForm::Note => loc.action_label("attach-note"),
        EventEditForm::Tag => loc.action_label("add-tag"),
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
                EventEditForm::Type => rsx! { EventTypeForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Date => rsx! { EventDateForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Description => rsx! { EventDescriptionForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Participant => rsx! { EventAddParticipantForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Citation => rsx! { EventAttachForm { human_id, field: "citation".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Media => rsx! { EventAttachForm { human_id, field: "media".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Note => rsx! { EventAttachForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                EventEditForm::Tag => rsx! { EventTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Add participant" form: a person `human_id` + a role select → [`EventEdit::AddParticipant`].
#[component]
fn EventAddParticipantForm(human_id: String, onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let roles = participant_role_choices();
    let options: Vec<SelectChoice> = roles
        .iter()
        .enumerate()
        .map(|(position, role)| SelectChoice {
            value: position.to_string(),
            label: loc.participant_role_label(role),
        })
        .collect();
    let mut person = use_signal(String::new);
    let mut role = use_signal(|| 0_usize);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("name"), name: "participant".to_owned(), oninput: move |event: FormEvent| person.set(event.value()) }
        Select {
            label: loc.field_label("role"),
            name: "role".to_owned(),
            value: Some(0.to_string()),
            options,
            onchange: move |event: FormEvent| role.set(event.value().parse::<usize>().unwrap_or(0)),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let person_id = person();
                if person_id.trim().is_empty() {
                    return;
                }
                let role = participant_role_choices().get(role()).cloned().unwrap_or(ParticipantRole::Primary);
                onsubmit.call((EventEdit::AddParticipant { human_id: human_id.clone(), person_id, role }, prov()));
            },
        }
    }
}

/// The "Attach citation/media/note by id" form → the matching [`EventEdit`] attach variant.
#[component]
fn EventAttachForm(human_id: String, field: String, onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut id = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    let field_label = loc.field_label(&field);
    rsx! {
        Input { label: field_label, name: field.clone(), oninput: move |event: FormEvent| id.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let id = id();
                if id.trim().is_empty() {
                    return;
                }
                let edit = match field.as_str() {
                    "citation" => EventEdit::AttachCitation { human_id: human_id.clone(), citation_id: id },
                    "note" => EventEdit::AttachNote { human_id: human_id.clone(), note_id: id },
                    _ => EventEdit::AttachMedia { human_id: human_id.clone(), media_id: id },
                };
                onsubmit.call((edit, prov()));
            },
        }
    }
}

/// The event "Add tag" form: a picker of existing tags by name → [`EventEdit::Tag`].
#[component]
fn EventTagForm(human_id: String, onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((EventEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

/// The participant roles offered by the "Add participant" form (a common subset; the model has more).
fn participant_role_choices() -> [ParticipantRole; 6] {
    [
        ParticipantRole::Primary,
        ParticipantRole::Witness,
        ParticipantRole::Officiator,
        ParticipantRole::Spouse,
        ParticipantRole::Godparent,
        ParticipantRole::Multiple,
    ]
}

/// The "Set type" form: an event-type picker → [`EventEdit::SetType`].
#[component]
fn EventTypeForm(human_id: String, onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let options: Vec<SelectChoice> = event_type_choices()
        .iter()
        .enumerate()
        .map(|(position, event_type)| SelectChoice {
            value: position.to_string(),
            label: loc.event_type_label(event_type),
        })
        .collect();
    let mut chosen = use_signal(|| 0_usize);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.field_label("type"),
            name: "type".to_owned(),
            value: Some(0.to_string()),
            options,
            onchange: move |event: FormEvent| chosen.set(event.value().parse::<usize>().unwrap_or(0)),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let event_type = event_type_choices().get(chosen()).cloned().unwrap_or(EventType::Birth);
                onsubmit.call((EventEdit::SetType { human_id: human_id.clone(), event_type }, prov()));
            },
        }
    }
}

/// The "Set date" form: year (required) + optional month/day → [`EventEdit::SetDate`].
#[component]
fn EventDateForm(human_id: String, onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut year = use_signal(String::new);
    let mut month = use_signal(String::new);
    let mut day = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("year"), name: "year".to_owned(), oninput: move |event: FormEvent| year.set(event.value()) }
        Input { label: loc.field_label("month"), name: "month".to_owned(), oninput: move |event: FormEvent| month.set(event.value()) }
        Input { label: loc.field_label("day"), name: "day".to_owned(), oninput: move |event: FormEvent| day.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let Ok(year) = year().trim().parse::<i32>() else {
                    return;
                };
                let date = DateParts {
                    year,
                    month: month().trim().parse::<u8>().ok(),
                    day: day().trim().parse::<u8>().ok(),
                };
                onsubmit.call((EventEdit::SetDate { human_id: human_id.clone(), date }, prov()));
            },
        }
    }
}

/// The "Set description" form: a single text field → [`EventEdit::SetDescription`].
#[component]
fn EventDescriptionForm(human_id: String, onsubmit: EventHandler<(EventEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut description = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("description"), name: "description".to_owned(), oninput: move |event: FormEvent| description.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| onsubmit.call((EventEdit::SetDescription { human_id: human_id.clone(), description: description() }, prov())),
        }
    }
}

/// The event types offered by the type picker (a common subset; the model has more).
fn event_type_choices() -> [EventType; 8] {
    [
        EventType::Birth,
        EventType::Death,
        EventType::Marriage,
        EventType::Baptism,
        EventType::Burial,
        EventType::Census,
        EventType::Residence,
        EventType::Immigration,
    ]
}
