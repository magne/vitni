//! The root component and shared application state.
//!
//! [`App`] performs the fallible startup (config + workspace resolution, plugin host) once and
//! provides the result as context. Screens read [`AppCtx`] for the services and localizers and hold
//! the active [`Screen`] as navigation state.

use std::path::PathBuf;
use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_app::config;
use genealogy_plugin_host::PluginHost;
use genealogy_ui::{Localizer, Screen};

use crate::i18n::Chrome;
use crate::screens::{PersonDetailScreen, PersonListScreen, PluginPanelScreen};
use crate::services::Services;

/// The ready application state: services plus the data and chrome localizers.
#[derive(Clone)]
pub struct AppState {
    inner: Rc<Ready>,
}

struct Ready {
    services: Services,
    data_loc: Localizer,
    chrome: Chrome,
}

impl AppState {
    /// The application services (config, workspace dir, plugin host).
    #[must_use]
    pub fn services(&self) -> &Services {
        &self.inner.services
    }

    /// The data localizer (names, sex, field labels, application errors).
    #[must_use]
    pub fn data_loc(&self) -> &Localizer {
        &self.inner.data_loc
    }

    /// The chrome localizer (window/navigation labels, renderer errors).
    #[must_use]
    pub fn chrome(&self) -> &Chrome {
        &self.inner.chrome
    }
}

/// The startup outcome, provided as context: either a ready state or a fatal startup error.
#[derive(Clone)]
pub enum AppCtx {
    /// Startup succeeded.
    Ready(AppState),
    /// Startup failed with a message to show.
    Failed(String),
}

/// The root component: runs startup once, provides [`AppCtx`], and renders the shell or a fatal error.
#[component]
pub fn App() -> Element {
    let ctx = use_context_provider(|| match build_state() {
        Ok(state) => AppCtx::Ready(state),
        Err(message) => AppCtx::Failed(message),
    });
    match ctx {
        AppCtx::Ready(_) => rsx! { Shell {} },
        AppCtx::Failed(message) => rsx! { div { class: "fatal", "{message}" } },
    }
}

/// The navigation shell: a nav bar over the active screen.
#[component]
fn Shell() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let people = state.chrome().nav_people();
    let plugin = state.chrome().nav_plugin();
    let mut screen = use_signal(|| Screen::PersonList);
    rsx! {
        div { class: "app",
            nav { class: "nav",
                button { onclick: move |_| screen.set(Screen::PersonList), "{people}" }
                button { onclick: move |_| screen.set(Screen::PluginPanel), "{plugin}" }
            }
            main { class: "content",
                {match screen() {
                    Screen::PersonList => rsx! {
                        PersonListScreen { on_open: move |human_id| screen.set(Screen::PersonDetail { human_id }) }
                    },
                    Screen::PersonDetail { human_id } => rsx! {
                        PersonDetailScreen { human_id, on_back: move |()| screen.set(Screen::PersonList) }
                    },
                    Screen::PluginPanel => rsx! { PluginPanelScreen {} },
                }}
            }
        }
    }
}

/// Resolves config, workspace, and the plugin host, building the localizers for the workspace.
fn build_state() -> Result<AppState, String> {
    let config =
        config::load(&config::config_path().map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    let dir = config
        .resolve_workspace(workspace_from_env().as_deref())
        .map_err(|error| error.to_string())?;
    let host = PluginHost::new().map_err(|error| error.to_string())?;
    let chrome = Chrome::for_workspace(&dir);
    let data_loc = Localizer::for_workspace(&dir);
    let services = Services {
        config,
        dir,
        host: Rc::new(host),
        plugin_path: plugin_path(),
    };
    Ok(AppState {
        inner: Rc::new(Ready {
            services,
            data_loc,
            chrome,
        }),
    })
}

/// The workspace name from `GENEALOGY_WORKSPACE`, if set.
fn workspace_from_env() -> Option<String> {
    std::env::var("GENEALOGY_WORKSPACE")
        .ok()
        .filter(|name| !name.is_empty())
}

/// The built `ui-panel` component, resolved relative to the source tree (the spike's directory-based
/// plugin layer, ADR 0011 §6). Run `cargo xtask build-plugins` to produce it.
fn plugin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/plugins/ui-panel.wasm")
}
