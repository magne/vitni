//! Global application configuration: the operator, the named-workspace registry, and the defaults
//! applied to new workspaces (ADR 0005).
//!
//! The global config (`~/.config/genealogy/config.toml`, resolved via the `directories` crate)
//! names workspaces (`[workspaces.<name>]`), records the default (last-used) one (`default`), the
//! operator (`[operator]`), and the `[defaults]` template seeded into each new workspace manifest.
//! A *workspace* is a directory with its own manifest (see [`crate::workspace`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

/// The application name for `directories` path resolution.
const APP_NAME: &str = "genealogy";

/// The default Person `HumanId` format (Gramps `gramps_id` analog — data-model §7).
fn default_person_format() -> String {
    "I%04d".to_owned()
}

/// Per-aggregate `HumanId` formats (Gramps-style printf). Only Person is used yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdFormats {
    /// The Person id format, default `I%04d`.
    #[serde(default = "default_person_format")]
    pub person: String,
}

impl Default for IdFormats {
    fn default() -> Self {
        Self {
            person: default_person_format(),
        }
    }
}

/// The database engine a new workspace is created with (ADR 0002).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// Embedded SQLite (the zero-setup default).
    #[default]
    Sqlite,
    /// Server Postgres (reserved — ADR 0002; not yet supported by `init`).
    Postgres,
}

/// Application-level defaults: settings about app behavior / how new things are created.
///
/// Consumed at the relevant action (e.g. `engine` is read once at `init` and frozen into the new
/// workspace's `database_url`); these are *not* live fallbacks. Contrast [`WorkspaceDefaults`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppDefaults {
    /// The engine a new workspace is created with.
    #[serde(default)]
    pub engine: Engine,
}

/// Defaults for *per-workspace configuration* — every field is a **live fallback** (ADR 0005).
///
/// A workspace manifest may override any of these; an unset field resolves from here each time the
/// workspace is opened, so editing a global default takes effect for every workspace that hasn't
/// pinned its own. Future per-workspace settings (privacy, locale, …) join this struct.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDefaults {
    /// The `HumanId` formats workspaces fall back to.
    #[serde(default)]
    pub id_formats: IdFormats,
}

/// The default operator stamped onto every assertion (ADR 0004 §1, ADR 0005).
///
/// `email` is the **portable identity**: it lets the same person be recognized across machines
/// even though `id` is generated locally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorConfig {
    /// The operator's stable id, generated once at bootstrap.
    pub id: AgentId,
    /// An optional display name (defaults to the OS user at bootstrap).
    pub display: Option<String>,
    /// An optional email — the portable cross-machine identity.
    pub email: Option<String>,
}

/// A registered workspace: a name mapped to its directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    /// The workspace directory.
    pub path: PathBuf,
}

/// The global configuration (ADR 0005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The workspace opened when none is named on the command line (the last used one).
    #[serde(default)]
    pub default: Option<String>,
    /// Known workspaces, keyed by name.
    #[serde(default)]
    pub workspaces: BTreeMap<String, WorkspaceEntry>,
    /// The default operator identity.
    pub operator: OperatorConfig,
    /// Application-level defaults (engine, …).
    #[serde(default)]
    pub defaults: AppDefaults,
    /// Live-fallback defaults for per-workspace configuration (id formats, …).
    #[serde(default, rename = "workspace-defaults")]
    pub workspace_defaults: WorkspaceDefaults,
}

impl Config {
    /// Builds the operator [`Agent`] stamped onto assertions for this run.
    #[must_use]
    pub fn operator_agent(&self) -> Agent {
        Agent {
            kind: AgentKind::Human,
            id: self.operator.id,
            display: self.operator.display.clone(),
        }
    }

    /// Registers workspace `name` at `path` and makes it the default.
    pub fn register_workspace(&mut self, name: String, path: PathBuf) {
        self.workspaces.insert(name.clone(), WorkspaceEntry { path });
        self.default = Some(name);
    }

    /// Resolves the workspace directory to open: `name` if given, else the configured default.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if no workspace is selected (and no default), or the name is unknown.
    pub fn resolve_workspace(&self, name: Option<&str>) -> Result<PathBuf, AppError> {
        let name = name
            .map(str::to_owned)
            .or_else(|| self.default.clone())
            .ok_or_else(|| {
                AppError::Config(
                    "no workspace given and no default set (run `genealogy init <name> <path>`)".to_owned(),
                )
            })?;
        let entry = self
            .workspaces
            .get(&name)
            .ok_or_else(|| AppError::Config(format!("unknown workspace {name:?} (not in the registry)")))?;
        Ok(entry.path.clone())
    }
}

/// Returns the platform project directories for the application.
fn project_dirs() -> Result<ProjectDirs, AppError> {
    ProjectDirs::from("", "", APP_NAME)
        .ok_or_else(|| AppError::Config("no valid home directory for config/data paths".to_owned()))
}

/// The global config file path, e.g. `~/.config/genealogy/config.toml` (ADR 0005).
///
/// # Errors
///
/// [`AppError::Config`] if no home directory can be determined.
pub fn config_path() -> Result<PathBuf, AppError> {
    Ok(project_dirs()?.config_dir().join("config.toml"))
}

/// The default directory for a workspace named `name`, e.g.
/// `~/.local/share/genealogy/workspaces/<name>` (ADR 0005).
///
/// # Errors
///
/// [`AppError::Config`] if no home directory can be determined.
pub fn default_workspace_dir(name: &str) -> Result<PathBuf, AppError> {
    Ok(project_dirs()?.data_dir().join("workspaces").join(name))
}

/// The shared application directory holding runtime localization overrides, e.g.
/// `~/.local/share/genealogy/i18n` (ADR 0003 — the shared-app-dir override layer).
///
/// # Errors
///
/// [`AppError::Config`] if no home directory can be determined.
pub fn shared_i18n_dir() -> Result<PathBuf, AppError> {
    Ok(project_dirs()?.data_dir().join("i18n"))
}

/// Best-effort display name for the OS user, used only as the bootstrap default.
fn os_display_name() -> Option<String> {
    whoami::realname().ok().or_else(|| whoami::username().ok())
}

/// Loads the global config from `path`.
///
/// # Errors
///
/// [`AppError::Config`] if the file is missing, unreadable, or not valid TOML.
pub fn load(path: &Path) -> Result<Config, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("reading {} (run `genealogy init`?): {e}", path.display())))?;
    toml::from_str(&text).map_err(|e| AppError::Config(format!("parsing {}: {e}", path.display())))
}

/// Loads the global config, bootstrapping a default one (with a fresh operator) if absent.
///
/// The generated [`AgentId`] is persistent: an existing file is loaded untouched, so the operator
/// identity is stable across runs (ADR 0005).
///
/// # Errors
///
/// [`AppError::Config`] if paths cannot be resolved or the file cannot be read/written.
pub fn load_or_bootstrap(path: &Path) -> Result<Config, AppError> {
    if path.exists() {
        return load(path);
    }
    let config = Config {
        default: None,
        workspaces: BTreeMap::new(),
        operator: OperatorConfig {
            id: AgentId::from_uuid(Uuid::now_v7()),
            display: os_display_name(),
            email: None,
        },
        defaults: AppDefaults::default(),
        workspace_defaults: WorkspaceDefaults::default(),
    };
    save(path, &config)?;
    Ok(config)
}

/// Writes the global config to `path` as TOML, creating parent directories as needed.
///
/// # Errors
///
/// [`AppError::Config`] if the directory or file cannot be written.
pub fn save(path: &Path, config: &Config) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Config(format!("creating {}: {e}", parent.display())))?;
    }
    let text = toml::to_string_pretty(config).map_err(|e| AppError::Config(format!("serializing config: {e}")))?;
    std::fs::write(path, text).map_err(|e| AppError::Config(format!("writing {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::{Config, Engine, load, load_or_bootstrap, save};
    use std::path::{Path, PathBuf};

    fn config_at(path: &Path) -> Config {
        load_or_bootstrap(path).expect("bootstrap")
    }

    #[test]
    fn bootstrap_then_reload_keeps_a_stable_operator_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let first = config_at(&path);
        let second = config_at(&path);
        assert_eq!(first.operator.id, second.operator.id, "operator id must persist");
    }

    #[test]
    fn register_then_resolve_by_name_and_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut config = config_at(&dir.path().join("config.toml"));
        config.register_workspace("gen".to_owned(), PathBuf::from("/data/gen"));
        config.register_workspace("tree2".to_owned(), PathBuf::from("/data/tree2"));

        assert_eq!(
            config.resolve_workspace(Some("gen")).expect("by name"),
            PathBuf::from("/data/gen")
        );
        // The most recently registered workspace is the default.
        assert_eq!(
            config.resolve_workspace(None).expect("default"),
            PathBuf::from("/data/tree2")
        );
    }

    #[test]
    fn resolve_errors_on_unknown_name_and_when_no_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = config_at(&dir.path().join("config.toml"));
        assert!(config.resolve_workspace(Some("nope")).is_err(), "unknown name");
        assert!(config.resolve_workspace(None).is_err(), "no default set");
    }

    #[test]
    fn the_hand_written_named_workspace_schema_parses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        // The schema chosen in review: named workspaces, default by name, top-level operator,
        // app-level [defaults] (engine), and per-workspace [workspace-defaults] (id formats).
        let toml = r#"
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
"#;
        std::fs::write(&path, toml).expect("write");
        let config = load(&path).expect("parse");
        assert_eq!(config.default.as_deref(), Some("gen"));
        assert_eq!(
            config.resolve_workspace(None).expect("default"),
            PathBuf::from("/home/magne/gen")
        );
        assert_eq!(config.defaults.engine, Engine::Sqlite);
        assert_eq!(config.workspace_defaults.id_formats.person, "I%04d");
    }

    #[test]
    fn defaults_round_trip_through_save_and_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let mut config = config_at(&path);
        config.defaults.engine = Engine::Sqlite;
        config.workspace_defaults.id_formats.person = "P-%05d".to_owned();
        config.operator.email = Some("ada@example.com".to_owned());
        save(&path, &config).expect("save");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.workspace_defaults.id_formats.person, "P-%05d");
        assert_eq!(loaded.operator.email.as_deref(), Some("ada@example.com"));
    }
}
