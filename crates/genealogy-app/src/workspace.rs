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

use crate::config::{AppDefaults, Engine, IdFormats, OperatorConfig, WorkspaceDefaults};
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

/// Per-workspace `HumanId` format overrides (ADR 0005).
///
/// Absent fields fall back **live** to the global `[defaults].id_formats`, re-resolved every time
/// the workspace is opened — so changing the global default takes effect for any workspace that
/// hasn't pinned its own. Setting a field here pins it for this workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdFormatOverrides {
    /// Override for the Person id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<String>,
    /// Override for the Family id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// Override for the Place id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// Override for the Source id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Override for the Citation id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citation: Option<String>,
    /// Override for the Event id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Override for the `DnaTest` id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dna_test: Option<String>,
    /// Override for the `DnaMatch` id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dna_match: Option<String>,
    /// Override for the Repository id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Override for the Note id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Override for the Media id format; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
}

/// The on-disk workspace manifest (`workspace.toml`, ADR 0005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    /// The database backing this workspace (SQLite file ref or Postgres URL), frozen at `init`.
    pub database_url: String,
    /// Per-aggregate `HumanId` format overrides; absent fields fall back to the global defaults.
    #[serde(default)]
    pub id_formats: IdFormatOverrides,
    /// Operators who have used this workspace, keyed by operator id.
    #[serde(default)]
    pub operators: BTreeMap<String, OperatorRecord>,
}

/// An open workspace: the engine-neutral store plus the effective (override-over-default) settings.
pub struct Workspace {
    store: Store,
    id_formats: IdFormats,
}

impl Workspace {
    /// Creates and initializes a workspace directory: subdirectories + a manifest, recording
    /// `operator` (ADR 0005).
    ///
    /// The `database_url` is fixed from `defaults.engine` (a workspace's database location can't
    /// change after creation). `id_formats` are **not** copied in — the manifest leaves them absent
    /// so they fall back live to the global defaults; a workspace pins one only by editing its
    /// manifest. Refuses to overwrite an existing manifest.
    ///
    /// # Errors
    ///
    /// [`AppError::Workspace`] if the tree/manifest cannot be written, or [`AppError::Config`] if the
    /// defaults select an unsupported engine.
    pub fn init(dir: &Path, operator: &OperatorConfig, defaults: &AppDefaults) -> Result<WorkspaceManifest, AppError> {
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
            database_url: database_url_for(defaults.engine)?,
            id_formats: IdFormatOverrides::default(),
            operators,
        };
        write_manifest(dir, &manifest)?;
        Ok(manifest)
    }

    /// Opens an existing workspace directory, recording `operator` if new and resolving the
    /// effective settings (manifest overrides over the global `defaults`, live — ADR 0005).
    ///
    /// # Errors
    ///
    /// [`AppError::Workspace`] if the manifest is missing/invalid, or [`AppError::Db`] if the store
    /// cannot be opened.
    pub async fn open(dir: &Path, operator: &OperatorConfig, defaults: &WorkspaceDefaults) -> Result<Self, AppError> {
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
        let id_formats = resolve_id_formats(&manifest.id_formats, defaults);
        Ok(Self { store, id_formats })
    }

    /// The engine-neutral event store.
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The parsed effective Person `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn person_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.person).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective Family `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn family_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.family).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective Place `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn place_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.place).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective Source `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn source_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.source).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective Citation `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn citation_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.citation).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective Event `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn event_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.event).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective `DnaTest` `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn dna_test_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.dna_test).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective `DnaMatch` `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn dna_match_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.dna_match).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective Repository `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn repository_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.repository).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective Note `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn note_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.note).map_err(|e| AppError::Config(e.to_string()))
    }

    /// The parsed effective Media `HumanId` format (override-over-default).
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved format string is malformed.
    pub fn media_id_format(&self) -> Result<IdFormat, AppError> {
        IdFormat::parse(&self.id_formats.media).map_err(|e| AppError::Config(e.to_string()))
    }
}

/// Resolves effective id formats: a manifest override wins, else the live global default.
fn resolve_id_formats(overrides: &IdFormatOverrides, defaults: &WorkspaceDefaults) -> IdFormats {
    IdFormats {
        person: overrides
            .person
            .clone()
            .unwrap_or_else(|| defaults.id_formats.person.clone()),
        family: overrides
            .family
            .clone()
            .unwrap_or_else(|| defaults.id_formats.family.clone()),
        place: overrides
            .place
            .clone()
            .unwrap_or_else(|| defaults.id_formats.place.clone()),
        source: overrides
            .source
            .clone()
            .unwrap_or_else(|| defaults.id_formats.source.clone()),
        citation: overrides
            .citation
            .clone()
            .unwrap_or_else(|| defaults.id_formats.citation.clone()),
        event: overrides
            .event
            .clone()
            .unwrap_or_else(|| defaults.id_formats.event.clone()),
        dna_test: overrides
            .dna_test
            .clone()
            .unwrap_or_else(|| defaults.id_formats.dna_test.clone()),
        dna_match: overrides
            .dna_match
            .clone()
            .unwrap_or_else(|| defaults.id_formats.dna_match.clone()),
        repository: overrides
            .repository
            .clone()
            .unwrap_or_else(|| defaults.id_formats.repository.clone()),
        note: overrides
            .note
            .clone()
            .unwrap_or_else(|| defaults.id_formats.note.clone()),
        media: overrides
            .media
            .clone()
            .unwrap_or_else(|| defaults.id_formats.media.clone()),
    }
}

/// The default `database_url` for a new workspace using `engine`.
fn database_url_for(engine: Engine) -> Result<String, AppError> {
    match engine {
        Engine::Sqlite => Ok(DEFAULT_DATABASE_URL.to_owned()),
        Engine::Postgres => Err(AppError::Config(
            "the postgres engine is not yet supported by `init`; set `database_url` in workspace.toml manually"
                .to_owned(),
        )),
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
    use super::{Workspace, database_url_for, read_manifest, resolve_database_url};
    use crate::config::{AppDefaults, Engine, IdFormats, OperatorConfig, WorkspaceDefaults};
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

    fn workspace_defaults_with(person: &str) -> WorkspaceDefaults {
        WorkspaceDefaults {
            id_formats: IdFormats {
                person: person.to_owned(),
                family: "F%04d".to_owned(),
                place: "P%04d".to_owned(),
                source: "S%04d".to_owned(),
                citation: "C%04d".to_owned(),
                event: "E%04d".to_owned(),
                dna_test: "D%04d".to_owned(),
                dna_match: "X%04d".to_owned(),
                repository: "R%04d".to_owned(),
                note: "N%04d".to_owned(),
                media: "O%04d".to_owned(),
            },
        }
    }

    #[test]
    fn init_creates_the_tree_and_leaves_id_formats_unset() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        // init never writes id-format overrides — formats stay a live fallback.
        Workspace::init(&ws, &operator(), &AppDefaults::default()).expect("init");

        assert!(ws.join("workspace.toml").is_file());
        assert!(ws.join("exports").is_dir());
        assert!(ws.join("backups").is_dir());
        assert!(ws.join("media").is_dir());

        let manifest = read_manifest(&ws).expect("manifest");
        assert_eq!(manifest.database_url, "sqlite://genealogy.sqlite3");
        assert_eq!(
            manifest.id_formats.person, None,
            "id formats are not seeded; they fall back live"
        );
        assert!(manifest.operators.contains_key(&Uuid::from_u128(1).to_string()));
    }

    #[test]
    fn init_refuses_to_overwrite_an_existing_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default()).expect("first init");
        let again = Workspace::init(&ws, &operator(), &AppDefaults::default());
        assert!(again.is_err(), "second init must not clobber the manifest");
    }

    #[test]
    fn postgres_engine_is_rejected_by_init() {
        assert!(database_url_for(Engine::Sqlite).is_ok());
        assert!(
            database_url_for(Engine::Postgres).is_err(),
            "postgres init is not yet supported"
        );
    }

    #[tokio::test]
    async fn effective_format_falls_back_to_the_live_global_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default()).expect("init");

        // No override in the manifest → the current global default applies, re-resolved each open.
        let first = Workspace::open(&ws, &operator(), &workspace_defaults_with("A%04d"))
            .await
            .expect("open");
        assert_eq!(first.person_id_format().expect("fmt").render(1), "A0001");

        let second = Workspace::open(&ws, &operator(), &workspace_defaults_with("B-%02d"))
            .await
            .expect("open");
        assert_eq!(
            second.person_id_format().expect("fmt").render(1),
            "B-01",
            "fallback is live"
        );
    }

    #[tokio::test]
    async fn a_manifest_override_pins_the_format_over_the_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).expect("dir");
        std::fs::write(
            ws.join("workspace.toml"),
            "database_url = \"sqlite://genealogy.sqlite3\"\n\n[id_formats]\nperson = \"Z%02d\"\n",
        )
        .expect("write manifest");

        let workspace = Workspace::open(&ws, &operator(), &workspace_defaults_with("A%04d"))
            .await
            .expect("open");
        assert_eq!(
            workspace.person_id_format().expect("fmt").render(3),
            "Z03",
            "override wins"
        );
    }

    #[tokio::test]
    async fn opening_a_postgres_backed_workspace_surfaces_a_db_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).expect("dir");
        std::fs::write(ws.join("workspace.toml"), "database_url = \"postgres://localhost/x\"\n")
            .expect("write manifest");

        let err = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default()).await;
        assert!(
            matches!(err, Err(crate::error::AppError::Db(_))),
            "postgres store is unsupported"
        );
    }

    #[tokio::test]
    async fn a_malformed_id_format_surfaces_as_a_config_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default()).expect("init");
        let workspace = Workspace::open(&ws, &operator(), &workspace_defaults_with("no-conversion-token"))
            .await
            .expect("open");
        assert!(matches!(
            workspace.person_id_format(),
            Err(crate::error::AppError::Config(_))
        ));
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
