//! Workspace-registry use-cases (ADR 0005): listing the registered workspaces and registering a new
//! one, for the Preferences "Workspaces" card. These mirror the CLI's `init` flow so a frontend
//! never re-implements workspace lifecycle; `vitni-cli init` delegates here too.

use std::path::{Path, PathBuf};

use crate::config::{Config, Engine, default_workspace_dir};
use crate::config_store::{ConfigStore, FileConfigStore};
use crate::error::AppError;
use crate::workspace::{Workspace, manifest_engine};

/// A registered workspace, summarized for the Preferences card: its name, directory, whether it is
/// the configured default, and its database engine (best-effort — `None` when the manifest is
/// missing or unreadable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSummary {
    /// The registry name.
    pub name: String,
    /// The workspace directory.
    pub path: PathBuf,
    /// Whether this is the configured default workspace.
    pub is_default: bool,
    /// The database engine read from the manifest, or `None` when it could not be determined.
    pub engine: Option<Engine>,
}

/// Summarizes the registered workspaces in stable name order (the config's `BTreeMap` iteration),
/// flagging the default and reading each engine best-effort. A missing or corrupt manifest yields
/// `engine: None` for that row rather than dropping it or failing the list.
#[must_use]
pub fn list_workspaces(config: &Config) -> Vec<WorkspaceSummary> {
    let mut summaries = Vec::with_capacity(config.workspaces.len());
    for (name, entry) in &config.workspaces {
        summaries.push(WorkspaceSummary {
            name: name.clone(),
            path: entry.path.clone(),
            is_default: config.default.as_deref() == Some(name.as_str()),
            engine: manifest_engine(&entry.path),
        });
    }
    summaries
}

/// Registers a new workspace and makes it the default (mirrors `vitni init`): bootstraps the
/// config, validates the name, creates the workspace directory + database, and persists the config.
///
/// `dir` defaults to [`default_workspace_dir`] when `None`; `database_url` overrides the engine
/// default (frozen into the manifest at creation).
///
/// If the final open fails (e.g. an unreachable Postgres server), the registration is rolled back —
/// the config entry is removed and re-persisted, and the workspace directory is deleted, but only if
/// this call created it — before the original error is returned. A rollback failure is logged rather
/// than propagated, since the caller already has the real error to report.
///
/// # Errors
///
/// [`AppError::Config`] if the name is empty/whitespace or already registered, or the config cannot
/// be read/written; [`AppError::Workspace`]/[`AppError::Db`] if the workspace cannot be created.
pub async fn register_workspace(
    config_path: &Path,
    name: &str,
    dir: Option<&Path>,
    database_url: Option<&str>,
) -> Result<WorkspaceSummary, AppError> {
    let store = FileConfigStore::new(config_path.to_path_buf(), None);
    let mut config = store.load_or_bootstrap_config()?;
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Config("workspace name must not be empty".to_owned()));
    }
    if config.workspaces.contains_key(name) {
        return Err(AppError::Config(format!("workspace {name:?} is already registered")));
    }
    let previous_default = config.default.clone();
    let dir = match dir {
        Some(dir) => dir.to_path_buf(),
        None => default_workspace_dir(name)?,
    };
    let dir_preexisted = dir.exists();
    Workspace::init(&dir, &config.operator, &config.defaults, database_url)?;
    config.register_workspace(name.to_owned(), dir.clone());
    store.store_config(&config)?;
    // Open once to create the database file and record the operator in the manifest.
    if let Err(error) = Workspace::open(&dir, &config.operator, &config.workspace_defaults).await {
        config.workspaces.remove(name);
        config.default = previous_default;
        if let Err(rollback_error) = store.store_config(&config) {
            tracing::warn!(
                %rollback_error,
                workspace = name,
                "could not roll back the config after a failed workspace open"
            );
        }
        if !dir_preexisted && let Err(remove_error) = std::fs::remove_dir_all(&dir) {
            tracing::warn!(
                %remove_error,
                path = %dir.display(),
                "could not remove the workspace directory after a failed open"
            );
        }
        return Err(error);
    }
    let engine = manifest_engine(&dir);
    Ok(WorkspaceSummary {
        name: name.to_owned(),
        path: dir,
        is_default: true,
        engine,
    })
}

#[cfg(test)]
mod tests {
    use super::{list_workspaces, register_workspace};
    use crate::config::{self, Engine};
    use crate::workspace::{Workspace, engine_of_url, manifest_engine};

    fn config_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("config.toml")
    }

    #[tokio::test]
    async fn list_orders_by_name_flags_default_and_reads_sqlite_engine() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = config_path(&home);
        register_workspace(&path, "beta", Some(&home.path().join("b")), None)
            .await
            .expect("register beta");
        register_workspace(&path, "alpha", Some(&home.path().join("a")), None)
            .await
            .expect("register alpha");

        let config = config::load(&path).expect("load");
        let list = list_workspaces(&config);
        let names: Vec<&str> = list.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["alpha", "beta"], "stable alphabetical order");
        // The most recently registered (alpha) is the default.
        assert!(list[0].is_default, "alpha is the default");
        assert!(!list[1].is_default, "beta is not the default");
        assert_eq!(
            list[0].engine,
            Some(Engine::Sqlite),
            "sqlite engine read from the manifest"
        );
    }

    #[tokio::test]
    async fn list_survives_a_missing_manifest() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = config_path(&home);
        let dir = home.path().join("ws");
        register_workspace(&path, "ws", Some(&dir), None)
            .await
            .expect("register");
        std::fs::remove_file(dir.join("workspace.toml")).expect("remove manifest");

        let config = config::load(&path).expect("load");
        let list = list_workspaces(&config);
        assert_eq!(list.len(), 1, "the row is still listed");
        assert_eq!(list[0].engine, None, "no engine without a manifest");
    }

    #[tokio::test]
    async fn list_derives_postgres_from_the_manifest_url() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = config_path(&home);
        let dir = home.path().join("pg");
        // Init (never open) a workspace with a postgres url, then register it in the config by hand —
        // opening a postgres store is out of scope for the list.
        let mut config = config::load_or_bootstrap(&path).expect("bootstrap");
        Workspace::init(
            &dir,
            &config.operator,
            &config.defaults,
            Some("postgres://localhost/gen"),
        )
        .expect("init postgres manifest");
        config.register_workspace("pg".to_owned(), dir.clone());

        let list = list_workspaces(&config);
        let pg = list.iter().find(|w| w.name == "pg").expect("pg row");
        assert_eq!(
            pg.engine,
            Some(Engine::Postgres),
            "postgres derived from the manifest url"
        );
    }

    #[tokio::test]
    async fn register_creates_registers_and_defaults() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = config_path(&home);
        let dir = home.path().join("ws");
        let summary = register_workspace(&path, "ws", Some(&dir), None)
            .await
            .expect("register");
        assert!(summary.is_default);

        let config = config::load(&path).expect("reload");
        assert!(config.workspaces.contains_key("ws"), "registered on disk");
        assert_eq!(config.default.as_deref(), Some("ws"), "made the default");
        assert!(dir.join("workspace.toml").is_file(), "manifest written");
        assert!(dir.join("exports").is_dir(), "subdirs created");
        assert!(dir.join("vitni.sqlite3").is_file(), "database file created");
    }

    #[tokio::test]
    async fn register_rejects_a_duplicate_name() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = config_path(&home);
        register_workspace(&path, "ws", Some(&home.path().join("a")), None)
            .await
            .expect("first register");
        let before = std::fs::read_to_string(&path).expect("read config");

        let result = register_workspace(&path, "ws", Some(&home.path().join("b")), None).await;
        assert!(result.is_err(), "duplicate name rejected");
        let after = std::fs::read_to_string(&path).expect("read config");
        assert_eq!(before, after, "disk config unchanged after a rejected duplicate");
    }

    #[tokio::test]
    async fn register_freezes_a_postgres_url_into_the_manifest() {
        // Mirrors `list_derives_postgres_from_the_manifest_url`: exercises `Workspace::init` — the
        // call `register_workspace` makes before it opens the store — directly, since actually
        // opening a postgres store needs a running server, which is out of scope for a manifest check.
        let home = tempfile::tempdir().expect("tempdir");
        let dir = home.path().join("pg");
        let config = config::load_or_bootstrap(&config_path(&home)).expect("bootstrap");
        Workspace::init(
            &dir,
            &config.operator,
            &config.defaults,
            Some("postgres://localhost/gen"),
        )
        .expect("init with a postgres url");

        let manifest = std::fs::read_to_string(dir.join("workspace.toml")).expect("read manifest");
        assert!(
            manifest.contains("postgres://localhost/gen"),
            "the manifest carries the given url:\n{manifest}"
        );
        assert_eq!(
            manifest_engine(&dir),
            Some(Engine::Postgres),
            "postgres engine derived from the frozen url"
        );
    }

    #[tokio::test]
    async fn register_rejects_an_unrecognized_database_url_scheme() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = config_path(&home);
        let dir = home.path().join("ws");
        let result = register_workspace(&path, "ws", Some(&dir), Some("mysql://x")).await;
        assert!(
            matches!(result, Err(crate::error::AppError::Config(_))),
            "an unrecognized scheme is a config error: {result:?}"
        );
        let config = config::load_or_bootstrap(&path).expect("bootstrap");
        assert!(!config.workspaces.contains_key("ws"), "nothing was registered");
    }

    #[tokio::test]
    async fn register_rolls_back_a_config_entry_when_open_fails() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = config_path(&home);
        let dir = home.path().join("pg");
        // Port 1 has no listener, so the connection is refused immediately — a fast, deterministic
        // way to exercise the open-failure rollback without a running server (mirrors
        // `workspace.rs`'s `opening_an_unreachable_postgres_workspace_surfaces_a_db_error`).
        let result = register_workspace(&path, "pg", Some(&dir), Some("postgres://localhost:1/x")).await;
        assert!(
            matches!(result, Err(crate::error::AppError::Db(_))),
            "the unreachable server surfaces as a db error: {result:?}"
        );
        let config = config::load_or_bootstrap(&path).expect("bootstrap");
        assert!(
            !config.workspaces.contains_key("pg"),
            "the failed registration left no config entry"
        );
        assert!(!dir.exists(), "the directory this call created was removed");
    }

    #[tokio::test]
    async fn register_rollback_never_deletes_a_preexisting_directory() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = config_path(&home);
        let dir = home.path().join("pg");
        std::fs::create_dir_all(&dir).expect("pre-create the directory");
        std::fs::write(dir.join("keepsake.txt"), b"already here").expect("seed a file");

        let result = register_workspace(&path, "pg", Some(&dir), Some("postgres://localhost:1/x")).await;
        assert!(result.is_err(), "the open still fails");
        assert!(
            dir.join("keepsake.txt").is_file(),
            "a pre-existing directory is left alone"
        );
    }

    #[tokio::test]
    async fn register_rejects_an_empty_or_whitespace_name() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = config_path(&home);
        assert!(
            register_workspace(&path, "", Some(&home.path().join("a")), None)
                .await
                .is_err(),
            "empty name rejected"
        );
        assert!(
            register_workspace(&path, "   ", Some(&home.path().join("b")), None)
                .await
                .is_err(),
            "whitespace name rejected"
        );
    }

    #[test]
    fn engine_of_url_maps_schemes() {
        assert_eq!(engine_of_url("sqlite://vitni.sqlite3"), Some(Engine::Sqlite));
        assert_eq!(engine_of_url("postgres://localhost/gen"), Some(Engine::Postgres));
        assert_eq!(engine_of_url("postgresql://localhost/gen"), Some(Engine::Postgres));
        assert_eq!(engine_of_url("mysql://localhost/gen"), None);
    }
}
