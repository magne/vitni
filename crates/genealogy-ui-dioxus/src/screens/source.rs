use super::prelude::*;

/// The source master-detail: a searchable list on the left, the selected source's detail on the right.
#[component]
pub fn SourceScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Sources.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().source_list_empty();
    let prompt = chrome.source_select_prompt();
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
                match create_source_record(services).await {
                    Ok(human_id) => nav.open_record(RecordRef {
                        category: Category::Sources,
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
        async move { load_screen(services, Intent::ShowSourceList).await }
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
                    category: Category::Sources,
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
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_),
        )) => rsx! {},
    };
    let detail_pane = match nav.active_record_ref() {
        Some(record) if record.category == Category::Sources => {
            let human_id = record.human_id;
            rsx! { SourceDetailPane { key: "{human_id}", human_id } }
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

/// Which source edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEditForm {
    /// Link a repository (by `human_id`) with a call number + medium.
    Repository,
    /// Add a typed attribute (key + value).
    Attribute,
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected source: header, related-item tabs, editing side panel, toast.
#[component]
fn SourceDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<SourceEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowSource { human_id }).await }
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |edit: SourceEdit| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_source_edit(services, edit).await {
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
        Some(ScreenData::Loaded(IntentOutcome::SourceDetail(detail))) => {
            source_detail(&state, detail, active, editing, on_submit, &human_id)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
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

/// Renders a loaded source's detail container: header, the tab strip, the active tab, and the panel.
fn source_detail(
    state: &AppState,
    detail: &SourceDetail,
    active: Signal<usize>,
    editing: Signal<Option<SourceEditForm>>,
    on_submit: Callback<SourceEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = source_tabs(detail, loc);
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
            avatar: "📚".to_owned(),
            extras: source_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {source_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {source_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a source (the mockup `resn-set`).
fn source_restriction_toggles(
    loc: &Localizer,
    detail: &SourceDetail,
    on_submit: Callback<SourceEdit>,
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
                on_submit.call(SourceEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next });
            },
        }
    }
}

/// The content of one source detail tab, with its contextual add affordances.
fn source_tab_content(
    state: &AppState,
    detail: &SourceDetail,
    tab_id: &str,
    mut editing: Signal<Option<SourceEditForm>>,
    on_submit: Callback<SourceEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "repositories" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("link-repository"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(SourceEditForm::Repository)) }
            }
            {source_repositories_table(loc, detail)}
        },
        "citations" => rsx! {
            div { class: "section-note", "{loc.source_citations_note()}" }
            {source_citations_table(loc, &detail.citations)}
        },
        "attributes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-attribute"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(SourceEditForm::Attribute)) }
            }
            {source_attributes_table(loc, detail)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(SourceEditForm::Media)) }
            }
            {family_media_gallery(loc, &detail.media)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(SourceEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => source_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => source_history_tab(loc, detail, on_submit, human_id),
        _ => source_overview(loc, detail),
    }
}

/// The Overview tab: the master-record note, a Bibliographic card, and a Reliability card.
pub fn source_overview(loc: &Localizer, detail: &SourceDetail) -> Element {
    let reliability = &detail.reliability;
    rsx! {
        div { class: "section-note", "{loc.source_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("bibliographic"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"title\")}" }
                        span { class: "grow", "{detail.title}" }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"author\")}" }
                        span { class: "grow", {detail.author.clone().unwrap_or_else(|| "—".to_owned())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"publication\")}" }
                        span { class: "grow", {detail.pub_info.clone().unwrap_or_else(|| "—".to_owned())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"abbreviation\")}" }
                        span { class: "grow mono", {detail.abbrev.clone().unwrap_or_else(|| "—".to_owned())} }
                    }
                }
            }
            Card { title: loc.section_label("reliability"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"typical-surety\")}" }
                        if let (Some(level), Some(label)) = (reliability.confidence, reliability.confidence_label.clone()) {
                            span { class: "grow", ConfidenceBadge { level, label } }
                        } else {
                            span { class: "grow muted", "—" }
                        }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"evidence\")}" }
                        span { class: "grow wrap",
                            for chip in reliability.evidence_axes.iter() {
                                EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                            }
                        }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"used-by\")}" }
                        span { class: "grow", "{loc.source_count(reliability.citation_count)}" }
                    }
                }
            }
        }
    }
}

/// The Repositories tab: a row per repository link with call number, medium, and surety.
pub fn source_repositories_table(loc: &Localizer, detail: &SourceDetail) -> Element {
    if detail.repositories.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.tab_label("repositories"),
                loc.field_label("call-number"),
                loc.field_label("media-type"),
                loc.field_label("surety"),
            ],
            for link in detail.repositories.iter() {
                tr {
                    td { "{link.name}" }
                    td { class: "mono", {link.call_number.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { Chip { label: link.media_type_label.clone() } }
                    td {
                        ConfidenceBadge { level: link.confidence, label: link.confidence_label.clone() }
                        {source_cue(loc, link.source_count)}
                    }
                }
            }
        }
    }
}

/// The Citations tab: a row per (citation, backing-record) pair — page · backs-record · surety · evidence.
pub fn source_citations_table(loc: &Localizer, citations: &[SourceCitationVm]) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("page"),
                loc.field_label("backs-record"),
                loc.field_label("surety"),
                loc.field_label("evidence"),
            ],
            for row in citations.iter() {
                {
                    let citation = row.citation.clone();
                    let backers = if row.backers.is_empty() {
                        vec![None]
                    } else {
                        row.backers.iter().cloned().map(Some).collect::<Vec<_>>()
                    };
                    rsx! {
                        for backer in backers.into_iter() {
                            {
                                let citation = citation.clone();
                                rsx! {
                                    tr {
                                        td { class: "muted", {citation.page.clone().unwrap_or_else(|| "—".to_owned())} }
                                        td { {backs_record_label(backer.as_ref())} }
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
                }
            }
        }
    }
}

/// The "Backs record" cell text: the record label, with its sub-context appended when present.
fn backs_record_label(backer: Option<&CitingRecordVm>) -> String {
    match backer {
        None => "—".to_owned(),
        Some(record) if record.context_label.is_empty() => record.label.clone(),
        Some(record) => format!("{} — {}", record.label, record.context_label),
    }
}

/// The Attributes tab: a row per attribute with key, value, and the evidence-first source cue.
pub fn source_attributes_table(loc: &Localizer, detail: &SourceDetail) -> Element {
    if detail.attributes.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("attribute-type"),
                loc.field_label("value"),
                loc.field_label("source"),
            ],
            for attribute in detail.attributes.iter() {
                tr {
                    td { Chip { label: attribute.attribute_type.clone() } }
                    td { class: "mono", "{attribute.value}" }
                    td { {source_cue(loc, attribute.source_count)} }
                }
            }
        }
    }
}

/// The source Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn source_tags_panel(
    loc: &Localizer,
    detail: &SourceDetail,
    mut editing: Signal<Option<SourceEditForm>>,
    on_submit: Callback<SourceEdit>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(SourceEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call(SourceEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The source History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn source_history_tab(
    loc: &Localizer,
    detail: &SourceDetail,
    on_submit: Callback<SourceEdit>,
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
                on_submit.call(SourceEdit::UndoAssertion { human_id: human_id.clone(), assertion_id });
            },
        }
    }
}

/// The source editing side panel: renders the form for the open [`SourceEditForm`], or nothing.
fn source_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<SourceEditForm>>,
    on_submit: Callback<SourceEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        SourceEditForm::Repository => loc.action_label("link-repository"),
        SourceEditForm::Attribute => loc.action_label("add-attribute"),
        SourceEditForm::Media => loc.action_label("attach-media"),
        SourceEditForm::Note => loc.action_label("attach-note"),
        SourceEditForm::Tag => loc.action_label("add-tag"),
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
                SourceEditForm::Repository => rsx! { SourceLinkRepositoryForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                SourceEditForm::Attribute => rsx! { SourceAttributeForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                SourceEditForm::Media => rsx! { SourceAttachForm { human_id, field: "media".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                SourceEditForm::Note => rsx! { SourceAttachForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                SourceEditForm::Tag => rsx! { SourceTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Link repository" form: a repository `human_id` + call number + medium → [`SourceEdit::LinkRepository`].
#[component]
fn SourceLinkRepositoryForm(human_id: String, onsubmit: EventHandler<SourceEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let media_types = source_media_type_choices();
    let options: Vec<SelectChoice> = media_types
        .iter()
        .enumerate()
        .map(|(position, media_type)| SelectChoice {
            value: position.to_string(),
            label: loc.source_media_type_label(media_type),
        })
        .collect();
    let mut repository = use_signal(String::new);
    let mut call_number = use_signal(String::new);
    let mut media = use_signal(|| 0_usize);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.tab_label("repositories"), name: "repository".to_owned(), oninput: move |event: FormEvent| repository.set(event.value()) }
        Input { label: loc.field_label("call-number"), name: "call-number".to_owned(), oninput: move |event: FormEvent| call_number.set(event.value()) }
        Select {
            label: loc.field_label("media-type"),
            name: "media-type".to_owned(),
            value: Some(0.to_string()),
            options,
            onchange: move |event: FormEvent| media.set(event.value().parse::<usize>().unwrap_or(0)),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let repository_id = repository();
                if repository_id.trim().is_empty() {
                    return;
                }
                let media_type = source_media_type_choices().get(media()).cloned().unwrap_or(SourceMediaType::Book);
                let call = call_number();
                let call_number = if call.trim().is_empty() { None } else { Some(call) };
                onsubmit.call(SourceEdit::LinkRepository { human_id: human_id.clone(), repository_id, call_number, media_type });
            },
        }
    }
}

/// The "Add attribute" form: a key + value → [`SourceEdit::AddAttribute`].
#[component]
fn SourceAttributeForm(human_id: String, onsubmit: EventHandler<SourceEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut attribute_type = use_signal(String::new);
    let mut value = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("attribute-type"), name: "attribute-type".to_owned(), oninput: move |event: FormEvent| attribute_type.set(event.value()) }
        Input { label: loc.field_label("value"), name: "value".to_owned(), oninput: move |event: FormEvent| value.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let attribute_type = attribute_type();
                if attribute_type.trim().is_empty() {
                    return;
                }
                onsubmit.call(SourceEdit::AddAttribute { human_id: human_id.clone(), attribute_type, value: value() });
            },
        }
    }
}

/// The "Attach media/note by id" form → the matching [`SourceEdit`] attach variant.
#[component]
fn SourceAttachForm(human_id: String, field: String, onsubmit: EventHandler<SourceEdit>) -> Element {
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
                    "note" => SourceEdit::AttachNote { human_id: human_id.clone(), note_id: id },
                    _ => SourceEdit::AttachMedia { human_id: human_id.clone(), media_id: id },
                };
                onsubmit.call(edit);
            },
        }
    }
}

/// The source "Add tag" form: a picker of existing tags by name → [`SourceEdit::Tag`].
#[component]
fn SourceTagForm(human_id: String, onsubmit: EventHandler<SourceEdit>) -> Element {
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
                        onsubmit.call(SourceEdit::Tag { human_id: human_id.clone(), tag_id, remove: false });
                    },
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Repository slice
// ---------------------------------------------------------------------------------------------------
