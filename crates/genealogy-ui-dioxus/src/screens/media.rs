use super::prelude::*;
// The media attribute row view-model (seeds the per-row attribute edit).
use genealogy_ui::MediaAttributeVm;

/// The create-mode media record: an uncommitted [`MediaDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Save commits the whole media object; Cancel discards.
#[component]
pub fn MediaCreateRecord() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::MediaDraft>();
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::MediaDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let label = request
            .file_path
            .clone()
            .or_else(|| request.web_path.clone())
            .unwrap_or_default();
        let services = services.clone();
        spawn(async move {
            match commit_media_change_set(services, request, prov).await {
                Ok(id) => nav.commit_draft(RecordRef {
                    category: Category::Media,
                    human_id: id.clone(),
                    label: if label.is_empty() { id } else { label },
                }),
                Err(message) => nav.notify(message),
            }
        });
    });
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| nav.cancel_draft(Category::Media) }
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
    create_record_frame(
        &loc.media_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {media_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The media object's scalar record fields (id · file path · web path · MIME), read-first: read boxes
/// in view mode, inputs with per-field reset in edit mode (`record-editing.html` §2/§3). Checksum is
/// locked (§3, disabled); the date is the structured `DraftDate` editor. A pure fn (the edit state's
/// signals passed in) so the create pane and SSR tests render it without `AppCtx`.
pub fn media_record_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::MediaDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let current = draft();
    let committed = seed.read().clone();
    rsx! {
        Card { title: loc.section_label("file"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "media-id".to_owned(),
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
                DraftText {
                    label: loc.field_label("file-path"),
                    name: "media-file-path".to_owned(),
                    editing,
                    value: current.file_path.clone(),
                    original: committed.file_path.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("file-path")),
                    mono: true,
                    oninput: move |value: String| draft.write().file_path = value,
                    onreset: move |()| {
                        let value = seed.read().file_path.clone();
                        draft.write().file_path = value;
                    },
                }
                DraftText {
                    label: loc.field_label("web-path"),
                    name: "media-web-path".to_owned(),
                    editing,
                    value: current.web_path.clone(),
                    original: committed.web_path.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("web-path")),
                    oninput: move |value: String| draft.write().web_path = value,
                    onreset: move |()| {
                        let value = seed.read().web_path.clone();
                        draft.write().web_path = value;
                    },
                }
                DraftText {
                    label: loc.field_label("mime"),
                    name: "media-mime".to_owned(),
                    editing,
                    value: current.mime.clone(),
                    original: committed.mime.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("mime")),
                    oninput: move |value: String| draft.write().mime = value,
                    onreset: move |()| {
                        let value = seed.read().mime.clone();
                        draft.write().mime = value;
                    },
                }
                DraftText {
                    label: loc.field_label("checksum"),
                    name: "media-checksum".to_owned(),
                    editing,
                    value: current.checksum.clone(),
                    original: committed.checksum.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("checksum")),
                    mono: true,
                    locked: true,
                    oninput: move |_: String| {},
                    onreset: move |()| {},
                }
                {date_draft_field(
                    loc,
                    "media-date",
                    editing,
                    current.date.clone(),
                    committed.date.clone(),
                    Callback::new(move |value: genealogy_ui::DateDraft| draft.write().date = value),
                    Callback::new(move |()| {
                        let value = seed.read().date.clone();
                        draft.write().date = value;
                    }),
                )}
            }
        }
    }
}

/// Which media collection-row edit form (if any) the side panel is showing. The media object's own
/// scalar record (id · paths · MIME) is edited in place via the sticky header; checksum and date are
/// locked (§3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaEditForm {
    /// Assert a typed attribute — `None` adds a new one, `Some(row)` edits (supersedes) an existing one.
    Attribute(Option<MediaAttributeVm>),
    /// Attach a citation by `human_id`.
    Citation,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected media object: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn MediaDetailPane(human_id: String) -> Element {
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
    let editing = use_signal(|| None::<MediaEditForm>);
    let mut retract = use_signal(|| None::<(String, String, bool)>);
    let mut retract_reason = use_signal(String::new);
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

    // The shared whole-record edit state, seeded from the loaded media object (empty until it loads);
    // it reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::MediaDetail(detail))) => genealogy_ui::MediaDraft::from_detail(detail),
        _ => genealogy_ui::MediaDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::MediaDraft>(&seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the media
    // object's title (`tab_label` falls back to `human_id` when the title is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::MediaDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::Media,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (MediaEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_media_edit(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    toast.set(Some(saved));
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    // A per-row Retract/Detach opens the shared retract panel; confirming dispatches an
    // `UndoAssertion` carrying the typed rationale (the retract note stays in History — ADR 0004 §2).
    let on_retract = use_callback(move |target: (String, String, bool)| {
        retract_reason.set(String::new());
        retract.set(Some(target));
    });
    let mut editing_for_open = editing;
    let on_edit_open = use_callback(move |form: MediaEditForm| editing_for_open.set(Some(form)));
    let media_tag_human = human_id.clone();
    let on_tag_remove = use_callback(move |tag_id: String| {
        on_submit.call((
            MediaEdit::Tag {
                human_id: media_tag_human.clone(),
                tag_id,
                remove: true,
            },
            ProvenanceDraft::default(),
        ));
    });
    let retract_services = services.clone();
    let retract_human = human_id.clone();
    let retract_saved = saved_label.clone();
    let on_retract_confirm = use_callback(move |()| {
        let Some((assertion_id, _, _)) = retract() else {
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
            let edit = MediaEdit::UndoAssertion { human_id, assertion_id };
            match save_media_edit(services, edit, prov).await {
                Ok(_) => {
                    retract.set(None);
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::MediaDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_media_edit).await;
            finish_record_save(effective, Category::Media, &current, record_nav, reload, toast, &saved);
        });
    });

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::MediaDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            MediaEdit::UndoAssertion {
                human_id: undo_human.clone(),
                assertion_id,
            },
            ProvenanceDraft::default(),
        ));
    });
    use_record_undo(nav, undo_busy, undo_history, undo_notice, on_undo);

    let body = match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::MediaDetail(detail))) => media_detail(
            &state,
            detail,
            MediaPane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            },
            MediaCallbacks {
                on_submit,
                on_record_save,
                on_retract,
                on_retract_confirm,
                on_edit_open,
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
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::DataQuality(_)
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

/// The signals a media object's detail threads to its tabs: the active tab, the collection-row side
/// panel, and the whole-record edit state.
#[derive(Clone, Copy)]
struct MediaPane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<MediaEditForm>>,
    /// The whole-record (id · paths · MIME) edit state.
    record: RecordEditState<genealogy_ui::MediaDraft>,
    /// The row being retracted/detached, if the retract panel is open: `(assertion_id, label, detach)`.
    retract: Signal<Option<(String, String, bool)>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a media object's detail wires in: one-command collection edits, the
/// whole-record save (the scalar edit via `edits_against`), and the per-row correction (edit-open +
/// retract-confirm).
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct MediaCallbacks {
    /// Commits one [`MediaEdit`] command (a collection row).
    on_submit: Callback<(MediaEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::MediaDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<MediaEditForm>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Untags a tag by id from the Tags tab (dispatches `Tag { remove: true }`).
    on_tag_remove: Callback<String>,
}

/// Renders a loaded media object's detail container: header (with the sticky-header record
/// Edit/Cancel/Save), the tab strip, the active tab, and the collection-row side panel.
fn media_detail(
    state: &AppState,
    detail: &MediaDetail,
    pane: MediaPane,
    callbacks: MediaCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let MediaPane {
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
    let labels = RecordActionLabels::resolve(loc);
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            id_label: Some(detail.human_id.clone()),
            avatar: "📷".to_owned(),
            extras: media_restriction_toggles(loc, detail, on_submit, human_id),
            actions: record_head_actions(&labels, record, rsx! {}, callbacks.on_record_save),
            tabs: tab_items,
            active,
            {media_tab_content(state, detail, active_id, editing, record, on_retract, on_edit_open, on_undo, on_tag_remove)}
        }
        {media_edit_panel(state, editing, on_submit, human_id)}
        {media_retract_panel(loc, retract, retract_reason, on_retract_confirm)}
    }
}

/// Renders the shared Retract/Detach side panel when a media collection row's action is armed. Reads
/// the armed `(assertion_id, label, detach)` and binds the rationale input; confirming dispatches
/// `UndoAssertion`. Closed (rendered empty) when nothing is armed. Never renders the target's
/// `AssertionId`.
fn media_retract_panel(
    loc: &Localizer,
    mut retract: Signal<Option<(String, String, bool)>>,
    reason: Signal<String>,
    on_confirm: Callback<()>,
) -> Element {
    let Some((_, label, detach)) = retract() else {
        return rsx! {};
    };
    let (title_id, button_id, note, accessible) = if detach {
        (
            "detach",
            "detach",
            loc.action_title("detach-citation"),
            loc.action_detach_row(&label),
        )
    } else {
        ("retract", "retract", loc.retract_note(), loc.action_retract_row(&label))
    };
    rsx! {
        SidePanel {
            title: loc.panel_title(title_id),
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| retract.set(None),
            footer: rsx! {},
            {retract_panel(loc, &loc.panel_title(title_id), &label, accessible, &note, loc.action_label(button_id), reason, on_confirm)}
        }
    }
}

/// The interactive privacy-restriction toggles for a media object (the mockup `resn-set`).
fn media_restriction_toggles(
    loc: &Localizer,
    detail: &MediaDetail,
    on_submit: Callback<(MediaEdit, ProvenanceDraft)>,
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
                on_submit.call((MediaEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one media detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn media_tab_content(
    state: &AppState,
    detail: &MediaDetail,
    tab_id: &str,
    mut editing: Signal<Option<MediaEditForm>>,
    record: RecordEditState<genealogy_ui::MediaDraft>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<MediaEditForm>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<String>,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "attributes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-attribute"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(MediaEditForm::Attribute(None))) }
            }
            {media_attributes_table(loc, &detail.attributes, on_edit_open, on_retract)}
        },
        "citations" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-citation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(MediaEditForm::Citation)) }
            }
            {media_citations_table(loc, &detail.citations, on_retract)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(MediaEditForm::Note)) }
            }
            {id_list(loc, &detail.notes, Some(on_retract))}
        },
        "tags" => tags_panel(loc, &detail.tags, editing, MediaEditForm::Tag, on_tag_remove),
        "history" => history_panel(loc, &detail.history, Some(on_undo)),
        _ => media_overview(loc, detail, record),
    }
}

/// The Attributes tab: a row per recorded `(type, value)` attribute, plus a per-row Edit (supersedes
/// via [`MediaEdit::AddAttribute`]) and Retract (retracts the attribute assertion — it stays in
/// History). Never renders the attribute's `AssertionId`.
pub fn media_attributes_table(
    loc: &Localizer,
    attributes: &[MediaAttributeVm],
    onedit: Callback<MediaEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if attributes.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![loc.field_label("attribute-type"), loc.field_label("value"), String::new()],
            for attribute in attributes.iter() {
                tr {
                    td { "{attribute.attribute_type}" }
                    td { class: "muted", "{attribute.value}" }
                    {row_actions_cell(
                        loc,
                        &attribute.attribute_type,
                        Some((MediaEditForm::Attribute(Some(attribute.clone())), None)), None,
                        Some(RowRetract { assertion_id: attribute.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
        }
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): a preview placeholder, the media
/// object's scalar record (id · paths · MIME, with checksum/date locked) as read boxes, and the "Used
/// by" card. Entering edit mode (via the sticky-header Edit) swaps the record fields to inputs and,
/// while dirty, shows the provenance block; the preview and "Used by" cards are hidden in edit mode.
pub fn media_overview(
    loc: &Localizer,
    detail: &MediaDetail,
    record: RecordEditState<genealogy_ui::MediaDraft>,
) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            {media_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        };
    }
    rsx! {
        Card { title: loc.media_preview(),
            div { class: "media-preview faint", aria_hidden: "true", "📷" }
            div { class: "muted", "{detail.title}" }
        }
        div { class: "grid-2",
            {media_record_fields(loc, record)}
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

/// The Citations tab: a row per citation with source, page, surety, and evidence axes, plus a per-row
/// Detach (retracts the attach assertion — it stays in History). A citation with no attach
/// `AssertionId` (shown as evidence, not an attachment) renders no Detach.
pub fn media_citations_table(
    loc: &Localizer,
    citations: &[CitationRefVm],
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("source"),
                loc.field_label("page"),
                loc.field_label("confidence"),
                loc.field_label("evidence"),
                String::new(),
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
                    {row_actions_cell::<MediaEditForm>(
                        loc,
                        &citation.human_id,
                        None, None,
                        citation.assertion_id.clone().map(|id| RowRetract { assertion_id: id, button_label: "detach", title: "detach-citation", detach: true }),
                        None,
                        onretract)}
                }
            }
        }
    }
}

/// The media editing side panel: renders the form for the open [`MediaEditForm`], or nothing.
fn media_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<MediaEditForm>>,
    on_submit: Callback<(MediaEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match &form {
        MediaEditForm::Attribute(None) => loc.action_label("add-attribute"),
        MediaEditForm::Attribute(Some(_)) => loc.panel_title("edit-attribute"),
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
                MediaEditForm::Attribute(seed) => rsx! { MediaAttributeForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                MediaEditForm::Citation => rsx! { MediaAttachForm { human_id, field: "citation".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                MediaEditForm::Note => rsx! { MediaAttachForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                MediaEditForm::Tag => rsx! { MediaTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Add attribute" form → [`MediaEdit::AddAttribute`]. `seed: None` adds a new attribute;
/// `Some(row)` edits an existing one — the type + value are pre-filled and the draft's `supersedes`
/// is seeded with the row's assertion id so Save supersedes (replaces) rather than appends (ADR 0004 §2).
#[component]
fn MediaAttributeForm(
    human_id: String,
    seed: Option<MediaAttributeVm>,
    onsubmit: EventHandler<(MediaEdit, ProvenanceDraft)>,
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
                onsubmit.call((MediaEdit::AddAttribute { human_id: human_id.clone(), attribute_type, value: value() }, prov()));
            },
        }
    }
}

/// The "Attach citation/note by id" form → the matching [`MediaEdit`] attach variant.
#[component]
fn MediaAttachForm(human_id: String, field: String, onsubmit: EventHandler<(MediaEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let category = if field == "citation" {
        Category::Citations
    } else {
        Category::Notes
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
            "citation" => MediaEdit::AttachCitation {
                human_id: human_id.clone(),
                citation_id: id,
            },
            _ => MediaEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
        };
        onsubmit.call((edit, prov()));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}

/// The media "Add tag" form: a picker of existing tags by name → [`MediaEdit::Tag`].
#[component]
fn MediaTagForm(human_id: String, onsubmit: EventHandler<(MediaEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((MediaEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}
