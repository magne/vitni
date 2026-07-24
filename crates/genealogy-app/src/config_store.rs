//! The configuration seam (ADR 0015): the language-request resolver and the [`ConfigStore`] trait.
//!
//! Configuration is grouped by *owner* into three scopes — workspace-functionality, operator, and
//! client/presentation (ADR 0015 §1) — read and written through [`ConfigStore`]. One
//! [`FileConfigStore`] backs it with the two ADR 0005 TOML files; a database backend plugs into the
//! same trait in Phase 13.
//!
//! The [`resolve_requested_languages`] resolver fixes the env-precedence bug (ADR 0015 §4): the
//! frontends built their Fluent localizers from the raw environment request and never consulted the
//! configured `ui_language`, so a bare `LANGUAGE` outranked stored config. The resolver keeps the
//! order **plain env < configured `ui_language` < `GENEALOGY_LANGUAGE`**. It is pure (takes the
//! environment as arguments) so every precedence case is unit-tested; the frontends supply the plain
//! request from `DesktopLanguageRequester`, keeping this crate free of `i18n_embed`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use unic_langid::LanguageIdentifier;

use crate::config::{
    self, AiConfig, Config, IdFormats, LocaleDefaults, MapConfig, OperatorConfig, PluginTrustConfig,
    SuretyLabelOverrides, ThemeMode,
};
use crate::error::AppError;
use crate::workspace::{
    self, IdFormatOverrides, LocaleOverrides, OperatorRecord, PluginPreferences, RecentItem, WindowGeometry,
};

/// The app-scoped UI-language override environment variable (ADR 0015 §4) — the highest-priority
/// signal, above configuration.
const LANGUAGE_ENV: &str = "GENEALOGY_LANGUAGE";

/// Resolves the ordered language request from the three signals, highest priority last (ADR 0015 §4):
/// the ambient `plain_env` (`LANGUAGE`/`LANG`), the configured `ui_language`, then the app-scoped
/// `prefixed_env` (`GENEALOGY_LANGUAGE`).
///
/// Returns `[prefixed_env]` if set, else `[config_ui_language]` if set, else `plain_env` verbatim —
/// so configuration wins over the ambient system locale (the bug fix) and the explicit env override
/// wins over both.
#[must_use]
pub fn resolve_requested_languages(
    config_ui_language: Option<&LanguageIdentifier>,
    plain_env: &[LanguageIdentifier],
    prefixed_env: Option<&LanguageIdentifier>,
) -> Vec<LanguageIdentifier> {
    if let Some(prefixed) = prefixed_env {
        return vec![prefixed.clone()];
    }
    if let Some(config) = config_ui_language {
        return vec![config.clone()];
    }
    plain_env.to_vec()
}

/// Reads and parses the `GENEALOGY_LANGUAGE` override (ADR 0015 §4); `None` when unset, empty, or not
/// a valid BCP-47 tag.
#[must_use]
pub fn genealogy_language_env() -> Option<LanguageIdentifier> {
    let value = std::env::var(LANGUAGE_ENV).ok()?;
    if value.is_empty() {
        return None;
    }
    value.parse().ok()
}

/// The language request for a real startup: overlays [`genealogy_language_env`] on top of the
/// configured `ui_language` and the ambient `plain_env` (ADR 0015 §4). Frontends call this with the
/// plain request from `DesktopLanguageRequester` to build their Fluent localizers.
#[must_use]
pub fn requested_languages_for(
    config_ui_language: Option<&LanguageIdentifier>,
    plain_env: &[LanguageIdentifier],
) -> Vec<LanguageIdentifier> {
    resolve_requested_languages(config_ui_language, plain_env, genealogy_language_env().as_ref())
}

/// The workspace-functionality scope (ADR 0015 §1): the dataset and how it behaves. The per-workspace
/// half of the manifest — `database_url`, the `id_formats` overrides, the `operators` list, and the
/// plugin toggles. Shared: for a remote workspace this lives server-side with the data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFunctionality {
    /// The database backing this workspace (SQLite file ref or Postgres URL), frozen at `init`.
    pub database_url: String,
    /// Per-aggregate `HumanId` format overrides; absent fields fall back to the global defaults.
    pub id_formats: IdFormatOverrides,
    /// Operators who have used this workspace, keyed by operator id.
    pub operators: BTreeMap<String, OperatorRecord>,
    /// Per-plugin enable/disable overrides.
    pub plugins: PluginPreferences,
}

/// The client/presentation scope (ADR 0015 §1): how *this* session presents the workspace. The
/// per-workspace half of the manifest — colour theme, native-window geometry, the "Jump back in"
/// recent list, and the locale overrides. Local to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presentation {
    /// The pinned colour-theme mode; `None` uses the global default.
    pub theme: Option<ThemeMode>,
    /// The saved native-window geometry; `None` until the window is first moved/resized.
    pub window: Option<WindowGeometry>,
    /// The recently-opened records, newest first.
    pub recent: Vec<RecentItem>,
    /// The language/locale/date/number overrides.
    pub locale: LocaleOverrides,
}

/// The configuration storage seam (ADR 0015 §2): reads and writes configuration grouped by owner into
/// the three scopes. One implementation ([`FileConfigStore`]) backs it with the two ADR 0005 TOML
/// files; a database backend plugs into the same trait in Phase 13.
///
/// The scope groups are: **operator** (the acting identity), **workspace-functionality** (the dataset
/// and how it behaves — the manifest functionality half plus the global registry / app defaults it
/// belongs to per ADR 0015 §1), and **client/presentation** (how this session presents the workspace —
/// the manifest presentation half plus the live app-level presentation default).
pub trait ConfigStore {
    // ===== Operator scope =====

    /// Loads the default operator identity.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the global config cannot be read.
    fn load_operator(&self) -> Result<OperatorConfig, AppError>;

    /// Persists the operator's display name and email (the stable `id` never changes).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the global config cannot be read or written.
    fn store_operator_identity(&self, display: Option<String>, email: Option<String>) -> Result<(), AppError>;

    // ===== Workspace-functionality scope =====

    /// Loads the whole global config (operator + registry + defaults).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the file is missing, unreadable, or not valid TOML.
    fn load_config(&self) -> Result<Config, AppError>;

    /// Loads the global config, bootstrapping a default one (with a fresh operator) if absent.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if paths cannot be resolved or the file cannot be read/written.
    fn load_or_bootstrap_config(&self) -> Result<Config, AppError>;

    /// Persists the whole global config.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the directory or file cannot be written.
    fn store_config(&self, config: &Config) -> Result<(), AppError>;

    /// Persists the live-fallback `HumanId` formats (`[workspace-defaults.id_formats]`).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the config cannot be read or written.
    fn store_workspace_default_id_formats(&self, id_formats: IdFormats) -> Result<(), AppError>;

    /// Persists the live-fallback surety-scheme label overrides (`[workspace-defaults.surety]`,
    /// ADR 0027).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the config cannot be read or written.
    fn store_workspace_default_surety(&self, surety: SuretyLabelOverrides) -> Result<(), AppError>;

    /// Switches the default (last-used) workspace by name.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the config cannot be read/written, or `name` is not registered.
    fn store_default_workspace(&self, name: &str) -> Result<(), AppError>;

    /// Loads the open workspace's functionality scope from its manifest.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if no workspace directory is set, or [`AppError::Workspace`] if the
    /// manifest is missing/invalid.
    fn load_workspace_functionality(&self) -> Result<WorkspaceFunctionality, AppError>;

    /// Persists the functionality scope into the manifest (read-modify-write, preserving the
    /// presentation half).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if no workspace directory is set, or [`AppError::Workspace`] if the
    /// manifest cannot be read/written.
    fn store_workspace_functionality(&self, functionality: &WorkspaceFunctionality) -> Result<(), AppError>;

    /// Persists whether plugin `id` is enabled (a manifest override).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if no workspace directory is set, or [`AppError::Workspace`] if the
    /// manifest cannot be read/written.
    fn store_plugin_enabled(&self, id: &str, enabled: bool) -> Result<(), AppError>;

    // ===== Client/presentation scope =====

    /// Persists the live-fallback locale defaults (`[workspace-defaults.locale]`).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the config cannot be read or written.
    fn store_workspace_default_locale(&self, locale: LocaleDefaults) -> Result<(), AppError>;

    /// Loads the open workspace's presentation scope from its manifest.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if no workspace directory is set, or [`AppError::Workspace`] if the
    /// manifest is missing/invalid.
    fn load_presentation(&self) -> Result<Presentation, AppError>;

    /// Persists the colour-theme mode into the manifest (read-modify-write).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if no workspace directory is set, or [`AppError::Workspace`] if the
    /// manifest cannot be read/written.
    fn store_theme(&self, theme: ThemeMode) -> Result<(), AppError>;

    /// Persists the locale overrides into the manifest (read-modify-write).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if no workspace directory is set, or [`AppError::Workspace`] if the
    /// manifest cannot be read/written.
    fn store_locale(&self, locale: LocaleOverrides) -> Result<(), AppError>;

    /// Persists the native-window geometry into the manifest (read-modify-write).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if no workspace directory is set, or [`AppError::Workspace`] if the
    /// manifest cannot be read/written.
    fn store_window(&self, geometry: WindowGeometry) -> Result<(), AppError>;

    /// Persists the "Jump back in" recent list into the manifest (read-modify-write).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if no workspace directory is set, or [`AppError::Workspace`] if the
    /// manifest cannot be read/written.
    fn store_recent(&self, recent: &[RecentItem]) -> Result<(), AppError>;

    /// Loads the `[ai]` provider inventory (ADR 0017 §4). Client/presentation scope: the providers
    /// are machine/user-local, so they live in the global config, not the workspace manifest.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the global config cannot be read.
    fn load_ai_config(&self) -> Result<AiConfig, AppError>;

    /// Persists the `[ai]` provider inventory into the global config (read-modify-write, preserving
    /// the rest).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the global config cannot be read or written.
    fn store_ai_config(&self, ai: &AiConfig) -> Result<(), AppError>;

    /// Loads the `[map]` provider descriptor (ADR 0025 §3). Client/presentation scope: the tile/style
    /// source is a per-client rendering choice, so it lives in the global config, not the workspace
    /// manifest.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the global config cannot be read.
    fn load_map_config(&self) -> Result<MapConfig, AppError>;

    /// Persists the `[map]` provider descriptor into the global config (read-modify-write, preserving
    /// the rest).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the global config cannot be read or written.
    fn store_map_config(&self, map: &MapConfig) -> Result<(), AppError>;

    /// Loads the `[plugin_trust]` pinned-publisher store (ADR 0014 §3). Client/presentation scope: a
    /// per-user trust decision, machine/user-local, so it lives in the global config.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the global config cannot be read.
    fn load_plugin_trust(&self) -> Result<PluginTrustConfig, AppError>;

    /// Persists the `[plugin_trust]` pinned-publisher store into the global config (read-modify-write,
    /// preserving the rest).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the global config cannot be read or written.
    fn store_plugin_trust(&self, trust: &PluginTrustConfig) -> Result<(), AppError>;
}

/// The file-backed [`ConfigStore`] (ADR 0015 §2): the global config lives at `config_path`, the open
/// workspace's manifest under `workspace_dir`. Either may be absent when a caller needs only the other
/// half (e.g. a theme toggle needs only the workspace); the methods that need a missing path return a
/// clear [`AppError::Config`].
#[derive(Debug, Clone)]
pub struct FileConfigStore {
    config_path: Option<PathBuf>,
    workspace_dir: Option<PathBuf>,
}

impl FileConfigStore {
    /// A store over the global config and, optionally, an open workspace.
    #[must_use]
    pub fn new(config_path: PathBuf, workspace_dir: Option<PathBuf>) -> Self {
        Self {
            config_path: Some(config_path),
            workspace_dir,
        }
    }

    /// A store over just an open workspace's manifest (no global config).
    #[must_use]
    pub fn for_workspace(workspace_dir: PathBuf) -> Self {
        Self {
            config_path: None,
            workspace_dir: Some(workspace_dir),
        }
    }

    fn config_path(&self) -> Result<&Path, AppError> {
        self.config_path
            .as_deref()
            .ok_or_else(|| AppError::Config("config store has no config path".to_owned()))
    }

    fn workspace_dir(&self) -> Result<&Path, AppError> {
        self.workspace_dir
            .as_deref()
            .ok_or_else(|| AppError::Config("config store has no workspace directory".to_owned()))
    }
}

impl ConfigStore for FileConfigStore {
    fn load_operator(&self) -> Result<OperatorConfig, AppError> {
        Ok(config::load(self.config_path()?)?.operator)
    }

    fn store_operator_identity(&self, display: Option<String>, email: Option<String>) -> Result<(), AppError> {
        config::set_operator_identity(self.config_path()?, display, email)
    }

    fn load_config(&self) -> Result<Config, AppError> {
        config::load(self.config_path()?)
    }

    fn load_or_bootstrap_config(&self) -> Result<Config, AppError> {
        config::load_or_bootstrap(self.config_path()?)
    }

    fn store_config(&self, config: &Config) -> Result<(), AppError> {
        config::save(self.config_path()?, config)
    }

    fn store_workspace_default_id_formats(&self, id_formats: IdFormats) -> Result<(), AppError> {
        config::set_workspace_default_id_formats(self.config_path()?, id_formats)
    }

    fn store_workspace_default_surety(&self, surety: SuretyLabelOverrides) -> Result<(), AppError> {
        config::set_workspace_default_surety(self.config_path()?, surety)
    }

    fn store_default_workspace(&self, name: &str) -> Result<(), AppError> {
        config::set_default_workspace(self.config_path()?, name)
    }

    fn load_workspace_functionality(&self) -> Result<WorkspaceFunctionality, AppError> {
        let manifest = workspace::read_manifest(self.workspace_dir()?)?;
        Ok(WorkspaceFunctionality {
            database_url: manifest.database_url,
            id_formats: manifest.id_formats,
            operators: manifest.operators,
            plugins: manifest.plugins,
        })
    }

    fn store_workspace_functionality(&self, functionality: &WorkspaceFunctionality) -> Result<(), AppError> {
        let dir = self.workspace_dir()?;
        let mut manifest = workspace::read_manifest(dir)?;
        manifest.database_url.clone_from(&functionality.database_url);
        manifest.id_formats.clone_from(&functionality.id_formats);
        manifest.operators.clone_from(&functionality.operators);
        manifest.plugins.clone_from(&functionality.plugins);
        workspace::write_manifest(dir, &manifest)
    }

    fn store_plugin_enabled(&self, id: &str, enabled: bool) -> Result<(), AppError> {
        workspace::save_plugin_enabled(self.workspace_dir()?, id, enabled)
    }

    fn store_workspace_default_locale(&self, locale: LocaleDefaults) -> Result<(), AppError> {
        config::set_workspace_default_locale(self.config_path()?, locale)
    }

    fn load_presentation(&self) -> Result<Presentation, AppError> {
        let manifest = workspace::read_manifest(self.workspace_dir()?)?;
        Ok(Presentation {
            theme: manifest.ui.theme,
            window: manifest.ui.window,
            recent: manifest.ui.recent,
            locale: manifest.locale,
        })
    }

    fn store_theme(&self, theme: ThemeMode) -> Result<(), AppError> {
        workspace::save_theme_mode(self.workspace_dir()?, theme)
    }

    fn store_locale(&self, locale: LocaleOverrides) -> Result<(), AppError> {
        workspace::save_locale_overrides(self.workspace_dir()?, locale)
    }

    fn store_window(&self, geometry: WindowGeometry) -> Result<(), AppError> {
        workspace::save_window_geometry(self.workspace_dir()?, geometry)
    }

    fn store_recent(&self, recent: &[RecentItem]) -> Result<(), AppError> {
        workspace::save_recent(self.workspace_dir()?, recent)
    }

    fn load_ai_config(&self) -> Result<AiConfig, AppError> {
        Ok(config::load(self.config_path()?)?.ai)
    }

    fn store_ai_config(&self, ai: &AiConfig) -> Result<(), AppError> {
        config::set_ai(self.config_path()?, ai.clone())
    }

    fn load_map_config(&self) -> Result<MapConfig, AppError> {
        Ok(config::load(self.config_path()?)?.map)
    }

    fn store_map_config(&self, map: &MapConfig) -> Result<(), AppError> {
        config::set_map(self.config_path()?, map.clone())
    }

    fn load_plugin_trust(&self) -> Result<PluginTrustConfig, AppError> {
        Ok(config::load(self.config_path()?)?.plugin_trust)
    }

    fn store_plugin_trust(&self, trust: &PluginTrustConfig) -> Result<(), AppError> {
        config::set_plugin_trust(self.config_path()?, trust.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_requested_languages;
    use unic_langid::LanguageIdentifier;

    fn lang(tag: &str) -> LanguageIdentifier {
        tag.parse().expect("valid language tag")
    }

    #[test]
    fn plain_env_used_when_no_config_or_prefix() {
        let resolved = resolve_requested_languages(None, &[lang("en")], None);
        assert_eq!(resolved, vec![lang("en")]);
    }

    #[test]
    fn config_overrides_plain_env() {
        // The bug: configured ui_language must win over a bare LANGUAGE in the environment.
        let resolved = resolve_requested_languages(Some(&lang("no")), &[lang("en")], None);
        assert_eq!(resolved, vec![lang("no")]);
    }

    #[test]
    fn prefixed_env_overrides_config_and_plain() {
        let resolved = resolve_requested_languages(Some(&lang("no")), &[lang("en")], Some(&lang("de")));
        assert_eq!(resolved, vec![lang("de")]);
    }

    #[test]
    fn prefixed_env_overrides_plain_when_no_config() {
        let resolved = resolve_requested_languages(None, &[lang("en")], Some(&lang("de")));
        assert_eq!(resolved, vec![lang("de")]);
    }

    #[test]
    fn empty_everything_yields_empty() {
        let resolved = resolve_requested_languages(None, &[], None);
        assert!(resolved.is_empty());
    }

    use super::{ConfigStore, FileConfigStore};
    use crate::config::{AppDefaults, OperatorConfig, ThemeMode};
    use crate::workspace::{LocaleOverrides, RecentItem, WindowGeometry, Workspace};
    use genealogy_core::ids::AgentId;
    use uuid::Uuid;

    fn operator(id: u128) -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(id)),
            display: Some("Ada".to_owned()),
            email: None,
        }
    }

    #[test]
    fn file_store_round_trips_operator_scope() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileConfigStore::new(dir.path().join("config.toml"), None);
        store.load_or_bootstrap_config().expect("bootstrap");

        store
            .store_operator_identity(Some("Ada Lovelace".to_owned()), Some("ada@example.com".to_owned()))
            .expect("store operator");

        let operator = store.load_operator().expect("load operator");
        assert_eq!(operator.display.as_deref(), Some("Ada Lovelace"));
        assert_eq!(operator.email.as_deref(), Some("ada@example.com"));
    }

    #[test]
    fn file_store_round_trips_workspace_functionality_scope() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(1), &AppDefaults::default(), None).expect("init");
        let store = FileConfigStore::for_workspace(ws.clone());

        let mut functionality = store.load_workspace_functionality().expect("load");
        functionality.id_formats.person = Some("Z%02d".to_owned());
        functionality.plugins.disabled.insert("gedcom-import".to_owned());
        store.store_workspace_functionality(&functionality).expect("store");

        let reloaded = store.load_workspace_functionality().expect("reload");
        assert_eq!(reloaded.id_formats.person.as_deref(), Some("Z%02d"));
        assert!(!reloaded.plugins.is_enabled("gedcom-import"));
        assert_eq!(reloaded.database_url, "sqlite://genealogy.sqlite3");
        assert!(reloaded.operators.contains_key(&Uuid::from_u128(1).to_string()));
    }

    #[test]
    fn file_store_round_trips_presentation_scope() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(2), &AppDefaults::default(), None).expect("init");
        let store = FileConfigStore::for_workspace(ws.clone());

        store.store_theme(ThemeMode::Dark).expect("theme");
        store
            .store_locale(LocaleOverrides {
                ui_language: Some("nn-NO".parse().expect("langid")),
                ..Default::default()
            })
            .expect("locale");
        let geometry = WindowGeometry {
            x: 10,
            y: 20,
            width: 800,
            height: 600,
            maximized: false,
        };
        store.store_window(geometry).expect("window");
        let recent = vec![RecentItem::Record {
            kind: "person".to_owned(),
            human_id: "I0001".to_owned(),
            label: "Ada".to_owned(),
        }];
        store.store_recent(&recent).expect("recent");

        let presentation = store.load_presentation().expect("load");
        assert_eq!(presentation.theme, Some(ThemeMode::Dark));
        assert_eq!(presentation.window, Some(geometry));
        assert_eq!(presentation.recent, recent);
        assert_eq!(presentation.locale.ui_language, Some("nn-NO".parse().expect("langid")));
    }

    #[test]
    fn file_store_round_trips_ai_config_in_the_client_scope() {
        use crate::config::{AiConfig, AiProvider};

        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileConfigStore::new(dir.path().join("config.toml"), None);
        store.load_or_bootstrap_config().expect("bootstrap");

        assert!(store.load_ai_config().expect("load empty").is_empty());

        let mut providers = std::collections::BTreeMap::new();
        providers.insert(
            "gemini".to_owned(),
            AiProvider::Command {
                command: "gemini".to_owned(),
                args: vec!["-p".to_owned(), "{prompt}".to_owned()],
                timeout_secs: 180,
            },
        );
        let ai = AiConfig {
            default: Some("gemini".to_owned()),
            providers,
        };
        store.store_ai_config(&ai).expect("store ai");

        assert_eq!(store.load_ai_config().expect("reload"), ai);
        // The operator scope is untouched by the client-scope write.
        assert!(store.load_operator().is_ok());
    }

    #[test]
    fn file_store_round_trips_map_config_in_the_client_scope() {
        use crate::config::MapProvider;

        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileConfigStore::new(dir.path().join("config.toml"), None);
        store.load_or_bootstrap_config().expect("bootstrap");

        // No `[map]` section yet: an empty config resolves to the built-in OSM default.
        let empty = store.load_map_config().expect("load empty");
        assert!(empty.provider.is_none());
        assert_eq!(empty.resolved_provider(), MapProvider::default_osm());

        let map = crate::config::MapConfig {
            provider: Some(MapProvider::MaplibreStyle {
                style_url: "https://example.test/style.json".to_owned(),
                attribution: "© Example".to_owned(),
                api_key_env: Some("EXAMPLE_MAP_KEY".to_owned()),
            }),
            net_allowlist: vec!["example.test".to_owned()],
        };
        store.store_map_config(&map).expect("store map");

        assert_eq!(store.load_map_config().expect("reload"), map);
        // The operator scope is untouched by the client-scope write.
        assert!(store.load_operator().is_ok());
    }

    #[test]
    fn file_store_round_trips_plugin_trust_in_the_client_scope() {
        use crate::config::PluginTrustConfig;

        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileConfigStore::new(dir.path().join("config.toml"), None);
        store.load_or_bootstrap_config().expect("bootstrap");

        assert!(store.load_plugin_trust().expect("load empty").is_empty());

        let mut publishers = std::collections::BTreeMap::new();
        publishers.insert(
            "acme-genealogy".to_owned(),
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff".to_owned(),
        );
        let trust = PluginTrustConfig { publishers };
        store.store_plugin_trust(&trust).expect("store trust");

        assert_eq!(store.load_plugin_trust().expect("reload"), trust);
        // The operator scope is untouched by the client-scope write.
        assert!(store.load_operator().is_ok());
    }

    #[test]
    fn new_layout_parses() {
        // Pins the on-disk shape the store reads: the two ADR 0005 files, retained (ADR 0015 §5).
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"
default = "gen"

[workspaces.gen]
path = "/home/magne/gen"

[operator]
id = "019ed99c-6bde-73c2-a71a-05934c744a49"
display = "Magne Rasmussen"

[defaults]
engine = "sqlite"

[workspace-defaults.id_formats]
person = "I%04d"
"#,
        )
        .expect("write config");
        let ws = dir.path().join("gen");
        std::fs::create_dir_all(&ws).expect("dir");
        std::fs::write(
            ws.join("workspace.toml"),
            r#"
database_url = "sqlite://genealogy.sqlite3"

[id_formats]
person = "Z%02d"

[ui]
theme = "dark"

[locale]
ui_language = "no"

[plugins]
disabled = ["gedcom-import"]
"#,
        )
        .expect("write manifest");

        let store = FileConfigStore::new(config_path, Some(ws));
        let operator = store.load_operator().expect("operator");
        assert_eq!(operator.display.as_deref(), Some("Magne Rasmussen"));

        let functionality = store.load_workspace_functionality().expect("functionality");
        assert_eq!(functionality.id_formats.person.as_deref(), Some("Z%02d"));
        assert!(!functionality.plugins.is_enabled("gedcom-import"));

        let presentation = store.load_presentation().expect("presentation");
        assert_eq!(presentation.theme, Some(ThemeMode::Dark));
        assert_eq!(presentation.locale.ui_language, Some("no".parse().expect("langid")));
    }
}
