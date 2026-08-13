//! Plugin discovery over the ADR 0014 §2 **bundle** format: each plugin is a directory `<id>/`
//! holding a `plugin.toml` manifest, a `plugin.wasm` component, an optional `plugin.sig` detached
//! signature, and any `i18n/` catalogue. Discovery reads the manifest, classifies the bundle into a
//! [`TrustTier`] by verifying its signature against the trust roots (A2), and **cross-checks** the
//! manifest against what the component genuinely imports/exports.
//!
//! **The manifest is the authoritative grant-request** (ADR 0014 §2): its declared `capabilities`
//! are what the grant UX surfaces, so [`PluginInfo::capabilities`] carries the *manifest's* declared
//! set, not the component's imports. The cross-check guarantees the manifest cannot lie about what
//! the code will attempt:
//!
//! - `role` and `host_api` must match the component **exactly** (the role it exports, the
//!   `vitni:host-api@X.Y.Z` version it imports).
//! - the component's genuinely imported capabilities must be a **subset** of the manifest's declared
//!   capabilities. wit-bindgen tree-shakes unused capability imports out of a component, so a
//!   component's *actual* imports are a subset of what its world/manifest declares (e.g. `ai` may be
//!   declared/granted yet tree-shaken out). A manifest declaring **more** than the component imports
//!   is therefore legitimate; only a component importing a capability the manifest does **not**
//!   declare (under-declaration) is an error.
//!
//! The three-layer override across workspace/app-dir/embedded lives one level up in
//! `vitni-app` (`plugins::resolve_bundles`), mirroring the i18n multiplexor; the host stays
//! single-dir — [`PluginHost::discover`] scans one directory of bundles.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use wasmtime::Engine;
use wasmtime::component::Component;

use crate::PluginHost;
use crate::capability::{Capability, Grants};
use crate::error::PluginError;
use crate::signing::PluginManifest;
use crate::trust::{self, TrustRoots, TrustTier};

/// The plugin role a component implements, inferred from the entry point(s) it exports (ADR 0011
/// §1's per-role worlds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginRole {
    /// Exports `run-import` (the `bulk-import` world, ADR 0013).
    BulkImport,
    /// Exports `run-export` (the `bulk-export` world, ADR 0013).
    BulkExport,
    /// Exports `run-ui-panel` + `handle-action` (the `ui-panel` world, ADR 0012, ADR 0022).
    UiPanel,
    /// Exports `run-assisted` (the `assisted-import` world, ADR 0017 §5).
    AssistedImport,
    /// Exports the test-only `fixture` world's entry points (`try-create`/`busy-loop`/`allocate`).
    TestFixture,
    /// Exports none of the known entry points — a component the host has no role for.
    Unknown,
}

/// A discovered plugin bundle's metadata: the manifest's authoritative declaration (ADR 0014 §2),
/// cross-checked against the component and joined with the bundle's on-disk location and trust tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    /// The plugin's id (the manifest's `id`; the bundle directory is named after it).
    pub id: String,
    /// The role, verified to match both the manifest's `role` and the component's exported entry
    /// point.
    pub role: PluginRole,
    /// The `vitni:host-api` package version, verified to match both the manifest's `host_api`
    /// and the version embedded in the component's imported interfaces.
    pub host_api_version: String,
    /// The capabilities the manifest **declares** — the authoritative grant-request the UX surfaces.
    /// The component's genuinely imported capabilities were verified to be a subset of this set (the
    /// manifest may declare more; wit-bindgen tree-shakes unused imports out of the component).
    pub capabilities: Vec<Capability>,
    /// The trust tier its signature places it in (ADR 0014 §3): unsigned → `Untrusted`, a signature
    /// verifying against a sanctioned/user-pinned key → `Sanctioned`/`UserTrusted`.
    pub trust: TrustTier,
    /// The bundle directory, so a caller can `load_bundle` the resolved plugin (ADR 0014 §2).
    pub bundle_dir: PathBuf,
}

impl PluginInfo {
    /// Resolves the **effective grant** for this plugin (ADR 0014 §5): the intersection of the
    /// capabilities it **declares** ([`Self::capabilities`]) with the operator's recorded decision,
    /// still deny-by-default and still gated per host call by `capability-error::denied`.
    ///
    /// `approved` is the persisted per-plugin approved-capability set (by [`Capability::interface_name`]),
    /// or `None` when the operator has recorded **no** decision yet:
    ///
    /// - `Some(set)` — effective = declared ∩ `set`. An approved name that is not declared is simply
    ///   absent from the result (intersection); an unknown name likewise contributes nothing.
    /// - `None` — the trust tier supplies the default (ADR 0014 §5): a `Sanctioned` or
    ///   `UserTrusted` plugin grants **all declared** capabilities (a trusted plugin defaults to
    ///   grant-all — the one-confirmation A5 prompt), while an `Untrusted` plugin grants **nothing**
    ///   ([`Grants::none`]) until the operator explicitly approves.
    #[must_use]
    pub fn effective_grants(&self, approved: Option<&BTreeSet<String>>) -> Grants {
        let mut grants = Grants::none();
        match approved {
            Some(approved) => {
                for capability in &self.capabilities {
                    if approved.contains(capability.interface_name()) {
                        grants = grants.with(*capability);
                    }
                }
            }
            None => match self.trust {
                TrustTier::Sanctioned | TrustTier::UserTrusted => {
                    for capability in &self.capabilities {
                        grants = grants.with(*capability);
                    }
                }
                TrustTier::Untrusted => {}
            },
        }
        grants
    }
}

/// Maps a WIT interface import name (`vitni:host-api/<name>@<version>`) to the [`Capability`] it
/// represents and the host-API version it pins. The name→[`Capability`] step is shared with the
/// manifest cross-check and the grant resolver via [`Capability::from_interface_name`].
fn capability_for_interface(name: &str) -> Option<(Capability, &str)> {
    let rest = name.strip_prefix("vitni:host-api/")?;
    let (interface, version) = rest.split_once('@')?;
    let capability = Capability::from_interface_name(interface)?;
    Some((capability, version))
}

/// Maps a `plugin.toml` `role` string to the [`PluginRole`] it names, or `None` for an unknown role.
fn role_from_manifest(role: &str) -> Option<PluginRole> {
    match role {
        "bulk-import" => Some(PluginRole::BulkImport),
        "bulk-export" => Some(PluginRole::BulkExport),
        "ui-panel" => Some(PluginRole::UiPanel),
        "assisted-import" => Some(PluginRole::AssistedImport),
        "test-fixture" => Some(PluginRole::TestFixture),
        _ => None,
    }
}

/// What [`inspect`] reads straight off the compiled component: its exported role, the host-API
/// version its imports pin, and the capabilities it genuinely imports (post-tree-shake).
struct Inspected {
    role: PluginRole,
    host_api_version: String,
    capabilities: Vec<Capability>,
}

/// Inspects `component`'s imports/exports (the genuinely-declared-by-the-code facts the cross-check
/// validates the manifest against).
fn inspect(engine: &Engine, component: &Component) -> Inspected {
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
        ["run-assisted"] => PluginRole::AssistedImport,
        ["handle-action", "run-ui-panel"] => PluginRole::UiPanel,
        [
            "allocate",
            "busy-loop",
            "run-assisted",
            "try-create",
            "try-fetch",
            "try-fetch-store",
            "try-interpret",
            "try-present",
            "try-store",
        ] => PluginRole::TestFixture,
        _ => PluginRole::Unknown,
    };

    Inspected {
        role,
        host_api_version,
        capabilities,
    }
}

/// Reads the optional `plugin.sig` beside the manifest, returning `None` when it is absent (an
/// unsigned bundle → [`TrustTier::Untrusted`]) and a [`PluginError::Runtime`] on any other I/O
/// failure.
fn read_optional_signature(path: &Path) -> Result<Option<Vec<u8>>, PluginError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PluginError::Runtime(format!("reading {}: {error}", path.display()))),
    }
}

/// Cross-checks `manifest` against the `inspected` component facts (ADR 0014 §2), returning the
/// manifest's declared capabilities mapped to [`Capability`] on success.
///
/// See the module docs: `role`/`host_api` must match exactly, and the component's imported
/// capabilities must be a subset of the manifest's declared set (declaring more is legitimate;
/// under-declaring is the error). An unknown capability or role string in the manifest is an error.
fn cross_check(id: &str, manifest: &PluginManifest, inspected: &Inspected) -> Result<Vec<Capability>, PluginError> {
    let declared_role = role_from_manifest(&manifest.role).ok_or_else(|| {
        PluginError::Runtime(format!(
            "plugin {id} manifest declares unknown role {:?} (ADR 0014 §2)",
            manifest.role
        ))
    })?;
    if declared_role != inspected.role {
        return Err(PluginError::Runtime(format!(
            "plugin {id} manifest declares role {:?} but the component exports the {:?} entry point",
            manifest.role, inspected.role
        )));
    }
    if manifest.host_api != inspected.host_api_version {
        return Err(PluginError::Runtime(format!(
            "plugin {id} manifest declares host-api {:?} but the component imports host-api {:?}",
            manifest.host_api, inspected.host_api_version
        )));
    }

    let mut declared = Vec::with_capacity(manifest.capabilities.len());
    for name in &manifest.capabilities {
        let capability = Capability::from_interface_name(name).ok_or_else(|| {
            PluginError::Runtime(format!(
                "plugin {id} manifest declares unknown capability {name:?} (ADR 0014 §2)"
            ))
        })?;
        declared.push(capability);
    }

    for capability in &inspected.capabilities {
        if !declared.contains(capability) {
            return Err(PluginError::Runtime(format!(
                "plugin {id} imports capability {capability:?} that its manifest does not declare — the \
                 manifest under-declares what the code will attempt (ADR 0014 §2)"
            )));
        }
    }

    Ok(declared)
}

impl PluginHost {
    /// Discovers, verifies, and cross-checks the single bundle directory `bundle_dir` (ADR 0014 §2),
    /// returning its [`PluginInfo`]. Reads `bundle_dir/plugin.toml`, loads `bundle_dir/plugin.wasm`,
    /// classifies against `roots` via any `bundle_dir/plugin.sig`, and cross-checks the manifest
    /// against the component (see the module docs).
    ///
    /// # Errors
    ///
    /// [`PluginError::Runtime`] if the manifest is missing/unparseable, the component is missing or
    /// invalid, or the manifest cross-check fails; [`PluginError::Signature`] if a present signature
    /// verifies against no trusted key (fails closed).
    pub fn discover_bundle(&self, bundle_dir: &Path, roots: &TrustRoots) -> Result<PluginInfo, PluginError> {
        let manifest_path = bundle_dir.join("plugin.toml");
        let wasm_path = bundle_dir.join("plugin.wasm");
        let signature_path = bundle_dir.join("plugin.sig");

        let manifest_bytes = std::fs::read(&manifest_path)
            .map_err(|error| PluginError::Runtime(format!("reading {}: {error}", manifest_path.display())))?;
        let manifest_text = String::from_utf8(manifest_bytes.clone())
            .map_err(|error| PluginError::Runtime(format!("{} is not UTF-8: {error}", manifest_path.display())))?;
        let manifest: PluginManifest = toml::from_str(&manifest_text)
            .map_err(|error| PluginError::Runtime(format!("parsing {}: {error}", manifest_path.display())))?;

        let component = self.load(&wasm_path)?;
        let inspected = inspect(self.engine(), &component);
        let capabilities = cross_check(&manifest.id, &manifest, &inspected)?;

        let wasm_bytes = std::fs::read(&wasm_path)
            .map_err(|error| PluginError::Runtime(format!("reading {}: {error}", wasm_path.display())))?;
        let signature = read_optional_signature(&signature_path)?;
        let trust = trust::classify(roots, &manifest_bytes, &wasm_bytes, signature.as_deref())?;

        Ok(PluginInfo {
            id: manifest.id,
            role: inspected.role,
            host_api_version: inspected.host_api_version,
            capabilities,
            trust,
            bundle_dir: bundle_dir.to_path_buf(),
        })
    }

    /// Scans `dir` for bundle subdirectories (each carrying a `plugin.toml`) and discovers each via
    /// [`Self::discover_bundle`] (ADR 0014 §2). Non-directory entries and directories without a
    /// `plugin.toml` are skipped. Plugins are returned in no particular order — the three-layer
    /// override across directories lives in `vitni-app`; this scans one layer.
    ///
    /// # Errors
    ///
    /// [`PluginError::Runtime`] if `dir` cannot be read, or a bundle is malformed (missing/invalid
    /// component, bad manifest, failed cross-check); [`PluginError::Signature`] if a bundle carries
    /// a present-but-unverifiable signature (fails closed).
    pub fn discover(&self, dir: &Path, roots: &TrustRoots) -> Result<Vec<PluginInfo>, PluginError> {
        let entries = std::fs::read_dir(dir)
            .map_err(|error| PluginError::Runtime(format!("reading plugins directory {}: {error}", dir.display())))?;

        let mut found = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| PluginError::Runtime(format!("reading directory entry: {error}")))?;
            let path = entry.path();
            if !path.is_dir() || !path.join("plugin.toml").is_file() {
                continue;
            }
            found.push(self.discover_bundle(&path, roots)?);
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::{PluginInfo, PluginRole};
    use crate::capability::{Capability, Grants};
    use crate::trust::TrustTier;

    fn info(trust: TrustTier, capabilities: Vec<Capability>) -> PluginInfo {
        PluginInfo {
            id: "sample".to_owned(),
            role: PluginRole::BulkImport,
            host_api_version: "0.22.0".to_owned(),
            capabilities,
            trust,
            bundle_dir: PathBuf::from("/tmp/sample"),
        }
    }

    fn approved(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn all_of(capabilities: &[Capability]) -> Grants {
        let mut grants = Grants::none();
        for capability in capabilities {
            grants = grants.with(*capability);
        }
        grants
    }

    #[test]
    fn a_recorded_decision_is_declared_intersect_approved() {
        let declared = vec![Capability::Log, Capability::Commands, Capability::ImportSource];
        let info = info(TrustTier::Sanctioned, declared);
        let grants = info.effective_grants(Some(&approved(&["log", "commands", "import-source"])));
        assert_eq!(
            grants,
            all_of(&[Capability::Log, Capability::Commands, Capability::ImportSource])
        );
    }

    #[test]
    fn an_explicit_set_narrows_the_declared_capabilities() {
        // The operator approved only two of the three declared capabilities — the third drops.
        let declared = vec![Capability::Log, Capability::Commands, Capability::ImportSource];
        let info = info(TrustTier::Sanctioned, declared);
        let grants = info.effective_grants(Some(&approved(&["log", "commands"])));
        assert_eq!(grants, all_of(&[Capability::Log, Capability::Commands]));
        assert!(
            !grants.allows(Capability::ImportSource),
            "the unapproved capability is denied"
        );
    }

    #[test]
    fn an_approved_name_outside_the_declared_set_is_ignored() {
        // Intersection: approving `net` for a plugin that never declared it grants nothing extra.
        let info = info(TrustTier::Sanctioned, vec![Capability::Log]);
        let grants = info.effective_grants(Some(&approved(&["log", "net"])));
        assert_eq!(grants, all_of(&[Capability::Log]));
        assert!(
            !grants.allows(Capability::Net),
            "an undeclared approved name is not granted"
        );
    }

    #[test]
    fn an_unknown_approved_name_is_ignored() {
        let info = info(TrustTier::Sanctioned, vec![Capability::Log]);
        let grants = info.effective_grants(Some(&approved(&["log", "does-not-exist"])));
        assert_eq!(grants, all_of(&[Capability::Log]));
    }

    #[test]
    fn sanctioned_with_no_decision_grants_all_declared() {
        let declared = vec![Capability::Log, Capability::Commands, Capability::ImportSource];
        let info = info(TrustTier::Sanctioned, declared.clone());
        assert_eq!(info.effective_grants(None), all_of(&declared));
    }

    #[test]
    fn user_trusted_with_no_decision_grants_all_declared() {
        let declared = vec![Capability::Log, Capability::Query];
        let info = info(TrustTier::UserTrusted, declared.clone());
        assert_eq!(info.effective_grants(None), all_of(&declared));
    }

    #[test]
    fn untrusted_with_no_decision_grants_nothing() {
        let info = info(TrustTier::Untrusted, vec![Capability::Log, Capability::Commands]);
        assert_eq!(info.effective_grants(None), Grants::none());
    }

    #[test]
    fn untrusted_still_honours_an_explicit_approval() {
        // An untrusted plugin is grant-nothing *by default*, but an explicit per-capability approval
        // (the A5 path) still narrows-from-declared normally.
        let info = info(TrustTier::Untrusted, vec![Capability::Log, Capability::Commands]);
        let grants = info.effective_grants(Some(&approved(&["log"])));
        assert_eq!(grants, all_of(&[Capability::Log]));
    }

    #[test]
    fn interface_name_round_trips_through_from_interface_name() {
        for capability in [
            Capability::Query,
            Capability::Commands,
            Capability::Log,
            Capability::Progress,
            Capability::ImportSource,
            Capability::ExportSink,
            Capability::Net,
            Capability::MediaStore,
            Capability::Ai,
            Capability::Present,
        ] {
            assert_eq!(
                Capability::from_interface_name(capability.interface_name()),
                Some(capability),
                "every capability's interface name maps back to itself"
            );
        }
    }
}
