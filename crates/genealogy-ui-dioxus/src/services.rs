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
    Config, IdFormats, LocaleDefaults, PreferenceLayers, ResolvedLocale, Session, TagSummary, Workspace,
    WorkspaceCounts, config, list_tags, read_preference_layers, read_resolved_locale, set_default_workspace,
    set_operator_identity, set_workspace_default_id_formats, set_workspace_default_locale, workspace_counts,
};
use genealogy_plugin_host::{Capability, Grants, PluginHost, PluginRole, ResourceBudget};
use genealogy_ui::{
    Category, CitationChangeSetRequest, CitationEdit, DnaMatchChangeSetRequest, DnaMatchEdit, DnaTestChangeSetRequest,
    DnaTestEdit, EventChangeSetRequest, EventEdit, FamilyChangeSetRequest, FamilyEdit, Form, Intent, IntentOutcome,
    Localizer, MediaChangeSetRequest, MediaEdit, MergePersons, MergeResultVm, NoteChangeSetRequest, NoteEdit,
    PersonChangeSetRequest, PersonEdit, PlaceChangeSetRequest, PlaceEdit, ProvenanceDraft, RepositoryChangeSetRequest,
    RepositoryEdit, SourceChangeSetRequest, SourceEdit, TagChangeSetRequest,
};
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
    /// The directory holding every built plugin component (the discovery scan root, PR21).
    pub plugins_dir: PathBuf,
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

/// Resolves the current display name of the record `(category, human_id)` for a record link, or
/// `None` when it has no name, does not exist, or the workspace cannot be opened (the link then
/// falls back to the human id). Best-effort: infrastructure errors are logged, not surfaced.
pub async fn resolve_record_name(services: Services, category: Category, human_id: String) -> Option<String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = match services.open().await {
        Ok(workspace) => workspace,
        Err(error) => {
            tracing::warn!(%error, "could not open the workspace to resolve a record name");
            return None;
        }
    };
    match genealogy_ui::resolve_record_name(&workspace, &loc, category, &human_id).await {
        Ok(name) => name,
        Err(error) => {
            tracing::warn!(%error, %human_id, "could not resolve a record name");
            None
        }
    }
}

/// Merges two persons through `genealogy_ui::dispatch_merge`, returning the localized outcome (or a
/// localized error). Opens a fresh workspace and mints a [`Session`] for the operator, matching every
/// other mutating helper here.
pub async fn merge_persons(services: Services, request: MergePersons) -> Result<MergeResultVm, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_merge(&workspace, &session, &loc, &request)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`PersonEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure. Opens a fresh workspace and mints a [`Session`] for the operator (the
/// app layer is the sole source of the clock + assertion id).
pub async fn save_edit(services: Services, edit: PersonEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered person dialog (a [`PersonChangeSetRequest`]) through the app's change-set,
/// returning the person's `human_id` (or a localized error). One operator action: the whole graph
/// — person + name + gender + tags + any new source/citation — commits together (or, on edit, only
/// the diff). Opens a fresh workspace and mints a [`Session`] for the operator.
pub async fn commit_person_change_set(services: Services, request: PersonChangeSetRequest) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    // The person screen's provenance block lands in slice 12; until then the change-set carries the
    // default provenance (Normal confidence, no rationale/citations).
    genealogy_ui::dispatch_person_change_set(&workspace, &session, &request, &ProvenanceDraft::default())
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`CitationEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_citation_edit(services: Services, edit: CitationEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_citation_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Creates a citation against a source (by its `human_id`), returning the new citation's `human_id`
/// (or a localized error).
pub async fn commit_citation_change_set(
    services: Services,
    request: CitationChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_citation_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`FamilyEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_family_edit(services: Services, edit: FamilyEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_family_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered family create form through the app's change-set, returning the new family's
/// `human_id`. Partners are resolved before any write; nothing is written until Save.
pub async fn commit_family_change_set(
    services: Services,
    request: FamilyChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_family_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves an [`EventEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_event_edit(services: Services, edit: EventEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_event_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered event create form through the app change-set, returning the new event's `human_id` (or a localized error).
pub async fn commit_event_change_set(
    services: Services,
    request: EventChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_event_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`PlaceEdit`] through the matching `genealogy-app` command use-case, returning a localized
/// error on failure.
pub async fn save_place_edit(services: Services, edit: PlaceEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_place_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered place create form through the app's change-set, returning the new place's
/// `human_id`. One operator action; nothing is written until Save.
pub async fn commit_place_change_set(
    services: Services,
    request: PlaceChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_place_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`SourceEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_source_edit(services: Services, edit: SourceEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_source_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered source create form (a [`SourceChangeSetRequest`] + its provenance block)
/// through the app's change-set, returning the new source's `human_id` (or a localized error). One
/// operator action: `CreateSource` plus a setter for each filled field. Nothing is written until Save.
pub async fn commit_source_change_set(
    services: Services,
    request: SourceChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_source_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`RepositoryEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_repository_edit(
    services: Services,
    edit: RepositoryEdit,
    prov: ProvenanceDraft,
) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_repository_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered repository create form through the app's change-set, returning the new
/// repository's `human_id`. One operator action; nothing is written until Save.
pub async fn commit_repository_change_set(
    services: Services,
    request: RepositoryChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_repository_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`MediaEdit`] through the matching `genealogy-app` command use-case, returning a localized
/// error on failure.
pub async fn save_media_edit(services: Services, edit: MediaEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_media_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered media create form through the app's change-set, returning the new media
/// object's `human_id`. One operator action; nothing is written until Save.
pub async fn commit_media_change_set(
    services: Services,
    request: MediaChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_media_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`NoteEdit`] through the matching `genealogy-app` command use-case, returning a localized
/// error on failure.
pub async fn save_note_edit(services: Services, edit: NoteEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_note_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered note create form through the app's change-set, returning the new note's
/// `human_id`. One operator action; nothing is written until Save.
pub async fn commit_note_change_set(
    services: Services,
    request: NoteChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_note_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered tag record (a [`TagChangeSetRequest`]) through the app's change-set, returning
/// the tag's aggregate id (the minted one on create) or a localized error. One operator action: the
/// name, priority, and colour commit together (or, on edit, only the changed fields).
pub async fn commit_tag_change_set(services: Services, request: TagChangeSetRequest) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    // The tag screen's provenance block lands in slice 11; until then the change-set carries the
    // default provenance (Normal confidence, no rationale/citations).
    genealogy_ui::dispatch_tag_change_set(&workspace, &session, &request, &ProvenanceDraft::default())
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`DnaTestEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_dna_test_edit(services: Services, edit: DnaTestEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_dna_test_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Creates a DNA test anchored to a person (by their `human_id`), returning the new test's `human_id`
/// (or a localized error). Provider/kit/type/build are added afterwards through the detail.
pub async fn commit_dna_test_change_set(
    services: Services,
    request: DnaTestChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_dna_test_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`DnaMatchEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_dna_match_edit(services: Services, edit: DnaMatchEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_dna_match_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered DNA-match create form through `observe_dna_match`, returning the new
/// match's `human_id`. The numeric fields are parsed at the UI boundary — an unparseable value never
/// reaches here (§7); nothing is written until Save.
pub async fn commit_dna_match_change_set(
    services: Services,
    request: DnaMatchChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = Localizer::for_workspace(&services.dir);
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_dna_match_change_set(&workspace, &session, &request, &prov)
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

/// One row of the plugin manager table (PR21): a discovered plugin's genuinely declared metadata
/// (see [`genealogy_plugin_host::PluginInfo`]) joined with its persisted enabled/disabled state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRow {
    /// The plugin's id (its component file's stem).
    pub id: String,
    /// The role inferred from its exported entry point.
    pub role: PluginRole,
    /// The `genealogy:host-api` version this component was compiled against.
    pub host_api_version: String,
    /// The capabilities its world imports (declared, not necessarily granted at run time).
    pub capabilities: Vec<Capability>,
    /// Whether the operator has this plugin enabled (persisted per workspace).
    pub enabled: bool,
}

/// Scans the built-plugins directory and joins each discovered plugin with its persisted
/// enabled/disabled override, sorted by id for a stable table order.
///
/// # Errors
///
/// A localized message if the plugins directory cannot be scanned (e.g. missing — the operator
/// needs to run `cargo xtask build-plugins` in a dev checkout).
pub async fn discover_plugins(services: Services) -> Result<Vec<PluginRow>, String> {
    let chrome = Chrome::for_workspace(&services.dir);
    let found = services
        .host
        .discover(&services.plugins_dir)
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let prefs = genealogy_app::read_plugin_preferences(&services.dir);
    let mut rows: Vec<PluginRow> = found
        .into_iter()
        .map(|info| PluginRow {
            enabled: prefs.is_enabled(&info.id),
            id: info.id,
            role: info.role,
            host_api_version: info.host_api_version,
            capabilities: info.capabilities,
        })
        .collect();
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(rows)
}

/// Persists whether plugin `id` is enabled (a per-workspace manifest override; PR21). Capabilities
/// remain deny-by-default regardless (ADR 0011 §2) — this flag only gates whether the plugin manager
/// offers to run it at all.
///
/// # Errors
///
/// A localized message if the manifest cannot be read or written.
pub async fn set_plugin_enabled(services: Services, id: String, enabled: bool) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    genealogy_app::save_plugin_enabled(&services.dir, &id, enabled).map_err(|error| loc.error(&error))
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

/// Everything the Preferences screen (PR 20) needs to render: the global config (operator identity,
/// the workspace registry + default), the open workspace's override-chain layers (theme, Person id
/// format), and the resolved language/locale/date/number preferences.
#[derive(Debug, Clone)]
pub struct PreferencesData {
    /// The global configuration (operator identity, workspace registry, live-fallback defaults).
    pub config: Config,
    /// The override-chain DTOs for the mockup's "Workspace defaults" card.
    pub layers: PreferenceLayers,
    /// The resolved language/locale/date/number preferences for the open workspace.
    pub locale: ResolvedLocale,
}

/// Loads the Preferences screen's data. Never opens the store (config + manifest reads only), so it
/// degrades gracefully: a missing/corrupt manifest resolves to "no workspace override" rather than
/// erroring (mirrors [`genealogy_app::read_preference_layers`]).
#[must_use]
pub fn load_preferences(services: &Services) -> PreferencesData {
    let layers = read_preference_layers(&services.dir, &services.config.workspace_defaults);
    let locale = read_resolved_locale(&services.dir, &services.config.workspace_defaults);
    PreferencesData {
        config: services.config.clone(),
        layers,
        locale,
    }
}

/// Saves the operator's display name and email, returning a localized error on failure. Config
/// writes are plain file I/O (no store to open), so unlike the other `save_*` helpers this is
/// synchronous — callers still invoke it from inside a `spawn`, matching the async ones' call sites.
pub fn save_operator_identity(
    services: &Services,
    display: Option<String>,
    email: Option<String>,
) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    set_operator_identity(&path, display, email).map_err(|error| loc.error(&error))
}

/// Saves the live-fallback `HumanId` formats, returning a localized error on failure.
pub fn save_id_format_defaults(services: &Services, id_formats: IdFormats) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    set_workspace_default_id_formats(&path, id_formats).map_err(|error| loc.error(&error))
}

/// Saves the live-fallback language/locale/date/number defaults, returning a localized error on
/// failure.
pub fn save_locale_defaults(services: &Services, locale: LocaleDefaults) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    set_workspace_default_locale(&path, locale).map_err(|error| loc.error(&error))
}

/// Switches the default (last-used) workspace by name, returning a localized error on failure. The
/// caller (the Preferences screen) is responsible for triggering the application-state restart
/// (`crate::app::request_restart`) once this succeeds — persistence and the renderer-side rebuild
/// are deliberately separate steps.
pub fn switch_workspace(services: &Services, name: &str) -> Result<(), String> {
    let loc = Localizer::for_workspace(&services.dir);
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    set_default_workspace(&path, name).map_err(|error| loc.error(&error))
}
