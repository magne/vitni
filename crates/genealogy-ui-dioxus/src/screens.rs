//! The application's own screens (ADR 0008 §5): per-framework RSX over the shared `genealogy-ui`
//! view-models, built on the design-system components. A master-detail person view (a `listbox` of
//! rows + a detail pane), plus the plugin panel that renders a plugin-supplied form through the
//! vocabulary interpreter.

use dioxus::prelude::*;
use genealogy_ui::{Intent, IntentOutcome};

use crate::app::AppCtx;
use crate::components::{Badge, Button, ButtonVariant, Card, LabeledValue, ListRow};
use crate::services::{ScreenData, load_plugin_form, load_screen};
use crate::vocabulary_render::FormView;

/// The person master-detail: a selectable list on the left, the selected person's detail on the right.
#[component]
pub fn PersonScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loading = state.chrome().loading();
    let empty = state.data_loc().list_empty();
    let prompt = state.chrome().select_prompt();
    let list_label = state.chrome().nav_people();
    let selected = use_signal(|| None::<String>);
    let list = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowList).await }
    });
    rsx! {
        div { class: "master-detail",
            aside { class: "list",
                {match &*list.read_unchecked() {
                    None => rsx! { p { class: "loading", "{loading}" } },
                    Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
                    Some(ScreenData::Loaded(IntentOutcome::List(rows))) if rows.is_empty() => {
                        rsx! { p { class: "empty", "{empty}" } }
                    }
                    Some(ScreenData::Loaded(IntentOutcome::List(rows))) => rsx! {
                        div { class: "list-rows", role: "listbox", aria_label: "{list_label}",
                            for row in rows.clone() {
                                PersonRowItem {
                                    human_id: row.human_id.clone(),
                                    name: row.name.clone(),
                                    sex: row.sex.clone(),
                                    selected,
                                }
                            }
                        }
                    },
                    Some(ScreenData::Loaded(IntentOutcome::Detail(_) | IntentOutcome::NotFound { .. })) => rsx! {},
                }}
            }
            section { class: "detail",
                {match selected() {
                    Some(human_id) => rsx! { PersonDetailPane { human_id } },
                    None => rsx! { p { class: "empty", "{prompt}" } },
                }}
            }
        }
    }
}

/// One selectable person row, wiring a [`ListRow`] to the selection signal.
#[component]
fn PersonRowItem(human_id: String, name: String, sex: String, selected: Signal<Option<String>>) -> Element {
    let is_selected = selected().as_deref() == Some(human_id.as_str());
    let id = human_id.clone();
    rsx! {
        ListRow {
            title: name,
            subtitle: sex,
            id_label: human_id,
            selected: is_selected,
            onclick: move |_| selected.set(Some(id.clone())),
        }
    }
}

/// The detail pane for the selected person.
#[component]
fn PersonDetailPane(human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let labels = Labels {
        id: loc.label_id(),
        given: loc.label_given(),
        surname: loc.label_surname(),
        sex: loc.label_sex(),
    };
    let private_tag = loc.private_tag();
    let id_for_resource = human_id.clone();
    let data = use_resource(move || {
        let services = services.clone();
        let human_id = id_for_resource.clone();
        async move { load_screen(services, Intent::ShowPerson { human_id }).await }
    });
    rsx! {
        {match &*data.read_unchecked() {
            None => rsx! { p { class: "loading", "{loading}" } },
            Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
            Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
                rsx! { p { class: "empty", "{chrome.not_found(human_id)}" } }
            }
            Some(ScreenData::Loaded(IntentOutcome::Detail(detail))) => rsx! {
                div { class: "detail-head",
                    div { class: "detail-title", "{detail.name}" }
                    if detail.private {
                        Badge { label: private_tag.clone() }
                    }
                }
                div { class: "tab-body",
                    Card {
                        LabeledValue { label: labels.id.clone(), value: detail.human_id.clone() }
                        LabeledValue { label: labels.given.clone(), value: detail.given.clone().unwrap_or_default() }
                        LabeledValue { label: labels.surname.clone(), value: detail.surname.clone().unwrap_or_default() }
                        LabeledValue { label: labels.sex.clone(), value: detail.sex.clone() }
                    }
                }
            },
            Some(ScreenData::Loaded(IntentOutcome::List(_))) => rsx! {},
        }}
    }
}

/// The localized field labels for the detail pane.
struct Labels {
    id: String,
    given: String,
    surname: String,
    sex: String,
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
