//! The application's own screens (ADR 0008 §5): per-framework RSX over the shared `genealogy-ui`
//! view-models, built on the design-system components and the generic master-detail framework
//! (`crate::master_detail`). [`PersonScreen`] is the reference slice — adding an aggregate copies it:
//! supply a row builder (`genealogy_ui::person_row`-style), a tab builder
//! (`genealogy_ui::person_tabs`-style), and the tab-id→content match below; the list/detail layout,
//! search, sort, and keyboard come from the framework. The plugin panel renders a plugin-supplied
//! form through the vocabulary interpreter.

use dioxus::prelude::*;
use genealogy_app::{PersonNameParts, Sex};
use genealogy_ui::{
    AssociationVm, ConfidenceLevel, EventRefVm, FactVm, FamilyVm, Intent, IntentOutcome, Localizer, NameVm,
    PersonDetail, PersonEdit, RestrictionKind, person_tabs,
};

use crate::app::{AppCtx, AppState};
use crate::components::{
    Button, ButtonVariant, Card, ConfidenceBadge, EmptyState, Input, LabeledValue, NoSourceFlag, RestrictionChoice,
    RestrictionSet, Select, SelectChoice, SidePanel, SourceLink, TabItem, Table, Toast,
};
use crate::master_detail::{DetailContainer, ListChrome, ListPane, MasterDetail};
use crate::services::{ScreenData, load_plugin_form, load_screen, save_edit};
use crate::vocabulary_render::FormView;

/// The person master-detail: a searchable/sortable list on the left, the selected person's detail
/// (an overview tab plus related-item tabs) on the right.
#[component]
pub fn PersonScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let entity = chrome.nav_people();
    let loading = chrome.loading();
    let empty = state.data_loc().list_empty();
    let prompt = chrome.select_prompt();
    let list_chrome = ListChrome {
        list_label: entity.clone(),
        filter_placeholder: chrome.list_filter(&entity),
        sort_label: chrome.list_sort(),
        sort_options: chrome.sort_options(),
        empty,
    };
    let selected = use_signal(|| None::<String>);
    let query = use_signal(genealogy_ui::ListQuery::default);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowList).await }
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
            }
        },
        Some(ScreenData::Loaded(IntentOutcome::Detail(_) | IntentOutcome::NotFound { .. })) => rsx! {},
    };
    let detail_pane = match selected() {
        Some(human_id) => rsx! { PersonDetailPane { human_id } },
        None => rsx! { p { class: "empty", "{prompt}" } },
    };
    rsx! {
        MasterDetail { list: list_pane, detail: detail_pane }
    }
}

/// Which edit form (if any) the side panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditForm {
    /// Assert an additional name.
    Name,
    /// Assert a fact, with confidence and an optional source.
    Fact,
    /// Assert the person's sex.
    Sex,
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
            person_detail(&state, detail, active, editing, on_submit, &human_id)
        }
        Some(ScreenData::Loaded(IntentOutcome::List(_))) => rsx! {},
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

/// Renders a loaded person's detail container: header, tab strip, the active tab's content, and the
/// editing side panel.
fn person_detail(
    state: &AppState,
    detail: &PersonDetail,
    active: Signal<usize>,
    editing: Signal<Option<EditForm>>,
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
    let badges: Vec<String> = detail
        .restrictions
        .iter()
        .map(|&kind| loc.restriction_label(kind))
        .collect();
    let active_id = tabs.get(active()).map_or("overview", |tab| tab.id);
    rsx! {
        DetailContainer {
            title: detail.name.clone(),
            subtitle: detail.sex.clone(),
            id_label: detail.human_id.clone(),
            badges,
            tabs: tab_items,
            active,
            {person_tab_content(state, detail, active_id, editing, on_submit, human_id)}
        }
        {edit_panel(state, editing, on_submit, human_id)}
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
            {id_list(loc, &detail.media)}
        },
        "notes" => rsx! {
            div { class: "tab-actions",
                Button { label: loc.action_label("attach-note"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Note)) }
            }
            {id_list(loc, &detail.notes)}
        },
        "tags" => id_list(loc, &detail.tags),
        "history" => rsx! { EmptyState { symbol: "🕓".to_owned(), message: loc.history_placeholder() } },
        _ => overview_tab(loc, detail, editing, on_submit, human_id),
    }
}

/// The overview tab: the core labelled values, the sex-edit affordance, and the restriction toggles.
fn overview_tab(
    loc: &Localizer,
    detail: &PersonDetail,
    mut editing: Signal<Option<EditForm>>,
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
        Card {
            LabeledValue { label: loc.label_id(), value: detail.human_id.clone() }
            LabeledValue { label: loc.label_given(), value: detail.given.clone().unwrap_or_default() }
            LabeledValue { label: loc.label_surname(), value: detail.surname.clone().unwrap_or_default() }
            LabeledValue { label: loc.label_sex(), value: detail.sex.clone() }
            div { class: "tab-actions",
                Button { label: loc.action_label("edit"), variant: ButtonVariant::Default, onclick: move |_| editing.set(Some(EditForm::Sex)) }
            }
            div { class: "field",
                label { "{loc.label_private()}" }
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
    }
}

/// The Names tab: every asserted name variant with its type.
pub fn names_table(loc: &Localizer, names: &[NameVm]) -> Element {
    if names.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    rsx! {
        Table { headers: vec![loc.field_label("name-type"), loc.label_name(), loc.field_label("nickname")],
            for name in names.iter() {
                tr {
                    td { "{name.type_label}" }
                    td { "{name.display}" }
                    td { {name.nickname.clone().unwrap_or_default()} }
                }
            }
        }
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
                loc.field_label("value"),
                loc.field_label("date"),
                loc.field_label("confidence"),
                loc.field_label("citation"),
            ],
            for fact in facts.iter() {
                tr {
                    td { "{fact.type_label}" }
                    td { {fact.value.clone().unwrap_or_default()} }
                    td { {fact.date.clone().unwrap_or_default()} }
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
        Table { headers: vec![loc.field_label("role"), loc.label_id(), loc.field_label("date")],
            for event in events.iter() {
                tr {
                    td { "{event.role_label}" }
                    td { "{event.event_id}" }
                    td { {event.date.clone().unwrap_or_default()} }
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
        Table { headers: vec![loc.field_label("association"), loc.field_label("role")],
            for association in associations.iter() {
                tr {
                    td { "{association.other_id}" }
                    td { "{association.role_label}" }
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
        div { class: "stack",
            for family in families.iter() {
                Card { title: format!("{} · {}", family.family_id, family.role_label),
                    Table { headers: vec![loc.field_label("association"), loc.field_label("role")],
                        for partner in family.partners.iter() {
                            tr {
                                td { "{partner}" }
                                td { {loc.partner_role_label()} }
                            }
                        }
                        for (child , relationship) in family.children.iter() {
                            tr {
                                td { "{child}" }
                                td { "{relationship}" }
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

/// The editing side panel: renders the form for the currently-open [`EditForm`], or nothing.
fn edit_panel(
    state: &AppState,
    mut editing: Signal<Option<EditForm>>,
    on_submit: Callback<PersonEdit>,
    human_id: &str,
) -> Element {
    let loc = state.data_loc();
    let Some(form) = editing() else {
        return rsx! {};
    };
    let title = match form {
        EditForm::Name => loc.action_label("add-name"),
        EditForm::Fact => loc.action_label("add-fact"),
        EditForm::Sex => loc.label_sex(),
        EditForm::Citation => loc.action_label("attach-citation"),
        EditForm::Media => loc.action_label("attach-media"),
        EditForm::Note => loc.action_label("attach-note"),
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
                EditForm::Name => rsx! { AddNameForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Fact => rsx! { AddFactForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
                EditForm::Sex => rsx! { EditSexForm { human_id, onsubmit: move |edit| on_submit.call(edit) } },
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

/// The "Edit sex" side-panel form: a sex picker → [`PersonEdit::AssertSex`].
#[component]
fn EditSexForm(human_id: String, onsubmit: EventHandler<PersonEdit>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let sexes = [Sex::Female, Sex::Male, Sex::Unknown, Sex::Intersex];
    let mut sex_index = use_signal(|| 0_usize);
    let options: Vec<SelectChoice> = sexes
        .iter()
        .enumerate()
        .map(|(index, sex)| SelectChoice {
            value: index.to_string(),
            label: loc.sex_label(Some(sex)),
        })
        .collect();
    let save_label = loc.action_label("save");
    rsx! {
        Select {
            label: loc.label_sex(),
            name: "sex".to_owned(),
            options,
            onchange: move |event: FormEvent| sex_index.set(event.value().parse().unwrap_or(0)),
        }
        Button {
            label: save_label,
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                let sex = sexes.get(sex_index()).cloned().unwrap_or(Sex::Unknown);
                onsubmit.call(PersonEdit::AssertSex { human_id: human_id.clone(), sex });
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
