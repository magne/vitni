use super::prelude::*;

/// The person master-detail: a searchable/sortable list on the left, the selected person's detail
/// (an overview tab plus related-item tabs) on the right.
#[component]
pub fn PersonScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.nav_people();
    let loading = chrome.loading();
    let empty = state.data_loc().list_empty();
    let prompt = chrome.select_prompt();
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
    // Keep the list-row highlight in sync with the active record tab (clicking a tab re-highlights).
    use_effect(move || selected.set(nav.active_record_ref().map(|record| record.human_id)));
    // The top-bar `New`/`⌘N` bump `new_request`; opening the create form here makes them work too.
    use_effect(move || {
        if *nav.new_request.read() > 0 {
            creating.set(true);
        }
    });
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowList).await }
    });
    let on_create = use_callback(move |(name, sex): (Option<PersonNameParts>, Option<Sex>)| {
        let services = create_services.clone();
        let label = name
            .as_ref()
            .map(|parts| {
                [parts.given.as_deref(), parts.surname.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|joined| !joined.is_empty());
        spawn(async move {
            match create_person(services, name, sex).await {
                Ok(human_id) => {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::People,
                        label: label.unwrap_or_else(|| human_id.clone()),
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
                    category: Category::People,
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
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::SourceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_),
        )) => rsx! {},
    };
    let detail_pane = match nav.active_record_ref() {
        Some(record) if record.category == Category::People => {
            let human_id = record.human_id;
            rsx! { PersonDetailPane { key: "{human_id}", human_id } }
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
                CreatePersonForm { onsubmit: move |payload| on_create.call(payload) }
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

/// The "New person" side-panel form: given + surname + optional sex → the create payload, which the
/// screen turns into `create_person` and then opens the new record.
#[component]
fn CreatePersonForm(onsubmit: EventHandler<(Option<PersonNameParts>, Option<Sex>)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut given = use_signal(String::new);
    let mut surname = use_signal(String::new);
    let sexes = [Sex::Female, Sex::Male, Sex::Unknown, Sex::Intersex];
    let mut sex_choice = use_signal(|| "none".to_owned());
    let mut options = vec![SelectChoice {
        value: "none".to_owned(),
        label: loc.sex_label(None),
    }];
    for (index, sex) in sexes.iter().enumerate() {
        options.push(SelectChoice {
            value: index.to_string(),
            label: loc.sex_label(Some(sex)),
        });
    }
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.label_given(), name: "given".to_owned(), oninput: move |event: FormEvent| given.set(event.value()) }
        Input { label: loc.label_surname(), name: "surname".to_owned(), oninput: move |event: FormEvent| surname.set(event.value()) }
        Select {
            label: loc.label_sex(),
            name: "sex".to_owned(),
            options,
            onchange: move |event: FormEvent| sex_choice.set(event.value()),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let name = match (non_empty(given()), non_empty(surname())) {
                    (None, None) => None,
                    (given, surname) => Some(PersonNameParts {
                        name_type: genealogy_app::NameType::BirthName,
                        given,
                        surname_prefix: None,
                        surname,
                        nickname: None,
                        prefix: None,
                        suffix: None,
                    }),
                };
                let sex = sex_choice().parse::<usize>().ok().and_then(|index| sexes.get(index).cloned());
                onsubmit.call((name, sex));
            },
        }
    }
}

/// Which edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditForm {
    /// Edit the person's identity (primary name + sex) — the detail-head Edit action.
    Identity,
    /// Assert an additional name.
    Name,
    /// Assert a fact, with confidence and an optional source.
    Fact,
    /// Attach an existing citation by id.
    Citation,
    /// Attach an existing media object by id.
    Media,
    /// Attach an existing note by id.
    Note,
}

/// The detail pane for the selected person: a header, the related-item tab strip, the editing side
/// panel, and a save toast. Owns the reload/editing/toast state; reads are reloaded after each save.
#[component]
fn PersonDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let nav = use_context::<NavState>();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let mut editing = use_signal(|| None::<EditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        // Subscribe to `reload`: bumping it after a save refetches the detail.
        let _ = reload();
        async move { load_screen(services, Intent::ShowPerson { human_id }).await }
    });

    let on_submit = use_callback(move |edit: PersonEdit| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_edit(services, edit).await {
                Ok(()) => {
                    editing.set(None);
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
        Some(ScreenData::Loaded(IntentOutcome::Detail(detail))) => {
            person_detail(&state, nav, detail, active, editing, on_submit, &human_id)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Dashboard(_)
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
            | IntentOutcome::DnaMatchDetail(_),
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

/// Renders a loaded person's detail container: header (avatar, vital subtitle, restriction toggles,
/// Edit/Compare actions), the tab strip, the active tab's content, and the editing side panel.
fn person_detail(
    state: &AppState,
    nav: NavState,
    detail: &PersonDetail,
    active: Signal<usize>,
    mut editing: Signal<Option<EditForm>>,
    on_submit: Callback<PersonEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = person_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_id = tabs.get(active()).map_or("overview", |tab| tab.id);
    let subtitle = match &detail.vitals {
        Some(vitals) => format!("{vitals} · {}", detail.sex),
        None => detail.sex.clone(),
    };
    let edit_label = loc.action_label("edit");
    let compare_label = loc.action_label("compare");
    let mut compare_nav = nav;
    let actions = rsx! {
        Button { label: compare_label, variant: ButtonVariant::Default, onclick: move |_| compare_nav.go_to(Destination::Tool(Tool::Merge)) }
        Button { label: edit_label, variant: ButtonVariant::Primary, onclick: move |_| editing.set(Some(EditForm::Identity)) }
    };
    rsx! {
        DetailContainer {
            title: detail.name.clone(),
            subtitle,
            id_label: detail.human_id.clone(),
            badges: vec![detail.evidence_level_label.clone()],
            avatar: person_initials(detail),
            extras: restriction_toggles(loc, detail, on_submit, human_id),
            actions,
            tabs: tab_items,
            active,
            {person_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {edit_panel(state, detail, editing, on_submit, human_id)}
    }
}

/// The person's initials (first letters of given + surname, uppercased), or `?` when unknown.
fn person_initials(detail: &PersonDetail) -> String {
    let mut initials = String::new();
    for part in [detail.given.as_deref(), detail.surname.as_deref()] {
        if let Some(first) = part.and_then(|name| name.chars().next()) {
            initials.extend(first.to_uppercase());
        }
    }
    if initials.is_empty() {
        initials.push('?');
    }
    initials
}

/// The interactive privacy-restriction toggles shown in the detail header (the mockup `resn-set`).
fn restriction_toggles(
    loc: &Localizer,
    detail: &PersonDetail,
    on_submit: Callback<PersonEdit>,
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
                on_submit.call(PersonEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next });
            },
        }
    }
}

/// The content of one person detail tab, with its contextual add/edit affordances.
fn person_tab_content(
    state: &AppState,
    detail: &PersonDetail,
    tab_id: &str,
    mut editing: Signal<Option<EditForm>>,
    on_submit: Callback<PersonEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "names" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-name"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Name)) }
            }
            {names_table(loc, &detail.names)}
        },
        "facts" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-fact"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Fact)) }
            }
            {facts_table(loc, &detail.facts)}
        },
        "events" => events_table(loc, &detail.events),
        "associations" => associations_table(loc, &detail.associations),
        "families" => families_panel(loc, &detail.families),
        "citations" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-citation"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Citation)) }
            }
            {person_citations_table(loc, &detail.citations)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Media)) }
            }
            {media_gallery(loc, &detail.media)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => tags_panel(loc, &detail.tags),
        "history" => history_tab(loc, detail, on_submit, human_id),
        _ => overview_tab(loc, detail),
    }
}

/// The History tab: the per-record audit timeline (who/when/why), each undoable entry carrying an
/// undo control. The event-sourced differentiator — free from the event log.
fn history_tab(loc: &Localizer, detail: &PersonDetail, on_submit: Callback<PersonEdit>, human_id: &str) -> Element {
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
                on_submit.call(PersonEdit::UndoAssertion { human_id: human_id.clone(), assertion_id });
            },
        }
    }
}

/// The overview tab: the evidence-first note plus two cards — vital facts (each with its surety and
/// source cue) and the immediate family.
fn overview_tab(loc: &Localizer, detail: &PersonDetail) -> Element {
    rsx! {
        div { class: "section-note", "{loc.overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.section_label("vitals"),
                if detail.facts.is_empty() {
                    span { class: "muted", "{loc.tab_empty()}" }
                } else {
                    div { class: "stack",
                        for fact in detail.facts.iter() {
                            div { class: "fact-row",
                                span { class: "field-label", style: "width:96px;margin:0", "{fact.type_label}" }
                                span { class: "grow", {fact_value_date(fact)} }
                                ConfidenceBadge { level: fact.confidence, label: fact.confidence_label.clone() }
                                if fact.has_source() {
                                    SourceLink { label: loc.source_count(fact.source_count), onclick: move |_| {} }
                                } else {
                                    NoSourceFlag { label: loc.no_source() }
                                }
                            }
                        }
                    }
                }
            }
            Card { title: loc.section_label("family"),
                if detail.families.is_empty() {
                    span { class: "muted", "{loc.tab_empty()}" }
                } else {
                    div { class: "stack",
                        for family in detail.families.iter() {
                            div { class: "fact-row",
                                span { class: "muted", "{family.role_label}" }
                                span { class: "grow", {family.partners.join(" · ")} }
                            }
                            if !family.children.is_empty() {
                                div { class: "fact-row",
                                    span { class: "muted", "{loc.family_children()}" }
                                    span { class: "grow", {family.children.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>().join(" · ")} }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Renders a fact's value and/or date as a single display string (`date · value`, or whichever is
/// present), or an em dash when neither is known.
fn fact_value_date(fact: &FactVm) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(date) = fact.date.as_deref() {
        parts.push(date.to_owned());
    }
    if let Some(value) = fact.value.as_deref() {
        parts.push(value.to_owned());
    }
    if parts.is_empty() {
        "—".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// The Names tab: every asserted name variant with its type chip, date / language, and its
/// evidence cues (surety badge + source-count / no-source flag — colour is never the only signal).
pub fn names_table(loc: &Localizer, names: &[NameVm]) -> Element {
    if names.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("name-type"),
                loc.label_name(),
                format!("{} / {}", loc.field_label("date"), loc.field_label("language")),
                loc.field_label("surety"),
                loc.field_label("source"),
            ],
            for name in names.iter() {
                tr {
                    td {
                        Chip { label: name.type_label.clone() }
                    }
                    td { "{name.display}" }
                    td { class: "muted", {name_date_language(name)} }
                    td {
                        ConfidenceBadge { level: name.confidence, label: name.confidence_label.clone() }
                    }
                    td {
                        if name.has_source() {
                            SourceLink { label: loc.source_count(name.source_count), onclick: move |_| {} }
                        } else {
                            NoSourceFlag { label: loc.no_source() }
                        }
                    }
                }
            }
        }
    }
}

/// Renders a name's `date / language` cell from whichever parts are present, or an em dash.
fn name_date_language(name: &NameVm) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(date) = name.date.as_deref() {
        parts.push(date.to_owned());
    }
    if let Some(language) = name.language.as_deref() {
        parts.push(language.to_owned());
    }
    if parts.is_empty() {
        "—".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// The Facts tab: each fact with its confidence badge and source count / no-source flag — the
/// evidence-first row (colour is never the only signal).
pub fn facts_table(loc: &Localizer, facts: &[FactVm]) -> Element {
    if facts.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("fact-type"),
                loc.field_label("date"),
                loc.field_label("value"),
                loc.field_label("surety"),
                loc.field_label("source"),
            ],
            for fact in facts.iter() {
                tr {
                    td { "{fact.type_label}" }
                    td { class: "muted", {fact.date.clone().unwrap_or_else(|| "—".to_owned())} }
                    td { {fact.value.clone().unwrap_or_else(|| "—".to_owned())} }
                    td {
                        ConfidenceBadge { level: fact.confidence, label: fact.confidence_label.clone() }
                    }
                    td {
                        if fact.has_source() {
                            SourceLink { label: loc.source_count(fact.source_count), onclick: move |_| {} }
                        } else {
                            NoSourceFlag { label: loc.no_source() }
                        }
                    }
                }
            }
        }
    }
}

/// The Events tab: each participation's role and the joined event id + date.
pub fn events_table(loc: &Localizer, events: &[EventRefVm]) -> Element {
    if events.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![loc.tab_label("events"), loc.field_label("role"), loc.field_label("date")],
            for event in events.iter() {
                tr {
                    td {
                        RecordLink {
                            category: Category::Events,
                            human_id: event.event_id.clone(),
                            label: event.event_id.clone(),
                        }
                    }
                    td {
                        Chip { label: event.role_label.clone() }
                    }
                    td { class: "muted", {event.date.clone().unwrap_or_else(|| "—".to_owned())} }
                }
            }
        }
    }
}

/// The Associations tab: each linked person, the role, and the evidence cues (surety + source).
pub fn associations_table(loc: &Localizer, associations: &[AssociationVm]) -> Element {
    if associations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.field_label("association"),
                loc.field_label("relationship"),
                loc.field_label("surety"),
                loc.field_label("source"),
            ],
            for association in associations.iter() {
                tr {
                    td {
                        RecordLink {
                            category: Category::People,
                            human_id: association.other_id.clone(),
                            label: association.other_id.clone(),
                        }
                    }
                    td {
                        Chip { label: association.role_label.clone() }
                    }
                    td {
                        ConfidenceBadge { level: association.confidence, label: association.confidence_label.clone() }
                    }
                    td {
                        if association.has_source() {
                            SourceLink { label: loc.source_count(association.source_count), onclick: move |_| {} }
                        } else {
                            NoSourceFlag { label: loc.no_source() }
                        }
                    }
                }
            }
        }
    }
}

/// The Citations tab: each backing citation's id, cited source, surety, and Evidence Explained axes
/// — the research-grade-citation differentiator surfaced on the person.
pub fn person_citations_table(loc: &Localizer, citations: &[CitationRefVm]) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table {
            headers: vec![
                loc.label_id(),
                loc.field_label("source"),
                loc.field_label("surety"),
                loc.field_label("evidence"),
            ],
            for citation in citations.iter() {
                tr {
                    td {
                        RecordLink {
                            category: Category::Citations,
                            human_id: citation.human_id.clone(),
                            label: citation.human_id.clone(),
                        }
                    }
                    td { class: "muted",
                        if let Some(source_id) = &citation.source_id {
                            RecordLink {
                                category: Category::Sources,
                                human_id: source_id.clone(),
                                label: citation.source.clone().unwrap_or_else(|| source_id.clone()),
                            }
                        } else {
                            {citation.source.clone().unwrap_or_else(|| "—".to_owned())}
                        }
                    }
                    td {
                        if let (Some(level), Some(label)) = (citation.confidence, citation.confidence_label.clone()) {
                            ConfidenceBadge { level, label }
                        } else {
                            "—"
                        }
                    }
                    td { class: "wrap",
                        if citation.evidence_axes.is_empty() {
                            "—"
                        } else {
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

/// The Families tab: each family the person belongs to, their role, and the members.
pub fn families_panel(loc: &Localizer, families: &[FamilyVm]) -> Element {
    if families.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "grid-2",
            for family in families.iter() {
                Card { title: format!("{} · {}", family.role_label, family.family_id),
                    div { class: "stack",
                        for partner in family.partners.iter() {
                            div { class: "fact-row",
                                span { class: "muted", "{loc.partner_role_label()}" }
                                span { class: "grow",
                                    RecordLink {
                                        category: Category::People,
                                        human_id: partner.clone(),
                                        label: partner.clone(),
                                    }
                                }
                            }
                        }
                        for (child , relationship) in family.children.iter() {
                            div { class: "fact-row",
                                span { class: "muted", "{relationship}" }
                                span { class: "grow",
                                    RecordLink {
                                        category: Category::People,
                                        human_id: child.clone(),
                                        label: child.clone(),
                                    }
                                }
                            }
                        }
                        RecordLink {
                            category: Category::Families,
                            human_id: family.family_id.clone(),
                            label: family.family_id.clone(),
                            button: true,
                        }
                    }
                }
            }
        }
    }
}

/// The editing side panel: renders the form for the currently-open [`EditForm`], or nothing.
fn edit_panel(
    state: &AppState,
    detail: &PersonDetail,
    mut editing: Signal<Option<EditForm>>,
    on_submit: Callback<PersonEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        EditForm::Identity => loc.action_label("edit"),
        EditForm::Name => loc.action_label("add-name"),
        EditForm::Fact => loc.action_label("add-fact"),
        EditForm::Citation => loc.action_label("attach-citation"),
        EditForm::Media => loc.action_label("attach-media"),
        EditForm::Note => loc.action_label("attach-note"),
    };
    let human_id = human_id.to_owned();
    let given = detail.given.clone().unwrap_or_default();
    let surname = detail.surname.clone().unwrap_or_default();
    rsx! {
        SidePanel {
            title,
            open: true,
            close_label: loc.action_label("cancel"),
            onclose: move |_| editing.set(None),
            footer: rsx! {},
            {match form {
                EditForm::Identity => rsx! { EditIdentityForm { human_id, given, surname, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Name => rsx! { AddNameForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Fact => rsx! { AddFactForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Citation => rsx! { AttachForm { human_id, kind: EditForm::Citation, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Media => rsx! { AttachForm { human_id, kind: EditForm::Media, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Note => rsx! { AttachForm { human_id, kind: EditForm::Note, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Add name" side-panel form: name parts → [`PersonEdit::AssertName`].
#[component]
fn AddNameForm(human_id: String, onsubmit: EventHandler<PersonEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut given = use_signal(String::new);
    let mut surname = use_signal(String::new);
    let mut nickname = use_signal(String::new);
    let mut prefix = use_signal(String::new);
    let mut suffix = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.label_given(), name: "given".to_owned(), oninput: move |event: FormEvent| given.set(event.value()) }
        Input { label: loc.label_surname(), name: "surname".to_owned(), oninput: move |event: FormEvent| surname.set(event.value()) }
        Input { label: loc.field_label("nickname"), name: "nickname".to_owned(), oninput: move |event: FormEvent| nickname.set(event.value()) }
        Input { label: loc.field_label("prefix"), name: "prefix".to_owned(), oninput: move |event: FormEvent| prefix.set(event.value()) }
        Input { label: loc.field_label("suffix"), name: "suffix".to_owned(), oninput: move |event: FormEvent| suffix.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let name = PersonNameParts {
                    name_type: genealogy_app::NameType::BirthName,
                    given: non_empty(given()),
                    surname_prefix: None,
                    surname: non_empty(surname()),
                    nickname: non_empty(nickname()),
                    prefix: non_empty(prefix()),
                    suffix: non_empty(suffix()),
                };
                onsubmit.call(PersonEdit::AssertName { human_id: human_id.clone(), name });
            },
        }
    }
}

/// The "Add fact" side-panel form: type + value + confidence + optional source →
/// [`PersonEdit::AssertFact`].
#[component]
fn AddFactForm(human_id: String, onsubmit: EventHandler<PersonEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let fact_choices = loc.fact_type_choices();
    let confidence_levels = ConfidenceLevel::all();
    let mut fact_index = use_signal(|| 0_usize);
    let mut value = use_signal(String::new);
    let mut confidence_index = use_signal(|| 2_usize);
    let mut citation = use_signal(String::new);
    let fact_options: Vec<SelectChoice> = fact_choices
        .iter()
        .enumerate()
        .map(|(index, (_, label))| SelectChoice {
            value: index.to_string(),
            label: label.clone(),
        })
        .collect();
    let confidence_options: Vec<SelectChoice> = confidence_levels
        .iter()
        .enumerate()
        .map(|(index, level)| SelectChoice {
            value: index.to_string(),
            label: loc.confidence_label(*level),
        })
        .collect();
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.field_label("fact-type"),
            name: "fact-type".to_owned(),
            options: fact_options,
            onchange: move |event: FormEvent| fact_index.set(event.value().parse().unwrap_or(0)),
        }
        Input { label: loc.field_label("value"), name: "value".to_owned(), oninput: move |event: FormEvent| value.set(event.value()) }
        Select {
            label: loc.field_label("confidence"),
            name: "confidence".to_owned(),
            value: Some(2.to_string()),
            options: confidence_options,
            onchange: move |event: FormEvent| confidence_index.set(event.value().parse().unwrap_or(2)),
        }
        Input { label: loc.field_label("citation"), name: "citation".to_owned(), oninput: move |event: FormEvent| citation.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let fact_type = fact_choices
                    .get(fact_index())
                    .map_or(genealogy_app::FactType::Occupation, |(kind, _)| kind.clone());
                let confidence = *confidence_levels.get(confidence_index()).unwrap_or(&ConfidenceLevel::Normal);
                onsubmit.call(PersonEdit::AssertFact {
                    human_id: human_id.clone(),
                    fact_type,
                    value: non_empty(value()),
                    confidence,
                    citation: non_empty(citation()),
                });
            },
        }
    }
}

/// The "Edit" identity side-panel form (detail-head Edit): primary name + sex, prefilled. Emits an
/// [`PersonEdit::AssertName`] when the name changed and an [`PersonEdit::AssertSex`] when a sex is
/// picked — names are append-only assertions, so the newest wins in the projection.
#[component]
fn EditIdentityForm(human_id: String, given: String, surname: String, onsubmit: EventHandler<PersonEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let initial_given = given.clone();
    let initial_surname = surname.clone();
    let mut given_value = use_signal(|| given.clone());
    let mut surname_value = use_signal(|| surname.clone());
    let sexes = [Sex::Female, Sex::Male, Sex::Unknown, Sex::Intersex];
    let mut sex_choice = use_signal(|| "keep".to_owned());
    let mut options = vec![SelectChoice {
        value: "keep".to_owned(),
        label: loc.sex_label(None),
    }];
    for (index, sex) in sexes.iter().enumerate() {
        options.push(SelectChoice {
            value: index.to_string(),
            label: loc.sex_label(Some(sex)),
        });
    }
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.label_given(), name: "given".to_owned(), value: Some(initial_given.clone()), oninput: move |event: FormEvent| given_value.set(event.value()) }
        Input { label: loc.label_surname(), name: "surname".to_owned(), value: Some(initial_surname.clone()), oninput: move |event: FormEvent| surname_value.set(event.value()) }
        Select {
            label: loc.label_sex(),
            name: "sex".to_owned(),
            options,
            onchange: move |event: FormEvent| sex_choice.set(event.value()),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                if given_value() != initial_given || surname_value() != initial_surname {
                    let name = PersonNameParts {
                        name_type: genealogy_app::NameType::BirthName,
                        given: non_empty(given_value()),
                        surname_prefix: None,
                        surname: non_empty(surname_value()),
                        nickname: None,
                        prefix: None,
                        suffix: None,
                    };
                    onsubmit.call(PersonEdit::AssertName { human_id: human_id.clone(), name });
                }
                if let Some(sex) = sex_choice().parse::<usize>().ok().and_then(|index| sexes.get(index).cloned()) {
                    onsubmit.call(PersonEdit::AssertSex { human_id: human_id.clone(), sex });
                }
            },
        }
    }
}

/// The "Attach by id" side-panel form for a citation/media/note → the matching attach edit.
#[component]
fn AttachForm(human_id: String, kind: EditForm, onsubmit: EventHandler<PersonEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let field = match kind {
        EditForm::Media => "media",
        EditForm::Note => "note",
        _ => "citation",
    };
    let mut id = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label(field), name: field.to_owned(), oninput: move |event: FormEvent| id.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let id = id();
                if id.is_empty() {
                    return;
                }
                let edit = match kind {
                    EditForm::Media => PersonEdit::AttachMedia { human_id: human_id.clone(), media_id: id },
                    EditForm::Note => PersonEdit::AttachNote { human_id: human_id.clone(), note_id: id },
                    _ => PersonEdit::AttachCitation { human_id: human_id.clone(), citation_id: id },
                };
                onsubmit.call(edit);
            },
        }
    }
}
