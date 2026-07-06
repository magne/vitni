use super::prelude::*;
use crate::screens::RecordDetail;

/// The selectable note types for the type-edit form, in display order.
fn note_type_choices() -> [NoteType; 4] {
    [
        NoteType::General,
        NoteType::Research,
        NoteType::Transcript,
        NoteType::Citation,
    ]
}

/// The note master-detail: a searchable list on the left, the selected note on the right.
#[component]
pub fn NoteScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Notes.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().note_list_empty();
    let dismiss_label = state.data_loc().action_label("dismiss");
    let list_chrome = ListChrome {
        list_label: entity.clone(),
        filter_placeholder: chrome.list_filter(&entity),
        empty,
    };
    let mut nav = use_context::<NavState>();
    let mut selected = use_signal(|| None::<String>);
    let mut toast = use_signal(|| None::<String>);
    use_effect(move || selected.set(nav.active_record_ref().map(|record| record.human_id)));
    use_effect(move || {
        if *nav.pending_create.read() == Some(Category::Notes) {
            nav.pending_create.set(None);
            let services = create_services.clone();
            spawn(async move {
                match create_note_record(services).await {
                    Ok(human_id) => nav.open_record(RecordRef {
                        category: Category::Notes,
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
        async move { load_screen(services, Intent::ShowNoteList).await }
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
                    category: Category::Notes,
                    human_id: row.id,
                    label: row.title,
                }),
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
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_)
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
        MasterDetail { list: list_pane, detail: rsx! { RecordDetail {} } }
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// Which note edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteEditForm {
    /// Set the note type.
    Type,
    /// Set the note's primary text.
    Text,
    /// Add a translation.
    Translation,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected note: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn NoteDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let mut nav = use_context::<NavState>();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<NoteEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowNote { human_id }).await }
    });

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the note's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::NoteDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        nav.set_record_label(
            Category::Notes,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (NoteEdit, ProvenanceDraft)| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_note_edit(services, edit, prov).await {
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
        Some(ScreenData::Loaded(IntentOutcome::NoteDetail(detail))) => {
            note_detail(&state, detail, active, editing, on_submit, &human_id)
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
            | IntentOutcome::Dashboard(_)
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

/// Renders a loaded note's detail container: header, the tab strip, the active tab, and the panel.
fn note_detail(
    state: &AppState,
    detail: &NoteDetail,
    active: Signal<usize>,
    editing: Signal<Option<NoteEditForm>>,
    on_submit: Callback<(NoteEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = note_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_id = tabs.get(active()).map_or("content", |tab| tab.id);
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            id_label: detail.human_id.clone(),
            avatar: "🗒".to_owned(),
            extras: note_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {note_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {note_edit_panel(state, detail, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a note.
fn note_restriction_toggles(
    loc: &Localizer,
    detail: &NoteDetail,
    on_submit: Callback<(NoteEdit, ProvenanceDraft)>,
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
                on_submit.call((NoteEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one note detail tab, with its contextual add affordances.
fn note_tab_content(
    state: &AppState,
    detail: &NoteDetail,
    tab_id: &str,
    mut editing: Signal<Option<NoteEditForm>>,
    on_submit: Callback<(NoteEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "language" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-translation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(NoteEditForm::Translation)) }
            }
            {note_language_tab(loc, detail)}
        },
        "references" => rsx! {
            div { class: "section-note", "{loc.note_references_note()}" }
            {note_references_table(loc, &detail.references)}
        },
        "tags" => note_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => note_history_tab(loc, detail, on_submit, human_id),
        _ => note_content_tab(loc, detail, editing),
    }
}

/// The Content tab: the type + rich-text note, with an Edit affordance for type/text.
pub fn note_content_tab(loc: &Localizer, detail: &NoteDetail, mut editing: Signal<Option<NoteEditForm>>) -> Element {
    let heading = match (detail.note_type_label.clone(), detail.language.clone()) {
        (Some(note_type), Some(language)) => format!("{note_type} · {language}"),
        (Some(note_type), None) => note_type,
        (None, Some(language)) => language,
        (None, None) => loc.tab_label("content"),
    };
    rsx! {
        div { class: "section-note", "{loc.note_content_note()}" }
        div { class: "tab-actions",
            Button { label: loc.action_label("edit"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(NoteEditForm::Text)) }
            Button { label: loc.field_label("type"), variant: ButtonVariant::Ghost, onclick: move |_| editing.set(Some(NoteEditForm::Type)) }
        }
        Card { title: heading,
            if let Some(text) = detail.text.clone() {
                for paragraph in text.split("\n\n") {
                    p { "{paragraph}" }
                }
            } else {
                p { class: "muted", "{loc.tab_empty()}" }
            }
        }
    }
}

/// The Language tab: the primary-language card and the translations table.
pub fn note_language_tab(loc: &Localizer, detail: &NoteDetail) -> Element {
    rsx! {
        Card { title: loc.section_label("primary-language"),
            div { class: "fact-row",
                span { class: "field-label", style: "width:120px;margin:0", "{loc.field_label(\"language\")}" }
                span { class: "grow", {detail.language.clone().unwrap_or_else(|| "—".to_owned())} }
            }
        }
        if detail.translations.is_empty() {
            EmptyState { message: loc.tab_empty() }
        } else {
            Table {
                headers: vec![
                    loc.field_label("language"),
                    loc.field_label("translation"),
                    loc.field_label("translator"),
                ],
                for translation in detail.translations.iter() {
                    tr {
                        td { Chip { label: translation.language.clone().unwrap_or_else(|| "—".to_owned()) } }
                        td { "{translation.text}" }
                        td { class: "muted", {translation.translator.clone().unwrap_or_else(|| "—".to_owned())} }
                    }
                }
            }
        }
    }
}

/// The References tab: a row per record that references this note (object · kind · id).
pub fn note_references_table(loc: &Localizer, references: &[UsingRecordVm]) -> Element {
    if references.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("object"),
                loc.field_label("type"),
                loc.field_label("id"),
            ],
            for record in references.iter() {
                tr {
                    td { "{record.label}" }
                    td { Chip { label: record.kind_label.clone() } }
                    td { class: "muted mono", "{record.human_id}" }
                }
            }
        }
    }
}

/// The note Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn note_tags_panel(
    loc: &Localizer,
    detail: &NoteDetail,
    mut editing: Signal<Option<NoteEditForm>>,
    on_submit: Callback<(NoteEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(NoteEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call((NoteEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The note History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn note_history_tab(
    loc: &Localizer,
    detail: &NoteDetail,
    on_submit: Callback<(NoteEdit, ProvenanceDraft)>,
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
                on_submit.call((NoteEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The note editing side panel: renders the form for the open [`NoteEditForm`], or nothing.
fn note_edit_panel(
    state: &AppState,
    detail: &NoteDetail,
    mut editing: Signal<Option<NoteEditForm>>,
    on_submit: Callback<(NoteEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        NoteEditForm::Type => loc.field_label("type"),
        NoteEditForm::Text => loc.action_label("edit"),
        NoteEditForm::Translation => loc.action_label("add-translation"),
        NoteEditForm::Tag => loc.action_label("add-tag"),
    };
    let human_id = human_id.to_owned();
    let current_text = detail.text.clone().unwrap_or_default();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| editing.set(None),
            footer: rsx! {},
            {match form {
                NoteEditForm::Type => rsx! { NoteTypeForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                NoteEditForm::Text => rsx! { NoteTextForm { human_id, current: current_text.clone(), onsubmit: move |edit| on_submit.call(edit) } },
                NoteEditForm::Translation => rsx! { NoteTranslationForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                NoteEditForm::Tag => rsx! { NoteTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Set type" form: a picker of note types → [`NoteEdit::SetType`].
#[component]
fn NoteTypeForm(human_id: String, onsubmit: EventHandler<(NoteEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let options: Vec<SelectChoice> = note_type_choices()
        .iter()
        .enumerate()
        .map(|(position, note_type)| SelectChoice {
            value: position.to_string(),
            label: loc.note_type_label(note_type),
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
                let note_type = note_type_choices().get(chosen()).cloned().unwrap_or(NoteType::General);
                onsubmit.call((NoteEdit::SetType { human_id: human_id.clone(), note_type }, prov()));
            },
        }
    }
}

/// The "Edit text" form: the note's Markdown body → [`NoteEdit::SetText`].
#[component]
fn NoteTextForm(human_id: String, current: String, onsubmit: EventHandler<(NoteEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut text = use_signal(|| current.clone());
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input {
            label: loc.tab_label("content"),
            name: "text".to_owned(),
            value: Some(current.clone()),
            oninput: move |event: FormEvent| text.set(event.value()),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| onsubmit.call((NoteEdit::SetText { human_id: human_id.clone(), text: text() }, prov())),
        }
    }
}

/// The "Add translation" form: language + text + translator → [`NoteEdit::AddTranslation`].
#[component]
fn NoteTranslationForm(human_id: String, onsubmit: EventHandler<(NoteEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut language = use_signal(String::new);
    let mut text = use_signal(String::new);
    let mut translator = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("language"), name: "language".to_owned(), oninput: move |event: FormEvent| language.set(event.value()) }
        Input { label: loc.field_label("translation"), name: "translation".to_owned(), oninput: move |event: FormEvent| text.set(event.value()) }
        Input { label: loc.field_label("translator"), name: "translator".to_owned(), oninput: move |event: FormEvent| translator.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let language = language();
                let text = text();
                if language.trim().is_empty() || text.trim().is_empty() {
                    return;
                }
                let translator = translator();
                let translator = if translator.trim().is_empty() { None } else { Some(translator) };
                onsubmit.call((NoteEdit::AddTranslation { human_id: human_id.clone(), language, text, translator }, prov()));
            },
        }
    }
}

/// The note "Add tag" form: a picker of existing tags by name → [`NoteEdit::Tag`].
#[component]
fn NoteTagForm(human_id: String, onsubmit: EventHandler<(NoteEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((NoteEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Tag slice
// ---------------------------------------------------------------------------------------------------
