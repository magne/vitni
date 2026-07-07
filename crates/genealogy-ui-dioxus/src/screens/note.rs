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
    let mut creating = use_signal(|| false);
    let mut toast = use_signal(|| None::<String>);
    use_effect(move || selected.set(nav.active_record_ref().map(|record| record.human_id)));
    // The top-bar `New` sets `pending_create`; open the draft here (nothing is created until Save).
    use_effect(move || {
        if *nav.pending_create.read() == Some(Category::Notes) {
            creating.set(true);
            nav.pending_create.set(None);
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
                onselect: move |row: RowVm| {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::Notes,
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
    let on_created = use_callback(move |(id, label): (String, String)| {
        creating.set(false);
        nav.open_record(RecordRef {
            category: Category::Notes,
            human_id: id.clone(),
            label: if label.is_empty() { id } else { label },
        });
    });
    let detail = if creating() {
        rsx! {
            NoteCreateRecord {
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

/// The create-mode note record: an uncommitted [`NoteDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Save commits the whole note; Cancel discards.
#[component]
fn NoteCreateRecord(
    oncreated: EventHandler<(String, String)>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::NoteDraft>();
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::NoteDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let label = request.text.clone().unwrap_or_default();
        let services = services.clone();
        spawn(async move {
            match commit_note_change_set(services, request, prov).await {
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
        {create_record_header(&loc.note_new_title(), &loc.record_draft_badge(), actions)}
        {note_record_fields(loc, record)}
        {record_edit_provenance(loc, record)}
    }
}

/// The note's scalar record fields (id · type · content · language), read-first: read boxes in view
/// mode, inputs with per-field reset in edit mode (`record-editing.html` §2/§3). Content is a textarea.
/// A pure fn (the edit state's signals passed in) so the create pane and the SSR tests render it
/// without `AppCtx`. Shared by view, edit, and create.
pub fn note_record_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::NoteDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let types = note_type_choices();
    let mut options = vec![SelectChoice {
        value: String::new(),
        label: loc.record_unset(),
    }];
    for (index, note_type) in types.iter().enumerate() {
        options.push(SelectChoice {
            value: index.to_string(),
            label: loc.note_type_label(note_type),
        });
    }
    let index_of = |note_type: &Option<NoteType>| {
        note_type
            .as_ref()
            .and_then(|chosen| note_type_choices().iter().position(|t| t == chosen))
            .map_or_else(String::new, |index| index.to_string())
    };
    let type_value = index_of(&draft().note_type);
    let type_original = index_of(&seed.read().note_type);
    let id_value = draft().human_id.clone();
    let id_original = seed.read().human_id.clone();
    let text_value = draft().text.clone();
    let text_original = seed.read().text.clone();
    let language_value = draft().language.clone();
    let language_original = seed.read().language.clone();
    rsx! {
        Card { title: loc.section_label("content"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "note-id".to_owned(),
                    editing,
                    value: id_value,
                    original: id_original,
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
                    name: "note-type".to_owned(),
                    editing,
                    value: type_value,
                    original: type_original,
                    reset_label: loc.action_reset_field(&loc.field_label("type")),
                    options,
                    onchange: move |value: String| {
                        let types = note_type_choices();
                        draft.write().note_type = value.parse::<usize>().ok().and_then(|index| types.get(index).cloned());
                    },
                    onreset: move |()| {
                        let value = seed.read().note_type.clone();
                        draft.write().note_type = value;
                    },
                }
                DraftText {
                    label: loc.field_label("content"),
                    name: "note-content".to_owned(),
                    editing,
                    value: text_value,
                    original: text_original,
                    reset_label: loc.action_reset_field(&loc.field_label("content")),
                    multiline: true,
                    oninput: move |value: String| draft.write().text = value,
                    onreset: move |()| {
                        let value = seed.read().text.clone();
                        draft.write().text = value;
                    },
                }
                DraftText {
                    label: loc.field_label("language"),
                    name: "note-language".to_owned(),
                    editing,
                    value: language_value,
                    original: language_original,
                    reset_label: loc.action_reset_field(&loc.field_label("language")),
                    oninput: move |value: String| draft.write().language = value,
                    onreset: move |()| {
                        let value = seed.read().language.clone();
                        draft.write().language = value;
                    },
                }
            }
        }
    }
}

/// Which note collection-row edit form (if any) the side panel is showing. The note's own scalar
/// record (id · type · content · language) is edited in place via the sticky-header Edit, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteEditForm {
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
    let nav = use_context::<NavState>();
    let mut label_nav = nav;
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

    // The shared whole-record edit state, seeded from the loaded note (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::NoteDetail(detail))) => genealogy_ui::NoteDraft::from_detail(detail),
        _ => genealogy_ui::NoteDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::NoteDraft>(&seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the note's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::NoteDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::Notes,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (NoteEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_note_edit(services, edit, prov).await {
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::NoteDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_note_edit).await;
            finish_record_save(effective, Category::Notes, &current, record_nav, reload, toast, &saved);
        });
    });

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::NoteDetail(detail))) => note_detail(
            &state,
            detail,
            NotePane {
                active,
                side_edit: editing,
                record,
            },
            NoteCallbacks {
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

/// The signals a note's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit state.
#[derive(Clone, Copy)]
struct NotePane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<NoteEditForm>>,
    /// The whole-record (id · type · content · language) edit state.
    record: RecordEditState<genealogy_ui::NoteDraft>,
}

/// The two commit callbacks a note's detail wires in: one-command collection edits and the
/// whole-record save (the scalar edit via `edits_against`).
#[derive(Clone, Copy)]
struct NoteCallbacks {
    /// Commits one [`NoteEdit`] command (a collection row).
    on_submit: Callback<(NoteEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::NoteDraft, ProvenanceDraft)>,
}

/// Renders a loaded note's detail container: header (with the sticky-header record Edit/Cancel/Save),
/// the tab strip, the active tab, and the collection-row side panel.
fn note_detail(
    state: &AppState,
    detail: &NoteDetail,
    pane: NotePane,
    callbacks: NoteCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let NotePane {
        active,
        side_edit: editing,
        record,
    } = pane;
    let on_submit = callbacks.on_submit;
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
    let labels = RecordActionLabels::resolve(loc);
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            id_label: Some(detail.human_id.clone()),
            avatar: "🗒".to_owned(),
            extras: note_restriction_toggles(loc, detail, on_submit, human_id),
            actions: record_head_actions(&labels, record, rsx! {}, callbacks.on_record_save),
            tabs: tab_items,
            active,
            {note_tab_content(state, detail, active_id, editing, record, on_submit, human_id)}
        }
        {note_edit_panel(state, editing, on_submit, human_id)}
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
    record: RecordEditState<genealogy_ui::NoteDraft>,
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
        _ => note_content_tab(loc, detail, record),
    }
}

/// The Content tab, read-first (`record-editing.html` §1/§2): the note's scalar record (id · type ·
/// content · language) as read boxes; entering edit mode (via the sticky-header Edit) swaps in the
/// inputs and, while dirty, the provenance block.
pub fn note_content_tab(
    loc: &Localizer,
    _detail: &NoteDetail,
    record: RecordEditState<genealogy_ui::NoteDraft>,
) -> Element {
    rsx! {
        div { class: "section-note", "{loc.note_content_note()}" }
        {note_record_fields(loc, record)}
        {record_edit_provenance(loc, record)}
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

/// The note collection-row editing side panel: renders the form for the open [`NoteEditForm`]
/// (translation or tag), or nothing. The note's scalar record is edited in place via the sticky header.
fn note_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<NoteEditForm>>,
    on_submit: Callback<(NoteEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        NoteEditForm::Translation => loc.action_label("add-translation"),
        NoteEditForm::Tag => loc.action_label("add-tag"),
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
                NoteEditForm::Translation => rsx! { NoteTranslationForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                NoteEditForm::Tag => rsx! { NoteTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
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
