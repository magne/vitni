use super::prelude::*;
use genealogy_app::RepositoryType;
// The collection view-model the prelude doesn't re-export; it seeds the per-row URL edit. `AddressVm`
// (the address card/form seed) is a shared prelude re-export.
use genealogy_ui::RepositoryUrlVm;

/// The create-mode repository record (`record-editing.html` §6): an empty [`RepositoryDraft`] rendered
/// in edit mode on the shared record frame, with Cancel/Save in the sticky header. Save commits the
/// whole repository; Cancel discards.
#[component]
pub fn RepositoryCreateRecord(draft_id: DraftId) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::RepositoryDraft>(Category::Repositories, draft_id);
    let created_label = loc.action_label("created");
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::RepositoryDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let services = services.clone();
        let created = created_label.clone();
        spawn(async move {
            let committed = commit_repository_change_set(services, request, prov).await;
            finish_draft_commit(
                committed,
                DraftCommit::new(Category::Repositories, draft_id, &draft, created),
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
    use_save_on_request(EditKey::draft(Category::Repositories, draft_id), record, save_now);
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
        &loc.repository_new_title(),
        &loc.record_draft_badge(),
        actions,
        rsx! {
            {repository_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        },
    )
}

/// The repository's scalar record fields (id · type · name), read-first: read boxes in view mode,
/// inputs with per-field reset in edit mode (`record-editing.html` §2/§3). A pure fn (the edit state's
/// signals passed in) so the create pane and the SSR tests render it without `AppCtx`. Shared by view,
/// edit, and create.
pub fn repository_record_fields(loc: &Localizer, record: RecordEditState<genealogy_ui::RepositoryDraft>) -> Element {
    let editing = record.editing.read().to_owned();
    let mut draft = record.draft;
    let seed = record.seed;
    let types = repository_type_choices();
    let mut options = vec![SelectChoice {
        value: String::new(),
        label: loc.record_unset(),
    }];
    for (index, repository_type) in types.iter().enumerate() {
        options.push(SelectChoice {
            value: index.to_string(),
            label: loc.repository_type_label(repository_type),
        });
    }
    let index_of = |repository_type: &Option<genealogy_app::RepositoryType>| {
        repository_type
            .as_ref()
            .and_then(|chosen| repository_type_choices().iter().position(|t| t == chosen))
            .map_or_else(String::new, |index| index.to_string())
    };
    let type_value = index_of(&draft().repository_type);
    let type_original = index_of(&seed.read().repository_type);
    let name_value = draft().name.clone();
    let name_original = seed.read().name.clone();
    let id_value = draft().human_id.clone();
    let id_original = seed.read().human_id.clone();
    rsx! {
        Card { title: loc.section_label("repository"),
            div { class: "stack",
                DraftText {
                    label: loc.field_label("id"),
                    name: "repository-id".to_owned(),
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
                    name: "repository-type".to_owned(),
                    editing,
                    value: type_value,
                    original: type_original,
                    reset_label: loc.action_reset_field(&loc.field_label("type")),
                    options,
                    onchange: move |value: String| {
                        let types = repository_type_choices();
                        draft.write().repository_type = value.parse::<usize>().ok().and_then(|index| types.get(index).cloned());
                    },
                    onreset: move |()| {
                        let value = seed.read().repository_type.clone();
                        draft.write().repository_type = value;
                    },
                }
                DraftText {
                    label: loc.field_label("name"),
                    name: "repository-name".to_owned(),
                    editing,
                    value: name_value,
                    original: name_original,
                    reset_label: loc.action_reset_field(&loc.field_label("name")),
                    oninput: move |value: String| draft.write().name = value,
                    onreset: move |()| {
                        let value = seed.read().name.clone();
                        draft.write().name = value;
                    },
                }
            }
        }
    }
}

/// Which repository collection-row edit form (if any) the side panel is showing. The repository's own
/// scalar record (id · type · name) is edited in place via the sticky-header Edit, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryEditForm {
    /// Postal address — `None` adds a new one, `Some(card)` edits (supersedes) an existing one.
    Address(Option<AddressVm>),
    /// Contact URL — `None` adds a new one, `Some(row)` edits (supersedes) an existing one.
    Url(Option<RepositoryUrlVm>),
    /// Link a source (by `human_id`) held here, with a call number + medium.
    Source,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected repository: header, related-item tabs, editing side panel, toast.
#[component]
pub(crate) fn RepositoryDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let nav = use_context::<NavState>();
    let mut label_nav = nav;
    let active = use_detail_tab(Category::Repositories, &human_id);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<RepositoryEditForm>);
    let mut retract = use_signal(|| None::<RetractTarget>);
    let mut retract_reason = use_signal(String::new);
    let saved_label = state.data_loc().action_label("saved");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowRepository { human_id }).await }
    });

    // The shared whole-record edit state, seeded from the loaded repository (empty until it loads);
    // it reseeds on a save reload while not editing (`use_record_edit`).
    let seed = match &*data.read_unchecked() {
        Some(ScreenData::Loaded(IntentOutcome::RepositoryDetail(detail))) => {
            genealogy_ui::RepositoryDraft::from_detail(detail)
        }
        _ => genealogy_ui::RepositoryDraft::new(),
    };
    let record = use_record_edit::<genealogy_ui::RepositoryDraft>(Category::Repositories, &human_id, &seed);

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the
    // repository's name (`tab_label` falls back to `human_id` when the name is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::RepositoryDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        label_nav.set_record_label(
            Category::Repositories,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let mut submit_nav = nav;
    let on_submit = use_callback(move |(edit, prov): (RepositoryEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_repository_edit(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    submit_nav.notify(saved);
                }
                Err(message) => submit_nav.notify_error(message),
            }
        });
    });

    // A per-row Retract/Detach opens the shared retract panel; confirming dispatches an
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
    let on_edit_open = use_callback(move |form: RepositoryEditForm| editing_for_open.set(Some(form)));
    let repository_tag_human = human_id.clone();
    let on_tag_remove = use_callback(move |tag_id: String| {
        on_submit.call((
            RepositoryEdit::Tag {
                human_id: repository_tag_human.clone(),
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
            let edit = RepositoryEdit::UndoAssertion { human_id, assertion_id };
            match save_repository_edit(services, edit, prov).await {
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::RepositoryDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_repository_edit).await;
            finish_record_save(effective, Category::Repositories, &current, record_nav, reload, &saved);
        });
    });

    // ⌘Z retracts the newest undoable assertion of this record's loaded change log (WP5).
    let undo_history = use_memo(move || match &*data.read() {
        Some(ScreenData::Loaded(IntentOutcome::RepositoryDetail(detail))) => detail.history.clone(),
        _ => Vec::new(),
    });
    let undo_busy = use_memo(move || editing.read().is_some() || *record.editing.read() || retract.read().is_some());
    let undo_notice = chrome.kbd_nothing_to_undo();
    let undo_human = human_id.clone();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((
            RepositoryEdit::UndoAssertion {
                human_id: undo_human.clone(),
                assertion_id,
            },
            ProvenanceDraft::default(),
        ));
    });
    use_record_undo(
        nav,
        Category::Repositories,
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
    use_save_on_request(EditKey::saved(Category::Repositories, &human_id), record, save_now);

    match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::RepositoryDetail(detail))) => repository_detail(
            &state,
            detail,
            RepositoryPane {
                active,
                side_edit: editing,
                record,
                retract,
                retract_reason,
            },
            RepositoryCallbacks {
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

/// The signals a repository's detail threads to its tabs: the active tab, the collection-row side
/// panel, and the whole-record edit state.
#[derive(Clone, Copy)]
struct RepositoryPane {
    /// The active tab index.
    active: Signal<usize>,
    /// Which collection-row side panel (if any) is open.
    side_edit: Signal<Option<RepositoryEditForm>>,
    /// The whole-record (id · type · name) edit state.
    record: RecordEditState<genealogy_ui::RepositoryDraft>,
    /// The row being retracted/detached, if the retract panel is open.
    retract: Signal<Option<RetractTarget>>,
    /// The rationale typed into the open retract panel.
    retract_reason: Signal<String>,
}

/// The commit callbacks a repository's detail wires in: one-command collection edits, the whole-record
/// save (the scalar edit via `edits_against`), and the per-row correction (edit-open + retract-confirm).
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "event-handler fields conventionally share the on_ prefix"
)]
struct RepositoryCallbacks {
    /// Commits one [`RepositoryEdit`] command (a collection row).
    on_submit: Callback<(RepositoryEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::RepositoryDraft, ProvenanceDraft)>,
    /// Opens the retract panel for a row: `(assertion_id, label, detach)`.
    on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches `UndoAssertion` with the typed rationale.
    on_retract_confirm: Callback<()>,
    /// Opens a collection-row edit form pre-filled from the row (Save supersedes by `AssertionId`).
    on_edit_open: Callback<RepositoryEditForm>,
    /// Retracts an assertion by id from the History tab (dispatches `UndoAssertion`).
    on_undo: Callback<String>,
    /// Untags a tag by id from the Tags tab (dispatches `Tag { remove: true }`).
    on_tag_remove: Callback<String>,
}

/// Renders a loaded repository's detail container: header (with the sticky-header record
/// Edit/Cancel/Save), the tab strip, the active tab, and the collection-row side panel.
fn repository_detail(
    state: &AppState,
    detail: &RepositoryDetail,
    pane: RepositoryPane,
    callbacks: RepositoryCallbacks,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let RepositoryPane {
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
    let tabs = repository_tabs(detail, loc);
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
            avatar: "🏛".to_owned(),
            extras: repository_restriction_toggles(loc, detail, on_submit, human_id),
            actions: record_head_actions(&labels, record, rsx! {}, callbacks.on_record_save),
            tabs: tab_items,
            active,
            {repository_tab_content(state, detail, active_id, editing, record, on_retract, on_edit_open, on_undo, on_tag_remove)}
        }
        {repository_edit_panel(state, editing, on_submit, human_id)}
        {retract_side_panel(loc, retract, retract_reason, on_retract_confirm, "detach-note")}
    }
}

/// The interactive privacy-restriction toggles for a repository (the mockup `resn-set`).
fn repository_restriction_toggles(
    loc: &Localizer,
    detail: &RepositoryDetail,
    on_submit: Callback<(RepositoryEdit, ProvenanceDraft)>,
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
                on_submit.call((RepositoryEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next }, ProvenanceDraft::default()));
            },
        }
    }
}

/// The content of one repository detail tab, with its contextual add/edit affordances.
#[expect(
    clippy::too_many_arguments,
    reason = "a tab dispatcher threads the pane's signals + callbacks"
)]
fn repository_tab_content(
    state: &AppState,
    detail: &RepositoryDetail,
    tab_id: &str,
    editing: Signal<Option<RepositoryEditForm>>,
    record: RecordEditState<genealogy_ui::RepositoryDraft>,
    on_retract: Callback<(String, String, bool)>,
    on_edit_open: Callback<RepositoryEditForm>,
    on_undo: Callback<String>,
    on_tag_remove: Callback<String>,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "addresses" => {
            let onedit =
                Callback::new(move |seed: AddressVm| on_edit_open.call(RepositoryEditForm::Address(Some(seed))));
            tab_with_add(
                loc,
                "add-address",
                editing,
                RepositoryEditForm::Address(None),
                rsx! {
                    {address_cards(loc, &detail.addresses, onedit, on_retract)}
                },
            )
        }
        "urls" => tab_with_add(
            loc,
            "add-url",
            editing,
            RepositoryEditForm::Url(None),
            rsx! {
                {repository_urls_table(loc, detail, on_edit_open, on_retract)}
            },
        ),
        "sources" => tab_with_add(
            loc,
            "link-source",
            editing,
            RepositoryEditForm::Source,
            rsx! {
                {repository_sources_table(loc, detail)}
            },
        ),
        "notes" => tab_with_add(
            loc,
            "attach-note",
            editing,
            RepositoryEditForm::Note,
            rsx! {
                {id_list(loc, &detail.notes, Some(on_retract))}
            },
        ),
        "tags" => tags_panel(loc, &detail.tags, editing, RepositoryEditForm::Tag, on_tag_remove),
        "history" => history_panel(loc, &detail.history, Some(on_undo)),
        _ => repository_overview(loc, detail, record),
    }
}

/// The Overview tab, read-first (`record-editing.html` §1/§2): the repository's scalar record (id ·
/// type · name) as read boxes plus the Primary-contact card. Entering edit mode (via the sticky-header
/// Edit) swaps the record fields to inputs and, while dirty, shows the provenance block; the contact
/// card is hidden in edit mode to keep the focus on the record being changed.
pub fn repository_overview(
    loc: &Localizer,
    detail: &RepositoryDetail,
    record: RecordEditState<genealogy_ui::RepositoryDraft>,
) -> Element {
    if record.editing.read().to_owned() {
        return rsx! {
            div { class: "section-note", "{loc.repository_overview_note()}" }
            {repository_record_fields(loc, record)}
            {record_edit_provenance(loc, record)}
        };
    }
    let primary = detail.addresses.first().map(|entry| &entry.address);
    rsx! {
        div { class: "section-note", "{loc.repository_overview_note()}" }
        div { class: "grid-2",
            {repository_record_fields(loc, record)}
            Card { title: loc.section_label("contact"),
                if let Some(address) = primary {
                    div { class: "stack",
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"street\")}" }
                            span { class: "grow", {address.lines.first().cloned().unwrap_or_else(|| "—".to_owned())} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"locality\")}" }
                            span { class: "grow", {address.locality.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"phone\")}" }
                            span { class: "grow mono", {address.phone.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"email\")}" }
                            span { class: "grow", {address.email.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                    }
                } else {
                    EmptyState { message: loc.tab_empty() }
                }
            }
        }
    }
}

/// The URLs tab: a row per recorded URL — type · link · description, plus a per-row Edit (supersedes
/// via [`RepositoryEdit::AddUrl`]) and Retract (retracts the URL assertion — it stays in History).
pub fn repository_urls_table(
    loc: &Localizer,
    detail: &RepositoryDetail,
    onedit: Callback<RepositoryEditForm>,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if detail.urls.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("addresses"),
            headers: vec![
                loc.field_label("type"),
                loc.field_label("url"),
                loc.field_label("description"),
                String::new(),
            ],
            for url in detail.urls.iter() {
                tr {
                    td {
                        if let Some(url_type) = url.url_type.clone() {
                            Chip { label: url_type }
                        } else {
                            span { class: "muted", "—" }
                        }
                    }
                    td { a { href: "{url.href}", "{url.href}" } }
                    td { class: "muted", {url.description.clone().unwrap_or_else(|| "—".to_owned())} }
                    {row_actions_cell(
                        loc,
                        &url.href,
                        Some((RepositoryEditForm::Url(Some(url.clone())), None)), None,
                        Some(RowRetract { assertion_id: url.assertion_id.clone(), button_label: "retract", title: "retract", detach: false }),
                        Some(onedit),
                        onretract)}
                }
            }
        }
    }
}

/// The Sources tab: a row per held source — source · call number · medium · citation count.
pub fn repository_sources_table(loc: &Localizer, detail: &RepositoryDetail) -> Element {
    if detail.sources.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            caption: loc.tab_label("sources"),
            headers: vec![
                loc.tab_label("sources"),
                loc.field_label("call-number"),
                loc.field_label("media-type"),
                loc.field_label("citations"),
            ],
            for held in detail.sources.iter() {
                tr {
                    td { "{held.title}" }
                    td { class: "mono", {held.call_number.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { Chip { label: held.media_type_label.clone() } }
                    td { {source_cue(loc, held.citation_count)} }
                }
            }
        }
    }
}

/// The repository editing side panel: renders the form for the open [`RepositoryEditForm`], or nothing.
fn repository_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<RepositoryEditForm>>,
    on_submit: Callback<(RepositoryEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match &form {
        RepositoryEditForm::Address(None) => loc.action_label("add-address"),
        RepositoryEditForm::Address(Some(_)) => loc.panel_title("edit-address"),
        RepositoryEditForm::Url(None) => loc.action_label("add-url"),
        RepositoryEditForm::Url(Some(_)) => loc.panel_title("edit-url"),
        RepositoryEditForm::Source => loc.action_label("link-source"),
        RepositoryEditForm::Note => loc.action_label("attach-note"),
        RepositoryEditForm::Tag => loc.action_label("add-tag"),
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
                RepositoryEditForm::Address(seed) => rsx! {
                    AddressForm {
                        seed,
                        onsubmit: move |(address, prov): (Address, ProvenanceDraft)| {
                            on_submit.call((RepositoryEdit::AddAddress { human_id: human_id.clone(), address }, prov));
                        },
                    }
                },
                RepositoryEditForm::Url(seed) => rsx! { RepositoryUrlForm { human_id, seed, onsubmit: move |edit| on_submit.call(edit) } },
                RepositoryEditForm::Source => rsx! { RepositoryLinkSourceForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                RepositoryEditForm::Note => rsx! { RepositoryNoteForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                RepositoryEditForm::Tag => rsx! { RepositoryTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Add URL" form → [`RepositoryEdit::AddUrl`]. `seed: None` adds a new URL; `Some(row)` edits an
/// existing one — the href + description are pre-filled, the row's type is preserved, and the draft's
/// `supersedes` is seeded with the row's assertion id so Save supersedes (replaces) rather than
/// appends (ADR 0004 §2).
#[component]
fn RepositoryUrlForm(
    human_id: String,
    seed: Option<RepositoryUrlVm>,
    onsubmit: EventHandler<(RepositoryEdit, ProvenanceDraft)>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut href = use_signal(|| seed.as_ref().map(|row| row.href.clone()).unwrap_or_default());
    let mut description = use_signal(|| {
        seed.as_ref()
            .and_then(|row| row.description.clone())
            .unwrap_or_default()
    });
    let url_type = seed.as_ref().and_then(|row| row.url_type.clone());
    let prov = use_signal(|| ProvenanceDraft {
        supersedes: seed.as_ref().map(|row| row.assertion_id.clone()),
        ..ProvenanceDraft::default()
    });
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("url"), name: "url".to_owned(), value: href(), oninput: move |event: FormEvent| href.set(event.value()) }
        Input { label: loc.field_label("description"), name: "description".to_owned(), value: description(), oninput: move |event: FormEvent| description.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let href = href();
                if href.trim().is_empty() {
                    return;
                }
                let description = description();
                let description = if description.trim().is_empty() { None } else { Some(description) };
                let url = Url { url_type: url_type.clone(), href, description };
                onsubmit.call((RepositoryEdit::AddUrl { human_id: human_id.clone(), url }, prov()));
            },
        }
    }
}

/// The "Link source" form: a source `human_id` + call number + medium → [`RepositoryEdit::LinkSource`].
#[component]
fn RepositoryLinkSourceForm(human_id: String, onsubmit: EventHandler<(RepositoryEdit, ProvenanceDraft)>) -> Element {
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
        Category::Sources,
        loc.tab_label("sources"),
        "source".to_owned(),
        loc.picker_entity(Category::Sources),
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
        let Some(source_id) = picker_selection_id(&picker_for_save) else {
            return;
        };
        let media_type = source_media_type_choices()
            .get(media())
            .cloned()
            .unwrap_or(SourceMediaType::Book);
        let call = call_number();
        let call_number = if call.trim().is_empty() { None } else { Some(call) };
        onsubmit.call((
            RepositoryEdit::LinkSource {
                human_id: human_id.clone(),
                source_id,
                call_number,
                media_type,
            },
            prov(),
        ));
    });
    attach_picker_form(loc, &picker, extra, prov, onsave)
}

/// The "Attach note by id" form → [`RepositoryEdit::AttachNote`].
#[component]
fn RepositoryNoteForm(human_id: String, onsubmit: EventHandler<(RepositoryEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let picker = use_existing_picker(
        services,
        Category::Notes,
        loc.field_label("note"),
        "note".to_owned(),
        loc.picker_entity(Category::Notes),
        Vec::new(),
    );
    let prov = use_signal(ProvenanceDraft::default);
    let picker_for_save = picker.clone();
    let onsave = use_callback(move |()| {
        let Some(id) = picker_selection_id(&picker_for_save) else {
            return;
        };
        onsubmit.call((
            RepositoryEdit::AttachNote {
                human_id: human_id.clone(),
                note_id: id,
            },
            prov(),
        ));
    });
    attach_picker_form(loc, &picker, rsx! {}, prov, onsave)
}

/// The repository "Add tag" form: a picker of existing tags by name → [`RepositoryEdit::Tag`].
#[component]
fn RepositoryTagForm(human_id: String, onsubmit: EventHandler<(RepositoryEdit, ProvenanceDraft)>) -> Element {
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
                        onsubmit.call((RepositoryEdit::Tag { human_id: human_id.clone(), tag_id, remove: false }, prov()));
                    },
                }
            }
        }
    }
}

/// The repository types offered by the type picker.
fn repository_type_choices() -> [RepositoryType; 7] {
    [
        RepositoryType::Library,
        RepositoryType::Archive,
        RepositoryType::Church,
        RepositoryType::Cemetery,
        RepositoryType::Museum,
        RepositoryType::Website,
        RepositoryType::Collection,
    ]
}
