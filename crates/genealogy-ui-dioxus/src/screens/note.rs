use super::prelude::*;

/// The selectable note types for the type-edit form, in display order.
fn note_type_choices() -> [NoteType; 4] {
    [
        NoteType::General,
        NoteType::Research,
        NoteType::Transcript,
        NoteType::Citation,
    ]
}

/// The create-mode note record: an uncommitted [`NoteDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Save commits the whole note; Cancel discards.
#[component]
pub fn NoteCreateRecord() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::NoteDraft>(Category::Notes);
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::NoteDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let label = request.text.clone().unwrap_or_default();
        let services = services.clone();
        spawn(async move {
            let committed = commit_note_change_set(services, request, prov).await;
            finish_draft_commit(committed, Category::Notes, Some(label), nav);
        });
    });
    // The close/quit confirm's Save runs this same commit (issue #240), so a ⌘W/⌘Q over a half-filled
    // create form can keep the draft instead of losing it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(Category::Notes, None, record, save_now);
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| nav.cancel_draft(Category::Notes) }
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| save_now.call(()),
        }
    };
    create_record_frame(
        &loc.note_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {note_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        },
    )
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteEditForm {
    /// Add or edit a translation — `None` adds a new one, `Some(row)` edits (supersedes the text
    /// assertion the translation lives in).
    Translation(Option<TranslationVm>),
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
    let record = use_record_edit::<genealogy_ui::NoteDraft>(Category::Notes, &human_id, &seed);

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

    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: NoteEditForm| editing_for_open.set(Some(form)));
    // The shared row-actions cell always takes an `onretract`; a translation is Edit-only (removing a
    // single translation has no app verb — a plain undo would drop the whole note text), so this is a
    // no-op the translations table never invokes (`retract: None`).
    let on_retract = use_callback(|_: (String, String, bool)| {});

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
    let note_tag_human = human_id.clone();
    let on_tag_remove = use_callback(move |tag_id: String| {
        on_submit.call((
            NoteEdit::Tag {
                human_id: note_tag_human.clone(),
                tag_id,
                remove: true,
            },
            ProvenanceDraft::default(),
        ));
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

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::NoteDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            NoteEdit::UndoAssertion {
                human_id: undo_human.clone(),
                assertion_id,
            },
            ProvenanceDraft::default(),
        ));
    });
    use_record_undo(nav, undo_busy, undo_history, undo_notice, on_undo);

    // The close/quit confirm's Save hands the record back to this pane (issue #240): it runs the same
    // whole-record commit the header's Save does, so ⌘W/⌘Q can keep the edit instead of discarding it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_record_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(Category::Notes, Some(&human_id), record, save_now);

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
                on_edit_open,
                on_retract,
                on_undo,
                on_tag_remove,
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
            | IntentOutcome::DataQuality(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Pedigree(_)
            | IntentOutcome::Relationship(_)
            | IntentOutcome::DuplicateCandidates(_)
            | IntentOutcome::MergeCompare(_)
            | IntentOutcome::ResearchNoteDetail(_)
            | IntentOutcome::Geography(_),
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
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct NoteCallbacks {
    /// Commits one [`NoteEdit`] command (a collection row).
    on_submit: Callback<(NoteEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::NoteDraft, ProvenanceDraft)>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes the text assertion).
    on_edit_open: Callback<NoteEditForm>,
    /// The row-actions cell's required retract callback; a no-op — translations are Edit-only.
    on_retract: Callback<(String, String, bool)>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Untags a tag by id from the Tags tab (dispatches `Tag { remove: true }`).
    on_tag_remove: Callback<String>,
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
    let on_edit_open = callbacks.on_edit_open;
    let on_retract = callbacks.on_retract;
    let on_undo = callbacks.on_undo;
    let on_tag_remove = callbacks.on_tag_remove;
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
            {note_tab_content(state, detail, active_id, editing, record, on_edit_open, on_retract, on_undo, on_tag_remove)}
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
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threading the note's edit callbacks"
)]
fn note_tab_content(
    state: &AppState,
    detail: &NoteDetail,
    tab_id: &str,
    editing: Signal<Option<NoteEditForm>>,
    record: RecordEditState<genealogy_ui::NoteDraft>,
    on_edit_open: Callback<NoteEditForm>,
    on_retract: Callback<(String, String, bool)>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<String>,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "language" => tab_with_add(
            loc,
            "add-translation",
            editing,
            NoteEditForm::Translation(None),
            rsx! {
                {note_language_tab(loc, detail, on_edit_open, on_retract)}
            },
        ),
        "references" => rsx! {
            div { class: "section-note", "{loc.note_references_note()}" }
            {note_references_table(loc, &detail.references)}
        },
        "tags" => tags_panel(loc, &detail.tags, editing, NoteEditForm::Tag, on_tag_remove),
        "history" => history_panel(loc, &detail.history, Some(on_undo)),
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

/// The Language tab: the primary-language card and the translations table. Each translation row
/// carries an Edit (opens the pre-filled form; Save supersedes the shared text assertion via
/// [`NoteEdit::AddTranslation`]) but **no** Retract — removing a single translation has no app verb,
/// and a plain undo would drop the whole note text. `onretract` is the shared cell's required
/// argument, never invoked here (`retract: None`).
pub fn note_language_tab(
    loc: &Localizer,
    detail: &NoteDetail,
    onedit: Callback<NoteEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
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
                caption: loc.tab_label("translations"),
                headers: vec![
                    loc.field_label("language"),
                    loc.field_label("translation"),
                    loc.field_label("translator"),
                    String::new(),
                ],
                for translation in detail.translations.iter() {
                    tr {
                        td { Chip { label: translation.language.clone().unwrap_or_else(|| "—".to_owned()) } }
                        td { "{translation.text}" }
                        td { class: "muted", {translation.translator.clone().unwrap_or_else(|| "—".to_owned())} }
                        {row_actions_cell::<NoteEditForm>(
                            loc,
                            &translation.language.clone().unwrap_or_else(|| translation.text.clone()),
                            Some((NoteEditForm::Translation(Some(translation.clone())), None)), None,
                            None,
                            Some(onedit),
                            onretract)}
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
            caption: loc.tab_label("references"),
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
    let title = match &form {
        NoteEditForm::Translation(None) => loc.action_label("add-translation"),
        NoteEditForm::Translation(Some(_)) => loc.panel_title("edit-translation"),
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
                NoteEditForm::Translation(seed) => rsx! { NoteTranslationForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                NoteEditForm::Tag => rsx! { NoteTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The translation form: language + text + translator → [`NoteEdit::AddTranslation`]. `seed: None`
/// adds a new translation; `Some(row)` edits an existing one — the fields are pre-filled and the
/// draft's `supersedes` is seeded with the row's (shared) text-assertion id so Save re-emits the
/// whole `RichText` as a supersede, upserting the translation by language (ADR 0004 §2).
#[component]
fn NoteTranslationForm(
    human_id: String,
    seed: Option<TranslationVm>,
    onsubmit: EventHandler<(NoteEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut language = use_signal(|| seed.as_ref().and_then(|row| row.language.clone()).unwrap_or_default());
    let mut text = use_signal(|| seed.as_ref().map(|row| row.text.clone()).unwrap_or_default());
    let mut translator = use_signal(|| seed.as_ref().and_then(|row| row.translator.clone()).unwrap_or_default());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("language"), name: "language".to_owned(), value: language(), oninput: move |event: FormEvent| language.set(event.value()) }
        Input { label: loc.field_label("translation"), name: "translation".to_owned(), value: text(), oninput: move |event: FormEvent| text.set(event.value()) }
        Input { label: loc.field_label("translator"), name: "translator".to_owned(), value: translator(), oninput: move |event: FormEvent| translator.set(event.value()) }
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
