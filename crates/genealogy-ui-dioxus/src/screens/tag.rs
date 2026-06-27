use super::prelude::*;

/// The tag master-detail screen: a list of tags (colour dot + name) on the left, the selected tag's
/// detail (overview + usage + history) on the right. Tags carry no `human_id`; the row id is the
/// stable tag id (never rendered — data-model §9). `New` creates a tag with a default name, opened
/// for renaming.
#[component]
pub fn TagScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Tags.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().tag_list_empty();
    let prompt = chrome.tag_select_prompt();
    let default_name = chrome.new_tag_name();
    let dismiss_label = state.data_loc().action_label("dismiss");
    let list_chrome = ListChrome {
        list_label: entity.clone(),
        filter_placeholder: chrome.list_filter(&entity),
        sort_label: chrome.list_sort(),
        sort_options: chrome.sort_options(),
        empty,
        new_label: chrome.list_new(),
    };
    let mut nav = use_context::<NavState>();
    let mut selected = use_signal(|| None::<String>);
    let mut toast = use_signal(|| None::<String>);
    use_effect(move || selected.set(nav.active_record_ref().map(|record| record.human_id)));
    use_effect(move || {
        if *nav.new_request.read() > 0 {
            let services = create_services.clone();
            let name = default_name.clone();
            spawn(async move {
                match create_tag_record(services, name).await {
                    Ok(id) => nav.open_record(RecordRef {
                        category: Category::Tags,
                        label: id.clone(),
                        human_id: id,
                    }),
                    Err(message) => toast.set(Some(message)),
                }
            });
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowTagList).await }
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
                onselect: move |row: RowVm| nav.open_record(RecordRef {
                    category: Category::Tags,
                    human_id: row.id,
                    label: row.title,
                }),
                onnew: move |()| nav.request_new(),
            }
        },
        Some(ScreenData::Loaded(
            IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::SourceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_),
        )) => rsx! {},
    };
    let detail_pane = match nav.active_record_ref() {
        Some(record) if record.category == Category::Tags => {
            let id = record.human_id;
            rsx! { TagDetailPane { key: "{id}", id } }
        }
        _ => rsx! { p { class: "empty", "{prompt}" } },
    };
    rsx! {
        MasterDetail { list: list_pane, detail: detail_pane }
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// Which tag edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagEditForm {
    /// Rename the tag.
    Name,
    /// Set the tag's priority.
    Priority,
    /// Set the tag's colour.
    Color,
}

/// The detail pane for the selected tag: header, overview/usage/history tabs, editing side panel.
#[component]
fn TagDetailPane(id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<TagEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowTag { id }).await }
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |edit: TagEdit| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_tag_edit(services, edit).await {
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
        Some(ScreenData::Loaded(IntentOutcome::TagDetail(detail))) => {
            tag_detail(&state, detail, active, editing, on_submit)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::SourceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Dashboard(_),
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

/// Renders a loaded tag's detail container: header (colour dot + name + priority), the tabs, the panel.
fn tag_detail(
    state: &AppState,
    detail: &TagDetail,
    active: Signal<usize>,
    editing: Signal<Option<TagEditForm>>,
    on_submit: Callback<TagEdit>,
) -> Element {
    let loc = state.data_loc();
    let tabs = tag_tabs(detail, loc);
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
            id_label: detail.id.clone(),
            avatar: "🏷".to_owned(),
            extras: rsx! {},
            actions: rsx! {},
            tabs: tab_items,
            active,
            {tag_tab_content(state, detail, active_id, editing, on_submit)}
        }
        {tag_edit_panel(state, detail, editing, on_submit)}
    }
}

/// The content of one tag detail tab, with its contextual edit affordances.
fn tag_tab_content(
    state: &AppState,
    detail: &TagDetail,
    tab_id: &str,
    editing: Signal<Option<TagEditForm>>,
    on_submit: Callback<TagEdit>,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "usage" => tag_usage_tab(loc, detail),
        "history" => tag_history_tab(loc, detail, on_submit),
        _ => tag_overview(loc, detail, editing),
    }
}

/// The tag Overview: the editable name/priority card and the colour card (with a live preview chip).
pub fn tag_overview(loc: &Localizer, detail: &TagDetail, mut editing: Signal<Option<TagEditForm>>) -> Element {
    let priority = detail.priority.map_or_else(|| "—".to_owned(), |p| p.to_string());
    let color = detail.color.clone().unwrap_or_else(|| "—".to_owned());
    rsx! {
        div { class: "section-note", "{loc.tag_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("tag"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"name\")}" }
                        span { class: "grow", {detail.name.clone().unwrap_or_else(|| loc.display_name(None))} }
                        Button { label: loc.action_label("edit"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(TagEditForm::Name)) }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"priority\")}" }
                        span { class: "grow", "{priority}" }
                        Button { label: loc.action_label("edit"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(TagEditForm::Priority)) }
                    }
                }
            }
            Card { title: loc.section_label("color"),
                div { class: "fact-row",
                    if let Some(color) = detail.color.clone() {
                        span { class: "dot", style: "width:36px;height:36px;border-radius:var(--r-md);background:{color};flex:none" }
                    }
                    span { class: "grow mono", "{color}" }
                    Button { label: loc.action_label("edit"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(TagEditForm::Color)) }
                }
                div { class: "wrap", style: "margin-top:8px",
                    Chip { label: detail.name.clone().unwrap_or_else(|| loc.display_name(None)), dot_color: detail.color.clone() }
                }
            }
        }
    }
}

/// The tag Usage tab: a row per object type with its count and a few example records.
pub fn tag_usage_tab(loc: &Localizer, detail: &TagDetail) -> Element {
    if detail.usage.is_empty() {
        return rsx! {
            div { class: "section-note", "{loc.tag_usage_note()}" }
            EmptyState { message: loc.tab_empty() }
        };
    }
    rsx! {
        div { class: "section-note", "{loc.tag_usage_note()}" }
        Table {
            headers: vec![
                loc.field_label("object-type"),
                loc.field_label("count"),
                loc.field_label("examples"),
            ],
            for group in detail.usage.iter() {
                {tag_usage_row(group)}
            }
        }
    }
}

/// One Usage-tab row: the object-type chip, the count, and a comma-joined example list.
fn tag_usage_row(group: &TagUsageGroupVm) -> Element {
    let examples = group
        .examples
        .iter()
        .map(|record| record.label.clone())
        .collect::<Vec<_>>()
        .join(", ");
    rsx! {
        tr {
            td { Chip { label: group.kind_label.clone() } }
            td { b { "{group.count}" } }
            td { class: "muted", "{examples}" }
        }
    }
}

/// The tag History tab: the audit timeline. Tags have no retraction command, so entries are not
/// undoable (no undo control).
fn tag_history_tab(loc: &Localizer, detail: &TagDetail, _on_submit: Callback<TagEdit>) -> Element {
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
    rsx! {
        div { class: "section-note", "{loc.history_note()}" }
        HistoryTimeline { entries, onundo: move |_assertion_id: String| {} }
    }
}

/// The tag editing side panel: the form for the open [`TagEditForm`], or nothing.
fn tag_edit_panel(
    state: &AppState,
    detail: &TagDetail,
    mut editing: Signal<Option<TagEditForm>>,
    on_submit: Callback<TagEdit>,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        TagEditForm::Name => loc.action_label("set-name"),
        TagEditForm::Priority => loc.action_label("set-priority"),
        TagEditForm::Color => loc.action_label("set-color"),
    };
    let id = detail.id.clone();
    let current = match form {
        TagEditForm::Name => detail.name.clone().unwrap_or_default(),
        TagEditForm::Priority => detail.priority.map(|p| p.to_string()).unwrap_or_default(),
        TagEditForm::Color => detail.color.clone().unwrap_or_default(),
    };
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| editing.set(None),
            footer: rsx! {},
            TagEditFormView { id, form, current, onsubmit: move |edit| on_submit.call(edit) }
        }
    }
}

/// The tag edit form: one field (name / priority / colour) → the matching [`TagEdit`] variant.
#[component]
fn TagEditFormView(id: String, form: TagEditForm, current: String, onsubmit: EventHandler<TagEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut value = use_signal(|| current.clone());
    let save_label = loc.action_label("save");
    let field = match form {
        TagEditForm::Name => "name",
        TagEditForm::Priority => "priority",
        TagEditForm::Color => "color",
    };
    let field_label = loc.field_label(field);
    rsx! {
        Input { label: field_label, name: field.to_owned(), value: Some(current), oninput: move |event: FormEvent| value.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let value = value();
                if value.trim().is_empty() {
                    return;
                }
                let edit = match form {
                    TagEditForm::Name => TagEdit::SetName { id: id.clone(), name: value },
                    TagEditForm::Priority => match value.trim().parse::<i32>() {
                        Ok(priority) => TagEdit::SetPriority { id: id.clone(), priority },
                        Err(_) => return,
                    },
                    TagEditForm::Color => TagEdit::SetColor { id: id.clone(), color: value },
                };
                onsubmit.call(edit);
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// DnaTest slice
// ---------------------------------------------------------------------------------------------------
