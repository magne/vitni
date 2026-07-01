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
    let create_services = services.clone();
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
    let mut toast = use_signal(|| None::<String>);
    use_effect(move || selected.set(nav.active_record_ref().map(|record| record.human_id)));
    use_effect(move || {
        if *nav.pending_create.read() == Some(Category::Repositories) {
            nav.pending_create.set(None);
            let services = create_services.clone();
            spawn(async move {
                match create_repository_record(services).await {
                    Ok(human_id) => nav.open_record(RecordRef {
                        category: Category::Repositories,
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
                onselect: move |row: RowVm| nav.open_record(RecordRef {
                    category: Category::Repositories,
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

/// Which repository edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryEditForm {
    /// Set the repository's name.
    Name,
    /// Set the repository's type.
    Type,
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
    let mut nav = use_context::<NavState>();
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

    // Once the detail loads, upgrade the tab label from the `human_id` placeholder to the
    // repository's name (`tab_label` falls back to `human_id` when the name is blank).
    let label_human_id = human_id.clone();
    use_effect(move || {
        let Some(ScreenData::Loaded(IntentOutcome::RepositoryDetail(detail))) = &*data.read_unchecked() else {
            return;
        };
        nav.set_record_label(
            Category::Repositories,
            &label_human_id,
            genealogy_ui::tab_label(Some(&detail.title), &label_human_id),
        );
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |edit: RepositoryEdit| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_repository_edit(services, edit).await {
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
        Some(ScreenData::Loaded(IntentOutcome::RepositoryDetail(detail))) => {
            repository_detail(&state, detail, active, editing, on_submit, &human_id)
        }
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

/// Renders a loaded repository's detail container: header, the tab strip, the active tab, the panel.
fn repository_detail(
    state: &AppState,
    detail: &RepositoryDetail,
    active: Signal<usize>,
    editing: Signal<Option<RepositoryEditForm>>,
    on_submit: Callback<RepositoryEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
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
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            id_label: detail.human_id.clone(),
            avatar: "🏛".to_owned(),
            extras: repository_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {repository_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {repository_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a repository (the mockup `resn-set`).
fn repository_restriction_toggles(
    loc: &Localizer,
    detail: &RepositoryDetail,
    on_submit: Callback<RepositoryEdit>,
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
                on_submit.call(RepositoryEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next });
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
    on_submit: Callback<RepositoryEdit>,
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
            {id_list(loc, &detail.notes)}
        },
        "tags" => repository_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => repository_history_tab(loc, detail, on_submit, human_id),
        _ => repository_overview(loc, detail, editing),
    }
}

/// The Overview tab: the holds-sources note, a Repository card, and a Primary-contact card.
pub fn repository_overview(
    loc: &Localizer,
    detail: &RepositoryDetail,
    mut editing: Signal<Option<RepositoryEditForm>>,
) -> Element {
    let primary = detail.addresses.first();
    rsx! {
        div { class: "section-note", "{loc.repository_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("repository"),
                div { class: "tab-actions",
                    Button { label: loc.field_label("name"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(RepositoryEditForm::Name)) }
                    Button { label: loc.field_label("type"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(RepositoryEditForm::Type)) }
                }
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"type\")}" }
                        if let Some(type_label) = detail.type_label.clone() {
                            span { class: "grow", Chip { label: type_label } }
                        } else {
                            span { class: "grow muted", "—" }
                        }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"name\")}" }
                        span { class: "grow", "{detail.title}" }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:80px;margin:0", "{loc.field_label(\"sources\")}" }
                        span { class: "grow", "{detail.sources.len()}" }
                    }
                }
            }
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
    on_submit: Callback<RepositoryEdit>,
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
                                    onclick: move |_| on_submit.call(RepositoryEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }),
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
    on_submit: Callback<RepositoryEdit>,
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
                on_submit.call(RepositoryEdit::UndoAssertion { human_id: human_id.clone(), assertion_id });
            },
        }
    }
}

/// The repository editing side panel: renders the form for the open [`RepositoryEditForm`], or nothing.
fn repository_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<RepositoryEditForm>>,
    on_submit: Callback<RepositoryEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        RepositoryEditForm::Name => loc.field_label("name"),
        RepositoryEditForm::Type => loc.field_label("type"),
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
                RepositoryEditForm::Name => rsx! { RepositoryNameForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                RepositoryEditForm::Type => rsx! { RepositoryTypeForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
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
fn RepositoryAddressForm(human_id: String, onsubmit: EventHandler<RepositoryEdit>) -> Element {
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
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("street"), name: "street".to_owned(), oninput: move |event: FormEvent| street.set(event.value()) }
        Input { label: loc.field_label("locality"), name: "locality".to_owned(), oninput: move |event: FormEvent| locality.set(event.value()) }
        Input { label: loc.field_label("region"), name: "region".to_owned(), oninput: move |event: FormEvent| region.set(event.value()) }
        Input { label: loc.field_label("postal-code"), name: "postal-code".to_owned(), oninput: move |event: FormEvent| postal_code.set(event.value()) }
        Input { label: loc.field_label("country"), name: "country".to_owned(), oninput: move |event: FormEvent| country.set(event.value()) }
        Input { label: loc.field_label("phone"), name: "phone".to_owned(), oninput: move |event: FormEvent| phone.set(event.value()) }
        Input { label: loc.field_label("email"), name: "email".to_owned(), oninput: move |event: FormEvent| email.set(event.value()) }
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
                onsubmit.call(RepositoryEdit::AddAddress { human_id: human_id.clone(), address });
            },
        }
    }
}

/// The "Add URL" form: href + description → [`RepositoryEdit::AddUrl`].
#[component]
fn RepositoryUrlForm(human_id: String, onsubmit: EventHandler<RepositoryEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut href = use_signal(String::new);
    let mut description = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("url"), name: "url".to_owned(), oninput: move |event: FormEvent| href.set(event.value()) }
        Input { label: loc.field_label("description"), name: "description".to_owned(), oninput: move |event: FormEvent| description.set(event.value()) }
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
                onsubmit.call(RepositoryEdit::AddUrl { human_id: human_id.clone(), url });
            },
        }
    }
}

/// The "Link source" form: a source `human_id` + call number + medium → [`RepositoryEdit::LinkSource`].
#[component]
fn RepositoryLinkSourceForm(human_id: String, onsubmit: EventHandler<RepositoryEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let media_types = source_media_type_choices();
    let options: Vec<SelectChoice> = media_types
        .iter()
        .enumerate()
        .map(|(position, media_type)| SelectChoice {
            value: position.to_string(),
            label: loc.source_media_type_label(media_type),
        })
        .collect();
    let mut source = use_signal(String::new);
    let mut call_number = use_signal(String::new);
    let mut media = use_signal(|| 0_usize);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.tab_label("sources"), name: "source".to_owned(), oninput: move |event: FormEvent| source.set(event.value()) }
        Input { label: loc.field_label("call-number"), name: "call-number".to_owned(), oninput: move |event: FormEvent| call_number.set(event.value()) }
        Select {
            label: loc.field_label("media-type"),
            name: "media-type".to_owned(),
            value: Some(0.to_string()),
            options,
            onchange: move |event: FormEvent| media.set(event.value().parse::<usize>().unwrap_or(0)),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let source_id = source();
                if source_id.trim().is_empty() {
                    return;
                }
                let media_type = source_media_type_choices().get(media()).cloned().unwrap_or(SourceMediaType::Book);
                let call = call_number();
                let call_number = if call.trim().is_empty() { None } else { Some(call) };
                onsubmit.call(RepositoryEdit::LinkSource { human_id: human_id.clone(), source_id, call_number, media_type });
            },
        }
    }
}

/// The "Attach note by id" form → [`RepositoryEdit::AttachNote`].
#[component]
fn RepositoryNoteForm(human_id: String, onsubmit: EventHandler<RepositoryEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut id = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("note"), name: "note".to_owned(), oninput: move |event: FormEvent| id.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let id = id();
                if id.trim().is_empty() {
                    return;
                }
                onsubmit.call(RepositoryEdit::AttachNote { human_id: human_id.clone(), note_id: id });
            },
        }
    }
}

/// The repository "Add tag" form: a picker of existing tags by name → [`RepositoryEdit::Tag`].
#[component]
fn RepositoryTagForm(human_id: String, onsubmit: EventHandler<RepositoryEdit>) -> Element {
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
                Button {
                    label: save_label,
                    variant: ButtonVariant::Primary,
                    onclick: move |_| {
                        let tag_id = chosen();
                        if tag_id.is_empty() {
                            return;
                        }
                        onsubmit.call(RepositoryEdit::Tag { human_id: human_id.clone(), tag_id, remove: false });
                    },
                }
            }
        }
    }
}

/// The "Set name" form: a single text field → [`RepositoryEdit::SetName`].
#[component]
fn RepositoryNameForm(human_id: String, onsubmit: EventHandler<RepositoryEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut name = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input {
            label: loc.field_label("name"),
            name: "name".to_owned(),
            value: None,
            oninput: move |event: FormEvent| name.set(event.value()),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| onsubmit.call(RepositoryEdit::SetName { human_id: human_id.clone(), name: name() }),
        }
    }
}

/// The "Set type" form: a repository-type picker → [`RepositoryEdit::SetType`].
#[component]
fn RepositoryTypeForm(human_id: String, onsubmit: EventHandler<RepositoryEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let options: Vec<SelectChoice> = repository_type_choices()
        .iter()
        .enumerate()
        .map(|(position, repository_type)| SelectChoice {
            value: position.to_string(),
            label: loc.repository_type_label(repository_type),
        })
        .collect();
    let mut chosen = use_signal(|| 0_usize);
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.field_label("type"),
            name: "type".to_owned(),
            value: Some(0.to_string()),
            options,
            onchange: move |event: FormEvent| chosen.set(event.value().parse::<usize>().unwrap_or(0)),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let repository_type = repository_type_choices().get(chosen()).cloned().unwrap_or(RepositoryType::Library);
                onsubmit.call(RepositoryEdit::SetType { human_id: human_id.clone(), repository_type });
            },
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
