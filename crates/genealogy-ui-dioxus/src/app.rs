//! The root component and shared application state.
//!
//! [`App`] performs the fallible startup (config + workspace resolution, plugin host) once and
//! provides the result as context. Screens read [`AppCtx`] for the services and localizers and hold
//! the active [`Screen`] as navigation state.

use std::path::PathBuf;
use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_app::{AppError, ThemeMode, WindowGeometry, config, workspace};
use genealogy_plugin_host::PluginHost;
use genealogy_ui::Localizer;

use crate::components::EmptyState;
use crate::i18n::Chrome;
use crate::services::Services;
use crate::shell::nav_state::{Theme, resolve_theme};
use crate::shell::{ChromeCtx, Shell};

/// The design-system tokens (light + dark via `[data-theme]`; default dark) and component styles
/// (`docs/phase5/assets/`), embedded at compile time. These files are a verbatim copy of the mockup
/// source of truth — never hand-edit; regenerate by copying.
const TOKENS_CSS: &str = include_str!("tokens.css");
const COMPONENTS_CSS: &str = include_str!("components.css");

/// The design-system CSS wrapped in `<style>` tags for the index `<head>`. The desktop entry point
/// injects this via `Config::with_custom_head` so the very first paint is already styled (no
/// flash-of-unstyled-content), rather than injecting the stylesheet from the render tree at runtime.
#[must_use]
pub fn styles_head() -> String {
    format!("<style>{TOKENS_CSS}</style><style>{COMPONENTS_CSS}</style>")
}

/// The plain-data startup preferences resolved before the window opens (theme + saved geometry),
/// passed into the component tree as a root context (`LaunchBuilder::with_context`). Plain `Copy`
/// data so it satisfies the context `Send + Sync + 'static` bound; absent under SSR tests, where
/// [`Default`] applies (System theme, no geometry).
#[derive(Debug, Clone, Copy)]
pub struct StartupPrefs {
    /// The persisted theme mode (System / Light / Dark).
    pub theme_mode: ThemeMode,
    /// The concrete theme `theme_mode` resolves to (computed once so the window background agrees).
    pub resolved_theme: Theme,
    /// The saved native-window geometry, if any.
    pub geometry: Option<WindowGeometry>,
}

impl Default for StartupPrefs {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            resolved_theme: resolve_theme(ThemeMode::System),
            geometry: None,
        }
    }
}

/// Resolves the startup preferences from the global config and the target workspace's manifest,
/// without opening the store. Best-effort: any failure (no config, unknown workspace, unreadable
/// manifest) yields [`StartupPrefs::default`] so startup is never blocked.
#[must_use]
pub fn resolve_startup_prefs() -> StartupPrefs {
    let resolved = (|| -> Result<StartupPrefs, AppError> {
        let config = config::load(&config::config_path()?)?;
        let dir = config.resolve_workspace(workspace_from_env().as_deref())?;
        let prefs = workspace::read_ui_preferences(&dir, &config.workspace_defaults);
        Ok(StartupPrefs {
            theme_mode: prefs.theme,
            resolved_theme: resolve_theme(prefs.theme),
            geometry: prefs.window,
        })
    })();
    resolved.unwrap_or_default()
}

/// The ready application state: services plus the data and chrome localizers.
#[derive(Clone)]
pub struct AppState {
    inner: Rc<Ready>,
}

struct Ready {
    services: Services,
    data_loc: Localizer,
    chrome: Rc<Chrome>,
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

    /// A shared handle to the chrome localizer, for providing it as context.
    #[must_use]
    pub fn chrome_rc(&self) -> Rc<Chrome> {
        Rc::clone(&self.inner.chrome)
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
        AppCtx::Ready(_) => rsx! {
            ReadyShell {}
        },
        AppCtx::Failed(message) => rsx! {
            FatalError { message }
        },
    }
}

/// Provides the chrome localizer as context, then renders the application [`Shell`].
#[component]
fn ReadyShell() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    use_context_provider(|| ChromeCtx(state.chrome_rc()));
    rsx! {
        Shell {}
    }
}

/// A fatal startup error, shown in place of the shell.
#[component]
fn FatalError(message: String) -> Element {
    rsx! {
        EmptyState { symbol: "⚠".to_owned(), message }
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
    let chrome = Rc::new(Chrome::for_workspace(&dir));
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
pub(crate) fn workspace_from_env() -> Option<String> {
    std::env::var("GENEALOGY_WORKSPACE")
        .ok()
        .filter(|name| !name.is_empty())
}

/// The built-plugins directory, resolved relative to the source tree (the spike's directory-based
/// plugin layer, ADR 0011 §6). Holds `<id>.wasm` and `<id>/i18n/`. Run `cargo xtask build-plugins`.
fn plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/plugins")
}
