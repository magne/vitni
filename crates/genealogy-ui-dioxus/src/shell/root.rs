//! The shell root: assembles the rail, top bar, tabstrip, work area, status bar, and overlays, and
//! installs the central keyboard dispatcher.

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination, Tool};

use crate::app::{AppCtx, StartupPrefs};
use crate::components::Toast;
use crate::screens::{
    CitationScreen, DashboardScreen, DnaMatchScreen, DnaTestScreen, EventScreen, FamilyScreen, HelpScreen, MediaScreen,
    MergeScreen, NoteScreen, PedigreeScreen, PersonScreen, PlaceScreen, PluginPanelScreen, PreferencesScreen,
    RepositoryScreen, SourceScreen, TagScreen,
};
use crate::services::load_counts;
use crate::shell::help_overlay::HelpOverlay;
use crate::shell::keyboard::{ShellNotices, dispatch, use_keyboard_dispatch};
use crate::shell::nav_state::NavState;
use crate::shell::palette::CommandPalette;
use crate::shell::rail::Rail;
use crate::shell::statusbar::ShellStatusbar;
use crate::shell::tabstrip::RecordTabstrip;
use crate::shell::topbar::Topbar;
use crate::shell::window_geometry::WindowGeometryManager;
use crate::shell::{ChromeCtx, CountsCtx, NameCache};

/// The application shell. Provides [`NavState`], installs the keyboard layer, and lays out the rail,
/// top bar, tabstrip, work area, status bar, and overlays.
#[component]
pub fn Shell() -> Element {
    let nav = use_context_provider(|| {
        let prefs = try_consume_context::<StartupPrefs>().unwrap_or_default();
        NavState::with_prefs(prefs.theme_mode, prefs.resolved_theme, prefs.recent)
    });
    // The shared record-name cache backing every `RecordLink` (resolved once per data version).
    use_context_provider(|| NameCache(Signal::new(std::collections::HashMap::new())));
    // Persist the "Jump back in" list whenever it changes — best-effort, never blocks the UI, and a
    // no-op under SSR tests (no workspace). Mirrors the theme/geometry persistence precedent.
    let recent_dir = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().dir.clone()),
        _ => None,
    };
    use_effect(move || {
        let recent = nav.recent.read().clone();
        let Some(dir) = &recent_dir else { return };
        if let Err(error) = genealogy_app::save_recent(dir, &recent) {
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
    rsx! {
        div {
            class: "app",
            "data-theme": "{theme}",
            tabindex: "-1",
            onkeydown: move |event| dispatch(&event, nav, gp, &notices),
            a { class: "skip-link", href: "#main", "{chrome.0.skip_to_content()}" }
            Rail {}
            div { class: "shell",
                Topbar {}
                RecordTabstrip {}
                main { class: "workarea", id: "main", role: "main", tabindex: "-1",
                    Workarea {}
                }
                ShellStatusbar {}
            }
            CommandPalette {}
            HelpOverlay {}
            Toast {
                visible: notice.is_some(),
                message: notice.unwrap_or_default(),
                action_label: notice_dismiss,
                onaction: move |_| notice_nav.dismiss_notice(),
            }
            WindowGeometryManager {}
        }
    }
}

/// The active destination's screen. Every entity category and tool now has a real screen.
#[component]
fn Workarea() -> Element {
    let nav = use_context::<NavState>();
    match *nav.active.read() {
        Destination::Category(Category::Dashboard) => rsx! { DashboardScreen {} },
        Destination::Category(Category::People) => rsx! { PersonScreen {} },
        Destination::Category(Category::Families) => rsx! { FamilyScreen {} },
        Destination::Category(Category::Events) => rsx! { EventScreen {} },
        Destination::Category(Category::Places) => rsx! { PlaceScreen {} },
        Destination::Category(Category::Citations) => rsx! { CitationScreen {} },
        Destination::Category(Category::Sources) => rsx! { SourceScreen {} },
        Destination::Category(Category::Repositories) => rsx! { RepositoryScreen {} },
        Destination::Category(Category::Media) => rsx! { MediaScreen {} },
        Destination::Category(Category::Notes) => rsx! { NoteScreen {} },
        Destination::Category(Category::Tags) => rsx! { TagScreen {} },
        Destination::Category(Category::DnaTests) => rsx! { DnaTestScreen {} },
        Destination::Category(Category::DnaMatches) => rsx! { DnaMatchScreen {} },
        Destination::Tool(Tool::Pedigree) => rsx! { PedigreeScreen {} },
        Destination::Tool(Tool::Merge) => rsx! { MergeScreen {} },
        Destination::Tool(Tool::Plugins) => rsx! { PluginPanelScreen {} },
        Destination::Tool(Tool::Preferences) => rsx! { PreferencesScreen {} },
        Destination::Help { topic } => rsx! { HelpScreen { topic } },
    }
}
