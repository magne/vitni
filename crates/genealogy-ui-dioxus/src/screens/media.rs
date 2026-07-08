use super::prelude::*;
use crate::screens::RecordDetail;

/// The media master-detail: a searchable list on the left, the selected media object on the right.
#[component]
pub fn MediaScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Media.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().media_list_empty();
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
        if *nav.pending_create.read() == Some(Category::Media) {
            creating.set(true);
            nav.pending_create.set(None);
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
                onselect: move |row: RowVm| {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::Media,
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
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::MediaDetail(_)
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
            category: Category::Media,
            human_id: id.clone(),
            label: if label.is_empty() { id } else { label },
        });
    });
    let detail = if creating() {
        rsx! {
            MediaCreateRecord {
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

/// The create-mode media record: an uncommitted [`MediaDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Save commits the whole media object; Cancel discards.
#[component]
fn MediaCreateRecord(
    oncreated: EventHandler<(String, String)>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
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
/// in view mode, inputs with per-field reset in edit mode (`record-editing.html` §2/§3). Checksum and
/// date are locked (§3) — disabled inputs seeded from the record, never editable here. A pure fn (the
/// edit state's signals passed in) so the create pane and SSR tests render it without `AppCtx`.
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
                DraftText {
                    label: loc.field_label("date"),
                    name: "media-date".to_owned(),
                    editing,
                    value: current.date.clone(),
                    original: committed.date.clone(),
                    reset_label: loc.action_reset_field(&loc.field_label("date")),
                    locked: true,
                    oninput: move |_: String| {},
                    onreset: move |()| {},
                }
            }
        }
    }
}

/// Which media collection-row edit form (if any) the side panel is showing. The media object's own
/// scalar record (id · paths · MIME) is edited in place via the sticky header; checksum and date are
/// locked (§3).
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
            },
            MediaCallbacks {
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
            | IntentOutcome::NoteDetail(_)
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
}

/// The two commit callbacks a media object's detail wires in: one-command collection edits and the
/// whole-record save (the scalar edit via `edits_against`).
#[derive(Clone, Copy)]
struct MediaCallbacks {
    /// Commits one [`MediaEdit`] command (a collection row).
    on_submit: Callback<(MediaEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::MediaDraft, ProvenanceDraft)>,
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
    } = pane;
    let on_submit = callbacks.on_submit;
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
            {media_tab_content(state, detail, active_id, editing, record, on_submit, human_id)}
        }
        {media_edit_panel(state, editing, on_submit, human_id)}
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

/// The content of one media detail tab, with its contextual add affordances.
fn media_tab_content(
    state: &AppState,
    detail: &MediaDetail,
    tab_id: &str,
    mut editing: Signal<Option<MediaEditForm>>,
    record: RecordEditState<genealogy_ui::MediaDraft>,
    on_submit: Callback<(MediaEdit, ProvenanceDraft)>,
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
            {id_list(loc, &detail.notes, None)}
        },
        "tags" => media_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => media_history_tab(loc, detail, on_submit, human_id),
        _ => media_overview(loc, detail, record),
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
    on_submit: Callback<(MediaEdit, ProvenanceDraft)>,
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
                                    onclick: move |_| on_submit.call((MediaEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
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
fn media_history_tab(
    loc: &Localizer,
    detail: &MediaDetail,
    on_submit: Callback<(MediaEdit, ProvenanceDraft)>,
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
                on_submit.call((MediaEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
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
