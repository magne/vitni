//! The renderer's bridge to the application and plugin host.
//!
//! [`Services`] bundles the resolved config, workspace directory, and plugin host the screens need.
//! The async helpers open a fresh workspace per action (as the CLI does per command — the store is
//! consumed by the plugin host and is cheap to reopen) and route data loading through
//! [`genealogy_ui::dispatch`]; the plugin form is fetched by running the `ui-panel` component and
//! parsing its JSON with [`genealogy_ui::parse`].

use std::path::PathBuf;
use std::rc::Rc;

use genealogy_app::{
    Config, NewCitation, PersonNameParts, Session, Sex, TagSummary, Workspace, WorkspaceCounts, create_citation,
    create_family, list_tags, workspace_counts,
};
use genealogy_plugin_host::{Capability, Grants, PluginHost, ResourceBudget};
use genealogy_ui::{CitationEdit, FamilyEdit, Form, Intent, IntentOutcome, Localizer, PersonEdit};
use i18n_embed::DesktopLanguageRequester;

use crate::i18n::Chrome;

/// The `ui-panel` plugin's Fluent catalogue domain (its `<domain>.ftl` file name, ADR 0012 §5).
const UI_PANEL_DOMAIN: &str = "ui-panel";

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
    /// Directory of the `ui-panel` plugin's shipped Fluent catalogue (`<locale>/ui-panel.ftl`).
    pub plugin_catalogue_dir: PathBuf,
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

/// Loads the per-aggregate record counts for the rail badges and dashboard, or `None` if the
/// workspace cannot be opened (the rail then shows no counts rather than an error).
pub async fn load_counts(services: Services) -> Option<WorkspaceCounts> {
    let workspace = services.open().await.ok()?;
    workspace_counts(&workspace).await.ok()
}

/// Saves a [`PersonEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure. Opens a fresh workspace and mints a [`Session`] for the operator (the
/// app layer is the sole source of the clock + assertion id).
pub async fn save_edit(services: Services, edit: PersonEdit) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_edit(&workspace, &session, &edit)
        .await
        .map_err(|error| loc.error(&error))
}

/// Creates a person from an optional initial name and sex, returning the new `human_id` (or a
/// localized error). Opens a fresh workspace and mints a [`Session`] for the operator.
pub async fn create_person(
    services: Services,
    name: Option<PersonNameParts>,
    sex: Option<Sex>,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_create(&workspace, &session, name, sex)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`CitationEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_citation_edit(services: Services, edit: CitationEdit) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_citation_edit(&workspace, &session, &edit)
        .await
        .map_err(|error| loc.error(&error))
}

/// Creates a citation against a source (by its `human_id`), returning the new citation's `human_id`
/// (or a localized error).
pub async fn create_citation_record(
    services: Services,
    source: String,
    page: Option<String>,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    create_citation(
        &workspace,
        &session,
        NewCitation {
            human_id: None,
            source,
            page,
        },
    )
    .await
    .map_err(|error| loc.error(&error))
}

/// Saves a [`FamilyEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_family_edit(services: Services, edit: FamilyEdit) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_family_edit(&workspace, &session, &edit)
        .await
        .map_err(|error| loc.error(&error))
}

/// Creates an (empty) family, returning the new family's `human_id` (or a localized error). Partners
/// and children are added afterwards through [`FamilyEdit`] (the detail's edit affordances).
pub async fn create_family_record(services: Services) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    create_family(&workspace, &session)
        .await
        .map_err(|error| loc.error(&error))
}

/// Lists every tag (id + name + colour + priority) for the tag picker. The id is used internally to
/// attach/detach; only the name/colour/priority are shown to the user.
pub async fn load_tags(services: Services) -> Result<Vec<TagSummary>, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    list_tags(&workspace).await.map_err(|error| loc.error(&error))
}

/// Runs the `ui-panel` plugin through the host, parses the form it emitted, and resolves its label
/// IDs against the plugin's own Fluent catalogue (ADR 0012 §5). The host returns the form as an
/// opaque JSON string; parsing and localization happen here, in the renderer.
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
    let form = genealogy_ui::parse(&json).map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let requested = DesktopLanguageRequester::requested_languages();
    Ok(genealogy_ui::resolve_form(
        &form,
        &services.plugin_catalogue_dir,
        UI_PANEL_DOMAIN,
        &requested,
    ))
}
