//! Workspace = a directory (ADR 0005): a manifest, the database, and `exports/ backups/ media/`.
//!
//! The manifest (`<dir>/workspace.toml`) records the `database_url` (a SQLite file ref — relative
//! resolved against the directory — or a Postgres URL), the per-aggregate `HumanId` formats, and
//! the operators known to this workspace (so the operator id is never loose — ADR 0005). [`Workspace`]
//! opens the engine-neutral [`Store`] and exposes it to the use-cases; the engine stays in
//! `genealogy-db`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use genealogy_core::id_format::IdFormat;
use genealogy_db::Store;
use serde::{Deserialize, Serialize};
use unic_langid::LanguageIdentifier;

use crate::aggregates::for_each_human_id_aggregate;
use crate::config::{
    AppDefaults, DateFormat, Engine, IdFormats, NumberFormat, OperatorConfig, ThemeMode, WorkspaceDefaults,
};
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

/// Generates the per-workspace `HumanId` override struct, the effective-format accessors on
/// [`Workspace`], and the override-over-default resolver — all from the canonical registry (#38).
macro_rules! id_format_overrides {
    ($(($snake:ident, $noun:literal, $fmt:literal, $fmt_fn:ident)),+ $(,)?) => {
        /// Per-workspace `HumanId` format overrides (ADR 0005).
        ///
        /// Absent fields fall back **live** to the global `[defaults].id_formats`, re-resolved every
        /// time the workspace is opened — so changing the global default takes effect for any
        /// workspace that hasn't pinned its own. Setting a field here pins it for this workspace.
        #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
        pub struct IdFormatOverrides {
            $(
                #[doc = concat!("Override for the ", $noun, " id format; `None` uses the global default.")]
                #[serde(default, skip_serializing_if = "Option::is_none")]
                pub $snake: Option<String>,
            )+
        }

        impl Workspace {
            $(
                #[doc = concat!("The parsed effective ", $noun, " `HumanId` format (override-over-default).")]
                ///
                /// # Errors
                ///
                /// [`AppError::Config`] if the resolved format string is malformed.
                pub fn $fmt_fn(&self) -> Result<IdFormat, AppError> {
                    IdFormat::parse(&self.id_formats.$snake).map_err(|e| AppError::Config(e.to_string()))
                }
            )+
        }

        /// Resolves effective id formats: a manifest override wins, else the live global default.
        fn resolve_id_formats(overrides: &IdFormatOverrides, defaults: &WorkspaceDefaults) -> IdFormats {
            IdFormats {
                $(
                    $snake: overrides
                        .$snake
                        .clone()
                        .unwrap_or_else(|| defaults.id_formats.$snake.clone()),
                )+
            }
        }
    };
}

for_each_human_id_aggregate!(id_format_overrides);

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
    /// Per-workspace UI preference overrides (colour theme, native-window geometry); absent fields
    /// fall back to the global defaults (theme) or a built-in size (geometry).
    #[serde(default)]
    pub ui: UiPreferences,
    /// Per-workspace language/locale/date/number overrides; absent fields fall back to the global
    /// defaults.
    #[serde(default)]
    pub locale: LocaleOverrides,
    /// Per-plugin enabled/disabled overrides (PR21); a plugin absent from the map is enabled.
    #[serde(default)]
    pub plugins: PluginPreferences,
}

/// Per-workspace plugin enable/disable overrides (ADR 0007 §6; PR21).
///
/// Plugins are **enabled by default** — capabilities remain deny-by-default (ADR 0011 §2)
/// regardless, so an unlisted plugin is merely eligible to run, not automatically granted anything.
/// Only explicitly *disabled* plugins are recorded, so a freshly discovered plugin needs no manifest
/// change to be usable, and disabling one is a minimal, readable diff.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPreferences {
    /// The ids of plugins the operator has turned off, by their discovery id (the component's file
    /// stem, e.g. `gedcom-import`).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub disabled: BTreeSet<String>,
}

impl PluginPreferences {
    /// Whether the plugin `id` is enabled (the default for any id not explicitly disabled).
    #[must_use]
    pub fn is_enabled(&self, id: &str) -> bool {
        !self.disabled.contains(id)
    }
}

/// Saved native-window geometry (per workspace only — there is no global default).
///
/// Stored in **logical** pixels (matching the window builder's `with_inner_size`) so it survives a
/// DPI change. Restored at startup; an off-screen position is recentred onto a visible monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// The outer x position (logical px).
    pub x: i32,
    /// The outer y position (logical px).
    pub y: i32,
    /// The inner width (logical px).
    pub width: u32,
    /// The inner height (logical px).
    pub height: u32,
    /// Whether the window was maximized.
    #[serde(default)]
    pub maximized: bool,
}

/// How many recently-opened items the dashboard "Jump back in" list keeps.
pub const RECENT_LIMIT: usize = 5;

/// A recently-opened item for the dashboard "Jump back in" list: a record or a tool/screen.
///
/// Frontend-neutral — `kind` is the stored aggregate-type string (e.g. `person`) and `tool` a stable
/// tool-key string, which the frontend maps to its own navigation types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "kebab-case")]
pub enum RecentItem {
    /// A record's detail screen.
    Record {
        /// The aggregate-type string (e.g. `person`).
        kind: String,
        /// The record's user-facing id (e.g. `I0001`).
        human_id: String,
        /// The display label captured when it was opened.
        label: String,
    },
    /// A tool / screen (Pedigree, Merge, Preferences, …).
    Tool {
        /// The tool's stable key string.
        tool: String,
    },
}

/// Prepends `item` to the recent list (newest first), dropping any prior duplicate and capping the
/// list at [`RECENT_LIMIT`]. Records match on `(kind, human_id)` and tools on `tool` — labels are
/// ignored, so reopening a renamed record refreshes it rather than duplicating it.
pub fn push_recent(recent: &mut Vec<RecentItem>, item: RecentItem) {
    recent.retain(|existing| !same_recent(existing, &item));
    recent.insert(0, item);
    recent.truncate(RECENT_LIMIT);
}

/// Whether two recent items refer to the same target (ignoring the display label).
fn same_recent(a: &RecentItem, b: &RecentItem) -> bool {
    match (a, b) {
        (
            RecentItem::Record {
                kind: a_kind,
                human_id: a_id,
                ..
            },
            RecentItem::Record {
                kind: b_kind,
                human_id: b_id,
                ..
            },
        ) => a_kind == b_kind && a_id == b_id,
        (RecentItem::Tool { tool: a_tool }, RecentItem::Tool { tool: b_tool }) => a_tool == b_tool,
        (RecentItem::Record { .. }, RecentItem::Tool { .. }) | (RecentItem::Tool { .. }, RecentItem::Record { .. }) => {
            false
        }
    }
}

/// Per-workspace UI preference overrides (`workspace.toml` `[ui]`, ADR 0005).
///
/// `theme` absent falls back **live** to the global `[workspace-defaults.ui].theme`; `window` is
/// per-workspace only (absent means use the built-in default size, centred by the OS); `recent` is
/// the persisted "Jump back in" list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiPreferences {
    /// The pinned colour-theme mode; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeMode>,
    /// The saved native-window geometry; `None` until the window is first moved/resized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowGeometry>,
    /// The recently-opened records/tools, newest first (the dashboard "Jump back in" list).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent: Vec<RecentItem>,
}

/// The fully-resolved UI preferences a frontend reads at startup (theme over the live default, plus
/// any saved geometry and the recent list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedUiPreferences {
    /// The effective theme mode (manifest override over the global default).
    pub theme: ThemeMode,
    /// The saved geometry, if any.
    pub window: Option<WindowGeometry>,
    /// The persisted "Jump back in" list, newest first.
    pub recent: Vec<RecentItem>,
}

/// Resolves effective UI prefs: theme override wins over the live global default; geometry and the
/// recent list are manifest-only (no default fallback). Mirrors [`resolve_id_formats`].
fn resolve_ui_preferences(overrides: &UiPreferences, defaults: &WorkspaceDefaults) -> ResolvedUiPreferences {
    ResolvedUiPreferences {
        theme: overrides.theme.unwrap_or(defaults.ui.theme),
        window: overrides.window,
        recent: overrides.recent.clone(),
    }
}

/// Reads a workspace's resolved UI preferences (theme + saved geometry) without opening the store.
///
/// Infallible by design: a missing directory or manifest, or any parse error, yields the defaults
/// (the global theme, no geometry) so a failed read never blocks startup.
#[must_use]
pub fn read_ui_preferences(dir: &Path, defaults: &WorkspaceDefaults) -> ResolvedUiPreferences {
    let overrides = read_manifest(dir).map(|manifest| manifest.ui).unwrap_or_default();
    resolve_ui_preferences(&overrides, defaults)
}

/// Which of the three ADR 0005/0006 layers supplied a resolved setting's value: a workspace
/// manifest override, the live shared-app `[workspace-defaults]`, or the built-in embedded baseline
/// (never overridden anywhere).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    /// A workspace manifest override won.
    Workspace,
    /// The live shared-app default won (no workspace override set).
    SharedDefault,
    /// The embedded baseline won (no workspace override and no shared default set).
    Embedded,
}

/// The three-layer override chain behind one resolved theme setting (mockup "Workspace defaults").
///
/// Carries each layer's own value (not just the winner) so a frontend can render the full `wins` /
/// `fallback` / `fallback` stack the mockup shows, alongside [`Self::winner`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeLayers {
    /// The workspace manifest's own override, if it has pinned one.
    pub workspace: Option<ThemeMode>,
    /// The live shared-app default (`[workspace-defaults.ui].theme`).
    pub shared_default: ThemeMode,
    /// The embedded baseline (never overridden — [`ThemeMode::default`]).
    pub embedded: ThemeMode,
    /// Which layer supplied the resolved value.
    pub winner: LayerKind,
}

/// Builds the theme override-chain DTO for the mockup's "Workspace defaults" card.
#[must_use]
pub fn theme_layers(overrides: &UiPreferences, defaults: &WorkspaceDefaults) -> ThemeLayers {
    let winner = if overrides.theme.is_some() {
        LayerKind::Workspace
    } else {
        LayerKind::SharedDefault
    };
    ThemeLayers {
        workspace: overrides.theme,
        shared_default: defaults.ui.theme,
        embedded: ThemeMode::default(),
        winner,
    }
}

/// The three-layer override chain behind one resolved `HumanId` format field (mockup "Workspace
/// defaults" — the Person id format is shown as the worked example).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdFormatLayers {
    /// The workspace manifest's own override, if it has pinned one.
    pub workspace: Option<String>,
    /// The live shared-app default (`[workspace-defaults.id_formats]`).
    pub shared_default: String,
    /// The embedded baseline (the aggregate's Gramps-style printf default).
    pub embedded: String,
    /// Which layer supplied the resolved value.
    pub winner: LayerKind,
}

/// Builds the person-id-format override-chain DTO for the mockup's "Workspace defaults" card.
#[must_use]
pub fn person_id_format_layers(overrides: &IdFormatOverrides, defaults: &WorkspaceDefaults) -> IdFormatLayers {
    let winner = if overrides.person.is_some() {
        LayerKind::Workspace
    } else {
        LayerKind::SharedDefault
    };
    IdFormatLayers {
        workspace: overrides.person.clone(),
        shared_default: defaults.id_formats.person.clone(),
        embedded: IdFormats::default().person,
        winner,
    }
}

/// The override-chain DTOs backing the mockup's "Workspace defaults" card, for the two worked
/// examples it shows (theme, the Person id format).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceLayers {
    /// The theme override chain.
    pub theme: ThemeLayers,
    /// The Person `HumanId` format override chain.
    pub person_id_format: IdFormatLayers,
}

/// Reads a workspace's override-chain DTOs without opening the store.
///
/// Infallible by design, mirroring [`read_ui_preferences`]: a missing directory or manifest, or any
/// parse error, degrades to "no workspace override" (so the shared default or embedded baseline
/// wins) rather than blocking the Preferences screen.
#[must_use]
pub fn read_preference_layers(dir: &Path, defaults: &WorkspaceDefaults) -> PreferenceLayers {
    let manifest = read_manifest(dir).ok();
    let ui = manifest
        .as_ref()
        .map(|manifest| manifest.ui.clone())
        .unwrap_or_default();
    let id_formats = manifest.map(|manifest| manifest.id_formats).unwrap_or_default();
    PreferenceLayers {
        theme: theme_layers(&ui, defaults),
        person_id_format: person_id_format_layers(&id_formats, defaults),
    }
}

/// Persists the colour-theme mode into the workspace manifest's `[ui]` block (read-modify-write,
/// preserving operators / id-format overrides / saved geometry). No store is opened.
///
/// # Errors
///
/// [`AppError::Workspace`] if the manifest is missing or cannot be read/written.
pub fn save_theme_mode(dir: &Path, mode: ThemeMode) -> Result<(), AppError> {
    let mut manifest = read_manifest(dir)?;
    manifest.ui.theme = Some(mode);
    write_manifest(dir, &manifest)
}

/// Persists the native-window geometry into the workspace manifest's `[ui]` block
/// (read-modify-write, preserving the rest). No store is opened.
///
/// # Errors
///
/// [`AppError::Workspace`] if the manifest is missing or cannot be read/written.
pub fn save_window_geometry(dir: &Path, geometry: WindowGeometry) -> Result<(), AppError> {
    let mut manifest = read_manifest(dir)?;
    manifest.ui.window = Some(geometry);
    write_manifest(dir, &manifest)
}

/// Persists the "Jump back in" recent list into the workspace manifest's `[ui]` block
/// (read-modify-write, preserving the rest). No store is opened.
///
/// # Errors
///
/// [`AppError::Workspace`] if the manifest is missing or cannot be read/written.
pub fn save_recent(dir: &Path, recent: &[RecentItem]) -> Result<(), AppError> {
    let mut manifest = read_manifest(dir)?;
    manifest.ui.recent = recent.to_vec();
    write_manifest(dir, &manifest)
}

/// Per-workspace language/locale/date/number overrides (`workspace.toml` `[locale]`, ADR 0005).
///
/// Every field absent falls back **live** to the global `[workspace-defaults.locale]`, mirroring
/// [`UiPreferences::theme`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocaleOverrides {
    /// The pinned UI-language override; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_language: Option<LanguageIdentifier>,
    /// The pinned data-locale override; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_locale: Option<LanguageIdentifier>,
    /// The pinned date-format override; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_format: Option<DateFormat>,
    /// The pinned number-format override; `None` uses the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<NumberFormat>,
}

/// The fully-resolved language/locale/date/number preferences a frontend reads (manifest override
/// over the live global default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocale {
    /// The effective UI-language override; `None` still means "follow the system".
    pub ui_language: Option<LanguageIdentifier>,
    /// The effective data-locale override; `None` still means "follow the system".
    pub data_locale: Option<LanguageIdentifier>,
    /// The effective date format.
    pub date_format: DateFormat,
    /// The effective number format.
    pub number_format: NumberFormat,
}

/// Resolves effective locale prefs: each manifest override wins over the live global default.
/// Mirrors [`resolve_ui_preferences`].
fn resolve_locale(overrides: &LocaleOverrides, defaults: &WorkspaceDefaults) -> ResolvedLocale {
    ResolvedLocale {
        ui_language: overrides
            .ui_language
            .clone()
            .or_else(|| defaults.locale.ui_language.clone()),
        data_locale: overrides
            .data_locale
            .clone()
            .or_else(|| defaults.locale.data_locale.clone()),
        date_format: overrides.date_format.unwrap_or(defaults.locale.date_format),
        number_format: overrides.number_format.unwrap_or(defaults.locale.number_format),
    }
}

/// Reads a workspace's resolved locale preferences without opening the store.
///
/// Infallible by design: a missing directory or manifest, or any parse error, yields the defaults
/// so a failed read never blocks startup. Mirrors [`read_ui_preferences`].
#[must_use]
pub fn read_resolved_locale(dir: &Path, defaults: &WorkspaceDefaults) -> ResolvedLocale {
    let overrides = read_manifest(dir).map(|manifest| manifest.locale).unwrap_or_default();
    resolve_locale(&overrides, defaults)
}

/// Reads a workspace's plugin enable/disable overrides without opening the store.
///
/// Infallible by design, matching [`read_ui_preferences`]: a missing directory or manifest, or any
/// parse error, yields the defaults (every plugin enabled) so a failed read never blocks the plugin
/// manager screen from rendering.
#[must_use]
pub fn read_plugin_preferences(dir: &Path) -> PluginPreferences {
    read_manifest(dir).map(|manifest| manifest.plugins).unwrap_or_default()
}

/// Persists the language/locale/date/number overrides into the workspace manifest's `[locale]`
/// block (read-modify-write, preserving the rest). No store is opened. Mirrors [`save_theme_mode`].
///
/// # Errors
///
/// [`AppError::Workspace`] if the manifest is missing or cannot be read/written.
pub fn save_locale_overrides(dir: &Path, locale: LocaleOverrides) -> Result<(), AppError> {
    let mut manifest = read_manifest(dir)?;
    manifest.locale = locale;
    write_manifest(dir, &manifest)
}

/// Persists whether plugin `id` is enabled into the workspace manifest's `[plugins]` block
/// (read-modify-write, preserving operators / id-format overrides / UI preferences). No store is
/// opened.
///
/// # Errors
///
/// [`AppError::Workspace`] if the manifest is missing or cannot be read/written.
pub fn save_plugin_enabled(dir: &Path, id: &str, enabled: bool) -> Result<(), AppError> {
    let mut manifest = read_manifest(dir)?;
    if enabled {
        manifest.plugins.disabled.remove(id);
    } else {
        manifest.plugins.disabled.insert(id.to_owned());
    }
    write_manifest(dir, &manifest)
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
    /// The `database_url` is frozen at creation (a workspace's database location can't change
    /// afterward), resolved by precedence: the `database_url` argument (the `--database-url` flag) >
    /// `defaults.database_url` > the `defaults.engine` default. `id_formats` are **not** copied in —
    /// the manifest leaves them absent so they fall back live to the global defaults; a workspace
    /// pins one only by editing its manifest. Refuses to overwrite an existing manifest.
    ///
    /// # Errors
    ///
    /// [`AppError::Workspace`] if the tree/manifest cannot be written, or [`AppError::Config`] if no
    /// `database_url` can be resolved or it has an unrecognized scheme.
    pub fn init(
        dir: &Path,
        operator: &OperatorConfig,
        defaults: &AppDefaults,
        database_url: Option<&str>,
    ) -> Result<WorkspaceManifest, AppError> {
        let database_url = resolve_init_database_url(defaults, database_url)?;
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
            database_url,
            id_formats: IdFormatOverrides::default(),
            operators,
            ui: UiPreferences::default(),
            locale: LocaleOverrides::default(),
            plugins: PluginPreferences::default(),
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

    /// Rebuilds every projection from the event log (ADR 0010): a maintenance operation backing the
    /// `genealogy rebuild` command. Engine-neutral — works for whichever backend this workspace uses.
    ///
    /// # Errors
    ///
    /// [`AppError::Db`] if clearing or replaying a projection fails.
    pub async fn rebuild_projections(&self) -> Result<(), AppError> {
        self.store.rebuild_projections().await.map_err(AppError::Db)
    }
}

/// Resolves the `database_url` frozen into a new workspace at `init`, by precedence: the explicit
/// `flag` (`--database-url`) > `defaults.database_url` > the `defaults.engine` default.
fn resolve_init_database_url(defaults: &AppDefaults, flag: Option<&str>) -> Result<String, AppError> {
    if let Some(url) = flag {
        return validated_database_url(url);
    }
    if let Some(url) = &defaults.database_url {
        return validated_database_url(url);
    }
    match defaults.engine {
        Engine::Sqlite => Ok(DEFAULT_DATABASE_URL.to_owned()),
        Engine::Postgres => Err(AppError::Config(
            "the postgres engine needs a database_url; pass `--database-url` or set `[defaults].database_url`"
                .to_owned(),
        )),
    }
}

/// Maps a `database_url` scheme to its [`Engine`], or `None` for an unrecognized scheme. Mirrors the
/// schemes [`validated_database_url`] accepts.
pub(crate) fn engine_of_url(url: &str) -> Option<Engine> {
    if url.starts_with("sqlite:") {
        Some(Engine::Sqlite)
    } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
        Some(Engine::Postgres)
    } else {
        None
    }
}

/// Best-effort read of a workspace's engine from its manifest `database_url`, without opening the
/// store. `None` when the directory or manifest is missing/corrupt or the scheme is unrecognized —
/// so the workspace registry can still list the row.
pub(crate) fn manifest_engine(dir: &Path) -> Option<Engine> {
    let manifest = read_manifest(dir).ok()?;
    engine_of_url(&manifest.database_url)
}

/// Validates a `database_url`'s scheme (the schemes [`Store`] dispatches on), returning it owned.
fn validated_database_url(url: &str) -> Result<String, AppError> {
    if url.starts_with("sqlite:") || url.starts_with("postgres:") || url.starts_with("postgresql:") {
        return Ok(url.to_owned());
    }
    Err(AppError::Config(format!(
        "unrecognized database_url scheme (expected sqlite:// or postgres://): {url}"
    )))
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
    use super::{
        IdFormatOverrides, LayerKind, LocaleOverrides, RECENT_LIMIT, RecentItem, UiPreferences, WindowGeometry,
        Workspace, person_id_format_layers, push_recent, read_manifest, read_plugin_preferences,
        read_preference_layers, read_resolved_locale, read_ui_preferences, resolve_database_url,
        resolve_init_database_url, resolve_locale, resolve_ui_preferences, save_locale_overrides, save_plugin_enabled,
        save_recent, save_theme_mode, save_window_geometry, theme_layers,
    };
    use crate::config::{
        AppDefaults, DateFormat, Engine, IdFormats, LocaleDefaults, NumberFormat, OperatorConfig, ThemeMode,
        UiDefaults, WorkspaceDefaults,
    };
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
            ..Default::default()
        }
    }

    #[test]
    fn init_creates_the_tree_and_leaves_id_formats_unset() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        // init never writes id-format overrides — formats stay a live fallback.
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");

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
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("first init");
        let again = Workspace::init(&ws, &operator(), &AppDefaults::default(), None);
        assert!(again.is_err(), "second init must not clobber the manifest");
    }

    #[test]
    fn init_database_url_resolves_by_precedence() {
        // Default engine (sqlite), no url anywhere → the sqlite default.
        let sqlite_defaults = AppDefaults::default();
        assert_eq!(
            resolve_init_database_url(&sqlite_defaults, None).expect("sqlite default"),
            "sqlite://genealogy.sqlite3"
        );

        // The postgres engine with no url → rejected (it needs a connection string).
        let pg_engine = AppDefaults {
            engine: Engine::Postgres,
            database_url: None,
        };
        assert!(
            resolve_init_database_url(&pg_engine, None).is_err(),
            "postgres engine needs a database_url"
        );

        // A configured [defaults].database_url wins over the engine default.
        let configured = AppDefaults {
            engine: Engine::Sqlite,
            database_url: Some("postgres://localhost/db".to_owned()),
        };
        assert_eq!(
            resolve_init_database_url(&configured, None).expect("configured url"),
            "postgres://localhost/db"
        );

        // The flag overrides everything, including a configured url.
        assert_eq!(
            resolve_init_database_url(&configured, Some("postgres://other/db2")).expect("flag url"),
            "postgres://other/db2"
        );

        // An unrecognized scheme is rejected.
        assert!(
            resolve_init_database_url(&sqlite_defaults, Some("mysql://x")).is_err(),
            "unknown scheme rejected"
        );
    }

    #[tokio::test]
    async fn effective_format_falls_back_to_the_live_global_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");

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
    async fn opening_an_unreachable_postgres_workspace_surfaces_a_db_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).expect("dir");
        // Port 1 has no listener, so the connection is refused immediately — a fast, deterministic
        // way to exercise the postgres open path's error mapping without a running server.
        std::fs::write(
            ws.join("workspace.toml"),
            "database_url = \"postgres://localhost:1/x\"\n",
        )
        .expect("write manifest");

        let err = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default()).await;
        assert!(
            matches!(err, Err(crate::error::AppError::Db(_))),
            "an unreachable postgres server surfaces as a db error"
        );
    }

    #[tokio::test]
    async fn a_malformed_id_format_surfaces_as_a_config_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
        let workspace = Workspace::open(&ws, &operator(), &workspace_defaults_with("no-conversion-token"))
            .await
            .expect("open");
        assert!(matches!(
            workspace.person_id_format(),
            Err(crate::error::AppError::Config(_))
        ));
    }

    fn defaults_with_theme(theme: ThemeMode) -> WorkspaceDefaults {
        WorkspaceDefaults {
            ui: UiDefaults { theme },
            ..Default::default()
        }
    }

    #[test]
    fn ui_theme_override_wins_over_the_live_default() {
        let overrides = UiPreferences {
            theme: Some(ThemeMode::Dark),
            window: None,
            recent: Vec::new(),
        };
        let resolved = resolve_ui_preferences(&overrides, &defaults_with_theme(ThemeMode::Light));
        assert_eq!(resolved.theme, ThemeMode::Dark, "the manifest override pins the theme");
    }

    #[test]
    fn ui_theme_falls_back_to_the_live_default_when_unset() {
        let resolved = resolve_ui_preferences(&UiPreferences::default(), &defaults_with_theme(ThemeMode::Light));
        assert_eq!(resolved.theme, ThemeMode::Light, "absent theme uses the live default");
        assert_eq!(resolved.window, None);
    }

    #[test]
    fn save_theme_and_geometry_persist_and_preserve_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");

        save_theme_mode(&ws, ThemeMode::Dark).expect("save theme");
        let geometry = WindowGeometry {
            x: 100,
            y: 80,
            width: 1024,
            height: 768,
            maximized: false,
        };
        save_window_geometry(&ws, geometry).expect("save geometry");
        let recent = vec![
            RecentItem::Record {
                kind: "person".to_owned(),
                human_id: "I0001".to_owned(),
                label: "Ada Lovelace".to_owned(),
            },
            RecentItem::Tool {
                tool: "pedigree".to_owned(),
            },
        ];
        save_recent(&ws, &recent).expect("save recent");

        let manifest = read_manifest(&ws).expect("manifest");
        assert_eq!(manifest.ui.theme, Some(ThemeMode::Dark));
        assert_eq!(manifest.ui.window, Some(geometry));
        assert_eq!(manifest.ui.recent, recent, "the recent list round-trips");
        // The operator recorded at init survives the read-modify-write saves.
        assert!(manifest.operators.contains_key(&Uuid::from_u128(1).to_string()));

        let resolved = read_ui_preferences(&ws, &defaults_with_theme(ThemeMode::System));
        assert_eq!(resolved.theme, ThemeMode::Dark, "override wins over the default");
        assert_eq!(resolved.window, Some(geometry));
        assert_eq!(resolved.recent, recent);
    }

    #[test]
    fn push_recent_dedups_by_target_keeps_newest_first_and_caps() {
        let record = |id: &str| RecentItem::Record {
            kind: "person".to_owned(),
            human_id: id.to_owned(),
            label: id.to_owned(),
        };
        let mut recent = Vec::new();
        for id in ["I0001", "I0002", "I0003", "I0004", "I0005", "I0006"] {
            push_recent(&mut recent, record(id));
        }
        assert_eq!(recent.len(), RECENT_LIMIT, "the list is capped");
        assert_eq!(recent.first(), Some(&record("I0006")), "newest first");
        assert!(!recent.contains(&record("I0001")), "the oldest fell off");

        // Reopening an existing record moves it to the front without duplicating (label ignored).
        push_recent(
            &mut recent,
            RecentItem::Record {
                kind: "person".to_owned(),
                human_id: "I0003".to_owned(),
                label: "renamed".to_owned(),
            },
        );
        assert_eq!(
            recent
                .iter()
                .filter(|item| matches!(item, RecentItem::Record { human_id, .. } if human_id == "I0003"))
                .count(),
            1
        );
        assert!(matches!(recent.first(), Some(RecentItem::Record { human_id, .. }) if human_id == "I0003"));
    }

    #[test]
    fn read_ui_preferences_degrades_to_defaults_without_a_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let resolved = read_ui_preferences(&dir.path().join("missing"), &defaults_with_theme(ThemeMode::Dark));
        assert_eq!(resolved.theme, ThemeMode::Dark, "no manifest => the global default");
        assert_eq!(resolved.window, None);
    }

    #[test]
    fn a_manifest_without_a_ui_table_parses_to_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        std::fs::create_dir_all(&ws).expect("dir");
        std::fs::write(
            ws.join("workspace.toml"),
            "database_url = \"sqlite://genealogy.sqlite3\"\n",
        )
        .expect("write");

        let manifest = read_manifest(&ws).expect("manifest");
        assert_eq!(manifest.ui, UiPreferences::default(), "absent [ui] => default");
    }

    fn defaults_with_locale(locale: LocaleDefaults) -> WorkspaceDefaults {
        WorkspaceDefaults {
            locale,
            ..Default::default()
        }
    }

    #[test]
    fn locale_override_wins_over_the_live_default() {
        let defaults = defaults_with_locale(LocaleDefaults {
            ui_language: Some("en".parse().expect("langid")),
            data_locale: Some("en-US".parse().expect("langid")),
            date_format: DateFormat::Long,
            number_format: NumberFormat::CommaPoint,
        });
        let overrides = LocaleOverrides {
            ui_language: Some("nb-NO".parse().expect("langid")),
            data_locale: None,
            date_format: Some(DateFormat::Numeric),
            number_format: None,
        };
        let resolved = resolve_locale(&overrides, &defaults);
        assert_eq!(
            resolved.ui_language,
            Some("nb-NO".parse().expect("langid")),
            "the pinned ui language wins"
        );
        assert_eq!(
            resolved.data_locale,
            Some("en-US".parse().expect("langid")),
            "an absent override falls back to the live default"
        );
        assert_eq!(resolved.date_format, DateFormat::Numeric, "the pinned date format wins");
        assert_eq!(
            resolved.number_format,
            NumberFormat::CommaPoint,
            "an absent number-format override falls back"
        );
    }

    #[test]
    fn locale_falls_back_entirely_when_unset() {
        let defaults = defaults_with_locale(LocaleDefaults::default());
        let resolved = resolve_locale(&LocaleOverrides::default(), &defaults);
        assert_eq!(
            resolved.ui_language, None,
            "no override, no default => follow the system"
        );
        assert_eq!(resolved.date_format, DateFormat::Long, "the built-in enum default");
        assert_eq!(
            resolved.number_format,
            NumberFormat::LocaleDefault,
            "the built-in enum default"
        );
    }

    #[test]
    fn save_locale_overrides_persists_and_preserves_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
        save_theme_mode(&ws, ThemeMode::Dark).expect("save theme");

        let locale = LocaleOverrides {
            ui_language: Some("nn-NO".parse().expect("langid")),
            data_locale: Some("nn-NO".parse().expect("langid")),
            date_format: Some(DateFormat::Medium),
            number_format: Some(NumberFormat::SpaceComma),
        };
        save_locale_overrides(&ws, locale.clone()).expect("save locale");

        let manifest = read_manifest(&ws).expect("manifest");
        assert_eq!(manifest.locale, locale, "the locale overrides round-trip");
        assert_eq!(
            manifest.ui.theme,
            Some(ThemeMode::Dark),
            "the earlier theme save survives"
        );
        assert!(manifest.operators.contains_key(&Uuid::from_u128(1).to_string()));

        let resolved = read_resolved_locale(&ws, &WorkspaceDefaults::default());
        assert_eq!(
            resolved.date_format,
            DateFormat::Medium,
            "override wins over the default"
        );
    }

    #[test]
    fn read_resolved_locale_degrades_to_defaults_without_a_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let defaults = defaults_with_locale(LocaleDefaults {
            date_format: DateFormat::Numeric,
            ..Default::default()
        });
        let resolved = read_resolved_locale(&dir.path().join("missing"), &defaults);
        assert_eq!(
            resolved.date_format,
            DateFormat::Numeric,
            "no manifest => the global default"
        );
    }

    #[test]
    fn theme_layers_reports_the_workspace_override_as_the_winner_when_pinned() {
        let overrides = UiPreferences {
            theme: Some(ThemeMode::Dark),
            ..Default::default()
        };
        let layers = theme_layers(&overrides, &defaults_with_theme(ThemeMode::Light));
        assert_eq!(layers.workspace, Some(ThemeMode::Dark));
        assert_eq!(layers.shared_default, ThemeMode::Light);
        assert_eq!(
            layers.embedded,
            ThemeMode::System,
            "the embedded baseline is the enum default"
        );
        assert_eq!(layers.winner, LayerKind::Workspace);
    }

    #[test]
    fn theme_layers_reports_the_shared_default_as_the_winner_when_unpinned() {
        let layers = theme_layers(&UiPreferences::default(), &defaults_with_theme(ThemeMode::Dark));
        assert_eq!(layers.workspace, None);
        assert_eq!(layers.shared_default, ThemeMode::Dark);
        assert_eq!(layers.winner, LayerKind::SharedDefault);
    }

    #[test]
    fn person_id_format_layers_reports_the_workspace_override_as_the_winner_when_pinned() {
        let overrides = IdFormatOverrides {
            person: Some("Z%02d".to_owned()),
            ..Default::default()
        };
        let layers = person_id_format_layers(&overrides, &workspace_defaults_with("A%04d"));
        assert_eq!(layers.workspace.as_deref(), Some("Z%02d"));
        assert_eq!(layers.shared_default, "A%04d");
        assert_eq!(
            layers.embedded, "I%04d",
            "the embedded baseline is the Gramps-style default"
        );
        assert_eq!(layers.winner, LayerKind::Workspace);
    }

    #[test]
    fn person_id_format_layers_reports_the_shared_default_as_the_winner_when_unpinned() {
        let layers = person_id_format_layers(&IdFormatOverrides::default(), &workspace_defaults_with("B-%02d"));
        assert_eq!(layers.workspace, None);
        assert_eq!(layers.shared_default, "B-%02d");
        assert_eq!(layers.winner, LayerKind::SharedDefault);
    }

    #[test]
    fn read_preference_layers_reflects_pinned_workspace_overrides() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
        save_theme_mode(&ws, ThemeMode::Dark).expect("save theme");

        let layers = read_preference_layers(&ws, &workspace_defaults_with("A%04d"));
        assert_eq!(layers.theme.workspace, Some(ThemeMode::Dark));
        assert_eq!(layers.theme.winner, LayerKind::Workspace);
        assert_eq!(
            layers.person_id_format.workspace, None,
            "no id-format override was pinned"
        );
        assert_eq!(layers.person_id_format.shared_default, "A%04d");
        assert_eq!(layers.person_id_format.winner, LayerKind::SharedDefault);
    }

    #[test]
    fn read_preference_layers_degrades_to_defaults_without_a_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let layers = read_preference_layers(&dir.path().join("missing"), &workspace_defaults_with("B-%02d"));
        assert_eq!(layers.theme.winner, LayerKind::SharedDefault);
        assert_eq!(layers.person_id_format.shared_default, "B-%02d");
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

    #[test]
    fn a_freshly_initialized_workspace_has_every_plugin_enabled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");

        let prefs = read_plugin_preferences(&ws);
        assert!(
            prefs.is_enabled("gedcom-import"),
            "an unlisted plugin defaults to enabled"
        );
        assert!(prefs.disabled.is_empty());
    }

    #[test]
    fn disabling_then_re_enabling_a_plugin_round_trips_and_preserves_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
        save_theme_mode(&ws, ThemeMode::Dark).expect("save theme");

        save_plugin_enabled(&ws, "gedcom-import", false).expect("disable");
        let prefs = read_plugin_preferences(&ws);
        assert!(
            !prefs.is_enabled("gedcom-import"),
            "disabled plugin reads back disabled"
        );
        assert!(prefs.is_enabled("gedcom-export"), "other plugins stay enabled");

        let manifest = read_manifest(&ws).expect("manifest");
        assert_eq!(
            manifest.ui.theme,
            Some(ThemeMode::Dark),
            "the earlier theme save survives"
        );

        save_plugin_enabled(&ws, "gedcom-import", true).expect("re-enable");
        let prefs = read_plugin_preferences(&ws);
        assert!(prefs.is_enabled("gedcom-import"), "re-enabling clears the override");
        assert!(prefs.disabled.is_empty(), "no plugins left disabled");
    }

    #[test]
    fn read_plugin_preferences_degrades_to_all_enabled_without_a_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let prefs = read_plugin_preferences(&dir.path().join("missing"));
        assert!(prefs.is_enabled("anything"), "no manifest => every plugin enabled");
    }
}
