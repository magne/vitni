//! The application's own screens (ADR 0008 §5): per-framework RSX over the shared `genealogy-ui`
//! view-models. Person list → detail, plus the plugin panel that renders a plugin-supplied form
//! through the vocabulary interpreter.

use dioxus::prelude::*;
use genealogy_ui::{Intent, IntentOutcome, PersonRow};

use crate::app::AppCtx;
use crate::services::{ScreenData, load_plugin_form, load_screen};
use crate::vocabulary_render::FormView;

/// The person list. Each row opens the detail view via `on_open`.
#[component]
pub fn PersonListScreen(on_open: EventHandler<String>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loading = state.chrome().loading();
    let empty = state.data_loc().list_empty();
    let data = use_resource(move || {
        let services = services.clone();
        async move { load_screen(services, Intent::ShowList).await }
    });
    match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "error", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::List(rows))) if rows.is_empty() => {
            rsx! { p { class: "empty", "{empty}" } }
        }
        Some(ScreenData::Loaded(IntentOutcome::List(rows))) => rsx! {
            ul { class: "person-list",
                for row in rows.clone() {
                    PersonRowView { row, on_open }
                }
            }
        },
        Some(ScreenData::Loaded(IntentOutcome::Detail(_) | IntentOutcome::NotFound { .. })) => rsx! {},
    }
}

/// One row of the person list.
#[component]
fn PersonRowView(row: PersonRow, on_open: EventHandler<String>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let private = if row.private {
        state.data_loc().private_tag()
    } else {
        String::new()
    };
    let id = row.human_id.clone();
    rsx! {
        li { class: "person-row", onclick: move |_| on_open.call(id.clone()),
            span { class: "id", "{row.human_id}" }
            span { class: "name", "{row.name}" }
            span { class: "sex", "{row.sex}" }
            span { class: "private", "{private}" }
        }
    }
}

/// One person's detail view. `on_back` returns to the list.
#[component]
pub fn PersonDetailScreen(human_id: String, on_back: EventHandler<()>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let back = chrome.back();
    let labels = Labels {
        id: loc.label_id(),
        name: loc.label_name(),
        given: loc.label_given(),
        surname: loc.label_surname(),
        sex: loc.label_sex(),
        private: loc.label_private(),
    };
    let private_tag = loc.private_tag();
    let id_for_resource = human_id.clone();
    let data = use_resource(move || {
        let services = services.clone();
        let human_id = id_for_resource.clone();
        async move { load_screen(services, Intent::ShowPerson { human_id }).await }
    });
    rsx! {
        button { class: "back", onclick: move |_| on_back.call(()), "{back}" }
        {match &*data.read_unchecked() {
            None => rsx! { p { class: "loading", "{loading}" } },
            Some(ScreenData::Error(message)) => rsx! { p { class: "error", "{message}" } },
            Some(ScreenData::Loaded(IntentOutcome::NotFound { human_id })) => {
                rsx! { p { class: "not-found", "{chrome.not_found(human_id)}" } }
            }
            Some(ScreenData::Loaded(IntentOutcome::Detail(detail))) => {
                let private = if detail.private { private_tag.clone() } else { String::new() };
                rsx! {
                    dl { class: "detail",
                        dt { "{labels.id}" }
                        dd { "{detail.human_id}" }
                        dt { "{labels.name}" }
                        dd { "{detail.name}" }
                        dt { "{labels.given}" }
                        dd { "{detail.given.clone().unwrap_or_default()}" }
                        dt { "{labels.surname}" }
                        dd { "{detail.surname.clone().unwrap_or_default()}" }
                        dt { "{labels.sex}" }
                        dd { "{detail.sex}" }
                        dt { "{labels.private}" }
                        dd { "{private}" }
                    }
                }
            }
            Some(ScreenData::Loaded(IntentOutcome::List(_))) => rsx! {},
        }}
    }
}

/// The localized field labels for the detail view.
struct Labels {
    id: String,
    name: String,
    given: String,
    surname: String,
    sex: String,
    private: String,
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
        button { class: "run-plugin", onclick: move |_| runs += 1, "{run_label}" }
        {match &*form.read_unchecked() {
            None => rsx! { p { class: "loading", "{loading}" } },
            Some(Err(message)) => rsx! { p { class: "error", "{message}" } },
            Some(Ok(form)) => rsx! { FormView { form: form.clone() } },
        }}
    }
}
