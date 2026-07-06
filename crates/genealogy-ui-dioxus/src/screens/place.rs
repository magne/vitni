use std::str::FromStr;

use genealogy_app::{GeoCoordinates, Microdegrees, PlaceType};

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
    let draft = use_signal(genealogy_ui::PlaceDraft::new);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty() && draft().is_valid();
    let on_save = use_callback(move |()| {
        let Some(request) = draft().to_request() else {
            return;
        };
        let label = request.name.clone().unwrap_or_default();
        let services = services.clone();
        let prov = prov();
        spawn(async move {
            match commit_place_change_set(services, request, prov).await {
                Ok(id) => oncreated.call((id, label)),
                Err(message) => onerror.call(message),
            }
        });
    });
    rsx! {
        {create_record_header(&loc.place_new_title(), &loc.record_draft_badge())}
        {place_create_fields(loc, draft)}
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

/// The place create form's field rows (`place.html` edit specimen): a required Type select,
/// Latitude/Longitude (raw decimal degrees, rejected — not zero-filled — when invalid), and a Code.
/// A pure fn (no `AppCtx`) so SSR tests can render it directly.
pub fn place_create_fields(loc: &Localizer, mut draft: Signal<genealogy_ui::PlaceDraft>) -> Element {
    let types = place_type_choices();
    let options: Vec<SelectChoice> = types
        .iter()
        .enumerate()
        .map(|(index, place_type)| SelectChoice {
            value: index.to_string(),
            label: loc.place_type_label(place_type),
        })
        .collect();
    let selected = types
        .iter()
        .position(|t| *t == draft().place_type)
        .unwrap_or(0)
        .to_string();
    let latitude_invalid = draft().latitude_invalid();
    let longitude_invalid = draft().longitude_invalid();
    let coordinate_error = loc.place_coordinate_invalid();
    rsx! {
        Card { title: loc.section_label("vitals"),
            div { class: "stack",
                Select {
                    label: loc.field_label("type"),
                    name: "place-type".to_owned(),
                    value: Some(selected),
                    options,
                    onchange: move |event: FormEvent| {
                        let types = place_type_choices();
                        if let Some(place_type) = event.value().parse::<usize>().ok().and_then(|index| types.get(index).cloned()) {
                            draft.write().place_type = place_type;
                        }
                    },
                }
                Input {
                    label: loc.field_label("name"),
                    name: "place-name".to_owned(),
                    value: draft().name.clone(),
                    oninput: move |event: FormEvent| draft.write().name = event.value(),
                }
                div { class: "field",
                    label { r#for: "place-latitude", "{loc.field_label(\"latitude\")}" }
                    input {
                        class: if latitude_invalid { "in invalid" } else { "in" },
                        r#type: "text",
                        id: "place-latitude",
                        name: "place-latitude",
                        value: "{draft().latitude}",
                        aria_invalid: if latitude_invalid { "true" } else { "false" },
                        oninput: move |event| draft.write().latitude = event.value(),
                    }
                    if latitude_invalid {
                        div { class: "field-error", "{coordinate_error}" }
                    }
                }
                div { class: "field",
                    label { r#for: "place-longitude", "{loc.field_label(\"longitude\")}" }
                    input {
                        class: if longitude_invalid { "in invalid" } else { "in" },
                        r#type: "text",
                        id: "place-longitude",
                        name: "place-longitude",
                        value: "{draft().longitude}",
                        aria_invalid: if longitude_invalid { "true" } else { "false" },
                        oninput: move |event| draft.write().longitude = event.value(),
                    }
                    if longitude_invalid {
                        div { class: "field-error", "{coordinate_error}" }
                    }
                }
                Input {
                    label: loc.field_label("code"),
                    name: "place-code".to_owned(),
                    value: draft().code.clone(),
                    oninput: move |event: FormEvent| draft.write().code = event.value(),
                }
            }
        }
    }
}

/// Which place edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceEditForm {
    /// Set the place's type.
    Type,
    /// Set the place's coordinates.
    Coordinates,
    /// Set the place's jurisdiction code.
    Code,
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
    let mut nav = use_context::<NavState>();
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

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the place's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        nav.set_record_label(
            Category::Places,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (PlaceEdit, ProvenanceDraft)| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_place_edit(services, edit, prov).await {
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
        Some(ScreenData::Loaded(IntentOutcome::PlaceDetail(detail))) => {
            place_detail(&state, detail, active, editing, on_submit, &human_id)
        }
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

/// Renders a loaded place's detail container: header, the tab strip, the active tab, and the panel.
fn place_detail(
    state: &AppState,
    detail: &PlaceDetail,
    active: Signal<usize>,
    editing: Signal<Option<PlaceEditForm>>,
    on_submit: Callback<(PlaceEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
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
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            id_label: detail.human_id.clone(),
            avatar: "📍".to_owned(),
            extras: place_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {place_tab_content(state, detail, active_id, editing, on_submit, human_id)}
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
        _ => place_overview(loc, detail, editing),
    }
}

/// The Overview tab: the name-history note, the Place card (type/coords/code), and an "Enclosed by" card.
pub fn place_overview(loc: &Localizer, detail: &PlaceDetail, mut editing: Signal<Option<PlaceEditForm>>) -> Element {
    rsx! {
        div { class: "section-note", "{loc.place_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.field_label("place"),
                div { class: "tab-actions",
                    Button { label: loc.field_label("type"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(PlaceEditForm::Type)) }
                    Button { label: loc.field_label("coordinates"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(PlaceEditForm::Coordinates)) }
                    Button { label: loc.field_label("code"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(PlaceEditForm::Code)) }
                }
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"attribute-type\")}" }
                        if let Some(type_label) = detail.type_label.clone() {
                            span { class: "grow", Chip { label: type_label } }
                        } else {
                            span { class: "grow muted", "—" }
                        }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"date\")}" }
                        span { class: "grow mono", {detail.coordinates.clone().unwrap_or_else(|| "—".to_owned())} }
                        if let (Some(level), Some(label)) = (detail.coordinates_confidence, detail.coordinates_confidence_label.clone()) {
                            ConfidenceBadge { level, label }
                        }
                        {provenance_cue(loc, loc.provenance_title_claim(&loc.field_label("coordinates")), &detail.coordinate_citations)}
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"value\")}" }
                        span { class: "grow mono", {detail.code.clone().unwrap_or_else(|| "—".to_owned())} }
                    }
                }
            }
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
        PlaceEditForm::Type => loc.field_label("type"),
        PlaceEditForm::Coordinates => loc.field_label("coordinates"),
        PlaceEditForm::Code => loc.field_label("code"),
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
                PlaceEditForm::Type => rsx! { PlaceTypeForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Coordinates => rsx! { PlaceCoordinatesForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Code => rsx! { PlaceTextForm { human_id, field: "code".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Name => rsx! { PlaceTextForm { human_id, field: "name".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Enclosing => rsx! { PlaceTextForm { human_id, field: "enclosing".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Citation => rsx! { PlaceTextForm { human_id, field: "citation".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Media => rsx! { PlaceTextForm { human_id, field: "media".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Note => rsx! { PlaceTextForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                PlaceEditForm::Tag => rsx! { PlaceTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// A single-text-field place form (name text, or an enclosing/citation/media/note `human_id`) → the
/// matching [`PlaceEdit`] variant.
#[component]
fn PlaceTextForm(human_id: String, field: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut value = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    let label = match field.as_str() {
        "name" => loc.field_label("name"),
        "code" => loc.field_label("code"),
        "enclosing" => loc.field_label("place"),
        "citation" => loc.field_label("citation"),
        "note" => loc.field_label("note"),
        _ => loc.field_label("media"),
    };
    rsx! {
        Input { label, name: field.clone(), oninput: move |event: FormEvent| value.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let value = value();
                if value.trim().is_empty() {
                    return;
                }
                let edit = match field.as_str() {
                    "name" => PlaceEdit::AddName { human_id: human_id.clone(), text: value },
                    "code" => PlaceEdit::SetCode { human_id: human_id.clone(), code: value },
                    "enclosing" => PlaceEdit::AddEnclosing { human_id: human_id.clone(), enclosing_id: value },
                    "citation" => PlaceEdit::AttachCitation { human_id: human_id.clone(), citation_id: value },
                    "note" => PlaceEdit::AttachNote { human_id: human_id.clone(), note_id: value },
                    _ => PlaceEdit::AttachMedia { human_id: human_id.clone(), media_id: value },
                };
                onsubmit.call((edit, prov()));
            },
        }
    }
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

/// The "Set type" form: a place-type picker → [`PlaceEdit::SetType`].
#[component]
fn PlaceTypeForm(human_id: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let options: Vec<SelectChoice> = place_type_choices()
        .iter()
        .enumerate()
        .map(|(position, place_type)| SelectChoice {
            value: position.to_string(),
            label: loc.place_type_label(place_type),
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
                let place_type = place_type_choices().get(chosen()).cloned().unwrap_or(PlaceType::City);
                onsubmit.call((PlaceEdit::SetType { human_id: human_id.clone(), place_type }, prov()));
            },
        }
    }
}

/// The "Set coordinates" form: latitude + longitude in decimal degrees → [`PlaceEdit::SetCoordinates`].
#[component]
fn PlaceCoordinatesForm(human_id: String, onsubmit: EventHandler<(PlaceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut latitude = use_signal(String::new);
    let mut longitude = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("latitude"), name: "latitude".to_owned(), oninput: move |event: FormEvent| latitude.set(event.value()) }
        Input { label: loc.field_label("longitude"), name: "longitude".to_owned(), oninput: move |event: FormEvent| longitude.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let (Ok(latitude), Ok(longitude)) =
                    (Microdegrees::from_str(latitude().trim()), Microdegrees::from_str(longitude().trim()))
                else {
                    return;
                };
                let coordinates = GeoCoordinates { latitude, longitude };
                onsubmit.call((PlaceEdit::SetCoordinates { human_id: human_id.clone(), coordinates }, prov()));
            },
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
