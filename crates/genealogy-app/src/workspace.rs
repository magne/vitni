//! Workspace = a directory (ADR 0005): a manifest, the database, and `exports/ backups/ media/`.
//!
//! The manifest (`<dir>/workspace.toml`) records the `database_url` (a SQLite file ref — relative
//! resolved against the directory — or a Postgres URL), the per-aggregate `HumanId` formats, and
//! the operators known to this workspace (so the operator id is never loose — ADR 0005). [`Workspace`]
//! opens the engine-neutral [`Store`] and exposes it to the use-cases; the engine stays in
//! `genealogy-db`.

use std::collections::BTreeMap;
use std::path::Path;

use genealogy_core::id_format::IdFormat;
use genealogy_db::Store;
use serde::{Deserialize, Serialize};

use crate::config::OperatorConfig;
use crate::error::AppError;

/// The workspace manifest file name.
const MANIFEST_FILE: &str = "workspace.toml";

/// The subdirectories created inside a workspace.
const SUBDIRS: [&str; 3] = ["exports", "backups", "media"];

/// The default SQLite database url, relative to the workspace directory.
const DEFAULT_DATABASE_URL: &str = "sqlite://genealogy.sqlite3";

/// An operator known to a workspace (ADR 0005); keyed in the manifest by the operator id string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorRecord {
    /// The operator's display name at the time they first used this workspace.
    pub display: Option<String>,
    /// The operator's email — the portable cross-machine identity.
    pub email: Option<String>,
}

/// Per-aggregate `HumanId` formats (Gramps-style printf — data-model §7). Only Person is used yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdFormats {
    /// The Person id format, default `I%04d`.
    #[serde(default = "default_person_format")]
    pub person: String,
}

/// The default Person `HumanId` format.
fn default_person_format() -> String {
    "I%04d".to_owned()
}

impl Default for IdFormats {
    fn default() -> Self {
        Self {
            person: default_person_format(),
        }
    }
}

/// The on-disk workspace manifest (`workspace.toml`, ADR 0005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    /// The database backing this workspace (SQLite file ref or Postgres URL).
    pub database_url: String,
    /// Per-aggregate `HumanId` formats.
    #[serde(default)]
    pub id_formats: IdFormats,
    /// Operators who have used this workspace, keyed by operator id.
    #[serde(default)]
    pub operators: BTreeMap<String, OperatorRecord>,
}

/// An open workspace: the engine-neutral store plus the manifest it was opened from.
pub struct Workspace {
    store: Store,
    manifest: WorkspaceManifest,
}

impl Workspace {
    /// Creates and initializes a workspace directory: subdirectories + a default manifest (ADR 0005).
    ///
    /// Idempotent on the directory tree; refuses to overwrite an existing manifest. Records the
    /// bootstrapping `operator`.
    ///
    /// # Errors
    ///
    /// [`AppError::Workspace`] if the directory tree or manifest cannot be written.
    pub fn init(dir: &Path, operator: &OperatorConfig) -> Result<WorkspaceManifest, AppError> {
        let manifest_path = dir.join(MANIFEST_FILE);
        if manifest_path.exists() {
            return Err(AppError::Workspace(format!(
                "{} already exists; workspace is already initialized",
                manifest_path.display()
            )));
        }
        create_dir(dir)?;
        for sub in SUBDIRS {
            create_dir(&dir.join(sub))?;
        }
        let mut operators = BTreeMap::new();
        operators.insert(operator.id.to_string(), record_of(operator));
        let manifest = WorkspaceManifest {
            database_url: DEFAULT_DATABASE_URL.to_owned(),
            id_formats: IdFormats::default(),
            operators,
        };
        write_manifest(dir, &manifest)?;
        Ok(manifest)
    }

    /// Opens an existing workspace directory, recording `operator` in the manifest if new (ADR 0005).
    ///
    /// # Errors
    ///
    /// [`AppError::Workspace`] if the manifest is missing/invalid, or [`AppError::Db`] if the store
    /// cannot be opened.
    pub async fn open(dir: &Path, operator: &OperatorConfig) -> Result<Self, AppError> {
        let mut manifest = read_manifest(dir)?;
        let store = Store::open(&resolve_database_url(dir, &manifest.database_url)).await?;

        let mut newly_recorded = false;
        manifest.operators.entry(operator.id.to_string()).or_insert_with(|| {
            newly_recorded = true;
            record_of(operator)
        });
        if newly_recorded {
            write_manifest(dir, &manifest)?;
        }
        Ok(Self { store, manifest })
    }

    /// The engine-neutral event store.
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The parsed Person `HumanId` format from the manifest.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the configured format string is malformed.
    pub fn person_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.manifest.id_formats.person).map_err(|e| AppError::Config(e.to_string()))
    }
}

/// Builds an [`OperatorRecord`] from the configured operator.
fn record_of(operator: &OperatorConfig) -> OperatorRecord {
    OperatorRecord {
        display: operator.display.clone(),
        email: operator.email.clone(),
    }
}

/// Resolves a manifest `database_url`: a relative SQLite path is taken against the workspace dir.
fn resolve_database_url(dir: &Path, database_url: &str) -> String {
    let Some(rest) = database_url.strip_prefix("sqlite://") else {
        return database_url.to_owned();
    };
    if Path::new(rest).is_absolute() {
        return database_url.to_owned();
    }
    format!("sqlite://{}", dir.join(rest).display())
}

/// Creates a directory (and parents), mapping I/O failure to [`AppError::Workspace`].
fn create_dir(dir: &Path) -> Result<(), AppError> {
    std::fs::create_dir_all(dir).map_err(|e| AppError::Workspace(format!("creating {}: {e}", dir.display())))
}

/// Reads and parses `<dir>/workspace.toml`.
fn read_manifest(dir: &Path) -> Result<WorkspaceManifest, AppError> {
    let path = dir.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| AppError::Workspace(format!("reading {} (run `genealogy init`?): {e}", path.display())))?;
    toml::from_str(&text).map_err(|e| AppError::Workspace(format!("parsing {}: {e}", path.display())))
}

/// Writes `<dir>/workspace.toml`.
fn write_manifest(dir: &Path, manifest: &WorkspaceManifest) -> Result<(), AppError> {
    let path = dir.join(MANIFEST_FILE);
    let text =
        toml::to_string_pretty(manifest).map_err(|e| AppError::Workspace(format!("serializing manifest: {e}")))?;
    std::fs::write(&path, text).map_err(|e| AppError::Workspace(format!("writing {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::{Workspace, read_manifest, resolve_database_url};
    use crate::config::OperatorConfig;
    use genealogy_core::ids::AgentId;
    use std::path::Path;
    use uuid::Uuid;

    fn operator() -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
            email: Some("ada@example.com".to_owned()),
        }
    }

    #[test]
    fn init_creates_the_tree_and_a_default_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator()).expect("init");

        assert!(ws.join("workspace.toml").is_file());
        assert!(ws.join("exports").is_dir());
        assert!(ws.join("backups").is_dir());
        assert!(ws.join("media").is_dir());

        let manifest = read_manifest(&ws).expect("manifest");
        assert_eq!(manifest.database_url, "sqlite://genealogy.sqlite3");
        assert_eq!(manifest.id_formats.person, "I%04d");
        assert!(manifest.operators.contains_key(&Uuid::from_u128(1).to_string()));
    }

    #[test]
    fn relative_sqlite_url_resolves_against_the_dir() {
        let resolved = resolve_database_url(Path::new("/data/ws"), "sqlite://genealogy.sqlite3");
        assert_eq!(resolved, "sqlite:///data/ws/genealogy.sqlite3");
    }

    #[test]
    fn absolute_and_postgres_urls_pass_through() {
        assert_eq!(
            resolve_database_url(Path::new("/data/ws"), "sqlite:///abs/db.sqlite3"),
            "sqlite:///abs/db.sqlite3"
        );
        assert_eq!(
            resolve_database_url(Path::new("/data/ws"), "postgres://localhost/db"),
            "postgres://localhost/db"
        );
    }
}
