//! The top bar (`banner`): an active-record breadcrumb, the global search box, and the theme and
//! help controls.

use dioxus::prelude::*;
use genealogy_app::{ConfigStore, ThemeMode};
use genealogy_ui::Destination;

use crate::app::AppCtx;
use crate::components::{Breadcrumb, IconButton, TextInput};
use crate::shell::ChromeCtx;
use crate::shell::nav_state::{NavState, Overlay, Theme};

/// The control glyph for a theme mode (system / light / dark).
fn theme_icon(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => "◐",
        ThemeMode::Light => "☀",
        ThemeMode::Dark => "☾",
    }
}

/// Persists the selected theme mode into the open workspace's manifest, best-effort (a write failure
/// is logged, never surfaced — the in-memory theme already changed). No-op under SSR/host-free tests
/// where the application state is absent.
fn persist_theme_mode(mode: ThemeMode) {
    if let Some(AppCtx::Ready(state)) = try_consume_context::<AppCtx>() {
        let dir = state.services().dir.clone();
        if let Err(error) = genealogy_app::FileConfigStore::for_workspace(dir).store_theme(mode) {
            tracing::warn!(%error, "could not persist the theme mode");
        }
    }
}

/// The shell top bar.
#[component]
pub fn Topbar() -> Element {
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();
    let mut query = use_signal(String::new);
    let mut focused = use_signal(|| false);
    let active = *nav.active.read();
    let mut segments = vec![chrome.0.rail_label(active.label_id())];
    // Append the active record only while its own category is showing — otherwise the breadcrumb on
    // the dashboard (or another screen) would trail a record that screen isn't displaying.
    if let Some(record) = nav.active_record_ref()
        && active == Destination::Category(record.category)
    {
        segments.push(record.label);
    }
    rsx! {
        header { class: "topbar", role: "banner",
            nav { class: "breadcrumb-wrap", aria_label: "{chrome.0.aria_breadcrumb()}",
                Breadcrumb { segments }
            }
            div { class: "search", role: "search",
                span { aria_hidden: "true", "🔍" }
                label { class: "sr-only", r#for: "global-search", "{chrome.0.search_label()}" }
                TextInput {
                    id: "global-search",
                    class: "",
                    placeholder: "{chrome.0.search_placeholder()}",
                    value: "{query}",
                    oninput: move |event: FormEvent| query.set(event.value()),
                    onfocusin: move |_| focused.set(true),
                    onfocusout: move |_| focused.set(false),
                    onkeydown_extra: move |event: KeyboardEvent| {
                        if event.key() == Key::Enter {
                            event.prevent_default();
                            nav.open_palette_seeded(query());
                        }
                    },
                }
                if focused() {
                    button {
                        class: "search-clear",
                        r#type: "button",
                        aria_label: chrome.0.search_clear(),
                        onmousedown: move |event| event.prevent_default(),
                        onclick: move |_| query.set(String::new()),
                        "✕"
                    }
                } else {
                    kbd { aria_hidden: "true", "⌘K" }
                }
            }
            IconButton {
                icon: theme_icon(*nav.theme_mode.read()).to_owned(),
                label: chrome.0.aria_theme_cycle(*nav.theme_mode.read()),
                title: chrome.0.theme_mode_status(*nav.theme_mode.read(), *nav.theme.read() == Theme::Dark),
                onclick: move |_| {
                    let next = nav.cycle_theme();
                    persist_theme_mode(next);
                },
            }
            IconButton {
                icon: "?".to_owned(),
                label: chrome.0.aria_help(),
                onclick: move |_| nav.overlay.set(Overlay::Help),
            }
        }
    }
}
