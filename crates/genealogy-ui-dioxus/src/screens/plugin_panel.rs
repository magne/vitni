use super::prelude::*;

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
