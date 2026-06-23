//! The application's own screens (ADR 0008 §5): per-framework RSX over the shared `genealogy-ui`
//! view-models, built on the design-system components and the generic master-detail framework
//! (`crate::master_detail`). [`PersonScreen`] is the reference slice — adding an aggregate copies it:
//! supply a row builder (`genealogy_ui::person_row`-style), a tab builder
//! (`genealogy_ui::person_tabs`-style), and the tab-id→content match below; the list/detail layout,
//! search, sort, and keyboard come from the framework. The plugin panel renders a plugin-supplied
//! form through the vocabulary interpreter.

use dioxus::prelude::*;
use genealogy_ui::{Intent, IntentOutcome, PersonDetail, person_tabs};

use crate::app::{AppCtx, AppState};
use crate::components::{Button, ButtonVariant, Card, EmptyState, LabeledValue, TabItem};
use crate::master_detail::{DetailContainer, ListChrome, ListPane, MasterDetail};
use crate::services::{ScreenData, load_plugin_form, load_screen};
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

/// The detail pane for the selected person: a header plus the related-item tab strip.
#[component]
fn PersonDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let active = use_signal(|| 0_usize);
    let id_for_resource = human_id.clone();
    let data = use_resource(move || {
        let services = services.clone();
        let human_id = id_for_resource.clone();
        async move { load_screen(services, Intent::ShowPerson { human_id }).await }
    });
    match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
            rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::Detail(detail))) => person_detail(&state, detail, active),
        Some(ScreenData::Loaded(IntentOutcome::List(_))) => rsx! {},
    }
}

/// Renders a loaded person's detail container: header, tab strip, and the active tab's content.
fn person_detail(state: &AppState, detail: &PersonDetail, active: Signal<usize>) -> Element {
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
    let badges = if detail.private {
        vec![loc.private_tag()]
    } else {
        Vec::new()
    };
    let active_id = tabs.get(active()).map_or("overview", |tab| tab.id);
    rsx! {
        DetailContainer {
            title: detail.name.clone(),
            subtitle: detail.sex.clone(),
            id_label: detail.human_id.clone(),
            badges,
            tabs: tab_items,
            active,
            {person_tab_content(state, detail, active_id)}
        }
    }
}

/// The content of one person detail tab. The overview is a labelled-value card; the related tabs
/// list their member ids (the rich tables land in the Person editing slice, PR4).
fn person_tab_content(state: &AppState, detail: &PersonDetail, tab_id: &str) -> Element {
    let loc = state.data_loc();
    match tab_id {
        "citations" => related_list(&detail.citations, &state.chrome().tab_empty()),
        "media" => related_list(&detail.media, &state.chrome().tab_empty()),
        "notes" => related_list(&detail.notes, &state.chrome().tab_empty()),
        "tags" => related_list(&detail.tags, &state.chrome().tab_empty()),
        _ => rsx! {
            Card {
                LabeledValue { label: loc.label_id(), value: detail.human_id.clone() }
                LabeledValue { label: loc.label_given(), value: detail.given.clone().unwrap_or_default() }
                LabeledValue { label: loc.label_surname(), value: detail.surname.clone().unwrap_or_default() }
                LabeledValue { label: loc.label_sex(), value: detail.sex.clone() }
            }
        },
    }
}

/// A minimal list of related-item ids, or an empty-state when there are none.
fn related_list(ids: &[String], empty: &str) -> Element {
    if ids.is_empty() {
        return rsx! { EmptyState { message: "{empty}" } };
    }
    rsx! {
        ul { class: "id-list",
            for id in ids.iter() {
                li { "{id}" }
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
