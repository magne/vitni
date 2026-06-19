//! The renderer's bridge to the application and plugin host.
//!
//! [`Services`] bundles the resolved config, workspace directory, and plugin host the screens need.
//! The async helpers open a fresh workspace per action (as the CLI does per command — the store is
//! consumed by the plugin host and is cheap to reopen) and route data loading through
//! [`genealogy_ui::dispatch`]; the plugin form is fetched by running the `ui-panel` component and
//! parsing its JSON with [`genealogy_ui::parse`].

use std::path::PathBuf;
use std::rc::Rc;

use genealogy_app::{Config, Session, Workspace};
use genealogy_plugin_host::{Capability, Grants, PluginHost, ResourceBudget};
use genealogy_ui::{Form, Intent, IntentOutcome, Localizer};

use crate::i18n::Chrome;

/// The application services shared with every screen.
#[derive(Clone)]
pub struct Services {
    /// Global configuration (operator identity, workspace registry).
    pub config: Config,
    /// The resolved workspace directory.
    pub dir: PathBuf,
    /// The plugin host (shared; reused across plugin runs).
    pub host: Rc<PluginHost>,
    /// Path to the built `ui-panel` plugin component.
    pub plugin_path: PathBuf,
}

/// The result of loading a data screen: the loaded outcome, or a localized error to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenData {
    /// The use-case outcome (a list, a detail, or a not-found).
    Loaded(IntentOutcome),
    /// A localized error message.
    Error(String),
}

impl Services {
    async fn open(&self) -> Result<Workspace, genealogy_app::AppError> {
        Workspace::open(&self.dir, &self.config.operator, &self.config.workspace_defaults).await
    }
}

/// Loads the data for `intent`, returning a localized [`ScreenData`].
pub async fn load_screen(services: Services, intent: Intent) -> ScreenData {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = match services.open().await {
        Ok(workspace) => workspace,
        Err(error) => return ScreenData::Error(loc.error(&error)),
    };
    match genealogy_ui::dispatch(&workspace, &loc, &intent).await {
        Ok(outcome) => ScreenData::Loaded(outcome),
        Err(error) => ScreenData::Error(loc.error(&error)),
    }
}

/// Runs the `ui-panel` plugin through the host and parses the form it emitted (ADR 0012). The host
/// returns the form as an opaque JSON string; parsing happens here, in the renderer.
pub async fn load_plugin_form(services: Services) -> Result<Form, String> {
    let chrome = Chrome::for_workspace(&services.dir);
    let loc = Localizer::for_workspace(&services.dir);
    let component = services
        .host
        .load(&services.plugin_path)
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    let grants = Grants::none().with(Capability::Log);
    let (json, _workspace) = services
        .host
        .run_ui_panel(&component, workspace, session, grants, ResourceBudget::default())
        .await
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    genealogy_ui::parse(&json).map_err(|error| chrome.plugin_error(&error.to_string()))
}
