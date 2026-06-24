//! The application's own screens (ADR 0008 §5): per-framework RSX over the shared `genealogy-ui`
//! view-models, built on the design-system components and the generic master-detail framework
//! (`crate::master_detail`). [`PersonScreen`] is the reference slice — adding an aggregate copies it:
//! supply a row builder (`genealogy_ui::person_row`-style), a tab builder
//! (`genealogy_ui::person_tabs`-style), and the tab-id→content match below; the list/detail layout,
//! search, sort, and keyboard come from the framework. The plugin panel renders a plugin-supplied
//! form through the vocabulary interpreter.

use dioxus::prelude::*;
use genealogy_app::{DateParts, EvidenceAnalysis, EvidenceKind, InformationKind, PersonNameParts, Sex, SourceQuality};
use genealogy_ui::{
    ActivityVm, AssociationVm, Category, CitationDetail, CitationEdit, ConfidenceLevel, DashboardVm, Destination,
    EventRefVm, FactVm, FamilyVm, Intent, IntentOutcome, JumpVm, Localizer, NameVm, PersonDetail, PersonEdit,
    RecordRef, RestrictionKind, RowVm, Tool, citation_tabs, person_tabs,
};

use crate::app::{AppCtx, AppState};
use crate::components::{
    Button, ButtonVariant, Card, Chip, ConfidenceBadge, EmptyState, EvidenceAxisChip, HistoryEntry, HistoryTimeline,
    Input, NoSourceFlag, RestrictionChoice, RestrictionSet, Select, SelectChoice, SidePanel, SourceLink, TabItem,
    Table, Toast,
};
use crate::master_detail::{DetailContainer, ListChrome, ListPane, MasterDetail};
use crate::services::{
    ScreenData, create_citation_record, create_person, load_plugin_form, load_screen, load_tags, save_citation_edit,
    save_edit,
};
use crate::shell::nav_state::NavState;
use crate::vocabulary_render::FormView;

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
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_),
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
            IntentOutcome::List(_) | IntentOutcome::Dashboard(_) | IntentOutcome::CitationDetail(_),
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
            {id_list(loc, &detail.citations)}
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

/// The Names tab: every asserted name variant with its type chip and date / language.
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
            ],
            for name in names.iter() {
                tr {
                    td {
                        Chip { label: name.type_label.clone() }
                    }
                    td { "{name.display}" }
                    td { class: "muted", {name_date_language(name)} }
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
                    td { "{event.event_id}" }
                    td {
                        Chip { label: event.role_label.clone() }
                    }
                    td { class: "muted", {event.date.clone().unwrap_or_else(|| "—".to_owned())} }
                }
            }
        }
    }
}

/// The Associations tab: each linked person and the association role.
pub fn associations_table(loc: &Localizer, associations: &[AssociationVm]) -> Element {
    if associations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table { headers: vec![loc.field_label("association"), loc.field_label("relationship")],
            for association in associations.iter() {
                tr {
                    td { "{association.other_id}" }
                    td {
                        Chip { label: association.role_label.clone() }
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
                                span { class: "grow", "{partner}" }
                            }
                        }
                        for (child , relationship) in family.children.iter() {
                            div { class: "fact-row",
                                span { class: "muted", "{relationship}" }
                                span { class: "grow", "{child}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A minimal list of related-item ids, or an empty-state when there are none.
pub fn id_list(loc: &Localizer, ids: &[String]) -> Element {
    if ids.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        ul { class: "id-list",
            for id in ids.iter() {
                li { "{id}" }
            }
        }
    }
}

/// The Media tab: a thumbnail gallery, one placeholder card per attached media id.
pub fn media_gallery(loc: &Localizer, media: &[String]) -> Element {
    if media.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "grid-3",
            for id in media.iter() {
                div { class: "card", style: "text-align:center",
                    div {
                        class: "faint",
                        style: "height:120px;background:var(--panel-2);border-radius:var(--r-md);display:grid;place-items:center",
                        "🖼"
                    }
                    div { style: "margin-top:8px", "{id}" }
                }
            }
        }
    }
}

/// The Tags tab: each applied tag as a chip. (Tag editing is a later slice.)
pub fn tags_panel(loc: &Localizer, tags: &[String]) -> Element {
    if tags.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        div { class: "wrap",
            for tag in tags.iter() {
                Chip { label: tag.clone() }
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

/// Returns `None` for a blank field (so an absent field is not asserted), else the value as typed.
fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() { None } else { Some(value) }
}

/// The workspace dashboard (ADR 0008 §5; `app-shell.html`): stat cards, a workspace-wide recent
/// activity feed, quick entry points, and the computable data-quality checks. Refetches whenever a
/// mutation bumps `data_version`.
#[component]
pub fn DashboardScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loading = state.chrome().loading();
    let nav = use_context::<NavState>();
    let data = use_resource(move || {
        let services = services.clone();
        // Subscribe to `data_version` so a create/edit/undo refreshes the counts and activity.
        let _ = nav.data_version.read();
        async move { load_screen(services, Intent::ShowDashboard).await }
    });
    // The dashboard is the workspace overview, rendered at the root of the work area (not inside a
    // record-tab body), matching `app-shell.html`.
    match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::Dashboard(dashboard))) => {
            dashboard_view(state.data_loc(), nav, dashboard)
        }
        Some(ScreenData::Loaded(
            IntentOutcome::List(_)
            | IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::NotFound { .. },
        )) => rsx! {},
    }
}

/// Renders a loaded dashboard: the "at a glance" stat cards, then the activity feed beside the quick
/// entry points and data-quality checks.
pub fn dashboard_view(loc: &Localizer, nav: NavState, dashboard: &DashboardVm) -> Element {
    let stats = &dashboard.stats;
    rsx! {
        h2 { style: "border:0;margin:0 0 12px", "{loc.dashboard_label(\"title\")}" }
        div { class: "grid-3", style: "margin-bottom:8px",
            Card { title: loc.dashboard_label("stat-people"),
                div { style: "font-size:28px;font-weight:700", "{stats.people}" }
                div { class: "muted", "{loc.dashboard_people_caption(stats.families, stats.events)}" }
            }
            Card { title: loc.dashboard_label("stat-evidence"),
                div { style: "font-size:28px;font-weight:700", "{stats.evidence_health_pct}%" }
                div { class: "muted", "{loc.dashboard_label(\"stat-evidence-caption\")}" }
            }
            Card { title: loc.dashboard_label("stat-attention"),
                div { style: "font-size:28px;font-weight:700;color:var(--warn)", "{stats.facts_without_source}" }
                div { class: "muted", "{loc.dashboard_label(\"no-source-facts\")}" }
            }
        }
        div { class: "grid-2",
            Card { title: loc.dashboard_label("recent-activity"),
                {activity_feed(loc, nav, &dashboard.recent)}
            }
            div { class: "stack",
                Card { title: loc.dashboard_label("jump-back"),
                    {jump_back(nav, &dashboard.jump_back)}
                }
                Card { title: loc.dashboard_label("data-quality"),
                    {data_quality(loc, stats)}
                }
            }
        }
    }
}

/// The workspace-wide recent-activity timeline; each row that resolves to a record links to it.
fn activity_feed(loc: &Localizer, nav: NavState, recent: &[ActivityVm]) -> Element {
    if recent.is_empty() {
        return rsx! { span { class: "muted", "{loc.dashboard_label(\"activity-empty\")}" } };
    }
    rsx! {
        div { class: "timeline", style: "margin-top:8px",
            for row in recent.iter() {
                div { class: "tl-item",
                    div { class: "tl-when", "{row.when}" }
                    div { class: "tl-what",
                        "{row.what}"
                        if let Some(record) = &row.record {
                            {activity_link(nav, record)}
                        }
                    }
                    div { class: "tl-who", "{row.who}" }
                }
            }
        }
    }
}

/// A link in an activity row that opens the affected record, prefixed with its entity icon.
fn activity_link(nav: NavState, record: &RecordRef) -> Element {
    let mut nav = nav;
    let record = record.clone();
    let label = record.label.clone();
    let icon = record.category.icon();
    rsx! {
        " — "
        button {
            class: "src-link",
            r#type: "button",
            onclick: move |_| {
                nav.go_to(Destination::Category(Category::People));
                nav.open_record(record.clone());
            },
            span { aria_hidden: "true", "{icon} " }
            "{label}"
        }
    }
}

/// The "Jump back in" quick entry points (the distinct recently-touched records).
fn jump_back(nav: NavState, jumps: &[JumpVm]) -> Element {
    rsx! {
        div { class: "wrap", style: "margin-top:8px",
            for jump in jumps.iter() {
                {jump_button(nav, &jump.record)}
            }
        }
    }
}

/// One quick-entry button that opens its record, prefixed with its entity icon.
fn jump_button(nav: NavState, record: &RecordRef) -> Element {
    let mut nav = nav;
    let record = record.clone();
    let label = record.label.clone();
    let icon = record.category.icon();
    rsx! {
        button {
            class: "btn",
            r#type: "button",
            onclick: move |_| {
                nav.go_to(Destination::Category(Category::People));
                nav.open_record(record.clone());
            },
            span { aria_hidden: "true", "{icon} " }
            "{label}"
        }
    }
}

/// The data-quality card: the computable no-source count now, with the remaining checks flagged as
/// arriving in a later milestone (no fabricated numbers).
fn data_quality(loc: &Localizer, stats: &genealogy_ui::DashboardStats) -> Element {
    rsx! {
        table { class: "tbl", style: "margin-top:4px",
            tbody {
                tr {
                    td {
                        NoSourceFlag { label: loc.dashboard_label("no-source-facts") }
                    }
                    td { class: "muted", "{stats.facts_without_source}" }
                }
                tr {
                    td { class: "muted", colspan: "2", "{loc.dashboard_label(\"later-milestone\")}" }
                }
            }
        }
    }
}

/// The plugin panel: runs the `ui-panel` plugin and renders the form it emits (ADR 0012).
#[component]
pub fn PluginPanelScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loading = state.chrome().loading();
    let run_label = state.chrome().run_plugin();
    let mut runs = use_signal(|| 0_u32);
    let form = use_resource(move || {
        let services = services.clone();
        // Reading `runs` subscribes the resource: clicking the button re-runs the plugin.
        let _ = runs();
        async move { load_plugin_form(services).await }
    });
    rsx! {
        div { class: "tab-body",
            Button { label: run_label, variant: ButtonVariant::Primary, onclick: move |_| runs += 1 }
            {match &*form.read_unchecked() {
                None => rsx! { p { class: "loading", "{loading}" } },
                Some(Err(message)) => rsx! { p { class: "empty", "{message}" } },
                Some(Ok(form)) => rsx! { FormView { form: form.clone() } },
            }}
        }
    }
}

/// The citation master-detail screen (ADR 0008 §5): a searchable list of citations on the left and
/// the selected citation's detail (overview + related-item tabs) on the right. Parallel to
/// [`PersonScreen`]; the research-grade Evidence Explained axes live on the overview.
#[component]
pub fn CitationScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let create_services = services.clone();
    let chrome = state.chrome();
    let entity = chrome.rail_label(Category::Citations.label_id());
    let loading = chrome.loading();
    let empty = state.data_loc().citation_list_empty();
    let prompt = chrome.citation_select_prompt();
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
        async move { load_screen(services, Intent::ShowCitationList).await }
    });
    let on_create = use_callback(move |(source, page): (String, Option<String>)| {
        let services = create_services.clone();
        spawn(async move {
            match create_citation_record(services, source, page).await {
                Ok(human_id) => {
                    creating.set(false);
                    nav.open_record(RecordRef {
                        category: Category::Citations,
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
                    category: Category::Citations,
                    human_id: row.id,
                    label: row.title,
                }),
                onnew: move |()| nav.request_new(),
            }
        },
        Some(ScreenData::Loaded(
            IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_),
        )) => rsx! {},
    };
    let detail_pane = match nav.active_record_ref() {
        Some(record) if record.category == Category::Citations => {
            let human_id = record.human_id;
            rsx! { CitationDetailPane { key: "{human_id}", human_id } }
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
                CreateCitationForm { onsubmit: move |payload| on_create.call(payload) }
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

/// The "New citation" form: a cited source `human_id` (required) plus an optional page.
#[component]
fn CreateCitationForm(onsubmit: EventHandler<(String, Option<String>)>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut source = use_signal(String::new);
    let mut page = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("source"), name: "source".to_owned(), oninput: move |event: FormEvent| source.set(event.value()) }
        Input { label: loc.field_label("page"), name: "page".to_owned(), oninput: move |event: FormEvent| page.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let source = source();
                if source.trim().is_empty() {
                    return;
                }
                onsubmit.call((source, non_empty(page())));
            },
        }
    }
}

/// Which citation edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitationEditForm {
    /// Set the page / locator.
    Page,
    /// Assert the cited record's date.
    Date,
    /// Set the operator's confidence.
    Confidence,
    /// Set the Evidence Explained analysis.
    Evidence,
    /// Add a typed attribute.
    Attribute,
    /// Attach a media object by `human_id`.
    Media,
    /// Attach a note by `human_id`.
    Note,
    /// Apply a tag (picked by name).
    Tag,
}

/// The detail pane for the selected citation: header, related-item tabs, editing side panel, toast.
#[component]
fn CitationDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let active = use_signal(|| 0_usize);
    let mut reload = use_signal(|| 0_u32);
    let editing = use_signal(|| None::<CitationEditForm>);
    let mut toast = use_signal(|| None::<String>);
    let saved_label = state.data_loc().action_label("saved");
    let dismiss_label = state.data_loc().action_label("dismiss");

    let id_for_resource = human_id.clone();
    let services_for_resource = services.clone();
    let data = use_resource(move || {
        let services = services_for_resource.clone();
        let human_id = id_for_resource.clone();
        let _ = reload();
        async move { load_screen(services, Intent::ShowCitation { human_id }).await }
    });

    let mut editing_for_submit = editing;
    let on_submit = use_callback(move |edit: CitationEdit| {
        let services = services.clone();
        let saved = saved_label.clone();
        spawn(async move {
            match save_citation_edit(services, edit).await {
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
        Some(ScreenData::Loaded(IntentOutcome::CitationDetail(detail))) => {
            citation_detail(&state, detail, active, editing, on_submit, &human_id)
        }
        Some(ScreenData::Loaded(IntentOutcome::List(_) | IntentOutcome::Detail(_) | IntentOutcome::Dashboard(_))) => {
            rsx! {}
        }
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

/// Renders a loaded citation's detail container: header (source, page, restriction toggles), the tab
/// strip, the active tab's content, and the editing side panel.
fn citation_detail(
    state: &AppState,
    detail: &CitationDetail,
    active: Signal<usize>,
    editing: Signal<Option<CitationEditForm>>,
    on_submit: Callback<CitationEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let tabs = citation_tabs(detail, loc);
    let tab_items: Vec<TabItem> = tabs
        .iter()
        .map(|tab| TabItem {
            id: tab.id.to_owned(),
            label: tab.label.clone(),
            count: tab.count,
        })
        .collect();
    let active_id = tabs.get(active()).map_or("overview", |tab| tab.id);
    let subtitle = detail.page.clone();
    rsx! {
        DetailContainer {
            title: detail.source.clone().unwrap_or_else(|| detail.human_id.clone()),
            subtitle,
            id_label: detail.human_id.clone(),
            avatar: "❝".to_owned(),
            extras: citation_restriction_toggles(loc, detail, on_submit, human_id),
            actions: rsx! {},
            tabs: tab_items,
            active,
            {citation_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {citation_edit_panel(state, editing, on_submit, human_id)}
    }
}

/// The interactive privacy-restriction toggles for a citation (the mockup `resn-set`).
fn citation_restriction_toggles(
    loc: &Localizer,
    detail: &CitationDetail,
    on_submit: Callback<CitationEdit>,
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
                on_submit.call(CitationEdit::SetRestrictions { human_id: human_id.clone(), restrictions: next });
            },
        }
    }
}

/// The content of one citation detail tab, with its contextual add/edit affordances.
fn citation_tab_content(
    state: &AppState,
    detail: &CitationDetail,
    tab_id: &str,
    mut editing: Signal<Option<CitationEditForm>>,
    on_submit: Callback<CitationEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "attributes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("add-attribute"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Attribute)) }
            }
            {citation_attributes_table(loc, &detail.attributes)}
        },
        "media" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-media"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Media)) }
            }
            {media_gallery(loc, &detail.media)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => citation_tags_panel(loc, detail, editing, on_submit, human_id),
        "history" => citation_history_tab(loc, detail, on_submit, human_id),
        _ => citation_overview(loc, detail, editing),
    }
}

/// The Overview tab: the evidence-first note, the source/page/date/confidence card with its edit
/// affordances, and the Evidence Explained axis chips (or a no-source flag when unsourced).
pub fn citation_overview(
    loc: &Localizer,
    detail: &CitationDetail,
    mut editing: Signal<Option<CitationEditForm>>,
) -> Element {
    rsx! {
        div { class: "section-note", "{loc.overview_note()}" }
        div { class: "grid-2",
            Card { title: loc.field_label("source"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"source\")}" }
                        span { class: "grow",
                            if let Some(source) = detail.source.as_deref() {
                                "{source}"
                            } else {
                                NoSourceFlag { label: loc.no_source() }
                            }
                        }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"page\")}" }
                        span { class: "grow", {detail.page.clone().unwrap_or_else(|| "—".to_owned())} }
                        Button { label: loc.action_label("set-page"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(CitationEditForm::Page)) }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"date\")}" }
                        span { class: "grow", {detail.date.clone().unwrap_or_else(|| "—".to_owned())} }
                        Button { label: loc.action_label("set-date"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(CitationEditForm::Date)) }
                    }
                }
            }
            Card { title: loc.field_label("evidence"),
                div { class: "stack",
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"surety\")}" }
                        span { class: "grow",
                            if let (Some(level), Some(label)) = (detail.confidence, detail.confidence_label.clone()) {
                                ConfidenceBadge { level, label }
                            } else {
                                "—"
                            }
                        }
                        Button { label: loc.action_label("set-confidence"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(CitationEditForm::Confidence)) }
                    }
                    div { class: "fact-row",
                        span { class: "field-label", style: "width:96px;margin:0", "{loc.field_label(\"evidence\")}" }
                        span { class: "grow wrap",
                            if detail.evidence_axes.is_empty() {
                                "—"
                            } else {
                                for chip in detail.evidence_axes.iter() {
                                    EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                                }
                            }
                        }
                        Button { label: loc.action_label("set-evidence"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| editing.set(Some(CitationEditForm::Evidence)) }
                    }
                }
            }
        }
    }
}

/// The Attributes tab: each recorded `(type, value)` attribute as a table row.
pub fn citation_attributes_table(loc: &Localizer, attributes: &[(String, String)]) -> Element {
    if attributes.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table { headers: vec![loc.field_label("attribute-type"), loc.field_label("value")],
            for (attribute_type , value) in attributes.iter() {
                tr {
                    td { "{attribute_type}" }
                    td { class: "muted", "{value}" }
                }
            }
        }
    }
}

/// The Tags tab: each applied tag as a colour-dot chip (name + colour, never the id) with a remove
/// control, plus an "Add tag" affordance.
pub fn citation_tags_panel(
    loc: &Localizer,
    detail: &CitationDetail,
    mut editing: Signal<Option<CitationEditForm>>,
    on_submit: Callback<CitationEdit>,
    human_id: &str,
) -> Element {
    let human_id = human_id.to_owned();
    rsx! {
        div { class: "tab-actions",
            Button { label: loc.action_label("add-tag"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(CitationEditForm::Tag)) }
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
                                    onclick: move |_| on_submit.call(CitationEdit::Tag { human_id: human_id.clone(), tag_id: tag_id.clone(), remove: true }),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The History tab: the per-record audit timeline, each undoable entry carrying an undo control.
fn citation_history_tab(
    loc: &Localizer,
    detail: &CitationDetail,
    on_submit: Callback<CitationEdit>,
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
                on_submit.call(CitationEdit::UndoAssertion { human_id: human_id.clone(), assertion_id });
            },
        }
    }
}

/// The citation editing side panel: renders the form for the open [`CitationEditForm`], or nothing.
fn citation_edit_panel(
    state: &AppState,
    mut editing: Signal<Option<CitationEditForm>>,
    on_submit: Callback<CitationEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        CitationEditForm::Page => loc.action_label("set-page"),
        CitationEditForm::Date => loc.action_label("set-date"),
        CitationEditForm::Confidence => loc.action_label("set-confidence"),
        CitationEditForm::Evidence => loc.action_label("set-evidence"),
        CitationEditForm::Attribute => loc.action_label("add-attribute"),
        CitationEditForm::Media => loc.action_label("attach-media"),
        CitationEditForm::Note => loc.action_label("attach-note"),
        CitationEditForm::Tag => loc.action_label("add-tag"),
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
                CitationEditForm::Page => rsx! { CitationPageForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Date => rsx! { CitationDateForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Confidence => rsx! { CitationConfidenceForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Evidence => rsx! { CitationEvidenceForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Attribute => rsx! { CitationAttributeForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Media => rsx! { CitationAttachForm { human_id, is_note: false, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Note => rsx! { CitationAttachForm { human_id, is_note: true, onsubmit: move |edit| on_submit.call(edit) } },
                CitationEditForm::Tag => rsx! { CitationTagForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
            }}
        }
    }
}

/// The "Set page" form → [`CitationEdit::SetPage`].
#[component]
fn CitationPageForm(human_id: String, onsubmit: EventHandler<CitationEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut page = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("page"), name: "page".to_owned(), oninput: move |event: FormEvent| page.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| onsubmit.call(CitationEdit::SetPage { human_id: human_id.clone(), page: page() }),
        }
    }
}

/// The "Set date" form (year required; month/day optional) → [`CitationEdit::SetDate`].
#[component]
fn CitationDateForm(human_id: String, onsubmit: EventHandler<CitationEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut year = use_signal(String::new);
    let mut month = use_signal(String::new);
    let mut day = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("date"), name: "year".to_owned(), oninput: move |event: FormEvent| year.set(event.value()) }
        Input { label: loc.field_label("attribute-type"), name: "month".to_owned(), oninput: move |event: FormEvent| month.set(event.value()) }
        Input { label: loc.field_label("value"), name: "day".to_owned(), oninput: move |event: FormEvent| day.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let Ok(year) = year().trim().parse::<i32>() else {
                    return;
                };
                let parts = DateParts {
                    year,
                    month: month().trim().parse::<u8>().ok(),
                    day: day().trim().parse::<u8>().ok(),
                };
                onsubmit.call(CitationEdit::SetDate { human_id: human_id.clone(), parts });
            },
        }
    }
}

/// The "Set confidence" form → [`CitationEdit::SetConfidence`].
#[component]
fn CitationConfidenceForm(human_id: String, onsubmit: EventHandler<CitationEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let levels = ConfidenceLevel::all();
    let mut index = use_signal(|| 2_usize);
    let options: Vec<SelectChoice> = levels
        .iter()
        .enumerate()
        .map(|(position, level)| SelectChoice {
            value: position.to_string(),
            label: loc.confidence_label(*level),
        })
        .collect();
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.field_label("confidence"),
            name: "confidence".to_owned(),
            value: Some(2.to_string()),
            options,
            onchange: move |event: FormEvent| index.set(event.value().parse().unwrap_or(2)),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let confidence = *levels.get(index()).unwrap_or(&ConfidenceLevel::Normal);
                onsubmit.call(CitationEdit::SetConfidence { human_id: human_id.clone(), confidence });
            },
        }
    }
}

/// The "Set evidence analysis" form: the three Evidence Explained axes → [`CitationEdit::SetEvidenceAnalysis`].
#[component]
fn CitationEvidenceForm(human_id: String, onsubmit: EventHandler<CitationEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let sources = [SourceQuality::Original, SourceQuality::Derivative];
    let informations = [InformationKind::Primary, InformationKind::Secondary];
    let evidences = [EvidenceKind::Direct, EvidenceKind::Indirect, EvidenceKind::Negative];
    let mut source_index = use_signal(|| 0_usize);
    let mut information_index = use_signal(|| 0_usize);
    let mut evidence_index = use_signal(|| 0_usize);
    let source_options: Vec<SelectChoice> = sources
        .iter()
        .enumerate()
        .map(|(position, quality)| SelectChoice {
            value: position.to_string(),
            label: loc.evidence_source_label(*quality),
        })
        .collect();
    let information_options: Vec<SelectChoice> = informations
        .iter()
        .enumerate()
        .map(|(position, kind)| SelectChoice {
            value: position.to_string(),
            label: loc.evidence_information_label(*kind),
        })
        .collect();
    let evidence_options: Vec<SelectChoice> = evidences
        .iter()
        .enumerate()
        .map(|(position, kind)| SelectChoice {
            value: position.to_string(),
            label: loc.evidence_kind_label(*kind),
        })
        .collect();
    let save_label = loc.action_label("save");
    rsx! {
        Select { label: loc.field_label("source"), name: "source".to_owned(), value: Some(0.to_string()), options: source_options, onchange: move |event: FormEvent| source_index.set(event.value().parse().unwrap_or(0)) }
        Select { label: loc.field_label("evidence"), name: "information".to_owned(), value: Some(0.to_string()), options: information_options, onchange: move |event: FormEvent| information_index.set(event.value().parse().unwrap_or(0)) }
        Select { label: loc.field_label("evidence"), name: "evidence".to_owned(), value: Some(0.to_string()), options: evidence_options, onchange: move |event: FormEvent| evidence_index.set(event.value().parse().unwrap_or(0)) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let analysis = EvidenceAnalysis {
                    source: *sources.get(source_index()).unwrap_or(&SourceQuality::Original),
                    information: *informations.get(information_index()).unwrap_or(&InformationKind::Primary),
                    evidence: *evidences.get(evidence_index()).unwrap_or(&EvidenceKind::Direct),
                };
                onsubmit.call(CitationEdit::SetEvidenceAnalysis { human_id: human_id.clone(), analysis });
            },
        }
    }
}

/// The "Add attribute" form → [`CitationEdit::AddAttribute`].
#[component]
fn CitationAttributeForm(human_id: String, onsubmit: EventHandler<CitationEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let mut attribute_type = use_signal(String::new);
    let mut value = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label("attribute-type"), name: "attribute-type".to_owned(), oninput: move |event: FormEvent| attribute_type.set(event.value()) }
        Input { label: loc.field_label("value"), name: "value".to_owned(), oninput: move |event: FormEvent| value.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let attribute_type = attribute_type();
                if attribute_type.trim().is_empty() {
                    return;
                }
                onsubmit.call(CitationEdit::AddAttribute { human_id: human_id.clone(), attribute_type, value: value() });
            },
        }
    }
}

/// The "Attach media/note by id" form → [`CitationEdit::AttachMedia`]/[`CitationEdit::AttachNote`].
#[component]
fn CitationAttachForm(human_id: String, is_note: bool, onsubmit: EventHandler<CitationEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let field = if is_note { "note" } else { "media" };
    let mut id = use_signal(String::new);
    let save_label = loc.action_label("save");
    rsx! {
        Input { label: loc.field_label(field), name: field.to_owned(), oninput: move |event: FormEvent| id.set(event.value()) }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let id = id();
                if id.trim().is_empty() {
                    return;
                }
                let edit = if is_note {
                    CitationEdit::AttachNote { human_id: human_id.clone(), note_id: id }
                } else {
                    CitationEdit::AttachMedia { human_id: human_id.clone(), media_id: id }
                };
                onsubmit.call(edit);
            },
        }
    }
}

/// The "Add tag" form: a picker of existing tags by name (the tag id is the option value, never
/// shown) → [`CitationEdit::Tag`].
#[component]
fn CitationTagForm(human_id: String, onsubmit: EventHandler<CitationEdit>) -> Element {
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
                        onsubmit.call(CitationEdit::Tag { human_id: human_id.clone(), tag_id, remove: false });
                    },
                }
            }
        }
    }
}
