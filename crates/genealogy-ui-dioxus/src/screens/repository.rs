use super::prelude::*;
use crate::screens::RecordDetail;
use genealogy_app::RepositoryType;

/// The repository master-detail: a searchable list on the left, the selected repository on the right.
#[component]
pub fn RepositoryScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Repositories.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().repository_list_empty();
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
        if *nav.pending_create.read() == Some(Category::Repositories) {
            creating.set(true);
            nav.pending_create.set(None);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowRepositoryList).await }
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
                        category: Category::Repositories,
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
            category: Category::Repositories,
            human_id: id.clone(),
            label: if label.is_empty() { id } else { label },
        });
    });
    let detail = if creating() {
        rsx! {
            RepositoryCreateRecord {
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

/// The create-mode repository record (`record-editing.html` §6): an empty [`RepositoryDraft`] rendered
/// in edit mode on the shared record frame, with Cancel/Save in the sticky header. Save commits the
/// whole repository; Cancel discards.
#[component]
fn RepositoryCreateRecord(
    oncreated: EventHandler<(String, String)>,
    oncancel: EventHandler<()>,
    onerror: EventHandler<String>,
) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let services = state.services().clone();
    let record = use_record_create::<genealogy_ui::RepositoryDraft>();
    let on_save = use_callback(move |(draft, prov): (genealogy_ui::RepositoryDraft, ProvenanceDraft)| {
        let request = draft.to_request();
        let label = request.name.clone().unwrap_or_default();
        let services = services.clone();
        spawn(async move {
            match commit_repository_change_set(services, request, prov).await {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryEditForm {
    /// Add a postal address.
    Address,
    /// Add a contact URL.
    Url,
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
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<RepositoryEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

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
    let record = use_record_edit::<genealogy_ui::RepositoryDraft>(&seed);

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
    let on_submit = use_callback(move |(edit, prov): (RepositoryEdit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match save_repository_edit(services, edit, prov).await {
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
    let on_record_save = use_callback(move |(draft, prov): (genealogy_ui::RepositoryDraft, ProvenanceDraft)| {
        let services = record_services.clone();
        let edits = draft.edits_against(&record.seed.read());
        let current = current_id.clone();
        let saved = saved_label.clone();
        spawn(async move {
            let effective = apply_record_edits(services, edits, prov, current.clone(), save_repository_edit).await;
            finish_record_save(
                effective,
                Category::Repositories,
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
        Some(ScreenData::Loaded(IntentOutcome::RepositoryDetail(detail))) => repository_detail(
            &state,
            detail,
            RepositoryPane {
                active,
                side_edit: editing,
                record,
            },
            RepositoryCallbacks {
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
}

/// The two commit callbacks a repository's detail wires in: one-command collection edits and the
/// whole-record save (the scalar edit via `edits_against`).
#[derive(Clone, Copy)]
struct RepositoryCallbacks {
    /// Commits one [`RepositoryEdit`] command (a collection row).
    on_submit: Callback<(RepositoryEdit, ProvenanceDraft)>,
    /// Commits the buffered scalar record as a diff of `Set*` edits.
    on_record_save: Callback<(genealogy_ui::RepositoryDraft, ProvenanceDraft)>,
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
    } = pane;
    let on_submit = callbacks.on_submit;
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
            {repository_tab_content(state, detail, active_id, editing, record, on_submit, human_id)}
        }
        {repository_edit_panel(state, editing, on_submit, human_id)}
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

/// The content of one repository detail tab, with its contextual add affordances.
fn repository_tab_content(
    state: &AppState,
    detail: &RepositoryDetail,
    tab_id: &str,
    mut editing: Signal<Option<RepositoryEditForm>>,
    record: RecordEditState<genealogy_ui::RepositoryDraft>,
    on_submit: Callback<(RepositoryEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "addresses" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-address"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(RepositoryEditForm::Address)) }
            }
            {repository_addresses_cards(loc, detail)}
        },
        "urls" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-url"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(RepositoryEditForm::Url)) }
            }
            {repository_urls_table(loc, detail)}
        },
        "sources" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("link-source"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(RepositoryEditForm::Source)) }
            }
            {repository_sources_table(loc, detail)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(RepositoryEditForm::Note)) }
            }
            {id_list(loc, &detail.notes, None)}
        },
        "tags" => repository_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => repository_history_tab(loc, detail, on_submit, human_id),
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
    let primary = detail.addresses.first();
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

/// The Addresses tab: one card per recorded postal address.
pub fn repository_addresses_cards(loc: &Localizer, detail: &RepositoryDetail) -> Element {
    if detail.addresses.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "grid-2",
            for address in detail.addresses.iter() {
                Card { title: address.locality.clone().unwrap_or_else(|| loc.section_label("contact")),
                    div { class: "stack",
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"street\")}" }
                            span { class: "grow", {address.lines.join(", ")} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"region\")}" }
                            span { class: "grow", {address.region.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"postal-code\")}" }
                            span { class: "grow mono", {address.postal_code.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"country\")}" }
                            span { class: "grow", {address.country.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"phone\")}" }
                            span { class: "grow mono", {address.phone.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                        div { class: "fact-row",
                            span { class: "field-label", style: "width:90px;margin:0", "{loc.field_label(\"email\")}" }
                            span { class: "grow", {address.email.clone().unwrap_or_else(|| "—".to_owned())} }
                        }
                    }
                }
            }
        }
    }
}

/// The URLs tab: a row per recorded URL — type · link · description.
pub fn repository_urls_table(loc: &Localizer, detail: &RepositoryDetail) -> Element {
    if detail.urls.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("type"),
                loc.field_label("url"),
                loc.field_label("description"),
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

/// The repository Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn repository_tags_panel(
    loc: &Localizer,
    detail: &RepositoryDetail,
    mut editing: Signal<Option<RepositoryEditForm>>,
    on_submit: Callback<(RepositoryEdit, ProvenanceDraft)>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(RepositoryEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call((RepositoryEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }, ProvenanceDraft::default())),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The repository History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn repository_history_tab(
    loc: &Localizer,
    detail: &RepositoryDetail,
    on_submit: Callback<(RepositoryEdit, ProvenanceDraft)>,
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
                on_submit.call((RepositoryEdit::UndoAssertion { human_id: human_id.clone(), assertion_id }, ProvenanceDraft::default()));
            },
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
    let title = match form {
        RepositoryEditForm::Address => loc.action_label("add-address"),
        RepositoryEditForm::Url => loc.action_label("add-url"),
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
            onclose: move |_| editing.set(None),
            footer: rsx! {},
            {match form {
                RepositoryEditForm::Address => rsx! { RepositoryAddressForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                RepositoryEditForm::Url => rsx! { RepositoryUrlForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                RepositoryEditForm::Source => rsx! { RepositoryLinkSourceForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                RepositoryEditForm::Note => rsx! { RepositoryNoteForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                RepositoryEditForm::Tag => rsx! { RepositoryTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Add address" form: street/locality/region/postal/country/phone/email → [`RepositoryEdit::AddAddress`].
#[component]
fn RepositoryAddressForm(human_id: String, onsubmit: EventHandler<(RepositoryEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut street = use_signal(String::new);
    let mut locality = use_signal(String::new);
    let mut region = use_signal(String::new);
    let mut postal_code = use_signal(String::new);
    let mut country = use_signal(String::new);
    let mut phone = use_signal(String::new);
    let mut email = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("street"), name: "street".to_owned(), oninput: move |event: FormEvent| street.set(event.value()) }
        Input { label: loc.field_label("locality"), name: "locality".to_owned(), oninput: move |event: FormEvent| locality.set(event.value()) }
        Input { label: loc.field_label("region"), name: "region".to_owned(), oninput: move |event: FormEvent| region.set(event.value()) }
        Input { label: loc.field_label("postal-code"), name: "postal-code".to_owned(), oninput: move |event: FormEvent| postal_code.set(event.value()) }
        Input { label: loc.field_label("country"), name: "country".to_owned(), oninput: move |event: FormEvent| country.set(event.value()) }
        Input { label: loc.field_label("phone"), name: "phone".to_owned(), oninput: move |event: FormEvent| phone.set(event.value()) }
        Input { label: loc.field_label("email"), name: "email".to_owned(), oninput: move |event: FormEvent| email.set(event.value()) }
        {provenance_block(loc, prov)}
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let optional = |value: String| if value.trim().is_empty() { None } else { Some(value) };
                let street_value = street();
                let lines = if street_value.trim().is_empty() { Vec::new() } else { vec![street_value] };
                let address = Address {
                    lines,
                    locality: optional(locality()),
                    region: optional(region()),
                    postal_code: optional(postal_code()),
                    country: optional(country()),
                    phone: optional(phone()),
                    email: optional(email()),
                    fax: None,
                    www: None,
                    original_text: None,
                };
                onsubmit.call((RepositoryEdit::AddAddress { human_id: human_id.clone(), address }, prov()));
            },
        }
    }
}

/// The "Add URL" form: href + description → [`RepositoryEdit::AddUrl`].
#[component]
fn RepositoryUrlForm(human_id: String, onsubmit: EventHandler<(RepositoryEdit, ProvenanceDraft)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut href = use_signal(String::new);
    let mut description = use_signal(String::new);
    let prov = use_signal(ProvenanceDraft::default);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("url"), name: "url".to_owned(), oninput: move |event: FormEvent| href.set(event.value()) }
        Input { label: loc.field_label("description"), name: "description".to_owned(), oninput: move |event: FormEvent| description.set(event.value()) }
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
                let url = Url { url_type: None, href, description };
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
