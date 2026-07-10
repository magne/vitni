//! Plugin discovery (PR21): listing the components in a directory and reading their genuinely
//! declared metadata straight off the compiled component — no invented or hand-maintained manifest.
//!
//! **What is (and is not) readable today.** ADR 0007 §8 names a bundle-metadata format carrying a
//! plugin's own stable id, its own semver version, and its declared capability *requests* — but that
//! format is not implemented; it is deferred to ADR 0014 (bundles/signing/trust tiers), which this PR
//! explicitly does not build. Until then:
//!
//! - **id** is the component file's stem (the same convention [`crate::PluginHost::load_by_id`]
//!   already uses to find a component by id) — genuine, but a filesystem convention, not a
//!   self-declared identity.
//! - **`host_api_version`** is the `genealogy:host-api@X.Y.Z` version embedded in the component's
//!   *imported* WIT interface names (e.g. `genealogy:host-api/log@0.13.0`). This is the host-API
//!   package version the plugin was compiled against (ADR 0007 §2) — **not** a plugin-owned semver
//!   (no such field exists on the component).
//! - **`role`** is inferred from which entry point the component *exports* (`run-import` →
//!   [`PluginRole::BulkImport`], etc.) — genuine, derived from the WIT world it implements.
//! - **`capabilities`** are the [`Capability`] variants whose WIT interface
//!   (`genealogy:host-api/<name>`) the component *imports* — genuine, read via
//!   [`wasmtime::component::types::Component::imports`], not guessed or hand-maintained per plugin.
//!
//! This is honestly a **declared-by-the-component-itself** capability list, not a capability the user
//! has granted (that remains a separate [`Grants`] the frontend builds — deny-by-default, ADR 0011
//! §2) and not a verified/signed manifest (ADR 0007 §9, deferred).

use std::path::Path;

use wasmtime::Engine;
use wasmtime::component::Component;

use crate::PluginHost;
use crate::capability::Capability;
use crate::error::PluginError;

/// The plugin role a component implements, inferred from the entry point(s) it exports (ADR 0011
/// §1's per-role worlds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginRole {
    /// Exports `run-import` (the `bulk-import` world, ADR 0013).
    BulkImport,
    /// Exports `run-export` (the `bulk-export` world, ADR 0013).
    BulkExport,
    /// Exports `run-ui-panel` (the `ui-panel` world, ADR 0012).
    UiPanel,
    /// Exports the test-only `fixture` world's entry points (`try-create`/`busy-loop`/`allocate`).
    TestFixture,
    /// Exports none of the known entry points — a component the host has no role for.
    Unknown,
}

/// A plugin component's genuinely declared metadata, read off the compiled `.wasm` itself.
///
/// See the module docs for exactly what is (and is not) self-declared today versus deferred to
/// ADR 0014.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    /// The plugin's id — the component file's stem (e.g. `gedcom-import`).
    pub id: String,
    /// The role inferred from its exported entry point.
    pub role: PluginRole,
    /// The `genealogy:host-api` package version this component was compiled against.
    pub host_api_version: String,
    /// The capabilities this component's world imports (declared, not yet granted).
    pub capabilities: Vec<Capability>,
}

/// Maps a WIT interface import name (`genealogy:host-api/<name>@<version>`) to the [`Capability`] it
/// represents, and — for the `log` interface, arbitrarily but consistently — the host-API version.
fn capability_for_interface(name: &str) -> Option<(Capability, &str)> {
    let rest = name.strip_prefix("genealogy:host-api/")?;
    let (interface, version) = rest.split_once('@')?;
    let capability = match interface {
        "log" => Capability::Log,
        "query" => Capability::Query,
        "commands" => Capability::Commands,
        "progress" => Capability::Progress,
        "import-source" => Capability::ImportSource,
        "export-sink" => Capability::ExportSink,
        _ => return None,
    };
    Some((capability, version))
}

/// Inspects `component`'s imports/exports and derives its [`PluginInfo`] (everything but `id`, which
/// the caller supplies from the file name).
fn inspect(engine: &Engine, component: &Component) -> PluginInfo {
    let ty = component.component_type();

    let mut capabilities = Vec::new();
    let mut host_api_version = String::new();
    for (name, _extern) in ty.imports(engine) {
        if let Some((capability, version)) = capability_for_interface(name) {
            capabilities.push(capability);
            if host_api_version.is_empty() {
                version.clone_into(&mut host_api_version);
            }
        }
    }

    let mut exports: Vec<&str> = ty.exports(engine).map(|(name, _extern)| name).collect();
    exports.sort_unstable();
    let role = match exports.as_slice() {
        ["run-import"] => PluginRole::BulkImport,
        ["run-export"] => PluginRole::BulkExport,
        ["run-ui-panel"] => PluginRole::UiPanel,
        ["try-create", "busy-loop", "allocate"] | ["allocate", "busy-loop", "try-create"] => PluginRole::TestFixture,
        _ => PluginRole::Unknown,
    };

    PluginInfo {
        id: String::new(),
        role,
        host_api_version,
        capabilities,
    }
}

impl PluginHost {
    /// Scans `dir` for `.wasm` components and reads each one's genuinely declared metadata (see the
    /// module docs for exactly what that covers). Non-`.wasm` entries are skipped. Plugins are
    /// returned in no particular order.
    ///
    /// # Errors
    ///
    /// [`PluginError::Runtime`] if `dir` cannot be read, or if a `.wasm` file is not a valid
    /// component.
    pub fn discover(&self, dir: &Path) -> Result<Vec<PluginInfo>, PluginError> {
        let entries = std::fs::read_dir(dir)
            .map_err(|error| PluginError::Runtime(format!("reading plugins directory {}: {error}", dir.display())))?;

        let mut found = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| PluginError::Runtime(format!("reading directory entry: {error}")))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("wasm") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let component = self.load(&path)?;
            let mut info = inspect(self.engine(), &component);
            id.clone_into(&mut info.id);
            found.push(info);
        }
        Ok(found)
    }
}
