use super::prelude::*;
use crate::screens::RecordDetail;

/// The source master-detail: a searchable list on the left, the selected source's detail on the right.
#[component]
pub fn SourceScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Sources.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().source_list_empty();
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
    // The top-bar `New` / new-record menu set `pending_create`; opening the draft here honours them
    // (nothing is created until Save — `record-editing.html` §6).
    use_effect(move || {
        if *nav.pending_create.read() == Some(Category::Sources) {
            creating.set(true);
            nav.pending_create.set(None);
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
                onselect: move |row: RowVm| {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::Sources,
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
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
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
            category: Category::Sources,
            human_id: id.clone(),
            label: if label.is_empty() { id } else { label },
        });
    });
    let detail = if creating() {
        rsx! {
            SourceCreateRecord {
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

/// The create-mode source record: an uncommitted [`SourceDraft`] rendered as the create form in the
/// detail pane (`record-editing.html` §6). Save commits the whole source through the change-set;
/// Cancel drops the draft. The provenance block above Save carries the operator's why/confidence/
/// citations onto every emitted assertion (§5b).
#[component]
fn SourceCreateRecord(
    oncreated: EventHandler<(String, String)>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::SourceDraft>();
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::SourceDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let label = request.title.clone().unwrap_or_default();
        let services = services.clone();
        spawn(async move {
            match commit_source_change_set(services, request, prov).await {
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
        {create_record_header(&loc.source_new_title(), &loc.record_draft_badge(), actions)}
        {source_record_fields(loc, record)}
        {record_edit_provenance(loc, record)}
    }
}

/// The source's scalar record fields (id · title · author · publication · abbreviation), read-first:
/// read boxes in view mode, inputs with per-field reset in edit mode (`record-editing.html` §2/§3). A
/// pure fn (the edit state's signals passed in) so the create pane and the SSR tests render it without
/// `AppCtx`. Shared by view, edit, and create.
pub fn source_record_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::SourceDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let field = |name: &'static str,
                 label: String,
                 value: String,
                 original: String,
                 set: fn(&mut genealogy_ui::SourceDraft, String),
                 get: fn(&genealogy_ui::SourceDraft) -> String| {
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
pub(crate) fn SourceDetailPane(human_id: String) -> Element {
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

    // The shared whole-record edit state, seeded from the loaded source (empty until it loads); it
    // reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::SourceDetail(detail))) => genealogy_ui::SourceDraft::from_detail(detail),
        _ => genealogy_ui::SourceDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::SourceDraft>(&seed);

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
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |(edit, prov): (SourceEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_source_edit(services, edit, prov).await {
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::SourceDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_source_edit).await;
            finish_record_save(
                effective,
                Category::Sources,
                &current,
                record_nav,
                reload,
                toast,
                &saved,
            );
        });
    });

    let body = match &*data.read_unchecked() {
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
            },
            SourceCallbacks {
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
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
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

/// The signals a source's detail threads to its tabs: the active tab, the collection-row side panel,
/// and the whole-record edit state.
#[derive(Clone, Copy)]
struct SourcePane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<SourceEditForm>>,
    /// The whole-record (id · title · author · publication · abbreviation) edit state.
    record: RecordEditState<genealogy_ui::SourceDraft>,
}

/// The two commit callbacks a source's detail wires in: one-command collection edits and the
/// whole-record save (the scalar edit via `edits_against`).
#[derive(Clone, Copy)]
struct SourceCallbacks {
    /// Commits one [`SourceEdit`] command (a collection row).
    on_submit: Callback<(SourceEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::SourceDraft, ProvenanceDraft)>,
}

/// Renders a loaded source's detail container: header (with the sticky-header record Edit/Cancel/Save),
/// the tab strip, the active tab, and the collection-row side panel.
fn source_detail(
    state: &AppState,
    detail: &SourceDetail,
    pane: SourcePane,
    callbacks: SourceCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let SourcePane {
        active,
        side_edit: editing,
        record,
    } = pane;
    let on_submit = callbacks.on_submit;
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
            {source_tab_content(state, detail, active_id, editing, record, on_submit, human_id)}
        }
        {source_edit_panel(state, editing, on_submit, human_id)}
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

/// The content of one source detail tab, with its contextual add affordances.
fn source_tab_content(
    state: &AppState,
    detail: &SourceDetail,
    tab_id: &str,
    mut editing: Signal<Option<SourceEditForm>>,
    record: RecordEditState<genealogy_ui::SourceDraft>,
    on_submit: Callback<(SourceEdit, ProvenanceDraft)>,
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
    record: RecordEditState<genealogy_ui::SourceDraft>,
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
    on_submit: Callback<(SourceEdit, ProvenanceDraft)>,
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
                                    onclick: move |_| on_submit.call((SourceEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
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
    on_submit: Callback<(SourceEdit, ProvenanceDraft)>,
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
                on_submit.call((SourceEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
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
fn SourceLinkRepositoryForm(human_id: String, onsubmit: EventHandler<(SourceEdit, ProvenanceDraft)>) -> Element {
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
    let picker = use_existing_picker(
        services,
        Category::Repositories,
        loc.tab_label("repositories"),
        "repository".to_owned(),
        loc.picker_entity(Category::Repositories),
        Vec::new(),
    );
    let mut call_number = use_signal(String::new);
    let mut media = use_signal(|| 0_usize);
    let prov = use_signal(ProvenanceDraft::default);
    let extra = rsx! {
        Input { label: loc.field_label("call-number"), name: "call-number".to_owned(), oninput: move |event: FormEvent| call_number.set(event.value()) }
        Select {
            label: loc.field_label("media-type"),
            name: "media-type".to_owned(),
            value: Some(0.to_string()),
            options,
            onchange: move |event: FormEvent| media.set(event.value().parse::<usize>().unwrap_or(0)),
        }
    };
    let picker_for_save = picker.clone();
    let onsave = use_callback(move |()| {
        let Some(repository_id) = picker_selection_id(&picker_for_save) else {
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
    attach_picker_form(loc, &picker, extra, prov, onsave)
}

/// The "Add attribute" form: a key + value → [`SourceEdit::AddAttribute`].
#[component]
fn SourceAttributeForm(human_id: String, onsubmit: EventHandler<(SourceEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut attribute_type = use_signal(String::new);
    let mut value = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("attribute-type"), name: "attribute-type".to_owned(), oninput: move |event: FormEvent| attribute_type.set(event.value()) }
        Input { label: loc.field_label("value"), name: "value".to_owned(), oninput: move |event: FormEvent| value.set(event.value()) }
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
