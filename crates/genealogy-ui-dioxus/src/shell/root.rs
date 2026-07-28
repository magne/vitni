//! The shell root: assembles the rail, top bar, tabstrip, work area, status bar, and overlays, and
//! installs the central keyboard dispatcher.

use dioxus::prelude::*;
use genealogy_app::ConfigStore;
use genealogy_ui::{Category, Destination, Tool};

use crate::app::{AppCtx, StartupPrefs};
use crate::components::Toast;
use crate::master_detail::MasterDetail;
use crate::screens::{
    DashboardScreen, ExportScreen, GeographyScreen, HelpScreen, ImportScreen, MergeScreen, PedigreeScreen,
    PluginPanelScreen, PreferencesScreen, RecordDetail,
};
use crate::services::load_counts;
use crate::shell::close_confirm::CloseConfirmDialog;
use crate::shell::explorer::Explorer;
use crate::shell::help_overlay::HelpOverlay;
use crate::shell::keyboard::{ShellNotices, dispatch, use_keyboard_dispatch};
use crate::shell::nav_state::{NavState, Overlay, entity_category};
use crate::shell::palette::CommandPalette;
use crate::shell::quit_manager::QuitManager;
use crate::shell::rail::Rail;
use crate::shell::statusbar::ShellStatusbar;
use crate::shell::tabstrip::RecordTabstrip;
use crate::shell::topbar::Topbar;
use crate::shell::window_geometry::WindowGeometryManager;
use crate::shell::{ChromeCtx, CountsCtx, NameCache, ShortcutsCtx};

/// The application shell. Provides [`NavState`], installs the keyboard layer, and lays out the rail,
/// top bar, tabstrip, work area, status bar, and overlays.
#[component]
pub fn Shell() -> Element {
    // An SSR test can inject a pre-configured `NavState` (e.g. with an overlay open) by providing one
    // above the shell; production never does, so the shell owns the state as before.
    let seeded_nav = try_consume_context::<NavState>();
    let nav = use_context_provider(|| {
        seeded_nav.unwrap_or_else(|| {
            let prefs = try_consume_context::<StartupPrefs>().unwrap_or_default();
            NavState::with_prefs(prefs.theme_mode, prefs.resolved_theme, prefs.recent)
        })
    });
    // The shared record-name cache backing every `RecordLink` (resolved once per data version).
    use_context_provider(|| NameCache(Signal::new(std::collections::HashMap::new())));
    // The live client-scope shortcut overrides (ADR 0030 §3): seeded from `StartupPrefs`, held as a
    // signal so saving the Preferences shortcuts card takes effect without a restart.
    let shortcuts_ctx = use_context_provider(|| {
        let prefs = try_consume_context::<StartupPrefs>().unwrap_or_default();
        ShortcutsCtx(Signal::new(prefs.shortcuts))
    });
    // Serve the workspace's media library to the webview at `/media/<rel>` (desktop only; a no-op
    // under SSR, which mounts no window). Registered once at shell mount.
    let media_root = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().dir.join("media")),
        _ => None,
    };
    crate::media_asset::use_media_asset_handler(media_root);
    // Persist the "Jump back in" list whenever it changes — best-effort, never blocks the UI, and a
    // no-op under SSR tests (no workspace). Mirrors the theme/geometry persistence precedent.
    let recent_dir = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().dir.clone()),
        _ => None,
    };
    use_effect(move || {
        let recent = nav.recent.read().clone();
        let Some(dir) = &recent_dir else { return };
        if let Err(error) = genealogy_app::FileConfigStore::for_workspace(dir.clone()).store_recent(&recent) {
            tracing::warn!(%error, "could not persist the recent list");
        }
    });
    let gp = use_keyboard_dispatch();
    let chrome = use_context::<ChromeCtx>();
    // The rail count badges: refetched whenever a mutation bumps `data_version`. Degrades to no
    // counts when the application state is absent (e.g. an SSR test renders the shell bare).
    let services = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        _ => None,
    };
    let counts = use_resource(move || {
        let services = services.clone();
        let _ = nav.data_version.read();
        async move {
            match services {
                Some(services) => load_counts(services).await,
                None => None,
            }
        }
    });
    use_context_provider(|| CountsCtx(counts));
    let theme = nav.theme.read().attr();
    let notices = ShellNotices {
        nothing_to_undo: chrome.0.kbd_nothing_to_undo(),
        redo_unavailable: chrome.0.kbd_redo_unavailable(),
    };
    let notice = nav.notice.read().clone();
    let notice_dismiss = chrome.0.notice_dismiss();
    let mut notice_nav = nav;
    // Two shell shapes (see `entity_category`): an entity category shows `rail | Explorer | editor`
    // (the record tabstrip + editor host mount); a tool/Dashboard/Help destination shows
    // `rail | screen` (no Explorer, no tabstrip — the screen fills the area right of the rail).
    let active_category = entity_category(*nav.active.read());
    let is_entity = active_category.is_some();
    let app_class = if is_entity { "app has-explorer" } else { "app" };
    // While a modal overlay (command palette / help sheet) is open the background shell is made inert
    // so Tab and assistive tech cannot reach behind the modal (ARIA APG modal pattern, U3). The
    // overlays are siblings of `.app`, never descendants, so inerting `.app` cannot disable them.
    // `inert`/`aria-hidden` are emitted via `then_some` (present only when open): `inert` is a boolean
    // HTML attribute, so on the live renderer a bare `inert: false` still renders the attribute and
    // freezes the whole app — it must be omitted, not set to false. (SSR omits false bools, so this
    // divergence is not caught by the SSR tests.)
    let overlay_open = *nav.overlay.read() != Overlay::None;
    rsx! {
        div {
            class: "{app_class}",
            "data-theme": "{theme}",
            tabindex: "-1",
            inert: overlay_open.then_some("true"),
            aria_hidden: overlay_open.then_some("true"),
            onkeydown: move |event| {
                let resolved = genealogy_ui::resolved_shortcuts(&shortcuts_ctx.0.read().bindings).0;
                dispatch(&event, nav, gp, &notices, &resolved);
            },
            a { class: "skip-link", href: "#main", "{chrome.0.skip_to_content()}" }
            Rail {}
            if let Some(category) = active_category {
                // Keyed by category so switching entity categories remounts the Explorer with the new
                // list. Without the key the component is reused and keeps the previous category's rows
                // (its list `use_resource` does not re-fetch on a prop change alone).
                Explorer { key: "{category.id()}" }
            }
            div { class: "shell",
                Topbar {}
                if is_entity {
                    RecordTabstrip {}
                }
                main { class: "workarea", id: "main", role: "main", tabindex: "-1",
                    Workarea {}
                }
                ShellStatusbar {}
            }
            Toast {
                visible: notice.is_some(),
                message: notice.unwrap_or_default(),
                action_label: notice_dismiss,
                onaction: move |_| notice_nav.dismiss_notice(),
            }
            WindowGeometryManager {}
            QuitManager {}
        }
        // Siblings of `.app` (not descendants) so inerting `.app` cannot inert the modal itself.
        CommandPalette {}
        HelpOverlay {}
        CloseConfirmDialog {}
    }
}

/// The work area's content. An entity category shows the editor host (the active record tab's detail
/// pane, plus the docked split); every tool, the Dashboard, and Help render their own full-width
/// screen unchanged.
#[component]
fn Workarea() -> Element {
    let nav = use_context::<NavState>();
    match *nav.active.read() {
        Destination::Category(Category::Dashboard) => rsx! { DashboardScreen {} },
        Destination::Category(_) => rsx! { MasterDetail { detail: rsx! { RecordDetail {} } } },
        Destination::Tool(Tool::Pedigree) => rsx! { PedigreeScreen {} },
        Destination::Tool(Tool::Merge) => rsx! { MergeScreen {} },
        Destination::Tool(Tool::Import) => rsx! { ImportScreen {} },
        Destination::Tool(Tool::Export) => rsx! { ExportScreen {} },
        Destination::Tool(Tool::Geography) => rsx! { GeographyScreen {} },
        Destination::Tool(Tool::Plugins) => rsx! { PluginPanelScreen {} },
        Destination::Tool(Tool::Preferences) => rsx! { PreferencesScreen {} },
        Destination::Help { topic } => rsx! { HelpScreen { topic } },
    }
}
