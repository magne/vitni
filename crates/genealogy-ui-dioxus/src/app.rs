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
use genealogy_ui::Localizer;

use crate::i18n::Chrome;
use crate::screens::{PersonScreen, PluginPanelScreen};
use crate::services::Services;

/// The dark theme matching the roadmap mockup (`docs/roadmap.html`), injected once at the root.
const APP_CSS: &str = r"
:root{--bg:#0f1419;--panel:#1a2129;--panel2:#222b35;--ink:#e6edf3;--muted:#9aa7b4;--line:#2d3742;--accent:#6cb6ff;--err:#f78166;}
*{box-sizing:border-box;}
body{margin:0;background:var(--bg);color:var(--ink);font:15px/1.6 -apple-system,BlinkMacSystemFont,'Segoe UI',Helvetica,Arial,sans-serif;}
.app{display:flex;flex-direction:column;height:100vh;}
.nav{display:flex;gap:6px;padding:10px 12px;border-bottom:1px solid var(--line);background:var(--panel);}
.nav button{background:var(--panel2);color:var(--ink);border:1px solid var(--line);border-radius:6px;padding:6px 14px;font-size:13px;cursor:pointer;}
.nav button.active{background:var(--accent);color:#04101f;font-weight:600;border-color:var(--accent);}
.content{flex:1;overflow:auto;}
.gui{display:grid;grid-template-columns:240px 1fr;height:100%;}
.side{background:var(--panel2);border-right:1px solid var(--line);padding:10px;overflow:auto;}
.side .item{padding:8px 10px;border-radius:6px;font-size:13px;cursor:pointer;}
.side .item:hover{background:var(--panel);}
.side .item.sel{background:var(--accent);color:#04101f;font-weight:600;}
.main{padding:18px 20px;overflow:auto;}
.detail-name{font-size:19px;font-weight:600;margin-bottom:12px;}
.field{margin-bottom:12px;}
.field label{display:block;color:var(--muted);font-size:11px;text-transform:uppercase;letter-spacing:.4px;}
.field .val{font-size:15px;}
.badge{font-size:11px;background:var(--panel2);border:1px solid var(--line);padding:1px 8px;border-radius:999px;color:var(--muted);margin-left:8px;}
.plugin-form{max-width:380px;padding:18px 20px;}
.plugin-form h2{font-size:18px;margin:0 0 12px;}
.plugin-form .field span{display:block;color:var(--muted);font-size:12px;margin-bottom:3px;}
.plugin-form input,.plugin-form select{width:100%;background:#0d1117;border:1px solid var(--line);color:var(--ink);padding:7px 10px;border-radius:6px;}
.plugin-form .checkbox{flex-direction:row;align-items:center;gap:8px;}
.plugin-form .checkbox input{width:auto;}
.plugin-form button.submit{margin-top:10px;background:var(--accent);color:#04101f;border:0;padding:8px 16px;border-radius:6px;font-weight:600;cursor:pointer;}
.run-plugin{margin:14px 20px;background:var(--accent);color:#04101f;border:0;padding:8px 16px;border-radius:6px;font-weight:600;cursor:pointer;}
.loading,.empty,.placeholder{padding:18px 20px;color:var(--muted);}
.error{padding:18px 20px;color:var(--err);}
.fatal{padding:24px;color:var(--err);}
";

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

/// The top-level navigation tab.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    /// The person master-detail.
    People,
    /// The plugin-supplied form panel.
    Plugin,
}

/// The root component: runs startup once, provides [`AppCtx`], and renders the shell or a fatal error.
#[component]
pub fn App() -> Element {
    let ctx = use_context_provider(|| match build_state() {
        Ok(state) => AppCtx::Ready(state),
        Err(message) => AppCtx::Failed(message),
    });
    match ctx {
        AppCtx::Ready(_) => rsx! {
            document::Style { {APP_CSS} }
            Shell {}
        },
        AppCtx::Failed(message) => rsx! {
            document::Style { {APP_CSS} }
            div { class: "fatal", "{message}" }
        },
    }
}

/// The navigation shell: a tab bar over the active screen.
#[component]
fn Shell() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let people = state.chrome().nav_people();
    let plugin = state.chrome().nav_plugin();
    let mut tab = use_signal(|| Tab::People);
    rsx! {
        div { class: "app",
            nav { class: "nav",
                button {
                    class: if tab() == Tab::People { "active" } else { "" },
                    onclick: move |_| tab.set(Tab::People),
                    "{people}"
                }
                button {
                    class: if tab() == Tab::Plugin { "active" } else { "" },
                    onclick: move |_| tab.set(Tab::Plugin),
                    "{plugin}"
                }
            }
            main { class: "content",
                {match tab() {
                    Tab::People => rsx! { PersonScreen {} },
                    Tab::Plugin => rsx! { PluginPanelScreen {} },
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
    let plugins_dir = plugins_dir();
    let services = Services {
        config,
        dir,
        host: Rc::new(host),
        plugin_path: plugins_dir.join("ui-panel.wasm"),
        plugin_catalogue_dir: plugins_dir.join("ui-panel").join("i18n"),
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

/// The built-plugins directory, resolved relative to the source tree (the spike's directory-based
/// plugin layer, ADR 0011 §6). Holds `<id>.wasm` and `<id>/i18n/`. Run `cargo xtask build-plugins`.
fn plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/plugins")
}
