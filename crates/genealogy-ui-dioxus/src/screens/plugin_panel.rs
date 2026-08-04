use super::prelude::*;
use crate::i18n::Chrome;
use crate::services::{
    PluginRow, discover_plugins, load_trust_store, pin_publisher, set_plugin_enabled, set_plugin_grants,
    unpin_publisher,
};
use genealogy_plugin_host::Capability;

/// The plugin manager (`plugin-manager.html`; ADR 0014): a table of the plugins discovered across the
/// bundle layers — name/id, host-api version, an enabled/disabled switch, declared capability badges,
/// and the trust tier its signature places it in — plus per-plugin capability-grant approval cards and
/// the client-scope pinned-publisher trust store. Trust tiers and declared capabilities are read
/// genuinely off each bundle ([`genealogy_plugin_host::PluginHost::discover_bundle`]); the effective
/// grant is the intersection of declared and user-approved (ADR 0014 §5).
#[component]
pub fn PluginPanelScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let chrome = state.chrome();
    let loading = chrome.loading();
    let mut reloads = use_signal(|| 0_u32);
    let mut nav = use_context::<NavState>();

    let plugins_services = services.clone();
    let plugins = use_resource(move || {
        let services = plugins_services.clone();
        // Reading `reloads` subscribes the resource: "Reload from disk", a toggle, and a grant save re-run it.
        let _ = reloads();
        async move { discover_plugins(services).await }
    });

    let toggle_services = services.clone();
    let on_toggle = Callback::new(move |(id, enabled): (String, bool)| {
        let services = toggle_services.clone();
        spawn(async move {
            match set_plugin_enabled(services, id, enabled).await {
                Ok(()) => reloads += 1,
                Err(message) => nav.notify_error(message),
            }
        });
    });

    let on_grants_saved = Callback::new(move |()| reloads += 1);

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
                Some(Ok(rows)) => rsx! {
                    {plugin_table(state.chrome(), state.data_loc(), rows, on_toggle)}
                    div { class: "stack", style: "margin-top:16px",
                        h3 { "{state.data_loc().plugin_grants_heading()}" }
                        for row in rows.iter() {
                            PluginGrantCard { key: "{row.id}", row: row.clone(), onsaved: on_grants_saved }
                        }
                    }
                },
            }}
            TrustStoreCard {}
            div { class: "card", style: "margin-top:16px",
                h3 { "{state.chrome().run_plugin()}" }
                p { class: "muted", "{state.chrome().plugin_trust_note()}" }
                PluginFormRunner {}
            }
        }
    }
}

/// The plugin table: one row per discovered plugin. Exported so the SSR a11y test can render it
/// directly over hand-built rows, the pattern already used for the pedigree tree ([`super::pedigree`]).
pub fn plugin_table(
    chrome: &Chrome,
    loc: &Localizer,
    rows: &[PluginRow],
    on_toggle: Callback<(String, bool)>,
) -> Element {
    rsx! {
        Table { caption: chrome.plugin_manager_title(), headers: chrome.plugin_table_headers(),
            for row in rows.iter() {
                {plugin_row(chrome, loc, row, on_toggle)}
            }
        }
    }
}

/// One plugin's table row: name/version caption, the enabled switch, its capability badges, and the
/// trust-tier badge (ADR 0014 §3).
fn plugin_row(chrome: &Chrome, loc: &Localizer, row: &PluginRow, on_toggle: Callback<(String, bool)>) -> Element {
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
                span { class: "badge", "{loc.plugin_trust_label(row.trust)}" }
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
        Capability::ImportSource
        | Capability::ExportSink
        | Capability::Net
        | Capability::MediaStore
        | Capability::Ai => "source",
        Capability::Log | Capability::Query | Capability::Progress | Capability::Present => "info",
    };
    let class = format!("ev {hue}");
    rsx! {
        span { class, "{chrome.plugin_capability_label(capability)}" }
    }
}

/// A per-plugin capability-grant card (ADR 0014 §5): a per-capability approve switch (deny-by-default),
/// an "approve all declared" action for a trusted plugin, and a clear "needs approval" state for a
/// plugin with no recorded decision. Saving persists the approved set through the intent, which the
/// host later intersects with the plugin's declared capabilities.
#[component]
fn PluginGrantCard(row: PluginRow, onsaved: Callback<()>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let mut nav = use_context::<NavState>();

    let declared: Vec<String> = row.capabilities.iter().map(|c| c.interface_name().to_owned()).collect();
    let vm = genealogy_ui::plugin_grant_vm(loc, &row.id, row.trust, &declared, row.approved.as_ref());
    let initial_approved = vm.approved_names();
    let mut approved = use_signal(move || initial_approved);

    let all_declared = vm.all_declared_names();
    let save_id = row.id.clone();
    let save_services = services.clone();
    let on_save = move |_| {
        let services = save_services.clone();
        let id = save_id.clone();
        let set = approved();
        spawn(async move {
            match set_plugin_grants(services, id, set).await {
                Ok(()) => onsaved.call(()),
                Err(message) => nav.notify_error(message),
            }
        });
    };

    rsx! {
        div { class: "card",
            div { class: "row-actions", style: "justify-content:space-between",
                div {
                    b { "{row.id}" }
                    span { class: "badge", style: "margin-left:8px", "{vm.trust_label}" }
                    if vm.pending {
                        span { class: "badge", style: "margin-left:8px;border-color:var(--warn);color:var(--warn)",
                            "{loc.plugin_grant_pending()}"
                        }
                    }
                }
                div { class: "row-actions",
                    if vm.allow_approve_all {
                        Button {
                            label: loc.plugin_action_approve_all(),
                            onclick: move |_| approved.set(all_declared.clone()),
                        }
                    }
                    Button {
                        label: loc.plugin_action_save_grants(),
                        variant: ButtonVariant::Primary,
                        onclick: on_save,
                    }
                }
            }
            div { class: "stack", style: "margin-top:8px",
                for capability in vm.capabilities.iter() {
                    {grant_switch(loc, capability, approved)}
                }
            }
        }
    }
}

/// One capability's approve switch inside a [`PluginGrantCard`], toggling its name in the approved set.
fn grant_switch(
    loc: &Localizer,
    capability: &genealogy_ui::CapabilityGrantVm,
    mut approved: Signal<std::collections::BTreeSet<String>>,
) -> Element {
    let name = capability.name.clone();
    // Read the live approved set (not the VM's initial value) so a toggle re-renders the switch.
    let checked = approved.read().contains(&capability.name);
    rsx! {
        div { class: "fact-row",
            Switch {
                checked,
                label: capability.label.clone(),
                state_text: capability.label.clone(),
                ontoggle: move |value: bool| {
                    approved.with_mut(|set| {
                        if value {
                            set.insert(name.clone());
                        } else {
                            set.remove(&name);
                        }
                    });
                },
            }
            span { class: "grow muted", "{loc.plugin_capability_label(&capability.name)}" }
        }
    }
}

/// The pinned-publisher trust-store editor (ADR 0014 §3): the list of pinned publishers with a short
/// key fingerprint and an unpin action, plus a form to pin a new publisher by identity + public key.
#[component]
fn TrustStoreCard() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let loc = state.data_loc();
    let mut reloads = use_signal(|| 0_u32);
    let mut nav = use_context::<NavState>();
    let mut publisher = use_signal(String::new);
    let mut key = use_signal(String::new);

    let load_services = services.clone();
    let store = use_resource(move || {
        let services = load_services.clone();
        let _ = reloads();
        async move { load_trust_store(services).await }
    });

    let pin_services = services.clone();
    let on_pin = move |_| {
        let services = pin_services.clone();
        let publisher_value = publisher().trim().to_owned();
        let key_value = key().trim().to_owned();
        spawn(async move {
            match pin_publisher(services, publisher_value, key_value).await {
                Ok(()) => {
                    publisher.set(String::new());
                    key.set(String::new());
                    reloads += 1;
                }
                Err(message) => nav.notify_error(message),
            }
        });
    };

    let unpin_services = services.clone();
    let on_unpin = Callback::new(move |name: String| {
        let services = unpin_services.clone();
        spawn(async move {
            match unpin_publisher(services, name).await {
                Ok(()) => reloads += 1,
                Err(message) => nav.notify_error(message),
            }
        });
    });

    rsx! {
        div { class: "card", style: "margin-top:16px",
            h3 { "{loc.plugin_trust_store_heading()}" }
            {match &*store.read_unchecked() {
                None => rsx! { p { class: "loading", "{state.chrome().loading()}" } },
                Some(Err(message)) => rsx! { p { class: "empty", "{message}" } },
                Some(Ok(config)) => {
                    let vm = genealogy_ui::trust_store_vm(config);
                    if vm.publishers.is_empty() {
                        rsx! { p { class: "muted", "{loc.plugin_trust_store_empty()}" } }
                    } else {
                        rsx! {
                            div { class: "stack",
                                for pin in vm.publishers.iter() {
                                    {pinned_publisher_row(loc, pin, on_unpin)}
                                }
                            }
                        }
                    }
                }
            }}
            div { class: "grid-2", style: "margin-top:10px",
                TextField {
                    label: loc.plugin_trust_publisher_label(),
                    name: "trust-publisher".to_owned(),
                    value: publisher(),
                    oninput: move |event: FormEvent| publisher.set(event.value()),
                }
                TextField {
                    label: loc.plugin_trust_key_label(),
                    name: "trust-key".to_owned(),
                    value: key(),
                    mono: true,
                    oninput: move |event: FormEvent| key.set(event.value()),
                }
            }
            div { class: "row-actions", style: "margin-top:8px",
                Button {
                    label: loc.plugin_action_pin(),
                    variant: ButtonVariant::Primary,
                    onclick: on_pin,
                }
            }
        }
    }
}

/// One pinned publisher's row: identity + short key fingerprint + an unpin action.
fn pinned_publisher_row(loc: &Localizer, pin: &genealogy_ui::PinnedPublisherVm, on_unpin: Callback<String>) -> Element {
    let publisher = pin.publisher.clone();
    rsx! {
        div { class: "fact-row",
            span { style: "min-width:160px", b { "{pin.publisher}" } }
            span { class: "grow mono muted", "{pin.fingerprint}…" }
            Button {
                label: loc.plugin_action_unpin(&pin.publisher),
                onclick: move |_| on_unpin.call(publisher.clone()),
            }
        }
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
