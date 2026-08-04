//! The `ResearchNote` screen (ADR 0028, issue #194): a written proof argument about one or more
//! conclusion-bearing records.
//!
//! Modelled on `note.rs`, with two differences the aggregate forces. Subjects are a **forward**
//! collection the note owns, so the Subjects tab adds and removes rows (a note's References tab is a
//! reverse index it cannot edit); and the id and title have no update verb, so they are editable only
//! while the record is still a draft.

use genealogy_ui::{ResearchNoteDetail, ResearchNoteEdit, SubjectVm, research_note_tabs};

use super::prelude::*;
use crate::services::{commit_research_note_change_set, save_research_note_edit};

/// The aggregates a research note may argue about (ADR 0028 §2), in display order.
fn subject_categories() -> [Category; 4] {
    [Category::People, Category::Families, Category::Events, Category::Places]
}

/// The create-mode research-note record: an uncommitted [`ResearchNoteDraft`](genealogy_ui::ResearchNoteDraft)
/// rendered as the create form in the detail pane (`record-editing.html` §6). Save commits the whole
/// note (subjects + title, then the body); Cancel discards.
///
/// Consumes [`NavState::research_note_subject`] once, on mount: opening this form from a record's
/// "Research notes" tab pre-seeds that record as the argument's first subject.
#[component]
pub fn ResearchNoteCreateRecord() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::ResearchNoteDraft>(Category::ResearchNotes);

    let mut seeded = record.draft;
    let mut subject_seed = nav.research_note_subject;
    use_effect(move || {
        let Some((category, human_id)) = subject_seed.write().take() else {
            return;
        };
        seeded.write().add_subject(SubjectVm {
            category,
            human_id: human_id.clone(),
            id: String::new(),
            kind_label: String::new(),
        });
    });

    let on_save = use_callback(
        move |(draft, prov): (genealogy_ui::ResearchNoteDraft, ProvenanceDraft)| {
            let request = draft.to_request();
            let label = request.title.clone().unwrap_or_default();
            let services = services.clone();
            spawn(async move {
                let committed = commit_research_note_change_set(services, request, prov).await;
                finish_draft_commit(committed, Category::ResearchNotes, Some(label), nav);
            });
        },
    );
    // The close/quit confirm's Save runs this same commit (issue #240), so a ⌘W/⌘Q over a half-filled
    // create form can keep the draft instead of losing it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(Category::ResearchNotes, None, record, save_now);
    let can_save = record.can_save();
    let actions = rsx! {
        Button {
            label: loc.action_label("cancel"),
            variant: ButtonVariant::Ghost,
            small: true,
            onclick: move |_| nav.cancel_draft(Category::ResearchNotes),
        }
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| save_now.call(()),
        }
    };
    create_record_frame(
        &loc.research_note_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {research_note_record_fields(loc, record)}
            {research_note_draft_subjects(loc, record)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The research note's scalar record fields (id · title · argument · language), read-first: read boxes
/// in view mode, inputs with per-field reset in edit mode (`record-editing.html` §2/§3). The argument is
/// a textarea. A pure fn (the edit state's signals passed in) so the create pane and the SSR tests
/// render it without `AppCtx`.
///
/// The id and title stay read-only on a saved note: the aggregate has no rename and no title-set verb,
/// so offering an input the Save could not honour would be a lie.
pub fn research_note_record_fields(
    loc: &Localizer,
    record: RecordEditState<genealogy_ui::ResearchNoteDraft>,
) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let creating = draft().existing_human_id.is_none();
    let fixed_editing = editing && creating;
    let id_value = draft().human_id.clone();
    let id_original = seed.read().human_id.clone();
    let title_value = draft().title.clone();
    let title_original = seed.read().title.clone();
    let body_value = draft().body.clone();
    let body_original = seed.read().body.clone();
    let language_value = draft().language.clone();
    let language_original = seed.read().language.clone();
    rsx! {
        Card { title: loc.section_label("content"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "research-note-id".to_owned(),
                    editing: fixed_editing,
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
                DraftText {
                    label: loc.field_label("title"),
                    name: "research-note-title".to_owned(),
                    editing: fixed_editing,
                    value: title_value,
                    original: title_original,
                    reset_label: loc.action_reset_field(&loc.field_label("title")),
                    oninput: move |value: String| draft.write().title = value,
                    onreset: move |()| {
                        let value = seed.read().title.clone();
                        draft.write().title = value;
                    },
                }
                DraftText {
                    label: loc.field_label("argument"),
                    name: "research-note-body".to_owned(),
                    editing,
                    value: body_value,
                    original: body_original,
                    reset_label: loc.action_reset_field(&loc.field_label("argument")),
                    multiline: true,
                    oninput: move |value: String| draft.write().body = value,
                    onreset: move |()| {
                        let value = seed.read().body.clone();
                        draft.write().body = value;
                    },
                }
                DraftText {
                    label: loc.field_label("language"),
                    name: "research-note-language".to_owned(),
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

/// The create form's subject editor: the already-named subjects as removable chips, a picker to name
/// one more, and the "at least one subject" validation note while the draft names none (ADR 0028 §2 —
/// the Save is disabled until it does). Renders nothing on a saved note, whose subjects are the
/// per-row collection on the Subjects tab.
pub fn research_note_draft_subjects(
    loc: &Localizer,
    record: RecordEditState<genealogy_ui::ResearchNoteDraft>,
) -> Element {
    let mut draft = record.draft;
    if draft().existing_human_id.is_some() {
        return rsx! {};
    }
    let subjects = draft().subjects.clone();
    let empty = subjects.is_empty();
    rsx! {
        Card { title: loc.tab_label("subjects"),
            div { class: "section-note", "{loc.research_note_subjects_note()}" }
            if empty {
                div { class: "field-error", "{loc.research_note_subject_required()}" }
            } else {
                div { class: "wrap",
                    for (index , subject) in subjects.iter().enumerate() {
                        Chip {
                            key: "{subject.category.id()}-{subject.human_id}",
                            label: format!("{} {}", subject.kind_label, subject.human_id),
                            delete_label: loc.action_remove_row(&subject.human_id),
                            delete_title: loc.action_title("remove-subject"),
                            ondelete: move |()| draft.write().remove_subject(index),
                        }
                    }
                }
            }
            SubjectChooser {
                save_label: loc.action_label("add-subject"),
                onpick: move |(category, selection): (Category, PickerSelection)| {
                    draft.write().add_subject(SubjectVm {
                        category,
                        human_id: selection.human_id,
                        id: String::new(),
                        kind_label: String::new(),
                    });
                },
            }
        }
    }
}

/// A kind select plus that kind's existing-record picker, emitting the picked `(category, selection)`.
/// The picker lives in a keyed child component so only the chosen kind's list is fetched — switching
/// kinds remounts the child rather than loading all four lists up front.
#[component]
fn SubjectChooser(save_label: String, onpick: EventHandler<(Category, PickerSelection)>) -> Element {
    // `try_consume_context` (not `use_context`): the create pane's SSR tests render this without an
    // `AppCtx`, and a picker with no workspace behind it has nothing to offer anyway.
    let Some(AppCtx::Ready(state)) = try_consume_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut kind_index = use_signal(|| 0_usize);
    let options: Vec<SelectChoice> = subject_categories()
        .iter()
        .enumerate()
        .map(|(index, category)| SelectChoice {
            value: index.to_string(),
            label: loc.subject_kind_label(*category),
        })
        .collect();
    let category = subject_categories()
        .get(kind_index())
        .copied()
        .unwrap_or(Category::People);
    rsx! {
        Select {
            label: loc.field_label("type"),
            name: "subject-kind".to_owned(),
            value: Some(kind_index().to_string()),
            options,
            onchange: move |event: FormEvent| kind_index.set(event.value().parse().unwrap_or(0)),
        }
        div { class: "stack",
            // The keyed picker must be the first node of its own block: Dioxus only honours `key` there,
            // and the key is what remounts the picker (dropping its option load) when the kind changes.
            SubjectPickField {
                key: "{category.id()}",
                category,
                save_label,
                onpick: move |selection: PickerSelection| onpick.call((category, selection)),
            }
        }
    }
}

/// One category's existing-record picker with its own Save, isolated in a component so its option load
/// is scoped to the chosen kind.
#[component]
fn SubjectPickField(category: Category, save_label: String, onpick: EventHandler<PickerSelection>) -> Element {
    let Some(AppCtx::Ready(state)) = try_consume_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let picker = use_existing_picker(
        services,
        category,
        loc.subject_kind_label(category),
        "subject".to_owned(),
        loc.picker_entity(category),
        Vec::new(),
    );
    let mut state_signal = picker.state;
    let disabled = picker.state.read().selection.is_none();
    let picked = picker.state.read().selection.clone();
    rsx! {
        {record_picker(loc, &picker)}
        Button {
            label: save_label,
            variant: ButtonVariant::Default,
            disabled,
            onclick: move |_| {
                if let Some(selection) = picked.clone() {
                    onpick.call(selection);
                    state_signal.write().clear();
                }
            },
        }
    }
}

/// Which research-note collection-row form (if any) the side panel is showing. The note's own scalar
/// record (argument · language) is edited in place via the sticky-header Edit, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchNoteEditForm {
    /// Name one more subject the argument is about.
    Subject,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected research note: header, tabs, editing side panel, toast.
#[component]
pub(crate) fn ResearchNoteDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let nav = use_context::<NavState>();
    let mut label_nav = nav;
    let active = use_detail_tab(Category::ResearchNotes, &human_id);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<ResearchNoteEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowResearchNote { human_id }).await }
    });

    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::ResearchNoteDetail(detail))) => {
            genealogy_ui::ResearchNoteDraft::from_detail(detail)
        }
        _ => genealogy_ui::ResearchNoteDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::ResearchNoteDraft>(Category::ResearchNotes, &human_id, &seed);

    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::ResearchNoteDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::ResearchNotes,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (ResearchNoteEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_research_note_edit(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });
    let tag_human = human_id.clone();
    let on_tag_remove = use_callback(move |tag_id: String| {
        on_submit.call((
            ResearchNoteEdit::Tag {
                human_id: tag_human.clone(),
                tag_id,
                remove: true,
            },
            ProvenanceDraft::default(),
        ));
    });
    let subject_human = human_id.clone();
    let on_subject_remove = use_callback(move |subject: SubjectVm| {
        on_submit.call((
            ResearchNoteEdit::RemoveSubject {
                human_id: subject_human.clone(),
                subject: subject.to_request(),
            },
            ProvenanceDraft::default(),
        ));
    });

    let record_services = services.clone();
    let record_nav = nav;
    let current_id = human_id.clone();
    let on_record_save = use_callback(
        move |(draft, prov): (genealogy_ui::ResearchNoteDraft, ProvenanceDraft)| {
            let services = record_services.clone();
            let edits = draft.edits_against(&record.seed.read());
            let current = current_id.clone();
            let saved = saved_label.clone();
            spawn(async move {
                let effective =
                    apply_record_edits(services, edits, prov, current.clone(), save_research_note_edit).await;
                finish_record_save(
                    effective,
                    Category::ResearchNotes,
                    &current,
                    record_nav,
                    reload,
                    toast,
                    &saved,
                );
            });
        },
    );

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log.
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::ResearchNoteDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            ResearchNoteEdit::UndoAssertion {
                human_id: undo_human.clone(),
                assertion_id,
            },
            ProvenanceDraft::default(),
        ));
    });
    use_record_undo(
        nav,
        Category::ResearchNotes,
        &human_id,
        undo_busy,
        undo_history,
        undo_notice,
        on_undo,
    );

    // The close/quit confirm's Save hands the record back to this pane (issue #240): it runs the same
    // whole-record commit the header's Save does, so ⌘W/⌘Q can keep the edit instead of discarding it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_record_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(Category::ResearchNotes, Some(&human_id), record, save_now);

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::ResearchNoteDetail(detail))) => research_note_detail(
            &state,
            detail,
            ResearchNotePane {
                active,
                side_edit: editing,
                record,
            },
            ResearchNoteCallbacks {
                on_submit,
                on_record_save,
                on_undo,
                on_tag_remove,
                on_subject_remove,
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
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::DataQuality(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Pedigree(_)
            | IntentOutcome::Relationship(_)
            | IntentOutcome::DuplicateCandidates(_)
            | IntentOutcome::MergeCompare(_)
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

/// The signals a research note's detail threads to its tabs.
#[derive(Clone, Copy)]
struct ResearchNotePane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<ResearchNoteEditForm>>,
    /// The whole-record (argument · language) edit state.
    record: RecordEditState<genealogy_ui::ResearchNoteDraft>,
}

/// The commit callbacks a research note's detail wires in.
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct ResearchNoteCallbacks {
    /// Commits one [`ResearchNoteEdit`] command (a collection row).
    on_submit: Callback<(ResearchNoteEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::ResearchNoteDraft, ProvenanceDraft)>,
    /// Retracts an assertion by id from the History tab.
    on_undo: Callback<String>,
    /// Untags a tag by id from the Tags tab.
    on_tag_remove: Callback<String>,
    /// Stops naming a subject from the Subjects tab.
    on_subject_remove: Callback<SubjectVm>,
}

/// Renders a loaded research note's detail container: header (with the sticky-header record
/// Edit/Cancel/Save), the tab strip, the active tab, and the collection-row side panel.
fn research_note_detail(
    state: &AppState,
    detail: &ResearchNoteDetail,
    pane: ResearchNotePane,
    callbacks: ResearchNoteCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let ResearchNotePane {
        active,
        side_edit: editing,
        record,
    } = pane;
    let on_submit = callbacks.on_submit;
    let tabs = research_note_tabs(detail, loc);
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
            avatar: "🧾".to_owned(),
            extras: research_note_restriction_toggles(loc, detail, on_submit, human_id),
            actions: record_head_actions(&labels, record, rsx! {}, callbacks.on_record_save),
            tabs: tab_items,
            active,
            {research_note_tab_content(state, detail, active_id, editing, record, callbacks)}
        }
        {research_note_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a research note.
fn research_note_restriction_toggles(
    loc: &Localizer,
    detail: &ResearchNoteDetail,
    on_submit: Callback<(ResearchNoteEdit, ProvenanceDraft)>,
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
                on_submit.call((ResearchNoteEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one research-note detail tab, with its contextual add affordances.
fn research_note_tab_content(
    state: &AppState,
    detail: &ResearchNoteDetail,
    tab_id: &str,
    editing: Signal<Option<ResearchNoteEditForm>>,
    record: RecordEditState<genealogy_ui::ResearchNoteDraft>,
    callbacks: ResearchNoteCallbacks,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "subjects" => tab_with_add(
            loc,
            "add-subject",
            editing,
            ResearchNoteEditForm::Subject,
            rsx! {
                {research_note_subjects_table(loc, &detail.subjects, callbacks.on_subject_remove)}
            },
        ),
        "tags" => tags_panel(
            loc,
            &detail.tags,
            editing,
            ResearchNoteEditForm::Tag,
            callbacks.on_tag_remove,
        ),
        "history" => history_panel(loc, &detail.history, Some(callbacks.on_undo)),
        _ => research_note_content_tab(loc, record),
    }
}

/// The Content tab, read-first (`record-editing.html` §1/§2): the note's argument and language as read
/// boxes; entering edit mode (via the sticky-header Edit) swaps in the inputs and, while dirty, the
/// provenance block.
pub fn research_note_content_tab(loc: &Localizer, record: RecordEditState<genealogy_ui::ResearchNoteDraft>) -> Element {
    rsx! {
        div { class: "section-note", "{loc.research_note_content_note()}" }
        {research_note_record_fields(loc, record)}
        {record_edit_provenance(loc, record)}
    }
}

/// The Subjects tab: a row per record the argument is about, each linking to that record and carrying a
/// Remove that stops naming it (the core refuses the last one — ADR 0028 §2).
pub fn research_note_subjects_table(loc: &Localizer, subjects: &[SubjectVm], onremove: Callback<SubjectVm>) -> Element {
    if subjects.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    let remove_title = loc.action_title("remove-subject");
    rsx! {
        div { class: "section-note", "{loc.research_note_subjects_note()}" }
        Table {
            caption: loc.tab_label("subjects"),
            headers: vec![
                loc.field_label("object"),
                loc.field_label("type"),
                loc.field_label("id"),
                String::new(),
            ],
            for subject in subjects.iter() {
                tr {
                    td {
                        RecordLink {
                            category: subject.category,
                            human_id: subject.human_id.clone(),
                            label: subject.human_id.clone(),
                        }
                    }
                    td { Chip { label: subject.kind_label.clone() } }
                    td { class: "muted mono", "{subject.human_id}" }
                    td { class: "row-actions",
                        {
                            let row = subject.clone();
                            let label = subject.human_id.clone();
                            let remove_title = remove_title.clone();
                            rsx! {
                                Button {
                                    label: loc.action_label("remove"),
                                    variant: ButtonVariant::Ghost,
                                    small: true,
                                    title: remove_title,
                                    aria_label: loc.action_remove_row(&label),
                                    onclick: move |_| onremove.call(row.clone()),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The "Research notes" reverse-lookup tab shared by the Person, Family, Event, and Place detail
/// screens (ADR 0028 §5): the arguments written about this record, each linking to its own detail, plus
/// a "New research note" action that opens a create draft with this record pre-seeded as the subject.
///
/// A component (not a pure fn like the other shared tabs) because its Add opens a **draft tab** rather
/// than a side panel, so it needs [`NavState`] — which it resolves from context, the [`RecordLink`]
/// idiom, keeping the four screens' tab dispatchers free of extra callback plumbing. Under bare SSR
/// (no `NavState`) the button renders and does nothing.
#[component]
pub fn ResearchNotesTab(category: Category, human_id: String, rows: Vec<RowVm>) -> Element {
    let Some(AppCtx::Ready(state)) = try_consume_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let nav = try_consume_context::<NavState>();
    let subject = human_id.clone();
    rsx! {
        div { class: "tab-actions",
            Button {
                label: loc.action_label("new-research-note"),
                variant: ButtonVariant::Default,
                onclick: move |_| {
                    if let Some(mut nav) = nav {
                        nav.open_research_note_about(category, subject.clone());
                    }
                },
            }
        }
        {research_notes_table(loc, &rows)}
    }
}

/// The reverse-lookup tab's table: one row per argument written about the record, linking to its detail.
/// A pure fn so the SSR tests render it without `AppCtx`.
pub fn research_notes_table(loc: &Localizer, rows: &[RowVm]) -> Element {
    rsx! {
        div { class: "section-note", "{loc.research_notes_about_note()}" }
        if rows.is_empty() {
            EmptyState { message: loc.tab_empty() }
        } else {
            Table {
                caption: loc.tab_label("research-notes"),
                headers: vec![loc.field_label("title"), loc.field_label("id")],
                for row in rows.iter() {
                    tr {
                        td {
                            RecordLink {
                                category: Category::ResearchNotes,
                                human_id: row.id.clone(),
                                label: row.title.clone(),
                            }
                        }
                        td { class: "muted mono", "{row.id}" }
                    }
                }
            }
        }
    }
}

/// The research-note collection-row editing side panel: the open [`ResearchNoteEditForm`]'s form, or
/// nothing. The note's scalar record is edited in place via the sticky header.
fn research_note_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<ResearchNoteEditForm>>,
    on_submit: Callback<(ResearchNoteEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match &form {
        ResearchNoteEditForm::Subject => loc.panel_title("add-subject"),
        ResearchNoteEditForm::Tag => loc.action_label("add-tag"),
    };
    let human_id = human_id.to_owned();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |()| editing.set(None),
            footer: rsx! {},
            {match form {
                ResearchNoteEditForm::Subject => rsx! { ResearchNoteSubjectForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                ResearchNoteEditForm::Tag => rsx! { ResearchNoteTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Add subject" form: a kind select + that kind's record picker + the provenance block →
/// [`ResearchNoteEdit::AddSubject`].
#[component]
fn ResearchNoteSubjectForm(human_id: String, onsubmit: EventHandler<(ResearchNoteEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let prov = use_signal(ProvenanceDraft::default);
    rsx! {
        SubjectChooser {
            save_label: loc.action_label("save"),
            onpick: move |(category, selection): (Category, PickerSelection)| {
                onsubmit.call((
                    ResearchNoteEdit::AddSubject {
                        human_id: human_id.clone(),
                        subject: genealogy_ui::SubjectRequest { category, human_id: selection.human_id },
                    },
                    prov(),
                ));
            },
        }
        {provenance_block(loc, prov)}
    }
}

/// The research-note "Add tag" form: a picker of existing tags by name → [`ResearchNoteEdit::Tag`].
#[component]
fn ResearchNoteTagForm(human_id: String, onsubmit: EventHandler<(ResearchNoteEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((ResearchNoteEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}
