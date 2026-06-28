use super::prelude::*;

/// The media master-detail: a searchable list on the left, the selected media object on the right.
#[component]
pub fn MediaScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Media.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().media_list_empty();
    let prompt = chrome.media_select_prompt();
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
            spawn(async move {
                match create_media_record(services).await {
                    Ok(human_id) => nav.open_record(RecordRef {
                        category: Category::Media,
                        label: human_id.clone(),
                        human_id,
                    }),
                    Err(message) => toast.set(Some(message)),
                }
            });
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowMediaList).await }
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
                    category: Category::Media,
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
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_),
        )) => rsx! {},
    };
    let detail_pane = match nav.active_record_ref() {
        Some(record) if record.category == Category::Media => {
            let human_id = record.human_id;
            rsx! { MediaDetailPane { key: "{human_id}", human_id } }
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

/// Which media edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaEditForm {
    /// Attach a citation by `human_id`.
    Citation,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected media object: header, related-item tabs, editing side panel, toast.
#[component]
fn MediaDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<MediaEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowMedia { human_id }).await }
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |edit: MediaEdit| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_media_edit(services, edit).await {
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
        Some(ScreenData::Loaded(IntentOutcome::MediaDetail(detail))) => {
            media_detail(&state, detail, active, editing, on_submit, &human_id)
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
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_),
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

/// Renders a loaded media object's detail container: header, the tab strip, the active tab, the panel.
fn media_detail(
    state: &AppState,
    detail: &MediaDetail,
    active: Signal<usize>,
    editing: Signal<Option<MediaEditForm>>,
    on_submit: Callback<MediaEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = media_tabs(detail, loc);
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
            avatar: "📷".to_owned(),
            extras: media_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {media_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {media_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a media object (the mockup `resn-set`).
fn media_restriction_toggles(
    loc: &Localizer,
    detail: &MediaDetail,
    on_submit: Callback<MediaEdit>,
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
                on_submit.call(MediaEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next });
            },
        }
    }
}

/// The content of one media detail tab, with its contextual add affordances.
fn media_tab_content(
    state: &AppState,
    detail: &MediaDetail,
    tab_id: &str,
    mut editing: Signal<Option<MediaEditForm>>,
    on_submit: Callback<MediaEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "citations" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-citation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(MediaEditForm::Citation)) }
            }
            {media_citations_table(loc, &detail.citations)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(MediaEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => media_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => media_history_tab(loc, detail, on_submit, human_id),
        _ => media_overview(loc, detail),
    }
}

/// The Overview tab: a preview placeholder, the File metadata card, and the "Used by" card.
pub fn media_overview(loc: &Localizer, detail: &MediaDetail) -> Element {
    rsx! {
        Card { title: loc.media_preview(),
            div { class: "media-preview faint", aria_hidden: "true", "📷" }
            div { class: "muted", "{detail.title}" }
        }
        div { class: "grid-2",
            Card { title: loc.section_label("file"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"file-path\")}" }
                        span { class: "grow mono", {detail.path.clone().unwrap_or_else(|| "—".to_owned())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"mime\")}" }
                        span { class: "grow", {detail.mime.clone().unwrap_or_else(|| "—".to_owned())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"date\")}" }
                        span { class: "grow", {detail.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"checksum\")}" }
                        span { class: "grow mono", style: "word-break:break-all", {detail.checksum.clone().unwrap_or_else(|| "—".to_owned())} }
                    }
                }
            }
            Card { title: loc.field_label("used-by"),
                {media_used_by(loc, &detail.used_by)}
            }
        }
    }
}

/// The "Used by" card body: a row per referencing record (kind chip + label), or an empty state.
fn media_used_by(loc: &Localizer, used_by: &[UsingRecordVm]) -> Element {
    if used_by.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "stack",
            for record in used_by.iter() {
                div { class: "fact-row",
                    Chip { label: record.kind_label.clone() }
                    span { class: "grow", "{record.label}" }
                    span { class: "muted mono", "{record.human_id}" }
                }
            }
        }
    }
}

/// The Citations tab: a row per citation with source, page, surety, and evidence axes.
pub fn media_citations_table(loc: &Localizer, citations: &[CitationRefVm]) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("source"),
                loc.field_label("page"),
                loc.field_label("surety"),
                loc.field_label("evidence"),
            ],
            for citation in citations.iter() {
                tr {
                    td { {citation.source.clone().unwrap_or_else(|| citation.human_id.clone())} }
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
                }
            }
        }
    }
}

/// The media Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn media_tags_panel(
    loc: &Localizer,
    detail: &MediaDetail,
    mut editing: Signal<Option<MediaEditForm>>,
    on_submit: Callback<MediaEdit>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(MediaEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call(MediaEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The media History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn media_history_tab(loc: &Localizer, detail: &MediaDetail, on_submit: Callback<MediaEdit>, human_id: &str) -> Element {
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
                on_submit.call(MediaEdit::UndoAssertion { human_id: human_id.clone(), assertion_id });
            },
        }
    }
}

/// The media editing side panel: renders the form for the open [`MediaEditForm`], or nothing.
fn media_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<MediaEditForm>>,
    on_submit: Callback<MediaEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        MediaEditForm::Citation => loc.action_label("attach-citation"),
        MediaEditForm::Note => loc.action_label("attach-note"),
        MediaEditForm::Tag => loc.action_label("add-tag"),
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
                MediaEditForm::Citation => rsx! { MediaAttachForm { human_id, field: "citation".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                MediaEditForm::Note => rsx! { MediaAttachForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                MediaEditForm::Tag => rsx! { MediaTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Attach citation/note by id" form → the matching [`MediaEdit`] attach variant.
#[component]
fn MediaAttachForm(human_id: String, field: String, onsubmit: EventHandler<MediaEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut id = use_signal(String::new);
    let save_label = loc.action_label("save");
    let field_label = loc.field_label(&field);
    rsx! {
        Input { label: field_label, name: field.clone(), oninput: move |event: FormEvent| id.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let id = id();
                if id.trim().is_empty() {
                    return;
                }
                let edit = match field.as_str() {
                    "citation" => MediaEdit::AttachCitation { human_id: human_id.clone(), citation_id: id },
                    _ => MediaEdit::AttachNote { human_id: human_id.clone(), note_id: id },
                };
                onsubmit.call(edit);
            },
        }
    }
}

/// The media "Add tag" form: a picker of existing tags by name → [`MediaEdit::Tag`].
#[component]
fn MediaTagForm(human_id: String, onsubmit: EventHandler<MediaEdit>) -> Element {
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
                Button {
                    label: save_label,
                    variant: ButtonVariant::Primary,
                    onclick: move |_| {
                        let tag_id = chosen();
                        if tag_id.is_empty() {
                            return;
                        }
                        onsubmit.call(MediaEdit::Tag { human_id: human_id.clone(), tag_id, remove: false });
                    },
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Note slice
// ---------------------------------------------------------------------------------------------------
