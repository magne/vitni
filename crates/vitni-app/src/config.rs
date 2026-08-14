//! Global application configuration: the operator, the named-workspace registry, and the defaults
//! applied to new workspaces (ADR 0005).
//!
//! The global config (`~/.config/vitni/config.toml`, resolved via the `directories` crate)
//! names workspaces (`[workspaces.<name>]`), records the default (last-used) one (`default`), the
//! operator (`[operator]`), and the `[defaults]` template seeded into each new workspace manifest.
//! A *workspace* is a directory with its own manifest (see [`crate::workspace`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use unic_langid::LanguageIdentifier;
use uuid::Uuid;
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind, Confidence};

use crate::aggregates::for_each_human_id_aggregate;
use crate::error::AppError;

/// The application name for `directories` path resolution.
const APP_NAME: &str = "vitni";

/// Generates the per-aggregate `HumanId` format struct from the canonical registry.
///
/// A missing field falls back to its default via the container `#[serde(default)]` (which reads the
/// generated [`Default`] impl), so the formats stay Gramps-style printf defaults (data-model §7).
macro_rules! id_formats {
    ($(($snake:ident, $noun:literal, $fmt:literal, $fmt_fn:ident)),+ $(,)?) => {
        /// Per-aggregate `HumanId` formats (Gramps-style printf).
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct IdFormats {
            $(
                #[doc = concat!("The ", $noun, " id format, default `", $fmt, "`.")]
                pub $snake: String,
            )+
        }

        impl Default for IdFormats {
            fn default() -> Self {
                Self { $( $snake: $fmt.to_owned(), )+ }
            }
        }
    };
}

for_each_human_id_aggregate!(id_formats);

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
/// Consumed at the relevant action (e.g. the database location is read once at `init` and frozen
/// into the new workspace's `database_url`); these are *not* live fallbacks. Contrast
/// [`WorkspaceDefaults`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppDefaults {
    /// The engine a new workspace is created with when no explicit `database_url` is given.
    #[serde(default)]
    pub engine: Engine,
    /// An explicit `database_url` for new workspaces (e.g. a Postgres connection string). When set,
    /// it takes precedence over `engine`; a `vitni init --database-url` flag overrides both. The
    /// resolved value is frozen into the workspace manifest at `init`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,
}

/// The persisted UI colour-theme preference, resolved to a concrete palette at render time.
///
/// `System` follows the OS appearance (resolved once at startup by the renderer); `Light`/`Dark`
/// pin a palette. Stored per-workspace (a manifest override) over this app-level default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Follow the operating-system appearance (the default).
    #[default]
    System,
    /// The light palette.
    Light,
    /// The dark palette.
    Dark,
}

/// Live-fallback UI defaults: the theme mode a workspace falls back to when it pins none.
///
/// Window geometry is deliberately *not* here — it is only ever saved per-workspace (a default
/// position/size makes no sense app-wide); a workspace with no saved geometry uses a built-in size.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiDefaults {
    /// The colour-theme mode workspaces fall back to.
    #[serde(default)]
    pub theme: ThemeMode,
}

/// The date-display format a workspace falls back to (mockup "Date & number format", PR 20).
///
/// This is presentation config, not UI chrome (ADR 0003): `vitni-app` stays string-free, so a
/// frontend renders these variants into localized/formatted example text itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DateFormat {
    /// A fully-spelled month name (e.g. `12 April 1850`).
    #[default]
    Long,
    /// An abbreviated month name (e.g. `12 Apr 1850`).
    Medium,
    /// A numeric date (e.g. `1850-04-12`).
    Numeric,
    /// Follow the resolved data locale's own convention rather than a fixed style.
    LocaleDefault,
}

/// The number/decimal display convention a workspace falls back to (mockup "Date & number format").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NumberFormat {
    /// Space thousands separator, comma decimal point (e.g. `1 234,56`).
    SpaceComma,
    /// Comma thousands separator, point decimal point (e.g. `1,234.56`).
    CommaPoint,
    /// Follow the resolved data locale's own convention.
    #[default]
    LocaleDefault,
}

/// Live-fallback language/locale/format defaults (mockup "Language & locale" / "Date & number
/// format", PR 20).
///
/// `ui_language`/`data_locale` are distinct BCP-47 overrides (ADR 0003 §"presentation vs data"):
/// `ui_language` is the Fluent chrome/data-catalogue negotiation target, `data_locale` drives
/// sort/name-display. `None` for either means "follow the system", matching how
/// [`Localizer`](https://docs.rs/vitni-ui)/`Chrome` already resolve languages via
/// `DesktopLanguageRequester` when no override is configured.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocaleDefaults {
    /// The UI chrome/data language override; `None` follows the system locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_language: Option<LanguageIdentifier>,
    /// The data (sort/name-display) locale override; `None` follows the system locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_locale: Option<LanguageIdentifier>,
    /// The date-display format.
    #[serde(default)]
    pub date_format: DateFormat,
    /// The number-display format.
    #[serde(default)]
    pub number_format: NumberFormat,
}

/// A workspace's own literal replacement for one surety-scheme ordinal's label (ADR 0027). Bypasses
/// the Fluent-resolved default text for that `Confidence` level; not itself localized — the
/// operator's own chosen wording is shown verbatim in every locale, exactly like any other free-text
/// override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuretyLabelOverride {
    /// The label shown instead of the Fluent-resolved default (e.g. "Hearsay").
    pub label: String,
    /// An optional longer description (e.g. a tooltip explaining what this level means locally).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Per-ordinal surety-label overrides for the five fixed [`Confidence`] levels (ADR 0027).
///
/// Used both as the live global default (`WorkspaceDefaults::surety`) and the per-workspace manifest
/// override (`WorkspaceManifest::surety`, `vitni_app::workspace`) — the shape is identical at both
/// scopes, since both are "an optional literal replacement for this ordinal," never a required value.
/// `Confidence`'s five variants and their wire encoding are unchanged (ADR 0027 §1); this is a
/// presentation-only override consulted by [`Self::label_for`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuretyLabelOverrides {
    /// Override for `Confidence::VeryLow`; `None` uses the Fluent-resolved default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub very_low: Option<SuretyLabelOverride>,
    /// Override for `Confidence::Low`; `None` uses the Fluent-resolved default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<SuretyLabelOverride>,
    /// Override for `Confidence::Normal`; `None` uses the Fluent-resolved default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<SuretyLabelOverride>,
    /// Override for `Confidence::High`; `None` uses the Fluent-resolved default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<SuretyLabelOverride>,
    /// Override for `Confidence::VeryHigh`; `None` uses the Fluent-resolved default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub very_high: Option<SuretyLabelOverride>,
}

impl SuretyLabelOverrides {
    /// The override for `confidence`'s ordinal, if one is set.
    #[must_use]
    pub fn label_for(&self, confidence: Confidence) -> Option<&SuretyLabelOverride> {
        match confidence {
            Confidence::VeryLow => self.very_low.as_ref(),
            Confidence::Low => self.low.as_ref(),
            Confidence::Normal => self.normal.as_ref(),
            Confidence::High => self.high.as_ref(),
            Confidence::VeryHigh => self.very_high.as_ref(),
        }
    }
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
    /// The UI defaults (colour-theme mode) workspaces fall back to.
    #[serde(default)]
    pub ui: UiDefaults,
    /// The language/locale/date/number defaults workspaces fall back to.
    #[serde(default)]
    pub locale: LocaleDefaults,
    /// The surety-scheme label overrides workspaces fall back to (ADR 0027).
    #[serde(default)]
    pub surety: SuretyLabelOverrides,
}

/// The default per-request timeout for an AI provider, in seconds (ADR 0017 §4).
const AI_DEFAULT_TIMEOUT_SECS: u64 = 180;

/// The serde default for a provider's `timeout-secs`.
const fn ai_default_timeout_secs() -> u64 {
    AI_DEFAULT_TIMEOUT_SECS
}

/// A configured AI provider (ADR 0017 §4), tagged by `kind`.
///
/// Providers live in **client/presentation scope** (ADR 0015): the inventory is a property of this
/// machine and user (a CLI on `PATH`, an env-var credential), not of the dataset — a collaborator
/// opening the same workspace need not have the same providers installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AiProvider {
    /// A local command invoked with an explicit argv vector — **no shell, ever**. `{prompt}` and
    /// `{media}` are substituted as whole argv elements, so plugin-authored prompt text cannot inject
    /// arguments or shell syntax.
    #[serde(rename_all = "kebab-case")]
    Command {
        /// The executable to run (found on `PATH`, or an absolute path).
        command: String,
        /// The argument template; each element's `{prompt}`/`{media}` placeholders are substituted.
        #[serde(default)]
        args: Vec<String>,
        /// The per-request timeout in seconds (default 180).
        #[serde(default = "ai_default_timeout_secs")]
        timeout_secs: u64,
    },
    /// An OpenAI-compatible chat-completions vision endpoint.
    #[serde(rename_all = "kebab-case")]
    VisionApi {
        /// The API base URL; the host POSTs to `{url}/chat/completions` (HTTPS only).
        url: String,
        /// The model name sent in the request body.
        model: String,
        /// The **name** of the environment variable holding the API key — the key itself never lives
        /// in config or logs (ADR 0017 §4).
        api_key_env: String,
        /// The per-request timeout in seconds (default 180).
        #[serde(default = "ai_default_timeout_secs")]
        timeout_secs: u64,
    },
    /// Reserved: a provider implemented as a WASM plugin (an `ai-provider` world, ADR 0017 §4). Named
    /// but **not yet supported** — the section parses so a config round-trips, but resolving one for
    /// use is a clear error.
    Plugin,
}

/// The `[ai]` configuration section (ADR 0017 §4): the default provider name and the named-provider
/// inventory. Client/presentation scope (ADR 0015 §1) — machine/user-local, not shipped with data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiConfig {
    /// The provider used when a caller names none; must be a key in [`Self::providers`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// The configured providers, keyed by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, AiProvider>,
}

impl AiConfig {
    /// Whether this section carries nothing (no default, no providers) — lets an empty `[ai]` table
    /// be omitted when serializing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.default.is_none() && self.providers.is_empty()
    }

    /// Resolves the provider a caller asked for: `name` when given, else [`Self::default`].
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the named (or default) provider is not configured, or `name` is `None`
    /// and no default is set. The message names the requested provider.
    pub fn resolve(&self, name: Option<&str>) -> Result<&AiProvider, AppError> {
        let key = match name {
            Some(name) => name,
            None => self.default.as_deref().ok_or_else(|| {
                AppError::Config("no AI provider was requested and no [ai].default is configured".to_owned())
            })?,
        };
        self.providers
            .get(key)
            .ok_or_else(|| AppError::Config(format!("unknown AI provider {key:?} (not in [ai.providers])")))
    }
}

/// A configured map tile/style provider (ADR 0025 §3), tagged by `kind`.
///
/// Client/presentation scope (ADR 0015): the provider is a per-client rendering choice, not part of
/// the dataset — a collaborator opening the same workspace can point at a different tile source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum MapProvider {
    /// A raster XYZ tile source (OpenStreetMap or a compatible raster host).
    #[serde(rename_all = "kebab-case")]
    OsmRaster {
        /// The `{z}/{x}/{y}` tile URL template.
        tile_url: String,
        /// The attribution string shown on the map, per the provider's terms.
        attribution: String,
    },
    /// A `MapLibre` GL JS vector style (a full `style.json` URL).
    #[serde(rename_all = "kebab-case")]
    MaplibreStyle {
        /// The style URL.
        style_url: String,
        /// The attribution string shown on the map, per the provider's terms.
        attribution: String,
        /// The **name** of the environment variable holding the style's API key, if it needs one —
        /// the key itself never lives in config or logs (mirrors [`AiProvider::VisionApi`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        api_key_env: Option<String>,
    },
    /// A paid Google Maps style.
    #[serde(rename_all = "kebab-case")]
    Google {
        /// The **name** of the environment variable holding the API key.
        api_key_env: String,
        /// The attribution string shown on the map, per the provider's terms.
        attribution: String,
    },
}

impl MapProvider {
    /// The built-in default when no `[map]` section is configured: OpenStreetMap raster tiles (the
    /// Phase 6 map MVP's provider), attributed per OSM's tile-usage policy. No API key, no `net`
    /// allowlist needed — `tile.openstreetmap.org` is the one host the MVP already fetches from.
    #[must_use]
    pub fn default_osm() -> Self {
        Self::OsmRaster {
            tile_url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_owned(),
            attribution: "© OpenStreetMap contributors".to_owned(),
        }
    }

    /// The credit this provider's terms require the map to display. Mandatory on every variant, so
    /// this is infallible — a frontend rendering the map has no case where it may show nothing.
    #[must_use]
    pub fn attribution(&self) -> &str {
        match self {
            Self::OsmRaster { attribution, .. }
            | Self::MaplibreStyle { attribution, .. }
            | Self::Google { attribution, .. } => attribution,
        }
    }

    /// The `{z}/{x}/{y}` template a `raster` tile source can fetch, or `None` for a provider served
    /// some other way — a vector style is a whole `style.json` and the Google adapter does not exist
    /// yet (ADR 0025 §4), so neither has a tile URL a raster source could use.
    #[must_use]
    pub fn raster_tile_url(&self) -> Option<&str> {
        match self {
            Self::OsmRaster { tile_url, .. } => Some(tile_url),
            Self::MaplibreStyle { .. } | Self::Google { .. } => None,
        }
    }
}

/// The reserved [`MapConfig::choices`]/[`MapConfig::resolve`] key naming the built-in OSM default — no
/// configured provider may use it as a name (the toolbar select's value for that choice is this empty
/// string).
pub const BUILT_IN_MAP_PROVIDER: &str = "";

/// The `[map]` configuration section (ADR 0025 §3 / ADR 0033): the named map provider inventory, the
/// active choice, and the outbound-host allowlist (the `net` capability boundary, ADR 0007 §2 /
/// 0011 §3). Client/presentation scope (ADR 0015 §1) — machine/user-local, not shipped with data.
/// Named providers mirror [`AiConfig`]'s shape so a provider's parameters survive even while it is not
/// the active one (switching away and back must not lose a style URL).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapConfig {
    /// The active provider's name, a key in [`Self::providers`]; `None` resolves to the built-in OSM
    /// default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// The configured providers, keyed by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, MapProvider>,
    /// Outbound hosts the map is allowed to fetch tiles/styles from; empty defers entirely to the
    /// provider's own host (no additional allowlisting needed for the built-in OSM default).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub net_allowlist: Vec<String>,
}

impl MapConfig {
    /// Whether this section carries nothing (no active provider, no configured providers, no
    /// allowlist) — lets an empty `[map]` table be omitted when serializing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.provider.is_none() && self.providers.is_empty() && self.net_allowlist.is_empty()
    }

    /// Resolves the provider a caller asked for: `name` when given, else [`Self::provider`] (the
    /// active choice), else the built-in default. [`BUILT_IN_MAP_PROVIDER`] (the empty string) always
    /// resolves to [`MapProvider::default_osm`], whether it arrives as `name` or as `Self::provider`.
    ///
    /// # Errors
    ///
    /// [`AppError::Config`] if the resolved key is non-empty and not in [`Self::providers`]. The
    /// message names the requested provider.
    pub fn resolve(&self, name: Option<&str>) -> Result<MapProvider, AppError> {
        let key = name.or(self.provider.as_deref()).unwrap_or(BUILT_IN_MAP_PROVIDER);
        if key == BUILT_IN_MAP_PROVIDER {
            return Ok(MapProvider::default_osm());
        }
        self.providers
            .get(key)
            .cloned()
            .ok_or_else(|| AppError::Config(format!("unknown map provider {key:?} (not in [map.providers])")))
    }

    /// Every provider a caller can pick from: the built-in default first (keyed
    /// [`BUILT_IN_MAP_PROVIDER`]), then the configured providers in name order — exactly what the
    /// toolbar select renders, so it can never offer a choice [`Self::resolve`] would reject.
    #[must_use]
    pub fn choices(&self) -> Vec<(String, MapProvider)> {
        let mut choices = vec![(BUILT_IN_MAP_PROVIDER.to_owned(), MapProvider::default_osm())];
        choices.extend(
            self.providers
                .iter()
                .map(|(name, provider)| (name.clone(), provider.clone())),
        );
        choices
    }
}

/// The `[plugin_trust]` configuration section (ADR 0014 §3): the publisher public keys this user has
/// pinned as trusted. Client/presentation scope (ADR 0015 §1) — a per-user trust decision,
/// machine/user-local, not shipped with a dataset (mirrors [`AiConfig`]/[`MapConfig`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginTrustConfig {
    /// Pinned publishers, mapping a publisher identity to its 64-hex-character ed25519 public key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub publishers: BTreeMap<String, String>,
}

impl PluginTrustConfig {
    /// Whether no publisher is pinned — lets an empty `[plugin_trust]` table be omitted when
    /// serializing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.publishers.is_empty()
    }
}

/// The `[shortcuts]` configuration section (ADR 0030 §3): the operator's rebound keyboard shortcuts,
/// each a `vitni_ui::ShortcutAction::config_id` mapped to a canonical chord string. Lives in the
/// global config only — there is no per-workspace layer. Client/
/// presentation scope (ADR 0015 §1) — machine/user-local, not shipped with data (mirrors
/// [`AiConfig`]/[`MapConfig`]/[`PluginTrustConfig`]).
///
/// `vitni-app` must not depend on `vitni-ui` (ADR 0008: app → ui is the fixed dependency
/// direction), so bindings are stored as plain strings, untouched; all chord parsing, validation, and
/// conflict detection lives in `vitni_ui::resolved_shortcuts`, which the renderer calls with this
/// map.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutConfig {
    /// The rebound chords, keyed by the action's config id (e.g. `"quit"` → `"mod+q"`), inline under
    /// the `[shortcuts]` table (`quit = "mod+q"`) rather than a nested `[shortcuts.bindings]`.
    #[serde(flatten)]
    pub bindings: BTreeMap<String, String>,
}

impl ShortcutConfig {
    /// Whether no shortcut is rebound — lets an empty `[shortcuts]` table be omitted when
    /// serializing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

/// Decodes a 64-hex-character ed25519 public key into its 32 raw bytes, or `None` for a bad length
/// or a non-hex character. The plugin host turns these bytes into verifying keys (`ed25519-dalek` is
/// its dependency, not this crate's).
fn decode_public_key_hex(hex: &str) -> Option<[u8; 32]> {
    let raw = hex.as_bytes();
    if raw.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, slot) in bytes.iter_mut().enumerate() {
        let high = hex_nibble(raw[2 * index])?;
        let low = hex_nibble(raw[2 * index + 1])?;
        *slot = (high << 4) | low;
    }
    Some(bytes)
}

/// Decodes one hex digit to its nibble value, or `None` for a non-hex byte.
const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Decodes the pinned publisher keys into `(publisher, 32-byte public key)` pairs for the plugin
/// host's trust roots (ADR 0014 §3). The host builds the `TrustRoots` (embedded + these pins); this
/// helper only validates and decodes so the app layer needs no crypto dependency.
///
/// # Errors
///
/// [`AppError::Config`] naming the offending publisher if any pinned key is not exactly 64 hex
/// characters.
pub fn resolve_trust_pins(trust: &PluginTrustConfig) -> Result<Vec<(String, [u8; 32])>, AppError> {
    let mut pins = Vec::with_capacity(trust.publishers.len());
    for (publisher, hex) in &trust.publishers {
        let bytes = decode_public_key_hex(hex).ok_or_else(|| {
            AppError::Config(format!(
                "pinned publisher {publisher:?} has an invalid public key (expected 64 hex characters)"
            ))
        })?;
        pins.push((publisher.clone(), bytes));
    }
    Ok(pins)
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
    /// The AI providers for assisted import (ADR 0017 §4); client/presentation scope,
    /// machine/user-local.
    #[serde(default, skip_serializing_if = "AiConfig::is_empty")]
    pub ai: AiConfig,
    /// The geography view's map provider (ADR 0025 §3); client/presentation scope, machine/user-local.
    #[serde(default, skip_serializing_if = "MapConfig::is_empty")]
    pub map: MapConfig,
    /// The pinned-publisher trust store (ADR 0014 §3); client/presentation scope, machine/user-local.
    #[serde(default, skip_serializing_if = "PluginTrustConfig::is_empty")]
    pub plugin_trust: PluginTrustConfig,
    /// The rebound keyboard shortcuts (ADR 0030 §3); client/presentation scope, machine/user-local.
    #[serde(default, skip_serializing_if = "ShortcutConfig::is_empty")]
    pub shortcuts: ShortcutConfig,
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
                AppError::Config("no workspace given and no default set (run `vitni init <name> <path>`)".to_owned())
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

/// The global config file path, e.g. `~/.config/vitni/config.toml` (ADR 0005).
///
/// # Errors
///
/// [`AppError::Config`] if no home directory can be determined.
pub fn config_path() -> Result<PathBuf, AppError> {
    Ok(project_dirs()?.config_dir().join("config.toml"))
}

/// The default directory for a workspace named `name`, e.g.
/// `~/.local/share/vitni/workspaces/<name>` (ADR 0005).
///
/// # Errors
///
/// [`AppError::Config`] if no home directory can be determined.
pub fn default_workspace_dir(name: &str) -> Result<PathBuf, AppError> {
    Ok(project_dirs()?.data_dir().join("workspaces").join(name))
}

/// The shared application directory holding runtime localization overrides, e.g.
/// `~/.local/share/vitni/i18n` (ADR 0003 — the shared-app-dir override layer).
///
/// # Errors
///
/// [`AppError::Config`] if no home directory can be determined.
pub fn shared_i18n_dir() -> Result<PathBuf, AppError> {
    Ok(project_dirs()?.data_dir().join("i18n"))
}

/// The shared application directory holding user-installed plugin bundles, e.g.
/// `~/.local/share/vitni/plugins` (ADR 0014 §4 — the app-dir loading layer). Parallel to
/// [`shared_i18n_dir`].
///
/// # Errors
///
/// [`AppError::Config`] if no home directory can be determined.
pub fn shared_plugins_dir() -> Result<PathBuf, AppError> {
    Ok(project_dirs()?.data_dir().join("plugins"))
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
        .map_err(|e| AppError::Config(format!("reading {} (run `vitni init`?): {e}", path.display())))?;
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
        ai: AiConfig::default(),
        map: MapConfig::default(),
        plugin_trust: PluginTrustConfig::default(),
        shortcuts: ShortcutConfig::default(),
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

/// Persists the operator's display name and email into the global config's `[operator]` table
/// (read-modify-write, preserving the workspace registry / defaults). The operator `id` is stable
/// and never changes here (ADR 0005).
///
/// # Errors
///
/// [`AppError::Config`] if the config cannot be read or written.
pub fn set_operator_identity(path: &Path, display: Option<String>, email: Option<String>) -> Result<(), AppError> {
    let mut config = load(path)?;
    config.operator.display = display;
    config.operator.email = email;
    save(path, &config)
}

/// Persists the live-fallback `HumanId` formats into the global config's
/// `[workspace-defaults.id_formats]` table (read-modify-write, preserving the rest).
///
/// # Errors
///
/// [`AppError::Config`] if the config cannot be read or written.
pub fn set_workspace_default_id_formats(path: &Path, id_formats: IdFormats) -> Result<(), AppError> {
    let mut config = load(path)?;
    config.workspace_defaults.id_formats = id_formats;
    save(path, &config)
}

/// Persists the live-fallback language/locale/date/number defaults into the global config's
/// `[workspace-defaults.locale]` table (read-modify-write, preserving the rest).
///
/// # Errors
///
/// [`AppError::Config`] if the config cannot be read or written.
pub fn set_workspace_default_locale(path: &Path, locale: LocaleDefaults) -> Result<(), AppError> {
    let mut config = load(path)?;
    config.workspace_defaults.locale = locale;
    save(path, &config)
}

/// Persists the live-fallback surety-scheme label overrides into the global config's
/// `[workspace-defaults.surety]` table (read-modify-write, preserving the rest).
///
/// # Errors
///
/// [`AppError::Config`] if the config cannot be read or written.
pub fn set_workspace_default_surety(path: &Path, surety: SuretyLabelOverrides) -> Result<(), AppError> {
    let mut config = load(path)?;
    config.workspace_defaults.surety = surety;
    save(path, &config)
}

/// Persists the `[ai]` provider config into the global config (read-modify-write, preserving the
/// rest). Client/presentation scope (ADR 0015 §1) — the provider inventory is machine/user-local.
///
/// # Errors
///
/// [`AppError::Config`] if the config cannot be read or written.
pub fn set_ai(path: &Path, ai: AiConfig) -> Result<(), AppError> {
    let mut config = load(path)?;
    config.ai = ai;
    save(path, &config)
}

/// Persists the `[map]` provider config into the global config (read-modify-write, preserving the
/// rest). Client/presentation scope (ADR 0015 §1) — the provider choice is machine/user-local.
///
/// # Errors
///
/// [`AppError::Config`] if the config cannot be read or written.
pub fn set_map(path: &Path, map: MapConfig) -> Result<(), AppError> {
    let mut config = load(path)?;
    config.map = map;
    save(path, &config)
}

/// Persists the `[plugin_trust]` pinned-publisher store into the global config (read-modify-write,
/// preserving the rest). Client/presentation scope (ADR 0015 §1) — the trust decision is
/// machine/user-local.
///
/// # Errors
///
/// [`AppError::Config`] if the config cannot be read or written.
pub fn set_plugin_trust(path: &Path, plugin_trust: PluginTrustConfig) -> Result<(), AppError> {
    let mut config = load(path)?;
    config.plugin_trust = plugin_trust;
    save(path, &config)
}

/// Persists the `[shortcuts]` rebound-chord map into the global config (read-modify-write,
/// preserving the rest). Client/presentation scope (ADR 0015 §1) — a keymap is machine/user-local.
///
/// # Errors
///
/// [`AppError::Config`] if the config cannot be read or written.
pub fn set_shortcuts(path: &Path, shortcuts: ShortcutConfig) -> Result<(), AppError> {
    let mut config = load(path)?;
    config.shortcuts = shortcuts;
    save(path, &config)
}

/// Pins `publisher`'s ed25519 public key (64 hex characters) into the client-scope `[plugin_trust]`
/// store (ADR 0014 §3), bootstrapping the config if it does not exist yet (read-modify-write,
/// preserving the rest). Re-pinning a publisher replaces its key.
///
/// # Errors
///
/// [`AppError::Config`] if `public_key_hex` is not exactly 64 hex characters, or the config cannot be
/// read or written.
pub fn add_trusted_publisher(path: &Path, publisher: &str, public_key_hex: &str) -> Result<(), AppError> {
    let normalized = public_key_hex.trim().to_lowercase();
    if decode_public_key_hex(&normalized).is_none() {
        return Err(AppError::Config(format!(
            "publisher {publisher:?} has an invalid public key (expected 64 hex characters)"
        )));
    }
    let mut config = load_or_bootstrap(path)?;
    config.plugin_trust.publishers.insert(publisher.to_owned(), normalized);
    save(path, &config)
}

/// Removes `publisher` from the client-scope `[plugin_trust]` store (ADR 0014 §3), preserving the
/// rest (read-modify-write).
///
/// # Errors
///
/// [`AppError::Config`] if `publisher` is not pinned, or the config cannot be read or written.
pub fn remove_trusted_publisher(path: &Path, publisher: &str) -> Result<(), AppError> {
    let mut config = load(path)?;
    if config.plugin_trust.publishers.remove(publisher).is_none() {
        return Err(AppError::Config(format!("publisher {publisher:?} is not pinned")));
    }
    save(path, &config)
}

/// Switches the default (last-used) workspace by name, persisting the change
/// (read-modify-write, preserving the rest). The operator is unaffected — it is app-level, not
/// per-workspace (ADR 0005).
///
/// # Errors
///
/// [`AppError::Config`] if the config cannot be read/written, or `name` is not a registered
/// workspace.
pub fn set_default_workspace(path: &Path, name: &str) -> Result<(), AppError> {
    let mut config = load(path)?;
    if !config.workspaces.contains_key(name) {
        return Err(AppError::Config(format!(
            "unknown workspace {name:?} (not in the registry)"
        )));
    }
    config.default = Some(name.to_owned());
    save(path, &config)
}

#[cfg(test)]
mod tests {
    use super::{
        Config, DateFormat, Engine, IdFormats, LocaleDefaults, MapConfig, MapProvider, NumberFormat,
        SuretyLabelOverride, SuretyLabelOverrides, ThemeMode, add_trusted_publisher, load, load_or_bootstrap,
        remove_trusted_publisher, save, set_default_workspace, set_operator_identity, set_workspace_default_id_formats,
        set_workspace_default_locale, set_workspace_default_surety,
    };
    use crate::error::AppError;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use vitni_core::provenance::Confidence;

    fn config_at(path: &Path) -> Config {
        load_or_bootstrap(path).expect("bootstrap")
    }

    /// A vector style, for the two accessor tests below: neither cares about the URL itself, only that
    /// a non-raster provider still names its credit and names no raster tile URL.
    fn maplibre_style() -> MapProvider {
        MapProvider::MaplibreStyle {
            style_url: "https://tiles.example/style.json".to_owned(),
            attribution: "© Example".to_owned(),
            api_key_env: None,
        }
    }

    /// The same for the Google variant, which has no tile adapter yet (ADR 0025 §4).
    fn google() -> MapProvider {
        MapProvider::Google {
            api_key_env: "GOOGLE_MAPS_KEY".to_owned(),
            attribution: "© Google".to_owned(),
        }
    }

    /// Every provider's terms require its credit to be displayed, which is why `attribution` is
    /// mandatory on all three variants — so reading it back is infallible, with no per-kind branch for
    /// a caller to get wrong.
    #[test]
    fn every_provider_kind_reports_the_credit_its_terms_require() {
        assert_eq!(MapProvider::default_osm().attribution(), "© OpenStreetMap contributors");
        assert_eq!(maplibre_style().attribution(), "© Example");
        assert_eq!(google().attribution(), "© Google");
    }

    /// Only a raster provider names a URL a `raster` tile source can fetch. A vector style URL is a
    /// whole `style.json` and Google needs an adapter that does not exist yet — reporting either here
    /// would put a non-tile URL where the map interpolates `{z}/{x}/{y}`.
    #[test]
    fn only_a_raster_provider_names_a_tile_url_the_map_can_fetch() {
        assert_eq!(
            MapProvider::default_osm().raster_tile_url(),
            Some("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
        );
        assert_eq!(maplibre_style().raster_tile_url(), None);
        assert_eq!(google().raster_tile_url(), None);
    }

    #[test]
    fn an_unconfigured_map_resolves_none_and_the_built_in_key_to_the_osm_default() {
        let map = MapConfig::default();
        assert_eq!(map.resolve(None).expect("resolve"), MapProvider::default_osm());
        assert_eq!(map.resolve(Some("")).expect("resolve"), MapProvider::default_osm());
    }

    #[test]
    fn resolving_an_unknown_provider_name_is_a_named_config_error() {
        let map = MapConfig::default();
        let error = map.resolve(Some("bogus")).expect_err("bogus is not configured");
        let AppError::Config(message) = error else {
            panic!("expected AppError::Config, got {error:?}");
        };
        assert!(
            message.contains("bogus"),
            "the error names the requested provider: {message}"
        );
    }

    #[test]
    fn none_resolves_the_active_configured_provider() {
        let mut providers = BTreeMap::new();
        providers.insert("carto".to_owned(), maplibre_style());
        let map = MapConfig {
            provider: Some("carto".to_owned()),
            providers,
            net_allowlist: Vec::new(),
        };
        assert_eq!(map.resolve(None).expect("resolve"), maplibre_style());
        // Naming a provider explicitly still overrides the active one.
        assert_eq!(map.resolve(Some("")).expect("resolve"), MapProvider::default_osm());
    }

    #[test]
    fn choices_always_offers_the_built_in_default_first_then_configured_providers_in_order() {
        let mut providers = BTreeMap::new();
        providers.insert("zzz-custom".to_owned(), google());
        providers.insert("carto".to_owned(), maplibre_style());
        let map = MapConfig {
            provider: None,
            providers,
            net_allowlist: Vec::new(),
        };
        assert_eq!(
            map.choices(),
            vec![
                (String::new(), MapProvider::default_osm()),
                ("carto".to_owned(), maplibre_style()),
                ("zzz-custom".to_owned(), google()),
            ]
        );
    }

    #[test]
    fn an_empty_map_config_has_no_provider_and_no_providers() {
        assert!(MapConfig::default().is_empty());
        let mut providers = BTreeMap::new();
        providers.insert("carto".to_owned(), maplibre_style());
        assert!(
            !MapConfig {
                provider: None,
                providers,
                net_allowlist: Vec::new(),
            }
            .is_empty()
        );
    }

    #[test]
    fn pin_then_unpin_a_publisher_round_trips_through_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let key = "a".repeat(64);
        add_trusted_publisher(&path, "acme", &key).expect("pin");
        assert_eq!(
            load(&path).expect("load").plugin_trust.publishers.get("acme"),
            Some(&key),
            "the pinned key persists"
        );
        remove_trusted_publisher(&path, "acme").expect("unpin");
        assert!(
            load(&path).expect("load").plugin_trust.publishers.is_empty(),
            "unpinning drops the entry"
        );
    }

    #[test]
    fn pinning_rejects_a_malformed_key_and_unpinning_an_absent_publisher_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        assert!(
            add_trusted_publisher(&path, "acme", "not-hex").is_err(),
            "a non-64-hex key is rejected"
        );
        config_at(&path);
        assert!(
            remove_trusted_publisher(&path, "ghost").is_err(),
            "removing an unpinned publisher is an error"
        );
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
path = "/home/user/gen"

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
            PathBuf::from("/home/user/gen")
        );
        assert_eq!(config.defaults.engine, Engine::Sqlite);
        assert_eq!(config.workspace_defaults.id_formats.person, "I%04d");
    }

    #[test]
    fn defaults_round_trip_through_save_and_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let mut config = config_at(&path);
        config.defaults.engine = Engine::Postgres;
        config.defaults.database_url = Some("postgres://localhost/vitni".to_owned());
        config.workspace_defaults.id_formats.person = "P-%05d".to_owned();
        config.workspace_defaults.ui.theme = ThemeMode::Light;
        config.operator.email = Some("ada@example.com".to_owned());
        save(&path, &config).expect("save");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.defaults.engine, Engine::Postgres);
        assert_eq!(
            loaded.defaults.database_url.as_deref(),
            Some("postgres://localhost/vitni")
        );
        assert_eq!(loaded.workspace_defaults.id_formats.person, "P-%05d");
        assert_eq!(loaded.workspace_defaults.ui.theme, ThemeMode::Light);
        assert_eq!(loaded.operator.email.as_deref(), Some("ada@example.com"));
    }

    #[test]
    fn ui_theme_defaults_to_system_and_is_omitted_when_absent() {
        // A config without a [workspace-defaults.ui] table parses to the System default.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let config = config_at(&path);
        assert_eq!(config.workspace_defaults.ui.theme, ThemeMode::System);
    }

    #[test]
    fn set_operator_identity_updates_display_and_email_and_preserves_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let mut config = config_at(&path);
        config.register_workspace("gen".to_owned(), PathBuf::from("/data/gen"));
        save(&path, &config).expect("save registry");

        set_operator_identity(
            &path,
            Some("Ada Lovelace".to_owned()),
            Some("ada@example.com".to_owned()),
        )
        .expect("set identity");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.operator.display.as_deref(), Some("Ada Lovelace"));
        assert_eq!(loaded.operator.email.as_deref(), Some("ada@example.com"));
        assert_eq!(loaded.operator.id, config.operator.id, "the operator id never changes");
        assert!(
            loaded.workspaces.contains_key("gen"),
            "the registry survives the read-modify-write"
        );
    }

    #[test]
    fn set_workspace_default_id_formats_round_trips_and_preserves_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let config = config_at(&path);
        let formats = IdFormats {
            person: "P-%05d".to_owned(),
            ..Default::default()
        };

        set_workspace_default_id_formats(&path, formats).expect("set formats");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.workspace_defaults.id_formats.person, "P-%05d");
        assert_eq!(
            loaded.operator.id, config.operator.id,
            "the operator survives the read-modify-write"
        );
    }

    #[test]
    fn surety_label_overrides_default_to_no_override_for_every_ordinal() {
        // No workspace has configured anything yet: every ordinal falls back to the frontend's own
        // Fluent-resolved default (ADR 0027 §2) — `label_for` reports no override at all.
        let overrides = SuretyLabelOverrides::default();
        assert_eq!(overrides.label_for(Confidence::VeryLow), None);
        assert_eq!(overrides.label_for(Confidence::Low), None);
        assert_eq!(overrides.label_for(Confidence::Normal), None);
        assert_eq!(overrides.label_for(Confidence::High), None);
        assert_eq!(overrides.label_for(Confidence::VeryHigh), None);
    }

    #[test]
    fn label_for_resolves_the_matching_ordinal_only() {
        let overrides = SuretyLabelOverrides {
            normal: Some(SuretyLabelOverride {
                label: "Balanced".to_owned(),
                description: Some("Neither confirms nor casts doubt".to_owned()),
            }),
            ..Default::default()
        };
        assert_eq!(
            overrides.label_for(Confidence::Normal).map(|o| o.label.as_str()),
            Some("Balanced")
        );
        assert_eq!(overrides.label_for(Confidence::High), None, "other ordinals stay unset");
    }

    #[test]
    fn set_workspace_default_surety_round_trips_and_preserves_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let config = config_at(&path);
        let surety = SuretyLabelOverrides {
            very_low: Some(SuretyLabelOverride {
                label: "Hearsay".to_owned(),
                description: None,
            }),
            very_high: Some(SuretyLabelOverride {
                label: "Certain".to_owned(),
                description: Some("Primary source, direct evidence".to_owned()),
            }),
            ..Default::default()
        };

        set_workspace_default_surety(&path, surety.clone()).expect("set surety");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.workspace_defaults.surety, surety);
        assert_eq!(
            loaded.operator.id, config.operator.id,
            "the operator survives the read-modify-write"
        );
    }

    #[test]
    fn set_workspace_default_locale_round_trips_and_preserves_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        config_at(&path);
        let locale = LocaleDefaults {
            ui_language: Some("nb-NO".parse().expect("langid")),
            data_locale: Some("nb-NO".parse().expect("langid")),
            date_format: DateFormat::Numeric,
            number_format: NumberFormat::CommaPoint,
        };

        set_workspace_default_locale(&path, locale.clone()).expect("set locale");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.workspace_defaults.locale, locale);
    }

    #[test]
    fn set_default_workspace_switches_the_default_and_rejects_unknown_names() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let mut config = config_at(&path);
        config.register_workspace("gen".to_owned(), PathBuf::from("/data/gen"));
        config.register_workspace("tree2".to_owned(), PathBuf::from("/data/tree2"));
        save(&path, &config).expect("save registry");

        // `register_workspace` last made "tree2" the default; switch back to "gen".
        set_default_workspace(&path, "gen").expect("switch");
        let loaded = load(&path).expect("load");
        assert_eq!(loaded.default.as_deref(), Some("gen"));

        let err = set_default_workspace(&path, "nope");
        assert!(err.is_err(), "an unregistered workspace name is rejected");
    }

    use super::{AiConfig, AiProvider, set_ai};

    const AI_TOML: &str = r#"
[operator]
id = "019ed99c-6bde-73c2-a71a-05934c744a49"
display = "Magne Rasmussen"

[defaults]
engine = "sqlite"

[ai]
default = "gemini"

[ai.providers.gemini]
kind = "command"
command = "gemini"
args = ["-p", "{prompt}", "{media}"]
timeout-secs = 120

[ai.providers.vision]
kind = "vision-api"
url = "https://api.example.com/v1"
model = "some-vision-model"
api-key-env = "EXAMPLE_API_KEY"
"#;

    #[test]
    fn ai_section_parses_both_provider_kinds_with_kebab_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, AI_TOML).expect("write");
        let config = load(&path).expect("parse");

        assert_eq!(config.ai.default.as_deref(), Some("gemini"));
        match config.ai.providers.get("gemini").expect("gemini provider") {
            AiProvider::Command {
                command,
                args,
                timeout_secs,
            } => {
                assert_eq!(command, "gemini");
                assert_eq!(
                    args,
                    &vec!["-p".to_owned(), "{prompt}".to_owned(), "{media}".to_owned()]
                );
                assert_eq!(*timeout_secs, 120, "the kebab-case `timeout-secs` key is read");
            }
            other => panic!("expected a command provider, got {other:?}"),
        }
        match config.ai.providers.get("vision").expect("vision provider") {
            AiProvider::VisionApi {
                url,
                model,
                api_key_env,
                timeout_secs,
            } => {
                assert_eq!(url, "https://api.example.com/v1");
                assert_eq!(model, "some-vision-model");
                assert_eq!(api_key_env, "EXAMPLE_API_KEY", "`api-key-env` is the env var name");
                assert_eq!(*timeout_secs, 180, "an omitted timeout falls back to the 180s default");
            }
            other => panic!("expected a vision-api provider, got {other:?}"),
        }
    }

    #[test]
    fn ai_section_round_trips_through_save_and_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, AI_TOML).expect("write");
        let config = load(&path).expect("parse");
        save(&path, &config).expect("save");
        let reloaded = load(&path).expect("reload");
        assert_eq!(
            config.ai, reloaded.ai,
            "the [ai] section survives a save/load round-trip"
        );
    }

    #[test]
    fn an_empty_ai_section_is_omitted_when_serializing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let config = config_at(&path);
        assert!(config.ai.is_empty());
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(!text.contains("[ai]"), "a default (empty) [ai] table is not written");
    }

    #[test]
    fn an_empty_shortcuts_section_is_omitted_when_serializing() {
        use super::ShortcutConfig;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let config = config_at(&path);
        assert!(config.shortcuts.is_empty());
        assert_eq!(config.shortcuts, ShortcutConfig::default());
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(
            !text.contains("[shortcuts]"),
            "a default (empty) [shortcuts] table is not written"
        );
    }

    #[test]
    fn an_unknown_provider_kind_is_a_parse_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[operator]
id = "019ed99c-6bde-73c2-a71a-05934c744a49"

[ai.providers.bogus]
kind = "sorcery"
"#,
        )
        .expect("write");
        assert!(load(&path).is_err(), "an unrecognized provider kind fails to parse");
    }

    #[test]
    fn resolve_finds_named_and_default_and_rejects_unknown() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, AI_TOML).expect("write");
        let ai = load(&path).expect("parse").ai;

        assert!(matches!(ai.resolve(Some("vision")), Ok(AiProvider::VisionApi { .. })));
        // `None` resolves the configured default (`gemini`).
        assert!(matches!(ai.resolve(None), Ok(AiProvider::Command { .. })));
        assert!(ai.resolve(Some("missing")).is_err(), "an unknown name is rejected");
    }

    #[test]
    fn resolve_without_a_default_is_an_error() {
        let ai = AiConfig::default();
        assert!(ai.resolve(None).is_err(), "no providers, no default → error");
    }

    #[test]
    fn the_reserved_plugin_kind_parses_but_is_not_a_runnable_kind() {
        // `kind = "plugin"` is reserved (ADR 0017 §4): it parses so a config round-trips, and the
        // host reports "not yet supported" only when a plugin actually tries to use it.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[operator]
id = "019ed99c-6bde-73c2-a71a-05934c744a49"

[ai]
default = "future"

[ai.providers.future]
kind = "plugin"
"#,
        )
        .expect("write");
        let ai = load(&path).expect("parse").ai;
        assert_eq!(ai.resolve(None).expect("resolves the entry"), &AiProvider::Plugin);
    }

    #[test]
    fn set_ai_round_trips_and_preserves_the_rest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let config = config_at(&path);
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

        set_ai(&path, ai.clone()).expect("set ai");
        let loaded = load(&path).expect("load");
        assert_eq!(loaded.ai, ai);
        assert_eq!(
            loaded.operator.id, config.operator.id,
            "the operator survives the read-modify-write"
        );
    }
}
