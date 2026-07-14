//! The root component and shared application state.
//!
//! [`App`] performs the fallible startup (config + workspace resolution, plugin host) once and
//! provides the result as context. Screens read [`AppCtx`] for the services and localizers and hold
//! the active [`Screen`] as navigation state.

use std::path::PathBuf;
use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_app::{AppError, RecentItem, ThemeMode, WindowGeometry, config, workspace};
use genealogy_plugin_host::PluginHost;
use genealogy_ui::Localizer;

use crate::components::EmptyState;
use crate::i18n::Chrome;
use crate::services::{DataQualityCache, Services};
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
///
/// Layered: the embedded base first, then an optional installation skin injected last so it wins.
#[must_use]
pub fn styles_head() -> String {
    let mut head = format!("<style>{TOKENS_CSS}</style><style>{COMPONENTS_CSS}</style>");
    if let Some(skin) = read_skin() {
        head.push_str("<style>");
        head.push_str(&skin);
        head.push_str("</style>");
    }
    head
}

/// An optional installation skin: `skin.css` beside the global config
/// (`~/.config/genealogy/skin.css`). Read at startup and injected after the embedded base so it
/// overrides the shipped tokens/components — the CSS parallel to config layering (ADR 0005): an
/// installation retunes colours/spacing/type without a rebuild. Absent or unreadable → no skin.
fn read_skin() -> Option<String> {
    let config_path = config::config_path().ok()?;
    let skin_path = config_path.parent()?.join("skin.css");
    std::fs::read_to_string(skin_path).ok()
}

/// The plain-data startup preferences resolved before the window opens (theme, saved geometry, and
/// the "Jump back in" list), passed into the component tree as a root context
/// (`LaunchBuilder::with_context`). `Send + Sync + 'static` so it satisfies the context bound; absent
/// under SSR tests, where [`Default`] applies (System theme, no geometry, empty recent list).
#[derive(Debug, Clone)]
pub struct StartupPrefs {
    /// The persisted theme mode (System / Light / Dark).
    pub theme_mode: ThemeMode,
    /// The concrete theme `theme_mode` resolves to (computed once so the window background agrees).
    pub resolved_theme: Theme,
    /// The saved native-window geometry, if any.
    pub geometry: Option<WindowGeometry>,
    /// The persisted "Jump back in" list (recently-opened records, newest first).
    pub recent: Vec<RecentItem>,
}

impl Default for StartupPrefs {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            resolved_theme: resolve_theme(ThemeMode::System),
            geometry: None,
            recent: Vec::new(),
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
            recent: prefs.recent,
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

/// A "restart the application state" trigger, provided as context above [`AppInner`].
///
/// Preferences' workspace switcher is the only writer: bumping it changes [`AppInner`]'s `key`,
/// which makes Dioxus unmount and remount it — re-running startup (`build_state`) from scratch so
/// every downstream `use_context_provider` (services, localizers, [`crate::shell::NavState`], …)
/// rebuilds against the newly-selected workspace. This is the same remount-by-key technique already
/// used for the record detail panes (`crate::screens::record_detail`), scaled up one level.
#[derive(Clone, Copy)]
pub struct RestartEpoch(pub Signal<u32>);

/// Requests a full application-state restart (after a workspace switch persists its new default).
/// A no-op if called where [`RestartEpoch`] was never provided (e.g. a bare SSR test).
pub fn request_restart() {
    if let Some(mut epoch) = try_consume_context::<RestartEpoch>() {
        *epoch.0.write() += 1;
    }
}

/// A session-only override of the workspace to open, set by the Preferences "Open" action. Never
/// persisted (unlike "Make default"); takes precedence in [`build_state`] over
/// `GENEALOGY_WORKSPACE` and the configured default. Held in a process-global signal so it survives
/// the [`RestartEpoch`] remount that "Open" triggers.
static WORKSPACE_OVERRIDE: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Opens the named workspace for this session: records the in-memory [`WORKSPACE_OVERRIDE`] and
/// restarts the application state so every use-case rebuilds against it. Persists nothing.
pub fn open_workspace(name: String) {
    *WORKSPACE_OVERRIDE.write() = Some(name);
    request_restart();
}

/// The root component: provides the [`RestartEpoch`] trigger and renders [`AppInner`], keyed so a
/// restart request remounts it.
#[component]
pub fn App() -> Element {
    let epoch = use_context_provider(|| RestartEpoch(Signal::new(0)));
    rsx! {
        AppInner { key: "{epoch.0}" }
        DevStyles {}
    }
}

/// Dev-only hot-reload layer: links the CSS files as assets so `dx serve` reflects edits without a
/// recompile. Emitted after `styles_head`'s embedded base + skin (they load first, in `<head>`), so
/// a live-edited base wins during development. Compiled out of release builds, where the embedded
/// `styles_head` is the only source. Never mounted by the SSR tests (they render views, not `App`).
#[cfg(debug_assertions)]
#[component]
fn DevStyles() -> Element {
    rsx! {
        document::Stylesheet { href: asset!("/src/tokens.css") }
        document::Stylesheet { href: asset!("/src/components.css") }
    }
}

/// Release stub: the embedded `styles_head` is the sole stylesheet source.
#[cfg(not(debug_assertions))]
#[component]
fn DevStyles() -> Element {
    rsx! {}
}

/// Runs startup once per mount, provides [`AppCtx`], and renders the shell or a fatal error.
#[component]
fn AppInner() -> Element {
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
    // Precedence: the in-memory Open override > GENEALOGY_WORKSPACE > the configured default.
    let selected = WORKSPACE_OVERRIDE
        .peek()
        .clone()
        .or_else(workspace_from_env)
        .or_else(|| config.default.clone());
    let dir = config
        .resolve_workspace(selected.as_deref())
        .map_err(|error| error.to_string())?;
    let open_workspace = selected.unwrap_or_default();
    let host = PluginHost::new().map_err(|error| error.to_string())?;
    let chrome = Rc::new(Chrome::for_workspace(&dir));
    let data_loc = Localizer::for_workspace(&dir);
    let plugins_dir = plugins_dir();
    let services = Services {
        config,
        dir,
        open_workspace,
        host: Rc::new(host),
        plugins_dir: plugins_dir.clone(),
        plugin_path: plugins_dir.join("ui-panel.wasm"),
        plugin_catalogue_dir: plugins_dir.join("ui-panel").join("i18n"),
        data_quality: DataQualityCache::default(),
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
