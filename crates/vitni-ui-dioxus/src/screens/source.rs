// The source row view-models the prelude doesn't re-export; they seed the per-row repository/attribute edits.
use vitni_ui::{RepositoryLinkVm, SourceAttributeVm};

use super::prelude::*;

/// The create-mode source record: an uncommitted [`SourceDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Save commits the whole source through the change-set;
/// Cancel drops the draft. The provenance block above Save carries the operator's why/confidence/
/// citations onto every emitted assertion (§5b).
#[component]
pub fn SourceCreateRecord(draft_id: DraftId) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<vitni_ui::SourceDraft>(Category::Sources, draft_id);
    let created_label = loc.action_label("created");
    let on_save = use_callback(move |(draft, prov): (vitni_ui::SourceDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let services = services.clone();
        let created = created_label.clone();
        spawn(async move {
            let committed = commit_source_change_set(services, request, prov).await;
            finish_draft_commit(
                committed,
                DraftCommit::new(Category::Sources, draft_id, &draft, created),
                nav,
            );
        });
    });
    // The close/quit confirm's Save runs this same commit (issue #240), so a ⌘W/⌘Q over a half-filled
    // create form can keep the draft instead of losing it.
    let save_now = use_callback(move |()| {
        if record.can_save() {
            on_save.call((record.draft.read().clone(), record.prov.read().clone()));
        }
    });
    use_save_on_request(EditKey::draft(Category::Sources, draft_id), record, save_now);
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| nav.cancel_draft(draft_id) }
        Button {
            label: loc.action_label("save"),
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| save_now.call(()),
        }
    };
    create_record_frame(
        &loc.source_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {source_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The source's scalar record fields (id · title · author · publication · abbreviation), read-first:
/// read boxes in view mode, inputs with per-field reset in edit mode (`record-editing.html` §2/§3). A
/// pure fn (the edit state's signals passed in) so the create pane and the SSR tests render it without
/// `AppCtx`. Shared by view, edit, and create.
pub fn source_record_fields(loc: &Localizer, record: RecordEditState<vitni_ui::SourceDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let field = |name: &'static str,
                 label: String,
                 value: String,
                 original: String,
                 set: fn(&mut vitni_ui::SourceDraft, String),
                 get: fn(&vitni_ui::SourceDraft) -> String| {
        rsx! {
            DraftText {
                label: label.clone(),
                name: name.to_owned(),
                editing,
                value,
                original,
                reset_label: loc.action_reset_field(&label),
                oninput: move |value: String| set(&mut draft.write(), value),
                onreset: move |()| {
                    let value = get(&seed.read());
                    set(&mut draft.write(), value);
                },
            }
        }
    };
    let current = draft();
    let committed = seed.read().clone();
    rsx! {
        Card { title: loc.section_label("bibliographic"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "source-id".to_owned(),
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
                {field("source-title", loc.field_label("title"), current.title.clone(), committed.title.clone(), |draft, value| draft.title = value, |draft| draft.title.clone())}
                {field("source-author", loc.field_label("author"), current.author.clone(), committed.author.clone(), |draft, value| draft.author = value, |draft| draft.author.clone())}
                {field("source-publication", loc.field_label("publication"), current.publication.clone(), committed.publication.clone(), |draft, value| draft.publication = value, |draft| draft.publication.clone())}
                {field("source-abbreviation", loc.field_label("abbreviation"), current.abbreviation.clone(), committed.abbreviation.clone(), |draft, value| draft.abbreviation = value, |draft| draft.abbreviation.clone())}
            }
        }
    }
}

/// Which source collection-row edit form (if any) the side panel is showing. The source's own scalar
/// record (id · title · author · publication · abbreviation) is edited in place via the sticky header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceEditForm {
    /// Link a repository — `None` adds a new link, `Some(row)` edits (supersedes) an existing one.
    Repository(Option<RepositoryLinkVm>),
    /// Assert a typed attribute — `None` adds a new one, `Some(row)` edits (supersedes) an existing one.
    Attribute(Option<SourceAttributeVm>),
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected source: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn SourceDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let nav = use_context::<NavState>();
    let mut label_nav = nav;
    let active = use_detail_tab(Category::Sources, &human_id);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<SourceEditForm>);
    let mut retract = use_signal(|| None::<RetractTarget>);
    let mut retract_reason = use_signal(String::new);
    let saved_label = state.data_loc().action_label("saved");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowSource { human_id }).await }
    });

    // The shared whole-record edit state, seeded from the loaded source (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::SourceDetail(detail))) => vitni_ui::SourceDraft::from_detail(detail),
        _ => vitni_ui::SourceDraft::new(),
    };
    let record = use_record_edit::<vitni_ui::SourceDraft>(Category::Sources, &human_id, &seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the source's
    // title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::SourceDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::Sources,
            &label_human_id,
            vitni_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let mut submit_nav = nav;
    let on_submit = use_callback(move |(edit, prov): (SourceEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_source_edit(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    submit_nav.notify(saved);
                }
                Err(message) => submit_nav.notify_error(message),
            }
        });
    });

    // A per-row Retract/Unlink/Detach opens the shared retract panel; confirming dispatches an
    // `UndoAssertion` carrying the typed rationale (the retract note stays in History — ADR 0004 §2).
    let on_retract = use_callback(move |(assertion_id, label, detach): (String, String, bool)| {
        retract_reason.set(String::new());
        retract.set(Some(RetractTarget {
            assertion_id,
            label,
            detach,
        }));
    });
    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: SourceEditForm| editing_for_open.set(Some(form)));
    let source_tag_human = human_id.clone();
    let on_tag_remove = use_callback(move |tag_id: String| {
        on_submit.call((
            SourceEdit::Tag {
                human_id: source_tag_human.clone(),
                tag_id,
                remove: true,
            },
            ProvenanceDraft::default(),
        ));
    });
    let retract_services = state.services().clone();
    let retract_human = human_id.clone();
    let retract_saved = saved_label.clone();
    let mut retract_nav = nav;
    let on_retract_confirm = use_callback(move |()| {
        let Some(RetractTarget { assertion_id, .. }) = retract() else {
            return;
        };
        let services = retract_services.clone();
        let human_id = retract_human.clone();
        let saved = retract_saved.clone();
        let prov = ProvenanceDraft {
            rationale: retract_reason(),
            ..ProvenanceDraft::default()
        };
        spawn(async move {
            let edit = SourceEdit::UndoAssertion { human_id, assertion_id };
            match save_source_edit(services, edit, prov).await {
                Ok(_) => {
                    retract.set(None);
                    reload += 1;
                    retract_nav.notify(saved);
                }
                Err(message) => retract_nav.notify_error(message),
            }
        });
    });

    let record_services = services.clone();
    let record_nav = nav;
    let current_id = human_id.clone();
    let on_record_save = use_callback(move |(draft, prov): (vitni_ui::SourceDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_source_edit).await;
            finish_record_save(effective, Category::Sources, &current, record_nav, reload, &saved);
        });
    });

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::SourceDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            SourceEdit::UndoAssertion {
                human_id: undo_human.clone(),
                assertion_id,
            },
            ProvenanceDraft::default(),
        ));
    });
    use_record_undo(
        nav,
        Category::Sources,
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
    use_save_on_request(EditKey::saved(Category::Sources, &human_id), record, save_now);

    // The Media tab's crop viewer: opening a card, and superseding its crop via `SetMediaRegion`.
    let media_viewing = use_signal(|| None::<MediaRefVm>);
    let on_view = use_callback(move |item: MediaRefVm| media_viewing.clone().set(Some(item)));
    let region_human = human_id.clone();
    let on_region = use_callback(
        move |(assertion_id, crop, caption): (String, Option<Rect>, Option<String>)| {
            on_submit.call((
                SourceEdit::SetMediaRegion {
                    human_id: region_human.clone(),
                    assertion_id,
                    crop,
                    caption,
                },
                ProvenanceDraft::default(),
            ));
        },
    );
    let media_state = MediaTabState {
        viewing: media_viewing,
        on_view,
        on_region,
    };

    match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::SourceDetail(detail))) => source_detail(
            &state,
            detail,
            SourcePane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            },
            &SourceCallbacks {
                on_submit,
                on_record_save,
                on_retract,
                on_retract_confirm,
                on_edit_open,
                on_undo,
                on_tag_remove,
                media_state,
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
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::DataQuality(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
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
    }
}

/// The signals a source's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit state.
#[derive(Clone, Copy)]
struct SourcePane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<SourceEditForm>>,
    /// The whole-record (id · title · author · publication · abbreviation) edit state.
    record: RecordEditState<vitni_ui::SourceDraft>,
    /// The row being retracted/detached, if the retract panel is open.
    retract: Signal<Option<RetractTarget>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a source's detail wires in: one-command collection edits, the whole-record
/// save (the scalar edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
struct SourceCallbacks {
    /// Commits one [`SourceEdit`] command (a collection row).
    on_submit: Callback<(SourceEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(vitni_ui::SourceDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<SourceEditForm>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Untags a tag by id from the Tags tab (dispatches `Tag { remove: true }`).
    on_tag_remove: Callback<String>,
    /// The Media tab's viewer state + crop-supersede wiring.
    media_state: MediaTabState,
}

/// Renders a loaded source's detail container: header (with the sticky-header record Edit/Cancel/Save),
/// the tab strip, the active tab, and the collection-row side panel.
fn source_detail(
    state: &AppState,
    detail: &SourceDetail,
    pane: SourcePane,
    callbacks: &SourceCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let SourcePane {
        active,
        side_edit: editing,
        record,
        retract,
        retract_reason,
    } = pane;
    let on_submit = callbacks.on_submit;
    let on_retract = callbacks.on_retract;
    let on_retract_confirm = callbacks.on_retract_confirm;
    let on_edit_open = callbacks.on_edit_open;
    let on_undo = callbacks.on_undo;
    let on_tag_remove = callbacks.on_tag_remove;
    let media_state = callbacks.media_state;
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
    let labels = RecordActionLabels::resolve(loc);
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            id_label: Some(detail.human_id.clone()),
            avatar: "📚".to_owned(),
            extras: source_restriction_toggles(loc, detail, on_submit, human_id),
            actions: record_head_actions(&labels, record, rsx! {}, callbacks.on_record_save),
            tabs: tab_items,
            active,
            {source_tab_content(state, detail, active_id, editing, record, on_retract, on_edit_open, on_undo, on_tag_remove, media_state)}
        }
        {source_edit_panel(state, editing, on_submit, human_id)}
        {retract_side_panel(loc, retract, retract_reason, on_retract_confirm, "detach-citation")}
    }
}

/// The interactive privacy-restriction toggles for a source (the mockup `resn-set`).
fn source_restriction_toggles(
    loc: &Localizer,
    detail: &SourceDetail,
    on_submit: Callback<(SourceEdit, ProvenanceDraft)>,
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
                on_submit.call((SourceEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one source detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn source_tab_content(
    state: &AppState,
    detail: &SourceDetail,
    tab_id: &str,
    editing: Signal<Option<SourceEditForm>>,
    record: RecordEditState<vitni_ui::SourceDraft>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<SourceEditForm>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<String>,
    media_state: MediaTabState,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "repositories" => tab_with_add(
            loc,
            "link-repository",
            editing,
            SourceEditForm::Repository(None),
            rsx! {
                {source_repositories_table(loc, detail, on_edit_open, on_retract)}
            },
        ),
        "citations" => rsx! {
            div { class: "section-note", "{loc.source_citations_note()}" }
            {source_citations_table(loc, &detail.citations)}
        },
        "attributes" => tab_with_add(
            loc,
            "add-attribute",
            editing,
            SourceEditForm::Attribute(None),
            rsx! {
                {source_attributes_table(loc, detail, on_edit_open, on_retract)}
            },
        ),
        "media" => tab_with_add(
            loc,
            "attach-media",
            editing,
            SourceEditForm::Media,
            rsx! {
                {media_tab(loc, &detail.media, Some(on_retract), media_state)}
            },
        ),
        "notes" => tab_with_add(
            loc,
            "attach-note",
            editing,
            SourceEditForm::Note,
            rsx! {
                {id_list(loc, &detail.notes, Some(on_retract))}
            },
        ),
        "tags" => tags_panel(loc, &detail.tags, editing, SourceEditForm::Tag, on_tag_remove),
        "history" => history_panel(loc, &detail.history, Some(on_undo)),
        _ => source_overview(loc, detail, record),
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): the source's scalar record (id · title ·
/// author · publication · abbreviation) as read boxes plus a Reliability card. Entering edit mode (via
/// the sticky-header Edit) swaps the record fields to inputs and, while dirty, shows the provenance
/// block; the reliability card is hidden in edit mode to keep the focus on the record being changed.
pub fn source_overview(
    loc: &Localizer,
    detail: &SourceDetail,
    record: RecordEditState<vitni_ui::SourceDraft>,
) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.source_overview_note()}" }
            {source_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        };
    }
    let reliability = &detail.reliability;
    rsx! {
        div { class: "section-note", "{loc.source_overview_note()}" }
        div { class: "grid-2",
            {source_record_fields(loc, record)}
            Card { title: loc.section_label("reliability"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"typical-confidence\")}" }
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
                        span { class: "grow", "{loc.record_count(reliability.record_count)}" }
                    }
                }
            }
        }
    }
}

/// The Repositories tab: a row per repository link with call number, medium, and surety, plus a
/// per-row Edit (supersedes via [`SourceEdit::LinkRepository`]) and Unlink (retracts the link
/// assertion — it stays in History).
pub fn source_repositories_table(
    loc: &Localizer,
    detail: &SourceDetail,
    onedit: Callback<SourceEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if detail.repositories.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("repositories"),
            headers: vec![
                loc.tab_label("repositories"),
                loc.field_label("call-number"),
                loc.field_label("media-type"),
                loc.field_label("confidence"),
                String::new(),
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
                    {row_actions_cell(
                        loc,
                        &link.name,
                        Some((SourceEditForm::Repository(Some(link.clone())), None)), None,
                        Some(RowRetract { assertion_id: link.assertion_id.clone(), button_label: "unlink", title: "unlink-repository", detach: false }),
                        Some(onedit),
                        onretract)}
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
            caption: loc.tab_label("citations"),
            headers: vec![
                loc.field_label("page"),
                loc.field_label("backs-record"),
                loc.field_label("confidence"),
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

/// The Attributes tab: a row per attribute with key, value, and the evidence-first source cue, plus a
/// per-row Edit (supersedes via [`SourceEdit::AddAttribute`]) and Retract (retracts the attribute
/// assertion — it stays in History).
pub fn source_attributes_table(
    loc: &Localizer,
    detail: &SourceDetail,
    onedit: Callback<SourceEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if detail.attributes.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("attributes"),
            headers: vec![
                loc.field_label("attribute-type"),
                loc.field_label("value"),
                String::new(),
            ],
            for attribute in detail.attributes.iter() {
                tr {
                    td { Chip { label: attribute.attribute_type.clone() } }
                    td { class: "mono", "{attribute.value}" }
                    {row_actions_cell(
                        loc,
                        &attribute.attribute_type,
                        Some((SourceEditForm::Attribute(Some(attribute.clone())), None)), None,
                        Some(RowRetract { assertion_id: attribute.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
        }
    }
}

/// The source editing side panel: renders the form for the open [`SourceEditForm`], or nothing.
fn source_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<SourceEditForm>>,
    on_submit: Callback<(SourceEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match &form {
        SourceEditForm::Repository(None) => loc.action_label("link-repository"),
        SourceEditForm::Repository(Some(_)) => loc.panel_title("edit-repository"),
        SourceEditForm::Attribute(None) => loc.action_label("add-attribute"),
        SourceEditForm::Attribute(Some(_)) => loc.panel_title("edit-attribute"),
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
            onclose: move |()| editing.set(None),
            footer: rsx! {},
            {match form {
                SourceEditForm::Repository(seed) => rsx! { SourceLinkRepositoryForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                SourceEditForm::Attribute(seed) => rsx! { SourceAttributeForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                SourceEditForm::Media => rsx! { SourceAttachForm { human_id, field: "media".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                SourceEditForm::Note => rsx! { SourceAttachForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                SourceEditForm::Tag => rsx! { SourceTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Link repository" form → [`SourceEdit::LinkRepository`]. `seed: None` links a new repository
/// over an existing-place picker; `Some(row)` edits an existing link — the repository is fixed (shown
/// as a link), the call number + medium are pre-filled, and the draft's `supersedes` is seeded with
/// the row's assertion id so Save supersedes (replaces) rather than appends (ADR 0004 §2).
#[component]
fn SourceLinkRepositoryForm(
    human_id: String,
    seed: Option<RepositoryLinkVm>,
    onsubmit: EventHandler<(SourceEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let media_types = source_media_type_choices();
    let options: Vec<SelectChoice> = media_types
        .iter()
        .enumerate()
        .map(|(position, media_type)| SelectChoice {
            value: position.to_string(),
            label: loc.source_media_type_label(media_type),
        })
        .collect();
    // Edit mode fixes the repository (only the link's call number/medium/provenance change); add mode
    // offers a picker.
    let fixed = seed
        .as_ref()
        .and_then(|row| row.human_id.clone().map(|id| (id, row.name.clone())));
    let seed_media = seed
        .as_ref()
        .and_then(|row| source_media_type_choices().iter().position(|m| *m == row.media_type))
        .unwrap_or(0);
    let picker = use_existing_picker(
        services,
        Category::Repositories,
        loc.tab_label("repositories"),
        "repository".to_owned(),
        loc.picker_entity(Category::Repositories),
        Vec::new(),
    );
    let mut call_number = use_signal(|| {
        seed.as_ref()
            .and_then(|row| row.call_number.clone())
            .unwrap_or_default()
    });
    let mut media = use_signal(|| seed_media);
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let extra = rsx! {
        Input {
            label: loc.field_label("call-number"),
            name: "call-number".to_owned(),
            value: call_number(),
            oninput: move |event: FormEvent| call_number.set(event.value()),
        }
        Select {
            label: loc.field_label("media-type"),
            name: "media-type".to_owned(),
            value: Some(seed_media.to_string()),
            options,
            onchange: move |event: FormEvent| media.set(event.value().parse::<usize>().unwrap_or(0)),
        }
    };
    let picker_for_save = picker.clone();
    let fixed_for_save = fixed.as_ref().map(|(id, _)| id.clone());
    let onsave = use_callback(move |()| {
        let Some(repository_id) = fixed_for_save.clone().or_else(|| picker_selection_id(&picker_for_save)) else {
            return;
        };
        let media_type = source_media_type_choices()
            .get(media())
            .cloned()
            .unwrap_or(SourceMediaType::Book);
        let call = call_number();
        let call_number = if call.trim().is_empty() { None } else { Some(call) };
        onsubmit.call((
            SourceEdit::LinkRepository {
                human_id: human_id.clone(),
                repository_id,
                call_number,
                media_type,
            },
            prov(),
        ));
    });
    if let Some((id, name)) = &fixed {
        rsx! {
            div { class: "field",
                label { "{loc.tab_label(\"repositories\")}" }
                RecordLink { category: Category::Repositories, human_id: id.clone(), label: name.clone() }
            }
            {extra}
            {provenance_block(loc, prov)}
            Button {
                label: loc.action_label("save"),
                variant: ButtonVariant::Primary,
                onclick: move |_| onsave.call(()),
            }
        }
    } else {
        attach_picker_form(loc, &picker, extra, prov, onsave)
    }
}

/// The "Add attribute" form → [`SourceEdit::AddAttribute`]. `seed: None` adds a new attribute;
/// `Some(row)` edits an existing one — the type + value are pre-filled and the draft's `supersedes`
/// is seeded with the row's assertion id so Save supersedes (replaces) rather than appends (ADR 0004 §2).
#[component]
fn SourceAttributeForm(
    human_id: String,
    seed: Option<SourceAttributeVm>,
    onsubmit: EventHandler<(SourceEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut attribute_type = use_signal(|| seed.as_ref().map(|row| row.attribute_type.clone()).unwrap_or_default());
    let mut value = use_signal(|| seed.as_ref().map(|row| row.value.clone()).unwrap_or_default());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_label("save");
    rsx! {
        Input {
            label: loc.field_label("attribute-type"),
            name: "attribute-type".to_owned(),
            value: attribute_type(),
            oninput: move |event: FormEvent| attribute_type.set(event.value()),
        }
        Input {
            label: loc.field_label("value"),
            name: "value".to_owned(),
            value: value(),
            oninput: move |event: FormEvent| value.set(event.value()),
        }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let attribute_type = attribute_type();
                if attribute_type.trim().is_empty() {
                    return;
                }
                onsubmit.call((SourceEdit::AddAttribute { human_id: human_id.clone(), attribute_type, value: value() }, prov()));
            },
        }
    }
}

/// The "Attach media/note by id" form → the matching [`SourceEdit`] attach variant.
#[component]
fn SourceAttachForm(human_id: String, field: String, onsubmit: EventHandler<(SourceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let category = if field == "note" {
        Category::Notes
    } else {
        Category::Media
    };
    let picker = use_existing_picker(
        services,
        category,
        loc.field_label(&field),
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
            "note" => SourceEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
            _ => SourceEdit::AttachMedia {
                human_id: human_id.clone(),
                media_id: id,
            },
        };
        onsubmit.call((edit, prov()));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}

/// The source "Add tag" form: a picker of existing tags by name → [`SourceEdit::Tag`].
#[component]
fn SourceTagForm(human_id: String, onsubmit: EventHandler<(SourceEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((SourceEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}
