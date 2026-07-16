use super::prelude::*;
use crate::i18n::Chrome;
use crate::services::{PluginRow, discover_plugins, set_plugin_enabled};
use genealogy_plugin_host::Capability;

/// The plugin manager (Phase 5 PR21; `plugin-manager.html`): a table of the plugins discovered on
/// disk — name/id, host-api version, an enabled/disabled switch, declared capability badges, and a
/// read-only trust tier — plus "Reload from disk" and the existing `ui-panel` plugin runner (ADR
/// 0012). Capabilities and the host-api version are read genuinely off each component
/// ([`genealogy_plugin_host::PluginHost::discover`]); no plugin-owned id/version/signature manifest
/// exists yet (ADR 0014, deferred), so every plugin is shown as `unsigned` — see
/// [`crate::i18n::Chrome::plugin_trust_note`].
#[component]
pub fn PluginPanelScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let mut reloads = use_signal(|| 0_u32);
    let mut toast = use_signal(|| None::<String>);

    let plugins_services = services.clone();
    let plugins = use_resource(move || {
        let services = plugins_services.clone();
        // Reading `reloads` subscribes the resource: "Reload from disk" and a toggle re-run it.
        let _ = reloads();
        async move { discover_plugins(services).await }
    });

    let toggle_services = services.clone();
    let on_toggle = Callback::new(move |(id, enabled): (String, bool)| {
        let services = toggle_services.clone();
        spawn(async move {
            match set_plugin_enabled(services, id, enabled).await {
                Ok(()) => reloads += 1,
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    rsx! {
        div { style: "padding:var(--sp-6);overflow:auto;height:100%",
            div { class: "row-actions", style: "justify-content:space-between;margin-bottom:12px",
                h1 { style: "border:0;margin:0", "{state.chrome().plugin_manager_title()}" }
                Button {
                    label: state.chrome().plugin_reload(),
                    onclick: move |_| reloads += 1,
                }
            }
            p { class: "muted", style: "margin-bottom:12px", "{state.chrome().plugin_manager_note()}" }
            {match &*plugins.read_unchecked() {
                None => rsx! { p { class: "loading", "{loading}" } },
                Some(Err(message)) => rsx! { p { class: "empty", "{message}" } },
                Some(Ok(rows)) if rows.is_empty() => rsx! {
                    EmptyState { message: state.chrome().plugin_manager_empty() }
                },
                Some(Ok(rows)) => plugin_table(state.chrome(), rows, on_toggle),
            }}
            Toast {
                visible: toast().is_some(),
                message: toast().unwrap_or_default(),
                action_label: Some(state.data_loc().action_label("dismiss")),
                onaction: move |_| toast.set(None),
            }
            div { class: "card", style: "margin-top:16px",
                h3 { "{state.chrome().run_plugin()}" }
                p { class: "muted", "{state.chrome().plugin_trust_note()}" }
                PluginFormRunner {}
            }
        }
    }
}

/// The plugin table: one row per discovered plugin (PR21). Exported so the SSR a11y test can render
/// it directly over hand-built rows, the pattern already used for the pedigree tree
/// ([`super::pedigree`]).
pub fn plugin_table(chrome: &Chrome, rows: &[PluginRow], on_toggle: Callback<(String, bool)>) -> Element {
    rsx! {
        Table { caption: chrome.plugin_manager_title(), headers: chrome.plugin_table_headers(),
            for row in rows.iter() {
                {plugin_row(chrome, row, on_toggle)}
            }
        }
    }
}

/// One plugin's table row: name/version caption, the enabled switch, its capability badges, and the
/// read-only trust badge.
fn plugin_row(chrome: &Chrome, row: &PluginRow, on_toggle: Callback<(String, bool)>) -> Element {
    let id = row.id.clone();
    let enabled = row.enabled;
    let switch_label = chrome.plugin_enabled_switch_label(&id);
    let state_label = chrome.plugin_enabled_state(enabled);
    let version_caption = chrome.plugin_host_api_version(&row.host_api_version);
    let role_label = chrome.plugin_role_label(row.role);
    rsx! {
        tr {
            td {
                div { b { "{row.id}" } }
                div { class: "muted", style: "font-size:var(--fs-sm)",
                    "{role_label} · "
                    span { class: "mono", "{version_caption}" }
                }
            }
            td {
                Switch {
                    checked: enabled,
                    label: switch_label,
                    state_text: state_label,
                    ontoggle: move |value| on_toggle.call((id.clone(), value)),
                }
            }
            td { class: "wrap",
                for capability in row.capabilities.iter().copied() {
                    {capability_badge(chrome, capability)}
                }
            }
            td {
                span { class: "badge", "{chrome.plugin_trust_unsigned()}" }
            }
        }
    }
}

/// A capability badge: text-only (no colour-only signal), naming the WIT interface the plugin's
/// world imports. The hue groups capabilities by what they let a plugin do — `commands` (the only
/// one that writes) stands out as `evidence`; the host-mediated bulk I/O capabilities are `source`;
/// the read-only/observability ones (`log`/`query`/`progress`) are `info`.
fn capability_badge(chrome: &Chrome, capability: Capability) -> Element {
    let hue = match capability {
        Capability::Commands => "evidence",
        Capability::ImportSource | Capability::ExportSink => "source",
        Capability::Log | Capability::Query | Capability::Progress => "info",
    };
    let class = format!("ev {hue}");
    rsx! {
        span { class, "{chrome.plugin_capability_label(capability)}" }
    }
}

/// Runs the `ui-panel` plugin, renders the panel it emits, and wires action submission (ADR 0012,
/// ADR 0022) — the plugin-manager's "try a plugin UI" affordance. On a successful submission that
/// returns a replacement panel the display swaps to it; a success/failure message is announced; and
/// "Run plugin" clears the submitted state and re-runs the plugin.
#[component]
fn PluginFormRunner() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loading = state.chrome().loading();
    let run_label = state.chrome().run_plugin();
    let mut runs = use_signal(|| 0_u32);
    let mut override_panel = use_signal(|| None::<Panel>);
    let mut outcome = use_signal(|| None::<SubmitResult>);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let load_services = services.clone();
    let loaded = use_resource(move || {
        let services = load_services.clone();
        // Reading `runs` subscribes the resource: clicking the button re-runs the plugin.
        let _ = runs();
        async move { load_plugin_panel(services).await }
    });

    let submit_services = services.clone();
    let onaction = Callback::new(move |action: PanelAction| {
        let services = submit_services.clone();
        busy.set(true);
        error.set(None);
        spawn(async move {
            match submit_plugin_panel(services, action.action, action.values).await {
                Ok(result) => {
                    if let SubmitResult::Success { panel: Some(panel), .. } = &result {
                        override_panel.set(Some(panel.clone()));
                    }
                    outcome.set(Some(result));
                }
                Err(message) => error.set(Some(message)),
            }
            busy.set(false);
        });
    });

    rsx! {
        div { class: "tab-body",
            Button {
                label: run_label,
                variant: ButtonVariant::Primary,
                onclick: move |_| {
                    override_panel.set(None);
                    outcome.set(None);
                    error.set(None);
                    runs += 1;
                },
            }
            if busy() {
                p { class: "loading", "{loading}" }
            }
            {submit_outcome_view(outcome().as_ref(), error().as_deref())}
            {match override_panel() {
                Some(panel) => rsx! { PanelView { panel, onaction } },
                None => match &*loaded.read_unchecked() {
                    None => rsx! { p { class: "loading", "{loading}" } },
                    Some(Err(message)) => rsx! { p { class: "empty", "{message}" } },
                    Some(Ok(panel)) => rsx! { PanelView { panel: panel.clone(), onaction } },
                },
            }}
        }
    }
}

/// Renders a submission outcome (ADR 0022 §2): a success confirmation as `role="status"`, and a
/// validation failure or a technical error as `role="alert"`. The messages are already resolved.
/// Exported so the SSR test can render it directly (the pattern [`plugin_table`] uses).
pub fn submit_outcome_view(outcome: Option<&SubmitResult>, error: Option<&str>) -> Element {
    rsx! {
        if let Some(error) = error {
            p { role: "alert", class: "empty", "{error}" }
        }
        match outcome {
            Some(SubmitResult::Success { message: Some(message), .. }) => rsx! {
                p { role: "status", class: "ok", "{message}" }
            },
            Some(SubmitResult::Failure { message }) => rsx! {
                p { role: "alert", class: "empty", "{message}" }
            },
            Some(SubmitResult::Success { message: None, .. }) | None => rsx! {},
        }
    }
}
