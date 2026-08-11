//! The renderer's bridge to the application and plugin host.
//!
//! [`Services`] bundles the resolved config, workspace directory, and plugin host the screens need.
//! The async helpers open a fresh workspace per action (as the CLI does per command — the store is
//! consumed by the plugin host and is cheap to reopen) and route data loading through
//! [`genealogy_ui::dispatch`]; the plugin form is fetched by running the `ui-panel` component and
//! parsing its JSON with [`genealogy_ui::parse`].

use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use genealogy_app::{
    AiConfig, Confidence, Config, ConfigStore, FileConfigStore, IdFormats, LocaleDefaults, MapConfig, MapProvider,
    MapSource, PluginTrust, PluginTrustConfig, PreferenceLayers, ResolvedLocale, Session, ShortcutConfig,
    SuretyLabelOverrides, TagSummary, Workspace, WorkspaceCounts, WorkspaceSummary, config, list_tags, list_workspaces,
    read_preference_layers, read_resolved_locale, read_resolved_surety_labels, read_surety_label_overrides,
    workspace_counts,
};
use genealogy_plugin_host::{
    Capability, ExportTarget, Grants, HostPattern, Invocation, NetPolicy, PluginHost, PluginRole, PresentError,
    Presenter, ProgressControl, ProgressUpdate, ResourceBudget, TrustRoots, TrustTier, resolve_trust_roots,
};
use genealogy_ui::{
    Category, CitationChangeSetRequest, CitationEdit, DataQualityVm, DnaMatchChangeSetRequest, DnaMatchEdit,
    DnaTestChangeSetRequest, DnaTestEdit, EventChangeSetRequest, EventEdit, FamilyChangeSetRequest, FamilyEdit,
    ImportTargetChoice, Intent, IntentOutcome, Localizer, MediaChangeSetRequest, MediaEdit, MergeFailure, MergePersons,
    MergeResultVm, NoteChangeSetRequest, NoteEdit, Panel, PersonChangeSetRequest, PersonEdit, PlaceChangeSetRequest,
    PlaceEdit, ProvenanceDraft, RepositoryChangeSetRequest, RepositoryEdit, ResearchNoteChangeSetRequest,
    ResearchNoteEdit, RowVm, SourceChangeSetRequest, SourceEdit, SubmitResult, TagChangeSetRequest, list_intent,
};
use i18n_embed::DesktopLanguageRequester;
use tokio::sync::{mpsc, oneshot};
use unic_langid::LanguageIdentifier;

use crate::i18n::Chrome;

/// The `ui-panel` plugin's Fluent catalogue domain (its `<domain>.ftl` file name, ADR 0012 §5).
const UI_PANEL_DOMAIN: &str = "ui-panel";

/// A session-scoped, in-memory cache of the dashboard's data-quality result, keyed by the navigation
/// `data_version` it was computed at.
///
/// In-memory only (no DB schema, per the PR decision): a workspace mutation bumps `data_version`, so
/// a stale key simply misses and the whole-workspace check pass reruns. Shared via `Rc` so every clone
/// of [`Services`] observes the same map; `Mutex`-guarded, and only ever the newest version is kept.
pub type DataQualityCache = Rc<Mutex<HashMap<u32, Box<DataQualityVm>>>>;

/// The application services shared with every screen.
#[derive(Clone)]
pub struct Services {
    /// The session-scoped data-quality cache keyed by `data_version` (see [`DataQualityCache`]).
    pub data_quality: DataQualityCache,
    /// Global configuration (operator identity, workspace registry).
    pub config: Config,
    /// The resolved workspace directory.
    pub dir: PathBuf,
    /// The name of the workspace opened this session (the resolved override / env / default). Lets
    /// the Preferences card distinguish the *open* workspace from the persisted *default*.
    pub open_workspace: String,
    /// The plugin host (shared; reused across plugin runs).
    pub host: Rc<PluginHost>,
    /// The embedded plugin layer (`target/plugins` in dev): the lowest-precedence ADR 0014 §4
    /// loading layer and the base for resolving bundles across the workspace/app-dir/embedded order.
    pub plugins_dir: PathBuf,
}

/// The result of loading a data screen: the loaded outcome, or a localized error to show.
///
/// Not `Eq`: [`IntentOutcome`] carries a place's decimal-degree map point, which has no total equality.
#[derive(Debug, Clone, PartialEq)]
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

    /// The configured UI-language override for the open workspace (manifest over the live default).
    fn config_ui_language(&self) -> Option<LanguageIdentifier> {
        read_resolved_locale(&self.dir, &self.config.workspace_defaults).ui_language
    }

    /// The resolved language request for this session (ADR 0015 §4): plain env < configured
    /// `ui_language` < `GENEALOGY_LANGUAGE`. `DesktopLanguageRequester` stays in the renderer so the
    /// app layer is `i18n_embed`-free.
    #[must_use]
    pub fn requested_languages(&self) -> Vec<LanguageIdentifier> {
        genealogy_app::requested_languages_for(
            self.config_ui_language().as_ref(),
            &DesktopLanguageRequester::requested_languages(),
        )
    }

    /// The data-string localizer for the open workspace, honouring the configured UI language and
    /// the workspace's own surety-scheme label overrides (ADR 0027).
    fn localizer(&self) -> Localizer {
        Localizer::for_workspace(&self.dir, self.config_ui_language().as_ref())
            .with_surety_overrides(read_resolved_surety_labels(&self.dir, &self.config.workspace_defaults))
    }

    /// The chrome localizer for the open workspace, honouring the configured UI language.
    ///
    /// `pub(crate)`, not private: `screens::geography`'s provider-switch handler needs one inside its
    /// own `spawn`ed async block (to localize a resolve/store failure with
    /// `Chrome::geography_provider_switch_error`) after `resolve_map_source`/`store_map_config`
    /// stopped building one themselves — see those functions' doc comments for why.
    pub(crate) fn chrome(&self) -> Chrome {
        Chrome::for_workspace(&self.dir, self.config_ui_language().as_ref())
    }

    /// The ADR 0014 §4 plugin layers, highest precedence first: the open workspace's `plugins/`, the
    /// shared app-dir, then the embedded fleet. Absent layers are skipped (as the i18n multiplexor
    /// skips absent dirs).
    fn plugin_layers(&self) -> Vec<PathBuf> {
        let shared = config::shared_plugins_dir().ok();
        genealogy_app::plugin_layers(Some(&self.dir), shared.as_deref(), &self.plugins_dir)
    }

    /// Resolves plugin `id` to its bundle directory across the layers (ADR 0014 §4).
    fn plugin_bundle(&self, id: &str) -> Option<PathBuf> {
        genealogy_app::resolve_bundle(&self.plugin_layers(), id)
    }

    /// The resolved plugin's Fluent catalogue directory (`<bundle>/i18n`, ADR 0012 §5), falling back
    /// to the embedded-layout path when the bundle is unresolved so i18n simply resolves to message
    /// keys rather than erroring.
    #[must_use]
    pub fn plugin_catalogue_dir(&self, id: &str) -> PathBuf {
        self.plugin_bundle(id)
            .unwrap_or_else(|| self.plugins_dir.join(id))
            .join("i18n")
    }

    /// The plugin trust roots for classification (ADR 0014 §3): the embedded sanctioned key(s) plus
    /// the user's client-scope pinned publishers, resolved from the global config. A missing config
    /// resolves to the embedded roots alone (no pins), so discovery still classifies first-party
    /// bundles.
    fn trust_roots(&self) -> Result<TrustRoots, String> {
        let chrome = self.chrome();
        let Ok(path) = config::config_path() else {
            return resolve_trust_roots(&[]).map_err(|error| chrome.plugin_error(&error.to_string()));
        };
        let trust = FileConfigStore::new(path, None)
            .load_plugin_trust()
            .map_err(|error| chrome.plugin_error(&error.to_string()))?;
        let pins =
            genealogy_app::resolve_trust_pins(&trust).map_err(|error| chrome.plugin_error(&error.to_string()))?;
        resolve_trust_roots(&pins).map_err(|error| chrome.plugin_error(&error.to_string()))
    }

    /// The effective capability grant (ADR 0014 §5) for the bundle at `bundle_dir`: discovers and
    /// classifies it against the trust roots, then intersects its declared capabilities with the open
    /// workspace's persisted approval. With no recorded decision a sanctioned/user-trusted plugin
    /// grants all its declared capabilities (unchanged for the first-party fleet) and an untrusted
    /// plugin grants nothing until explicitly approved. Callers narrow this ceiling to each
    /// invocation's needs with [`invocation_grants`].
    fn effective_grants(&self, bundle_dir: &Path) -> Result<Grants, String> {
        let chrome = self.chrome();
        let roots = self.trust_roots()?;
        let info = self
            .host
            .discover_bundle(bundle_dir, &roots)
            .map_err(|error| chrome.plugin_error(&error.to_string()))?;
        let prefs = genealogy_app::read_plugin_preferences(&self.dir);
        Ok(info.effective_grants(prefs.approved_grants(&info.id)))
    }
}

/// Loads the data for `intent`, returning a localized [`ScreenData`].
pub async fn load_screen(services: Services, intent: Intent) -> ScreenData {
    let loc = services.localizer();
    let workspace = match services.open().await {
        Ok(workspace) => workspace,
        Err(error) => return ScreenData::Error(loc.error(&error)),
    };
    match genealogy_ui::dispatch(&workspace, &loc, &intent).await {
        Ok(outcome) => ScreenData::Loaded(outcome),
        Err(error) => ScreenData::Error(loc.error(&error)),
    }
}

/// Loads the dashboard's data-quality results for `data_version`, returning the cached result when one
/// was already computed at that version and recomputing (then caching) otherwise.
///
/// The cache holds only the newest `data_version`: a hit returns instantly, and a mutation (which
/// bumps `data_version`) misses, reruns the whole-workspace check pass, and replaces the entry. An
/// error result is never cached, so a transient failure is retried on the next load.
pub async fn load_data_quality(services: Services, data_version: u32) -> ScreenData {
    if let Ok(cache) = services.data_quality.lock()
        && let Some(cached) = cache.get(&data_version)
    {
        return ScreenData::Loaded(IntentOutcome::DataQuality(cached.clone()));
    }
    let screen = load_screen(services.clone(), Intent::ShowDataQuality).await;
    if let ScreenData::Loaded(IntentOutcome::DataQuality(vm)) = &screen
        && let Ok(mut cache) = services.data_quality.lock()
    {
        cache.clear();
        cache.insert(data_version, vm.clone());
    }
    screen
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
    let loc = services.localizer();
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

/// Merges two persons through `genealogy_ui::dispatch_merge`, returning the localized outcome or a
/// [`MergeFailure`] — a resolvable [`Blocked`](MergeFailure::Blocked) conflict (the screen shows a
/// blocked card) or any [`Other`](MergeFailure::Other) failure (a plain toast). Opens a fresh
/// workspace and mints a [`Session`] for the operator, matching every other mutating helper here.
pub async fn merge_persons(services: Services, request: MergePersons) -> Result<MergeResultVm, MergeFailure> {
    let loc = services.localizer();
    let workspace = services
        .open()
        .await
        .map_err(|error| MergeFailure::from_error(&error, &loc))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_merge(&workspace, &session, &loc, &request)
        .await
        .map_err(|error| MergeFailure::from_error(&error, &loc))
}

/// Saves a [`PersonEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure. Opens a fresh workspace and mints a [`Session`] for the operator (the
/// app layer is the sole source of the clock + assertion id).
pub async fn save_edit(services: Services, edit: PersonEdit, prov: ProvenanceDraft) -> Result<(), String> {
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered person record (a [`PersonChangeSetRequest`] + its provenance block) through
/// the app's change-set, returning the person's `human_id` (or a localized error). One operator
/// action: the whole graph — person + name + gender + tags + any new source/citation — commits
/// together (or, on edit, only the diff). Opens a fresh workspace and mints a [`Session`].
pub async fn commit_person_change_set(
    services: Services,
    request: PersonChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_person_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`CitationEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_citation_edit(
    services: Services,
    edit: CitationEdit,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_citation_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`FamilyEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_family_edit(services: Services, edit: FamilyEdit, prov: ProvenanceDraft) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_family_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves an [`EventEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_event_edit(services: Services, edit: EventEdit, prov: ProvenanceDraft) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_event_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`PlaceEdit`] through the matching `genealogy-app` command use-case, returning the place's
/// effective `human_id` (the possibly-renamed id after a `SetHumanId`) or a localized error.
pub async fn save_place_edit(services: Services, edit: PlaceEdit, prov: ProvenanceDraft) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_place_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`SourceEdit`] through the matching `genealogy-app` command use-case, returning the
/// source's effective `human_id` (the possibly-renamed id after a `SetHumanId`) or a localized error.
pub async fn save_source_edit(services: Services, edit: SourceEdit, prov: ProvenanceDraft) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_source_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`RepositoryEdit`] through the matching `genealogy-app` command use-case, returning the
/// repository's effective `human_id` (the possibly-renamed id after a `SetHumanId`) or a localized error.
pub async fn save_repository_edit(
    services: Services,
    edit: RepositoryEdit,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_repository_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`MediaEdit`] through the matching `genealogy-app` command use-case, returning the media
/// object's effective `human_id` (the possibly-renamed id after a `SetHumanId`) or a localized error.
pub async fn save_media_edit(services: Services, edit: MediaEdit, prov: ProvenanceDraft) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_media_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`NoteEdit`] through the matching `genealogy-app` command use-case, returning the note's
/// effective `human_id` (the possibly-renamed id after a `SetHumanId`) or a localized error.
pub async fn save_note_edit(services: Services, edit: NoteEdit, prov: ProvenanceDraft) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_note_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`ResearchNoteEdit`] through the matching `genealogy-app` command use-case, returning the
/// research note's `human_id` (unchanged — the aggregate has no rename) or a localized error.
pub async fn save_research_note_edit(
    services: Services,
    edit: ResearchNoteEdit,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_research_note_edit(&workspace, &session, &edit, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered research-note create form, returning the new note's `human_id`. One operator
/// action; nothing is written until Save.
pub async fn commit_research_note_change_set(
    services: Services,
    request: ResearchNoteChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_research_note_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Commits the buffered tag record (a [`TagChangeSetRequest`]) through the app's change-set, returning
/// the tag's aggregate id (the minted one on create) or a localized error. One operator action: the
/// name, priority, and colour commit together (or, on edit, only the changed fields).
pub async fn commit_tag_change_set(
    services: Services,
    request: TagChangeSetRequest,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_tag_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`DnaTestEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_dna_test_edit(
    services: Services,
    edit: DnaTestEdit,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_dna_test_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Saves a [`DnaMatchEdit`] through the matching `genealogy-app` command use-case, returning a
/// localized error on failure.
pub async fn save_dna_match_edit(
    services: Services,
    edit: DnaMatchEdit,
    prov: ProvenanceDraft,
) -> Result<String, String> {
    let loc = services.localizer();
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
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    genealogy_ui::dispatch_dna_match_change_set(&workspace, &session, &request, &prov)
        .await
        .map_err(|error| loc.error(&error))
}

/// Loads a record picker's options: every row of `category`'s list, through the same `list_*`
/// use-case the list screens use ([`list_intent`] maps the category to its intent). A non-pickable
/// category (Dashboard/Tags — never picked by id) yields no rows. The picker filters these
/// client-side; a server-side `search_*` with a `LIMIT` is a flagged follow-up.
pub async fn load_picker_rows(services: Services, category: Category) -> Result<Vec<RowVm>, String> {
    let Some(intent) = list_intent(category) else {
        return Ok(Vec::new());
    };
    match load_screen(services, intent).await {
        ScreenData::Loaded(IntentOutcome::List(rows)) => Ok(rows),
        ScreenData::Loaded(_) => Ok(Vec::new()),
        ScreenData::Error(message) => Err(message),
    }
}

/// Loads the command palette's record options: every pickable category's list (people, places,
/// sources, …), each through [`load_picker_rows`], in [`Category::all`] order. Non-pickable
/// categories (Dashboard, Tags — never searched by id) and empty lists are omitted. The palette
/// filters these client-side per keystroke; a server-side `search_*` with a `LIMIT` is a flagged
/// follow-up (see `picker.rs`).
pub async fn load_palette_rows(services: Services) -> Vec<(Category, Vec<RowVm>)> {
    let mut groups: Vec<(Category, Vec<RowVm>)> = Vec::new();
    for category in Category::all() {
        if list_intent(category).is_none() {
            continue;
        }
        // Boxed: this aggregates one dispatch future per category, and `dispatch`'s own future is large
        // enough (one branch per intent) that inlining it here pushes the palette's resource future past
        // the `large_futures` budget.
        match Box::pin(load_picker_rows(services.clone(), category)).await {
            Ok(rows) if !rows.is_empty() => groups.push((category, rows)),
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, ?category, "could not load palette rows for a category"),
        }
    }
    groups
}

/// Lists every tag (id + name + colour + priority) for the tag picker. The id is used internally to
/// attach/detach; only the name/colour/priority are shown to the user.
pub async fn load_tags(services: Services) -> Result<Vec<TagSummary>, String> {
    let loc = services.localizer();
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
    /// The trust tier its signature places it in (ADR 0014 §3), as the frontend-visible DTO.
    pub trust: PluginTrust,
    /// The operator's persisted approved-capability decision, or `None` when none is recorded yet
    /// (the pending, trust-tier-default state — ADR 0014 §5).
    pub approved: Option<BTreeSet<String>>,
}

/// Maps the plugin host's [`TrustTier`] onto the frontend-visible [`PluginTrust`] DTO, so
/// `genealogy-ui` view-models stay free of plugin-host types (ADR 0014 §3).
fn map_trust(trust: TrustTier) -> PluginTrust {
    match trust {
        TrustTier::Sanctioned => PluginTrust::Sanctioned,
        TrustTier::UserTrusted => PluginTrust::UserTrusted,
        TrustTier::Untrusted => PluginTrust::Untrusted,
    }
}

/// Scans the built-plugins directory and joins each discovered plugin with its persisted
/// enabled/disabled override, sorted by id for a stable table order.
///
/// # Errors
///
/// A localized message if the plugins directory cannot be scanned (e.g. missing — the operator
/// needs to run `cargo xtask build-plugins` in a dev checkout).
pub async fn discover_plugins(services: Services) -> Result<Vec<PluginRow>, String> {
    let chrome = services.chrome();
    let roots = services.trust_roots()?;
    let bundles = genealogy_app::resolve_bundles(&services.plugin_layers());
    let prefs = genealogy_app::read_plugin_preferences(&services.dir);
    let mut rows: Vec<PluginRow> = Vec::with_capacity(bundles.len());
    for bundle_dir in bundles.values() {
        let info = services
            .host
            .discover_bundle(bundle_dir, &roots)
            .map_err(|error| chrome.plugin_error(&error.to_string()))?;
        rows.push(PluginRow {
            enabled: prefs.is_enabled(&info.id),
            trust: map_trust(info.trust),
            approved: prefs.approved_grants(&info.id).cloned(),
            id: info.id,
            role: info.role,
            host_api_version: info.host_api_version,
            capabilities: info.capabilities,
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(rows)
}

/// Persists the operator's approved-capability decision for plugin `id` (ADR 0014 §5), the effective
/// grant the host later intersects with the plugin's declared capabilities. An empty set records
/// "deny everything declared".
///
/// # Errors
///
/// A localized message if the workspace manifest cannot be read or written.
pub async fn set_plugin_grants(services: Services, id: String, approved: BTreeSet<String>) -> Result<(), String> {
    let loc = services.localizer();
    genealogy_ui::approve_plugin_grants(&services.dir, &id, &approved).map_err(|error| loc.error(&error))
}

/// Loads the client-scope pinned-publisher trust store (ADR 0014 §3) for the trust-store editor.
///
/// # Errors
///
/// A localized message if the global config path cannot be resolved or the config cannot be read.
pub async fn load_trust_store(services: Services) -> Result<PluginTrustConfig, String> {
    let chrome = services.chrome();
    let path = config::config_path().map_err(|error| chrome.plugin_error(&error.to_string()))?;
    FileConfigStore::new(path, None)
        .load_plugin_trust()
        .map_err(|error| chrome.plugin_error(&error.to_string()))
}

/// Pins `publisher`'s ed25519 public key (64 hex characters) into the client-scope trust store
/// (ADR 0014 §3).
///
/// # Errors
///
/// A localized message if the key is malformed or the global config cannot be read or written.
pub async fn pin_publisher(services: Services, publisher: String, public_key_hex: String) -> Result<(), String> {
    let chrome = services.chrome();
    let path = config::config_path().map_err(|error| chrome.plugin_error(&error.to_string()))?;
    genealogy_ui::pin_publisher(&path, &publisher, &public_key_hex)
        .map_err(|error| chrome.plugin_error(&error.to_string()))
}

/// Unpins `publisher` from the client-scope trust store (ADR 0014 §3).
///
/// # Errors
///
/// A localized message if the global config cannot be read or written.
pub async fn unpin_publisher(services: Services, publisher: String) -> Result<(), String> {
    let chrome = services.chrome();
    let path = config::config_path().map_err(|error| chrome.plugin_error(&error.to_string()))?;
    genealogy_ui::unpin_publisher(&path, &publisher).map_err(|error| chrome.plugin_error(&error.to_string()))
}

/// Persists whether plugin `id` is enabled (a per-workspace manifest override; PR21). Capabilities
/// remain deny-by-default regardless (ADR 0011 §2) — this flag only gates whether the plugin manager
/// offers to run it at all.
///
/// # Errors
///
/// A localized message if the manifest cannot be read or written.
pub async fn set_plugin_enabled(services: Services, id: String, enabled: bool) -> Result<(), String> {
    let loc = services.localizer();
    FileConfigStore::for_workspace(services.dir.clone())
        .store_plugin_enabled(&id, enabled)
        .map_err(|error| loc.error(&error))
}

/// Runs the `ui-panel` plugin through the host, parses the panel it emitted, and resolves its label
/// IDs against the plugin's own Fluent catalogue (ADR 0012 §5). The host returns the panel as an
/// opaque JSON string; parsing and localization happen here, in the renderer. The render invocation
/// grants only `log` (ADR 0022 §3); submission grants `commands` separately.
pub async fn load_plugin_panel(services: Services) -> Result<Panel, String> {
    let chrome = services.chrome();
    let loc = services.localizer();
    let bundle = services
        .plugin_bundle(UI_PANEL_DOMAIN)
        .ok_or_else(|| chrome.plugin_error(&format!("no plugin bundle found for {UI_PANEL_DOMAIN:?}")))?;
    let effective = services.effective_grants(&bundle)?;
    let component = services
        .host
        .load_bundle(&bundle)
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::new(services.config.operator_agent());
    let grants = invocation_grants(&effective, &[Capability::Log]);
    let (json, _workspace) = services
        .host
        .run_ui_panel(&component, workspace, session, grants, ResourceBudget::default())
        .await
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let panel = genealogy_ui::parse(&json).map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let requested = services.requested_languages();
    Ok(genealogy_ui::resolve_panel(
        &panel,
        &services.plugin_catalogue_dir(UI_PANEL_DOMAIN),
        UI_PANEL_DOMAIN,
        &requested,
    ))
}

/// Submits an activated action's field values to the `ui-panel` plugin (ADR 0022 §2), returning the
/// resolved [`SubmitResult`]. The submission runs under a Software operator (ADR 0011 §5) and grants
/// `log` + `commands` (deny-by-default), so a plugin mutation is audited through the app boundary. A
/// technical failure (a trap or a denied capability) is a localized error string; validation feedback
/// rides the `SubmitResult::Failure` the plugin returns.
pub async fn submit_plugin_panel(services: Services, action: String, values: String) -> Result<SubmitResult, String> {
    let chrome = services.chrome();
    let loc = services.localizer();
    let bundle = services
        .plugin_bundle(UI_PANEL_DOMAIN)
        .ok_or_else(|| chrome.plugin_error(&format!("no plugin bundle found for {UI_PANEL_DOMAIN:?}")))?;
    let effective = services.effective_grants(&bundle)?;
    let component = services
        .host
        .load_bundle(&bundle)
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let session = Session::software(UI_PANEL_DOMAIN, env!("CARGO_PKG_VERSION"));
    let grants = invocation_grants(&effective, &[Capability::Log, Capability::Commands]);
    let (json, _workspace) = services
        .host
        .run_ui_panel_action(
            &component,
            workspace,
            session,
            grants,
            ResourceBudget::default(),
            &action,
            &values,
        )
        .await
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let result = genealogy_ui::parse_submit_result(&json).map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let requested = services.requested_languages();
    Ok(genealogy_ui::resolve_submit_result(
        &result,
        &services.plugin_catalogue_dir(UI_PANEL_DOMAIN),
        UI_PANEL_DOMAIN,
        &requested,
    ))
}

/// The per-plugin `net` allowlist for assisted-import plugins (ADR 0017 §6): the grant-site host
/// allowlist, hardcoded per plugin role (as the plugin grant sets are). A plugin not listed here gets
/// no `net` access — nothing in the WIT is archive-specific; the archive lives only at this grant
/// site. Digitalarkivet is the first entry (`*.digitalarkivet.no`).
const ASSISTED_NET_ALLOWLIST: &[(&str, &str)] = &[("digitalarkivet-import", "*.digitalarkivet.no")];

/// One request the assisted-import invocation makes of the wizard: the opaque `present` payload plus
/// the one-shot channel the wizard answers on. The wizard renders `payload` (parsing it with
/// [`genealogy_ui::parse_payload`]) and replies with an
/// [`ImportResponse`](genealogy_ui::ImportResponse) JSON string through `responder`.
pub struct PresentRequest {
    /// The opaque payload the plugin sent through `present` (the typed assisted-import contract).
    pub payload: String,
    /// The channel the wizard answers on; dropping it cancels the current present (the plugin sees a
    /// `backend` error, ADR 0017 §5).
    pub responder: oneshot::Sender<String>,
}

/// The handle the wizard screen (PR8) consumes to drive an assisted-import session: a stream of
/// [`PresentRequest`]s (each carrying its own response channel) and the eventual session outcome.
/// Dropping the handle cancels the session — the next `present` the plugin makes fails with a
/// `backend` error, which a well-behaved plugin propagates.
pub struct AssistedImportHandle {
    /// Each payload the plugin presents, in order; the wizard answers via the request's `responder`.
    pub requests: mpsc::Receiver<PresentRequest>,
    /// Resolves with the plugin's JSON session summary, or a localized error, when the invocation ends.
    pub outcome: oneshot::Receiver<Result<String, String>>,
}

/// The GUI's [`Presenter`]: forwards each payload to the wizard over the request channel and awaits
/// the wizard's response. A closed request channel or a dropped responder is a cancelled/gone wizard,
/// which the host maps onto `capability-error::backend` (ADR 0017 §5).
struct ChannelPresenter {
    requests: mpsc::Sender<PresentRequest>,
}

#[async_trait]
impl Presenter for ChannelPresenter {
    async fn present(&mut self, payload: String) -> Result<String, PresentError> {
        let (responder, response) = oneshot::channel();
        self.requests
            .send(PresentRequest { payload, responder })
            .await
            .map_err(|_| PresentError::Backend("the import wizard is no longer listening".to_owned()))?;
        response
            .await
            .map_err(|_| PresentError::Backend("the import wizard dropped the response channel".to_owned()))
    }
}

/// The `net` policy for an assisted-import `plugin_id`, from its grant-site allowlist
/// ([`ASSISTED_NET_ALLOWLIST`]). An unlisted plugin gets a deny-all policy.
fn assisted_net_policy(plugin_id: &str) -> NetPolicy {
    let hosts = ASSISTED_NET_ALLOWLIST
        .iter()
        .filter(|(id, _)| *id == plugin_id)
        .map(|(_, pattern)| HostPattern::parse(pattern))
        .collect::<Vec<_>>();
    if hosts.is_empty() {
        NetPolicy::deny_all()
    } else {
        NetPolicy::allow(hosts)
    }
}

/// Narrows an `effective` grant (the ADR 0014 §5 declared∩approved ceiling) down to only the
/// capabilities a specific invocation `needs`. This keeps each host call minimal (e.g. a `ui-panel`
/// render grants only `log` while its submission adds `commands`, ADR 0022 §3) while never exceeding
/// what the operator approved — a capability the operator did not approve stays denied on every call.
fn invocation_grants(effective: &Grants, needs: &[Capability]) -> Grants {
    let mut grants = Grants::none();
    for &capability in needs {
        if effective.allows(capability) {
            grants = grants.with(capability);
        }
    }
    grants
}

/// Starts an assisted-import session for `plugin_id` and `request`, returning the [`AssistedImportHandle`]
/// the wizard drives plus the session future the caller spawns.
///
/// The future is deliberately **not** `Send` (`Services` holds `Rc`s), so the wizard spawns it on the
/// renderer's local executor (Dioxus `spawn`), where it runs concurrently with the UI: each time the
/// plugin calls `present`, the future suspends on the wizard's answer (no fuel burns during the wait,
/// ADR 0017 §8). The invocation runs under a Software operator (ADR 0011 §5), the plugin's grant-site
/// `net` allowlist ([`assisted_net_policy`]), the full assisted grant set, and a
/// [`Confidence::Low`](genealogy_app::Confidence::Low) provenance template (ADR 0017 §7).
pub fn start_assisted_import(
    services: Services,
    plugin_id: String,
    request: String,
) -> (AssistedImportHandle, impl Future<Output = ()>) {
    let (request_tx, request_rx) = mpsc::channel::<PresentRequest>(1);
    let (outcome_tx, outcome_rx) = oneshot::channel();
    let presenter = ChannelPresenter { requests: request_tx };
    let future = async move {
        let outcome = run_assisted_session(services, plugin_id, request, Box::new(presenter)).await;
        // A dropped receiver just means the wizard closed first; nothing else needs the outcome.
        drop(outcome_tx.send(outcome));
    };
    (
        AssistedImportHandle {
            requests: request_rx,
            outcome: outcome_rx,
        },
        future,
    )
}

/// Runs the assisted-import invocation to completion, returning the plugin's session summary or a
/// localized error. Loads the plugin component, opens a fresh workspace, and drives
/// [`genealogy_plugin_host::PluginHost::run_assisted_import`] with the channel-backed `presenter`.
async fn run_assisted_session(
    services: Services,
    plugin_id: String,
    request: String,
    presenter: Box<dyn Presenter>,
) -> Result<String, String> {
    let chrome = services.chrome();
    let loc = services.localizer();
    let bundle = services
        .plugin_bundle(&plugin_id)
        .ok_or_else(|| chrome.plugin_error(&format!("no plugin bundle found for {plugin_id:?}")))?;
    let grants = services.effective_grants(&bundle)?;
    let component = services
        .host
        .load_bundle(&bundle)
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let invocation = Invocation {
        session: Session::software(plugin_id.clone(), env!("CARGO_PKG_VERSION")),
        net_policy: assisted_net_policy(&plugin_id),
        workspace,
        grants,
        budget: ResourceBudget::assisted(),
        ai_config: ai_config(&services),
        provenance_confidence: Some(Confidence::Low),
    };
    let (summary, _workspace) = services
        .host
        .run_assisted_import(&component, invocation, &request, presenter, |_update| {
            ProgressControl::Proceed
        })
        .await
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    Ok(summary)
}

/// The AI provider inventory for the assisted flow: the client-scope `[ai]` config for the open
/// workspace, falling back to the global config's copy if the workspace store cannot be read.
fn ai_config(services: &Services) -> AiConfig {
    match FileConfigStore::for_workspace(services.dir.clone()).load_ai_config() {
        Ok(ai) => ai,
        Err(error) => {
            tracing::warn!(%error, "could not read the AI config; falling back to the global config");
            services.config.ai.clone()
        }
    }
}

/// Reads the client-scope `[map]` config (ADR 0025 §3 / ADR 0033) from the already-loaded global
/// config. Unlike [`ai_config`], this has only the one path: the workspace-scoped
/// `FileConfigStore::for_workspace` has no `config_path` set (`config_store.rs`), so `[map]` — which
/// lives in the *global* config, not the workspace manifest — could never round-trip through it. That
/// was #283's root cause: every switch persisted nowhere and every read silently fell back to the
/// built-in default, so the toolbar select always looked reset.
pub(crate) fn map_config(services: &Services) -> MapConfig {
    services.config.map.clone()
}

/// Persists `map` as the client-scope `[map]` config (ADR 0025 §3 / ADR 0033) — the global config,
/// the only file [`map_config`] can ever read it back from. Needs no `Services` — the global config
/// path is not workspace-scoped.
///
/// # Errors
///
/// The technical cause (a config path/read/write failure) as plain text — the caller wraps it in its
/// own localized template (`Chrome::geography_provider_switch_error`), since a generic
/// [`Chrome::plugin_error`] would misname this a plugin failure.
pub async fn store_map_config(map: MapConfig) -> Result<(), String> {
    let path = config::config_path().map_err(|error| error.to_string())?;
    FileConfigStore::new(path, None)
        .store_map_config(&map)
        .map_err(|error| error.to_string())
}

/// Resolves `provider` into the [`MapSource`] a renderer mounts (ADR 0033): env substitution for a
/// `MapLibre` style, or a minted Google session for the Google adapter. The tile source a renderer
/// paints, never the configured `kind` — see `genealogy_app::resolve_map_source`'s doc comment. Needs
/// no `Services` — the whole resolution is env vars and outbound HTTP, nothing workspace-scoped.
///
/// # Errors
///
/// The technical cause (a missing API-key env var, or an unreachable style/session endpoint) as plain
/// text — see [`store_map_config`]'s doc comment for why this is not already localized.
pub async fn resolve_map_source(provider: MapProvider) -> Result<MapSource, String> {
    genealogy_app::resolve_map_source(&provider)
        .await
        .map_err(|error| error.to_string())
}

/// Refreshes the live per-viewport attribution Google's Map Tiles terms require, for the camera at
/// `zoom`/`bounds` (`north, south, east, west`) — a no-op (`Ok(None)`) for every provider but
/// [`genealogy_app::MapProvider::Google`], whose terms are the only ones requiring a dynamic credit.
/// Needs no `Services`, for the same reason [`resolve_map_source`] does not.
///
/// # Errors
///
/// The technical cause as plain text — see [`store_map_config`]'s doc comment for why this is not
/// already localized (the caller's own refresh silently drops this one rather than toasting it, since
/// a failed background refresh is not worth interrupting the operator over).
pub async fn refresh_map_attribution(
    provider: MapProvider,
    zoom: f64,
    bounds: (f64, f64, f64, f64),
) -> Result<Option<String>, String> {
    genealogy_app::refresh_map_attribution(&provider, zoom, bounds)
        .await
        .map_err(|error| error.to_string())
}

/// How many progress reports a bulk-export or bulk-import channel buffers. The guest reports far
/// faster than the UI repaints, and a full buffer simply drops the update (the next one supersedes it
/// anyway), so this only has to be deep enough that a repaint-sized burst is not lost.
const BULK_PROGRESS_BUFFER: usize = 16;

/// The handle the export wizard consumes to follow a bulk-export run: the stream of progress reports,
/// the cancel flag it sets to stop the run, and the eventual outcome.
///
/// Cancelling is cooperative: the flag is read by the progress sink, so the run stops at the guest's
/// next progress report and then fails out through `outcome`.
pub struct BulkExportHandle {
    /// Each progress report the plugin makes, in order. Dropping the receiver does not stop the run.
    pub progress: mpsc::Receiver<ProgressUpdate>,
    /// Set to `true` to cancel: the next progress report answers [`ProgressControl::Cancel`].
    pub cancel: Arc<AtomicBool>,
    /// Resolves with the records written and the destination the run wrote to, or a localized error.
    pub outcome: oneshot::Receiver<Result<(u32, PathBuf), String>>,
}

/// Starts a bulk export of the open workspace with `plugin_id` into `target` (ADR 0013), returning the
/// [`BulkExportHandle`] the wizard follows plus the session future the caller spawns.
///
/// As with [`start_assisted_import`], the future is deliberately **not** `Send` ([`Services`] holds
/// `Rc`s) and is spawned on the renderer's local executor. The host's progress sink, by contrast,
/// *must* be `Send + 'static`, so it captures only the channel and the cancel flag — never
/// [`Services`]. The invocation runs under a Software operator with a deny-all network policy, as the
/// CLI's `genealogy export` does.
pub fn start_bulk_export(
    services: Services,
    plugin_id: String,
    target: ExportTarget,
) -> (BulkExportHandle, impl Future<Output = ()>) {
    let (progress_tx, progress_rx) = mpsc::channel::<ProgressUpdate>(BULK_PROGRESS_BUFFER);
    let (outcome_tx, outcome_rx) = oneshot::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let sink = bulk_progress_sink(progress_tx, Arc::clone(&cancel));
    let future = async move {
        let outcome = run_export_session(services, plugin_id, target, sink).await;
        // A dropped receiver just means the wizard closed first; nothing else needs the outcome.
        drop(outcome_tx.send(outcome));
    };
    (
        BulkExportHandle {
            progress: progress_rx,
            cancel,
            outcome: outcome_rx,
        },
        future,
    )
}

/// The host progress sink shared by the bulk-export and bulk-import wizards: forwards each report to
/// the wizard and answers with the cancel flag's current value.
///
/// The forward is a non-blocking `try_send` — the host calls this from the guest's progress hook, so
/// it must never block, and a full or closed channel is not a reason to stop the run (the wizard may
/// simply have closed its progress view). Only the flag cancels.
fn bulk_progress_sink(
    updates: mpsc::Sender<ProgressUpdate>,
    cancel: Arc<AtomicBool>,
) -> impl FnMut(ProgressUpdate) -> ProgressControl + Send + 'static {
    move |update| {
        if cancel.load(Ordering::Relaxed) {
            return ProgressControl::Cancel;
        }
        drop(updates.try_send(update));
        ProgressControl::Proceed
    }
}

/// Runs the bulk-export invocation to completion, returning the records written and the destination
/// or a localized error. Mirrors the CLI's `genealogy export` (ADR 0013): resolve the bundle, take the
/// operator's effective grant, open a fresh workspace, and run the plugin under a Software session.
///
/// The destination reported back is the target's own path: with an [`ExportTarget::Directory`] the
/// plugin's suggested file name decides the leaf, which the host resolves and does not report, so the
/// directory is the most the wizard can name — the same thing the CLI prints.
async fn run_export_session(
    services: Services,
    plugin_id: String,
    target: ExportTarget,
    progress: impl FnMut(ProgressUpdate) -> ProgressControl + Send + 'static,
) -> Result<(u32, PathBuf), String> {
    let chrome = services.chrome();
    let loc = services.localizer();
    let destination = match &target {
        ExportTarget::File(path) | ExportTarget::Directory(path) => path.clone(),
    };
    let bundle = services
        .plugin_bundle(&plugin_id)
        .ok_or_else(|| chrome.plugin_error(&format!("no plugin bundle found for {plugin_id:?}")))?;
    let grants = services.effective_grants(&bundle)?;
    let component = services
        .host
        .load_bundle(&bundle)
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    let invocation = Invocation {
        session: Session::software(plugin_id, env!("CARGO_PKG_VERSION")),
        net_policy: NetPolicy::deny_all(),
        workspace,
        grants,
        budget: ResourceBudget::default(),
        ai_config: AiConfig::default(),
        provenance_confidence: None,
    };
    let (records, _workspace) = services
        .host
        .run_bulk_export(&component, invocation, target, progress)
        .await
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    Ok((records, destination))
}

/// The handle the bulk-import wizard consumes to follow a bulk-import run: the stream of progress
/// reports, the cancel flag it sets to stop the run, and the eventual outcome. Mirrors
/// [`BulkExportHandle`], minus the destination round-trip an import does not need — the source is
/// exactly what the operator typed, so the caller already has it.
///
/// Cancelling is cooperative: the flag is read by the progress sink, so the run stops at the guest's
/// next progress report and then fails out through `outcome`.
pub struct BulkImportHandle {
    /// Each progress report the plugin makes, in order. Dropping the receiver does not stop the run.
    pub progress: mpsc::Receiver<ProgressUpdate>,
    /// Set to `true` to cancel: the next progress report answers [`ProgressControl::Cancel`].
    pub cancel: Arc<AtomicBool>,
    /// Resolves with the number of records imported, or a localized error.
    pub outcome: oneshot::Receiver<Result<u32, String>>,
}

/// Starts a bulk import of `source` with `plugin_id` into `target` (issue #191), returning the
/// [`BulkImportHandle`] the wizard follows plus the session future the caller spawns.
///
/// As with [`start_bulk_export`], the future is deliberately **not** `Send` ([`Services`] holds `Rc`s)
/// and is spawned on the renderer's local executor. The host's progress sink, by contrast, *must* be
/// `Send + 'static`, so it captures only the channel and the cancel flag — never [`Services`]. The
/// invocation runs under a Software operator with a deny-all network policy, as the CLI's `genealogy
/// import` does.
pub fn start_bulk_import(
    services: Services,
    plugin_id: String,
    source: PathBuf,
    target: ImportTargetChoice,
) -> (BulkImportHandle, impl Future<Output = ()>) {
    let (progress_tx, progress_rx) = mpsc::channel::<ProgressUpdate>(BULK_PROGRESS_BUFFER);
    let (outcome_tx, outcome_rx) = oneshot::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let sink = bulk_progress_sink(progress_tx, Arc::clone(&cancel));
    let future = async move {
        let outcome = run_bulk_import_session(services, plugin_id, source, target, sink).await;
        // A dropped receiver just means the wizard closed first; nothing else needs the outcome.
        drop(outcome_tx.send(outcome));
    };
    (
        BulkImportHandle {
            progress: progress_rx,
            cancel,
            outcome: outcome_rx,
        },
        future,
    )
}

/// Opens a registered workspace by name, which may not be the one currently open — mirroring the
/// CLI's `--into NAME` resolution (`main.rs:396-401`). Shared by the bulk-import run itself and its
/// non-empty-workspace probe ([`count_workspace_persons`]).
async fn open_workspace_by_name(services: &Services, name: &str) -> Result<Workspace, String> {
    let loc = services.localizer();
    let dir = services
        .config
        .resolve_workspace(Some(name))
        .map_err(|error| loc.error(&error))?;
    Workspace::open(&dir, &services.config.operator, &services.config.workspace_defaults)
        .await
        .map_err(|error| loc.error(&error))
}

/// Counts the persons already in a registered workspace, opening it fresh by name. The bulk-import
/// target stage runs this before an import into an *existing* workspace and confirms in a `Modal` when
/// it is non-empty — the GUI shape of the CLI's own confirm (`main.rs:350-359`). A freshly registered
/// (`ImportTargetChoice::New`) workspace is always empty, so it is never probed.
///
/// # Errors
/// A localized error if the workspace cannot be resolved or opened, or its persons cannot be listed.
pub async fn count_workspace_persons(services: &Services, workspace: &str) -> Result<usize, String> {
    let loc = services.localizer();
    let opened = open_workspace_by_name(services, workspace).await?;
    let persons = genealogy_app::list_persons(&opened)
        .await
        .map_err(|error| loc.error(&error))?;
    Ok(persons.len())
}

/// Opens the bulk import's target workspace: an already-registered one by name, or a freshly
/// registered one. Registering calls `genealogy_app::register_workspace` directly (not this module's
/// `register_workspace` wrapper) so the new workspace's path — needed to open it — is not thrown away;
/// there is no separate confirm step for a `New` target, since a fresh workspace is always empty
/// (mirrors the CLI's `--new NAME PATH`, `main.rs:371-393`).
async fn open_import_target(services: &Services, target: &ImportTargetChoice) -> Result<Workspace, String> {
    let loc = services.localizer();
    match target {
        ImportTargetChoice::Existing { workspace } => open_workspace_by_name(services, workspace).await,
        ImportTargetChoice::New {
            name,
            directory,
            database_url,
        } => {
            let config_path = config::config_path().map_err(|error| loc.error(&error))?;
            let summary =
                genealogy_app::register_workspace(&config_path, name, directory.as_deref(), database_url.as_deref())
                    .await
                    .map_err(|error| loc.error(&error))?;
            Workspace::open(
                &summary.path,
                &services.config.operator,
                &services.config.workspace_defaults,
            )
            .await
            .map_err(|error| loc.error(&error))
        }
    }
}

/// Runs the bulk-import invocation to completion, returning the number of records imported or a
/// localized error. Mirrors the CLI's `genealogy import` (ADR 0013): resolve the bundle (from the
/// currently open workspace's plugin layers, same as the bulk-export and assisted-import wizards),
/// take the operator's effective grant, open the *target* workspace (which may differ from the one
/// currently open), and run the plugin under a Software session.
async fn run_bulk_import_session(
    services: Services,
    plugin_id: String,
    source: PathBuf,
    target: ImportTargetChoice,
    progress: impl FnMut(ProgressUpdate) -> ProgressControl + Send + 'static,
) -> Result<u32, String> {
    let chrome = services.chrome();
    let bundle = services
        .plugin_bundle(&plugin_id)
        .ok_or_else(|| chrome.plugin_error(&format!("no plugin bundle found for {plugin_id:?}")))?;
    let grants = services.effective_grants(&bundle)?;
    let component = services
        .host
        .load_bundle(&bundle)
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    let workspace = open_import_target(&services, &target).await?;
    let invocation = Invocation {
        session: Session::software(plugin_id, env!("CARGO_PKG_VERSION")),
        net_policy: NetPolicy::deny_all(),
        workspace,
        grants,
        budget: ResourceBudget::default(),
        ai_config: AiConfig::default(),
        provenance_confidence: None,
    };
    let (records, _workspace) = services
        .host
        .run_bulk_import(&component, invocation, source, progress)
        .await
        .map_err(|error| chrome.plugin_error(&error.to_string()))?;
    Ok(records)
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
    /// The workspace manifest's **own** `[surety]` overrides, unresolved: what the Surety card edits
    /// and writes back in the workspace scope. The shared scope's own values are
    /// `config.workspace_defaults.surety`.
    pub surety_workspace: SuretyLabelOverrides,
    /// The registered workspaces (name order, default + engine flagged) for the "Workspaces" card.
    pub workspaces: Vec<WorkspaceSummary>,
    /// The name of the workspace open this session — the row it matches shows the "Active" badge.
    pub open_workspace: String,
    /// The client-scope `[shortcuts]` rebound-chord overrides (ADR 0030 §3) — global-only, no
    /// workspace-manifest layer.
    pub shortcuts: ShortcutConfig,
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
        surety_workspace: read_surety_label_overrides(&services.dir),
        workspaces: list_workspaces(&services.config),
        open_workspace: services.open_workspace.clone(),
        shortcuts: services.config.shortcuts.clone(),
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
    let loc = services.localizer();
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    FileConfigStore::new(path, None)
        .store_operator_identity(display, email)
        .map_err(|error| loc.error(&error))
}

/// Saves the live-fallback `HumanId` formats, returning a localized error on failure.
pub fn save_id_format_defaults(services: &Services, id_formats: IdFormats) -> Result<(), String> {
    let loc = services.localizer();
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    FileConfigStore::new(path, None)
        .store_workspace_default_id_formats(id_formats)
        .map_err(|error| loc.error(&error))
}

/// Saves the live-fallback language/locale/date/number defaults, returning a localized error on
/// failure.
pub fn save_locale_defaults(services: &Services, locale: LocaleDefaults) -> Result<(), String> {
    let loc = services.localizer();
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    FileConfigStore::new(path, None)
        .store_workspace_default_locale(locale)
        .map_err(|error| loc.error(&error))
}

/// Saves the live-fallback surety-scheme label overrides, returning a localized error on failure
/// (ADR 0027).
pub fn save_surety_defaults(services: &Services, surety: SuretyLabelOverrides) -> Result<(), String> {
    let loc = services.localizer();
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    FileConfigStore::new(path, None)
        .store_workspace_default_surety(surety)
        .map_err(|error| loc.error(&error))
}

/// Saves the open workspace's own surety-scheme label overrides into its manifest `[surety]` block,
/// returning a localized error on failure (ADR 0027). The per-workspace counterpart to
/// [`save_surety_defaults`]: this scope wins over that live fallback, per ordinal.
pub fn save_surety_workspace_overrides(services: &Services, surety: SuretyLabelOverrides) -> Result<(), String> {
    let loc = services.localizer();
    FileConfigStore::for_workspace(services.dir.clone())
        .store_surety_label_overrides(surety)
        .map_err(|error| loc.error(&error))
}

/// Saves the client-scope `[shortcuts]` rebound-chord map, returning a localized error on failure
/// (ADR 0030 §3).
pub fn save_shortcuts(services: &Services, shortcuts: &ShortcutConfig) -> Result<(), String> {
    let loc = services.localizer();
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    FileConfigStore::new(path, None)
        .store_shortcuts(shortcuts)
        .map_err(|error| loc.error(&error))
}

/// Makes the named workspace the persisted default (last-used), returning a localized error on
/// failure. Persist only — no restart, unlike [`crate::app::open_workspace`]: the currently-open
/// session is unchanged, so the caller just refreshes the card's data.
pub fn make_default_workspace(services: &Services, name: &str) -> Result<(), String> {
    let loc = services.localizer();
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    FileConfigStore::new(path, None)
        .store_default_workspace(name)
        .map_err(|error| loc.error(&error))
}

/// Registers a new workspace (and makes it the default), returning a localized error on failure.
/// `dir` is the optional workspace directory; `None` uses the default data directory. `database_url`
/// overrides the engine default (`None` ⇒ SQLite); the Preferences form only ever fills it in when
/// the `postgres` feature is compiled in, but this signature stays unconditional so the plumbing
/// needs no `cfg`. Creates the directory, database, and manifest via
/// [`genealogy_app::register_workspace`]. The caller triggers the application-state restart on
/// success.
pub async fn register_workspace(
    services: &Services,
    name: &str,
    dir: Option<PathBuf>,
    database_url: Option<&str>,
) -> Result<(), String> {
    let loc = services.localizer();
    let path = config::config_path().map_err(|error| loc.error(&error))?;
    genealogy_app::register_workspace(&path, name, dir.as_deref(), database_url)
        .await
        .map(|_summary| ())
        .map_err(|error| loc.error(&error))
}

/// Rebuilds every projection from the event log for the open workspace, returning a localized error
/// on failure. The Preferences Maintenance card's counterpart to the CLI's `genealogy rebuild`
/// (`Workspace::rebuild_projections`, an ADR 0010 maintenance op) — the tool for recovering after a
/// `genealogy-db` schema change.
pub async fn rebuild_projections(services: Services) -> Result<(), String> {
    let loc = services.localizer();
    let workspace = services.open().await.map_err(|error| loc.error(&error))?;
    workspace.rebuild_projections().await.map_err(|error| loc.error(&error))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use genealogy_plugin_host::{PresentError, Presenter, ProgressControl, ProgressUpdate};
    use tokio::sync::mpsc;

    use super::{ChannelPresenter, PresentRequest, bulk_progress_sink};

    fn update(step: &str, processed: u32) -> ProgressUpdate {
        ProgressUpdate {
            step: step.to_owned(),
            processed,
            total: Some(120),
        }
    }

    /// The export progress sink forwards every report to the wizard and lets the run proceed.
    #[tokio::test]
    async fn export_progress_reports_reach_the_wizard() {
        let (updates, mut reports) = mpsc::channel::<ProgressUpdate>(4);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut sink = bulk_progress_sink(updates, cancel);

        assert_eq!(sink(update("persons", 40)), ProgressControl::Proceed);
        assert_eq!(sink(update("families", 90)), ProgressControl::Proceed);

        let first = reports.recv().await.expect("the first report arrives");
        assert_eq!(
            (first.step.as_str(), first.processed, first.total),
            ("persons", 40, Some(120))
        );
        let second = reports.recv().await.expect("the second report arrives");
        assert_eq!(second.step, "families");
    }

    /// Raising the cancel flag is the whole cancel mechanism: the next report answers `Cancel`.
    #[tokio::test]
    async fn a_raised_cancel_flag_stops_the_run() {
        let (updates, mut reports) = mpsc::channel::<ProgressUpdate>(4);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut sink = bulk_progress_sink(updates, Arc::clone(&cancel));

        assert_eq!(sink(update("persons", 10)), ProgressControl::Proceed);
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(sink(update("persons", 20)), ProgressControl::Cancel);

        // The cancelled report is not forwarded — only the one before it.
        assert_eq!(reports.recv().await.map(|report| report.processed), Some(10));
        assert!(reports.try_recv().is_err(), "the cancelled report is not forwarded");
    }

    /// A wizard that stopped listening (its receiver dropped) does not abort the export — only the
    /// cancel flag does. The report is dropped and the run proceeds.
    #[tokio::test]
    async fn a_dropped_progress_receiver_does_not_stop_the_run() {
        let (updates, reports) = mpsc::channel::<ProgressUpdate>(4);
        drop(reports);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut sink = bulk_progress_sink(updates, cancel);

        assert_eq!(sink(update("persons", 40)), ProgressControl::Proceed);
    }

    /// A full buffer is likewise not a reason to stop: the guest reports faster than the UI repaints,
    /// and the next report supersedes the dropped one.
    #[tokio::test]
    async fn a_full_progress_buffer_does_not_stop_the_run() {
        let (updates, _reports) = mpsc::channel::<ProgressUpdate>(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut sink = bulk_progress_sink(updates, cancel);

        assert_eq!(sink(update("persons", 1)), ProgressControl::Proceed);
        assert_eq!(sink(update("persons", 2)), ProgressControl::Proceed);
    }

    /// The channel-backed presenter forwards a payload and returns the wizard's answer — the
    /// round-trip the wizard drives, exercised without a webview.
    #[tokio::test]
    async fn channel_presenter_round_trips_a_payload_and_response() {
        let (request_tx, mut request_rx) = mpsc::channel::<PresentRequest>(1);
        let mut presenter = ChannelPresenter { requests: request_tx };

        // A stand-in wizard: read the one request and answer it.
        let wizard = tokio::spawn(async move {
            let PresentRequest { payload, responder } = request_rx.recv().await.expect("a request arrives");
            assert_eq!(payload, r#"{"kind":"summary","imported":[],"skipped":0}"#);
            responder
                .send(r#"{"kind":"submit","action":"done"}"#.to_owned())
                .expect("the presenter is still awaiting");
        });

        let response = presenter
            .present(r#"{"kind":"summary","imported":[],"skipped":0}"#.to_owned())
            .await
            .expect("the wizard answers");
        assert_eq!(response, r#"{"kind":"submit","action":"done"}"#);
        wizard.await.expect("wizard task");
    }

    /// A dropped responder (the wizard closed mid-present) surfaces as a `backend` failure the plugin
    /// can propagate — the cancellation-by-channel-drop path (ADR 0017 §5).
    #[tokio::test]
    async fn a_dropped_responder_is_a_backend_failure() {
        let (request_tx, mut request_rx) = mpsc::channel::<PresentRequest>(1);
        let mut presenter = ChannelPresenter { requests: request_tx };

        let wizard = tokio::spawn(async move {
            let PresentRequest { responder, .. } = request_rx.recv().await.expect("a request arrives");
            drop(responder); // the wizard closes without answering
        });

        let error = presenter
            .present(r#"{"kind":"summary"}"#.to_owned())
            .await
            .expect_err("a dropped responder fails the present");
        assert!(matches!(error, PresentError::Backend(_)));
        wizard.await.expect("wizard task");
    }

    /// A closed request channel (the wizard was never started, or is gone) is likewise a `backend`
    /// failure — the presenter cannot reach the wizard.
    #[tokio::test]
    async fn a_closed_request_channel_is_a_backend_failure() {
        let (request_tx, request_rx) = mpsc::channel::<PresentRequest>(1);
        drop(request_rx);
        let mut presenter = ChannelPresenter { requests: request_tx };
        let error = presenter
            .present(r#"{"kind":"summary"}"#.to_owned())
            .await
            .expect_err("a closed channel fails the present");
        assert!(matches!(error, PresentError::Backend(_)));
    }
}
