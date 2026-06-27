use super::prelude::*;

/// The DNA-test master-detail screen: a list of tests on the left, the selected test's detail
/// (kit metadata + haplogroups + matches + notes/tags + history) on the right. `New` opens a form
/// collecting the anchoring person's `human_id`.
#[component]
pub fn DnaTestScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::DnaTests.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().dna_test_list_empty();
    let prompt = chrome.dna_test_select_prompt();
    let create_title = chrome.list_new();
    let cancel_label = state.data_loc().action_label("cancel");
    let dismiss_label = state.data_loc().action_label("dismiss");
    let list_chrome = ListChrome {
        list_label: entity.clone(),
        filter_placeholder: chrome.list_filter(&entity),
        sort_label: chrome.list_sort(),
        sort_options: chrome.sort_options(),
        empty,
        new_label: chrome.list_new(),
    };
    let mut nav = use_context::<NavState>();
    let mut selected = use_signal(|| None::<String>);
    let mut creating = use_signal(|| false);
    let mut toast = use_signal(|| None::<String>);
    use_effect(move || selected.set(nav.active_record_ref().map(|record| record.human_id)));
    use_effect(move || {
        if *nav.new_request.read() > 0 {
            creating.set(true);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowDnaTestList).await }
    });
    let on_create = use_callback(move |person: String| {
        let services = create_services.clone();
        spawn(async move {
            match create_dna_test_record(services, person).await {
                Ok(human_id) => {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::DnaTests,
                        label: human_id.clone(),
                        human_id,
                    });
                }
                Err(message) => toast.set(Some(message)),
            }
        });
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
                    category: Category::DnaTests,
                    human_id: row.id,
                    label: row.title,
                }),
                onnew: move |()| nav.request_new(),
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
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_),
        )) => rsx! {},
    };
    let detail_pane = match nav.active_record_ref() {
        Some(record) if record.category == Category::DnaTests => {
            let human_id = record.human_id;
            rsx! { DnaTestDetailPane { key: "{human_id}", human_id } }
        }
        _ => rsx! { p { class: "empty", "{prompt}" } },
    };
    rsx! {
        MasterDetail { list: list_pane, detail: detail_pane }
        if creating() {
            SidePanel {
                title: create_title,
                open: true,
                close_label: cancel_label,
                onclose: move |_| creating.set(false),
                footer: rsx! {},
                CreateDnaTestForm { onsubmit: move |person| on_create.call(person) }
            }
        }
        Toast {
            visible: toast().is_some(),
            message: toast().unwrap_or_default(),
            action_label: dismiss_label,
            onaction: move |_| toast.set(None),
        }
    }
}

/// The "New DNA test" form: the anchoring person's `human_id` (required).
#[component]
fn CreateDnaTestForm(onsubmit: EventHandler<String>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut person = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("person"), name: "person".to_owned(), oninput: move |event: FormEvent| person.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let person = person();
                if person.trim().is_empty() {
                    return;
                }
                onsubmit.call(person);
            },
        }
    }
}

/// Which DNA-test edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnaTestEditForm {
    /// Assert a haplogroup.
    Haplogroup,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected DNA test: header, related-item tabs, editing side panel.
#[component]
fn DnaTestDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<DnaTestEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowDnaTest { human_id }).await }
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |edit: DnaTestEdit| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_dna_test_edit(services, edit).await {
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
        Some(ScreenData::Loaded(IntentOutcome::DnaTestDetail(detail))) => {
            dna_test_detail(&state, detail, active, editing, on_submit, &human_id)
        }
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
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Dashboard(_),
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

/// Renders a loaded DNA test's detail container: header, the tab strip, the active tab, the panel.
fn dna_test_detail(
    state: &AppState,
    detail: &DnaTestDetail,
    active: Signal<usize>,
    editing: Signal<Option<DnaTestEditForm>>,
    on_submit: Callback<DnaTestEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = dna_test_tabs(detail, loc);
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
            avatar: "🧬".to_owned(),
            extras: dna_test_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {dna_test_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {dna_test_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a DNA test (the mockup `resn-set`).
fn dna_test_restriction_toggles(
    loc: &Localizer,
    detail: &DnaTestDetail,
    on_submit: Callback<DnaTestEdit>,
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
                on_submit.call(DnaTestEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next });
            },
        }
    }
}

/// The content of one DNA-test detail tab, with its contextual add affordances.
fn dna_test_tab_content(
    state: &AppState,
    detail: &DnaTestDetail,
    tab_id: &str,
    mut editing: Signal<Option<DnaTestEditForm>>,
    on_submit: Callback<DnaTestEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "haplogroups" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-haplogroup"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaTestEditForm::Haplogroup)) }
            }
            {dna_test_haplogroups_table(loc, &detail.haplogroups)}
        },
        "matches" => dna_test_matches_table(loc, &detail.matches),
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaTestEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => dna_test_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => dna_test_history_tab(loc, detail, on_submit, human_id),
        _ => dna_test_overview(loc, detail),
    }
}

/// The DNA-test Overview: the Kit details card, the Tested-person card, and the ethnicity note.
pub fn dna_test_overview(loc: &Localizer, detail: &DnaTestDetail) -> Element {
    let dash = "—".to_owned();
    rsx! {
        div { class: "section-note", "{loc.dna_test_overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("kit"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"provider\")}" }
                        span { class: "grow", {detail.provider.clone().unwrap_or_else(|| dash.clone())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"test-type\")}" }
                        span { class: "grow", {detail.test_type.clone().unwrap_or_else(|| dash.clone())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"kit-id\")}" }
                        span { class: "grow mono", {detail.kit_id.clone().unwrap_or_else(|| dash.clone())} }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"genome-build\")}" }
                        span { class: "grow", {detail.genome_build.clone().unwrap_or_else(|| dash.clone())} }
                    }
                }
            }
            Card { title: loc.section_label("tested-person"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.field_label(\"person\")}" }
                        span { class: "grow", {detail.person_name.clone().unwrap_or_else(|| dash.clone())} }
                        if let Some(person) = &detail.person {
                            span { class: "muted mono", "{person.human_id}" }
                        }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:110px;margin:0", "{loc.tab_label(\"matches\")}" }
                        span { class: "grow", "{detail.matches.len()}" }
                    }
                }
            }
        }
        Card { title: loc.section_label("ethnicity"),
            div { class: "section-note", style: "margin:0", "{loc.dna_test_ethnicity_note()}" }
        }
    }
}

/// The DNA-test Haplogroups tab: one row per recorded haplogroup.
pub fn dna_test_haplogroups_table(loc: &Localizer, haplogroups: &[String]) -> Element {
    if haplogroups.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![loc.field_label("haplogroup")],
            for haplogroup in haplogroups.iter() {
                tr { td { b { "{haplogroup}" } } }
            }
        }
    }
}

/// The DNA-test Matches tab: one row per match this kit produced (match, compared test, cM, %, predicted).
pub fn dna_test_matches_table(loc: &Localizer, matches: &[DnaTestMatchVm]) -> Element {
    if matches.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    let dash = "—".to_owned();
    rsx! {
        Table {
            headers: vec![
                loc.tab_label("matches"),
                loc.field_label("compared-test"),
                loc.field_label("shared-cm"),
                loc.field_label("percent-shared"),
                loc.field_label("predicted"),
            ],
            for row in matches.iter() {
                tr {
                    td { "{row.match_ref.human_id}" }
                    td { class: "muted mono", {row.compared_test.as_ref().map_or_else(|| dash.clone(), |t| t.human_id.clone())} }
                    td { b { {row.shared_cm.clone().unwrap_or_else(|| dash.clone())} } }
                    td { {row.percent_shared.clone().unwrap_or_else(|| dash.clone())} }
                    td { if let Some(predicted) = row.predicted.clone() { Chip { label: predicted } } }
                }
            }
        }
    }
}

/// The DNA-test Tags tab: each applied tag as a colour-dot chip (name + colour, never id) with remove.
pub fn dna_test_tags_panel(
    loc: &Localizer,
    detail: &DnaTestDetail,
    mut editing: Signal<Option<DnaTestEditForm>>,
    on_submit: Callback<DnaTestEdit>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(DnaTestEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call(DnaTestEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The DNA-test History tab: the audit timeline, each undoable entry carrying an undo control.
fn dna_test_history_tab(
    loc: &Localizer,
    detail: &DnaTestDetail,
    on_submit: Callback<DnaTestEdit>,
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
                on_submit.call(DnaTestEdit::UndoAssertion { human_id: human_id.clone(), assertion_id });
            },
        }
    }
}

/// The DNA-test editing side panel: renders the form for the open [`DnaTestEditForm`], or nothing.
fn dna_test_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<DnaTestEditForm>>,
    on_submit: Callback<DnaTestEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        DnaTestEditForm::Haplogroup => loc.action_label("add-haplogroup"),
        DnaTestEditForm::Note => loc.action_label("attach-note"),
        DnaTestEditForm::Tag => loc.action_label("add-tag"),
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
                DnaTestEditForm::Haplogroup => rsx! { DnaTestFieldForm { human_id, field: "haplogroup".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::Note => rsx! { DnaTestFieldForm { human_id, field: "note".to_owned(), onsubmit: move |edit| on_submit.call(edit) } },
                DnaTestEditForm::Tag => rsx! { DnaTestTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "add haplogroup / attach note by id" form → the matching [`DnaTestEdit`] variant.
#[component]
fn DnaTestFieldForm(human_id: String, field: String, onsubmit: EventHandler<DnaTestEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut value = use_signal(String::new);
    let save_label = loc.action_label("save");
    let field_label = loc.field_label(&field);
    rsx! {
        Input { label: field_label, name: field.clone(), oninput: move |event: FormEvent| value.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let value = value();
                if value.trim().is_empty() {
                    return;
                }
                let edit = match field.as_str() {
                    "haplogroup" => DnaTestEdit::AddHaplogroup { human_id: human_id.clone(), haplogroup: value },
                    _ => DnaTestEdit::AttachNote { human_id: human_id.clone(), note_id: value },
                };
                onsubmit.call(edit);
            },
        }
    }
}

/// The DNA-test "Add tag" form: a picker of existing tags by name → [`DnaTestEdit::Tag`].
#[component]
fn DnaTestTagForm(human_id: String, onsubmit: EventHandler<DnaTestEdit>) -> Element {
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
                        onsubmit.call(DnaTestEdit::Tag { human_id: human_id.clone(), tag_id, remove: false });
                    },
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// DnaMatch slice
// ---------------------------------------------------------------------------------------------------
