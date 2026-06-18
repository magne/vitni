//! Global application configuration: the operator identity and the workspace registry (ADR 0005).
//!
//! The global config (`~/.config/genealogy/config.toml`, resolved via the `directories` crate)
//! holds the default operator `Agent` and a registry of known workspaces with the default
//! (last-used) one. A *workspace* is a directory with its own manifest (see [`crate::workspace`]);
//! this file only records where they are and who the operator is.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

/// The application name for `directories` path resolution.
const APP_NAME: &str = "genealogy";

/// The default operator stamped onto every assertion (ADR 0004 §1, ADR 0005).
///
/// `email` is the **portable identity**: it lets the same person be recognized across machines
/// even though `id` is generated locally (see ADR 0005 for the operator direction).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorConfig {
    /// The operator's stable id, generated once at bootstrap.
    pub id: AgentId,
    /// An optional display name (defaults to the OS user at bootstrap).
    pub display: Option<String>,
    /// An optional email — the portable cross-machine identity.
    pub email: Option<String>,
}

/// The global configuration: the operator and the workspace registry (ADR 0005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The workspace opened when none is given on the command line (the last used one).
    #[serde(default)]
    pub default_workspace: Option<PathBuf>,
    /// Every known workspace directory.
    #[serde(default)]
    pub workspaces: Vec<PathBuf>,
    /// The default operator identity.
    pub operator: OperatorConfig,
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

    /// Records `dir` in the registry (if absent) and makes it the default workspace.
    pub fn register_workspace(&mut self, dir: PathBuf) {
        if !self.workspaces.contains(&dir) {
            self.workspaces.push(dir.clone());
        }
        self.default_workspace = Some(dir);
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

/// The default workspace directory, e.g. `~/.local/share/genealogy/workspaces/default` (ADR 0005).
///
/// # Errors
///
/// [`AppError::Config`] if no home directory can be determined.
pub fn default_workspace_dir() -> Result<PathBuf, AppError> {
    Ok(project_dirs()?.data_dir().join("workspaces").join("default"))
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
        default_workspace: None,
        workspaces: Vec::new(),
        operator: OperatorConfig {
            id: AgentId::from_uuid(Uuid::now_v7()),
            display: os_display_name(),
            email: None,
        },
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
    use super::{load, load_or_bootstrap, save};
    use genealogy_core::ids::AgentId;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn bootstrap_then_reload_keeps_a_stable_operator_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");

        let first = load_or_bootstrap(&path).expect("bootstrap");
        let second = load_or_bootstrap(&path).expect("reload");
        assert_eq!(first.operator.id, second.operator.id, "operator id must persist");
    }

    #[test]
    fn register_workspace_records_and_defaults_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let mut config = load_or_bootstrap(&path).expect("bootstrap");

        let ws = PathBuf::from("/data/ws-a");
        config.register_workspace(ws.clone());
        config.register_workspace(ws.clone());
        assert_eq!(config.workspaces, vec![ws.clone()], "no duplicate registry entries");
        assert_eq!(config.default_workspace, Some(ws));
    }

    #[test]
    fn config_with_email_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let mut config = load_or_bootstrap(&path).expect("bootstrap");
        config.operator.id = AgentId::from_uuid(Uuid::from_u128(9));
        config.operator.email = Some("ada@example.com".to_owned());
        save(&path, &config).expect("save");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.operator.email.as_deref(), Some("ada@example.com"));
        assert_eq!(loaded.operator.id, AgentId::from_uuid(Uuid::from_u128(9)));
    }
}
