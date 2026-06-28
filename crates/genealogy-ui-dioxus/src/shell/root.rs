//! The shell root: assembles the rail, top bar, tabstrip, work area, status bar, and overlays, and
//! installs the central keyboard dispatcher.

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination, Tool};

use crate::app::AppCtx;
use crate::components::EmptyState;
use crate::screens::{
    CitationScreen, DashboardScreen, DnaMatchScreen, DnaTestScreen, EventScreen, FamilyScreen, MediaScreen, NoteScreen,
    PersonScreen, PlaceScreen, PluginPanelScreen, RepositoryScreen, SourceScreen, TagScreen,
};
use crate::services::load_counts;
use crate::shell::help_overlay::HelpOverlay;
use crate::shell::keyboard::{dispatch, use_keyboard_dispatch};
use crate::shell::nav_state::NavState;
use crate::shell::palette::CommandPalette;
use crate::shell::rail::Rail;
use crate::shell::statusbar::ShellStatusbar;
use crate::shell::tabstrip::RecordTabstrip;
use crate::shell::topbar::Topbar;
use crate::shell::{ChromeCtx, CountsCtx};

/// The application shell. Provides [`NavState`], installs the keyboard layer, and lays out the rail,
/// top bar, tabstrip, work area, status bar, and overlays.
#[component]
pub fn Shell() -> Element {
    let nav = use_context_provider(NavState::new);
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
    rsx! {
        div {
            class: "app",
            "data-theme": "{theme}",
            tabindex: "-1",
            onkeydown: move |event| dispatch(&event, nav, gp),
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
        }
    }
}

/// The active destination's screen: a real screen for People and Plugins, an "under construction"
/// placeholder for the destinations whose slices have not landed yet.
#[component]
fn Workarea() -> Element {
    let nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
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
        Destination::Tool(Tool::Plugins) => rsx! { PluginPanelScreen {} },
        other => {
            let name = chrome.0.rail_label(other.label_id());
            rsx! { EmptyState { symbol: "🚧".to_owned(), message: chrome.0.coming_soon(&name) } }
        }
    }
}
