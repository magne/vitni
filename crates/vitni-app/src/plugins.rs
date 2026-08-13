//! Three-layer plugin-bundle resolution (ADR 0014 §4), mirroring the i18n `layered_assets`
//! multiplexor (`vitni-i18n`).
//!
//! Where i18n multiplexes `.ftl` assets across an ordered list of directories (highest precedence
//! first, missing layers skipped), this multiplexes per-plugin **bundle directories** keyed by
//! plugin id. A plugin bundle (ADR 0014 §2) is a directory `<id>/` holding `plugin.toml`,
//! `plugin.wasm`, an optional `plugin.sig`, and any `i18n/` catalogue; its directory name is the
//! plugin id. The loading layers, highest precedence first, are:
//!
//! 1. **Workspace** — `<workspace>/plugins/`.
//! 2. **App-dir** — the shared app plugin dir ([`crate::config::shared_plugins_dir`]).
//! 3. **Embedded** — the sanctioned first-party fleet shipped with the binary (dev: `target/plugins`).
//!
//! This module owns only the *layering* — which directories participate, in what precedence, and the
//! id-keyed merge. Inspecting and verifying a bundle's component (Wasmtime, trust roots) stays in
//! `vitni-plugin-host`, which sits **above** this crate (so this crate cannot depend on it): a
//! frontend resolves an id → bundle directory here, then hands the directory to the host's
//! `load_bundle`/`discover_bundle` to inspect and classify it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The trust tier a discovered plugin's signature places it in (ADR 0014 §3), as a frontend-visible
/// DTO.
///
/// This crate cannot depend on `vitni-plugin-host` (which owns the `TrustTier` the host actually
/// computes — Wasmtime and the crypto live above this layer), so this plain mirror is what a frontend
/// carries into a view-model. A renderer maps the host's `TrustTier` onto this when it discovers a
/// bundle; `vitni-ui` builds its grant view-model from this DTO, staying free of plugin-host
/// types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTrust {
    /// Signed by an embedded sanctioned project key — every declared capability is grantable.
    Sanctioned,
    /// Signed by a publisher key the user pinned in their client-scope trust store.
    UserTrusted,
    /// Unsigned, or signed by a key the host does not trust. Loadable, but never auto-granted.
    Untrusted,
}

/// The manifest that marks a directory as a plugin bundle (ADR 0014 §2).
const BUNDLE_MANIFEST: &str = "plugin.toml";
/// The component every bundle carries (ADR 0014 §2).
const BUNDLE_COMPONENT: &str = "plugin.wasm";

/// The ordered plugin-bundle layer directories (ADR 0014 §4), highest precedence first, with absent
/// directories skipped — exactly as `vitni_i18n::layered_assets` skips absent i18n dirs.
///
/// `workspace_dir` is a workspace **root** (its `plugins/` subdirectory is the layer); `shared_dir`
/// and `embedded` already point at plugin directories.
#[must_use]
pub fn plugin_layers(workspace_dir: Option<&Path>, shared_dir: Option<&Path>, embedded: &Path) -> Vec<PathBuf> {
    let mut layers = Vec::new();
    if let Some(dir) = workspace_dir {
        push_if_dir(&mut layers, dir.join("plugins"));
    }
    if let Some(dir) = shared_dir {
        push_if_dir(&mut layers, dir.to_path_buf());
    }
    push_if_dir(&mut layers, embedded.to_path_buf());
    layers
}

/// Adds `dir` to `layers` if it exists as a directory (an absent layer contributes nothing).
fn push_if_dir(layers: &mut Vec<PathBuf>, dir: PathBuf) {
    if dir.is_dir() {
        layers.push(dir);
    }
}

/// Resolves every discoverable bundle across `layers` (highest precedence first) into an id-keyed
/// map of bundle directories.
///
/// A subdirectory is a bundle when it carries both `plugin.toml` and `plugin.wasm`; its directory
/// name is the id. When the same id appears in more than one layer the higher layer wins.
#[must_use]
pub fn resolve_bundles(layers: &[PathBuf]) -> BTreeMap<String, PathBuf> {
    let mut resolved: BTreeMap<String, PathBuf> = BTreeMap::new();
    for layer in layers {
        let Ok(entries) = std::fs::read_dir(layer) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        dirs.sort();
        for dir in dirs {
            let Some(id) = dir.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !is_bundle(&dir) {
                continue;
            }
            // ADR 0014 §4: higher layer wins — layers are highest-precedence first, so the first
            // occurrence of an id is kept and lower layers do not overwrite it.
            resolved.entry(id.to_owned()).or_insert(dir);
        }
    }
    resolved
}

/// Resolves a single plugin `id` to its bundle directory, taking the highest layer that carries it
/// (ADR 0014 §4). `None` when no layer holds a bundle for `id`.
#[must_use]
pub fn resolve_bundle(layers: &[PathBuf], id: &str) -> Option<PathBuf> {
    for layer in layers {
        let candidate = layer.join(id);
        if is_bundle(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Whether `dir` is a plugin bundle: a directory holding both the manifest and the component.
fn is_bundle(dir: &Path) -> bool {
    dir.join(BUNDLE_MANIFEST).is_file() && dir.join(BUNDLE_COMPONENT).is_file()
}

#[cfg(test)]
mod tests {
    use super::{plugin_layers, resolve_bundle, resolve_bundles};
    use std::path::Path;

    /// Lays out a bundle `<root>/<id>/` with `plugin.toml` + `plugin.wasm`.
    fn write_bundle(root: &Path, id: &str) {
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).expect("bundle dir");
        std::fs::write(dir.join("plugin.toml"), b"id = 'x'").expect("manifest");
        std::fs::write(dir.join("plugin.wasm"), b"\0asm").expect("component");
    }

    #[test]
    fn missing_layers_are_skipped() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let embedded = tmp.path().join("target/plugins");
        std::fs::create_dir_all(&embedded).expect("embedded");
        write_bundle(&embedded, "gedcom-import");

        let workspace = tmp.path().join("workspace");
        let layers = plugin_layers(Some(&workspace), None, &embedded);
        // The workspace has no plugins/ dir, so only the embedded layer survives.
        assert_eq!(layers, vec![embedded]);
    }

    #[test]
    fn a_higher_layer_overrides_a_lower_layer_for_the_same_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path().join("workspace");
        let ws_plugins = workspace.join("plugins");
        std::fs::create_dir_all(&ws_plugins).expect("ws plugins");
        let embedded = tmp.path().join("target/plugins");
        std::fs::create_dir_all(&embedded).expect("embedded");
        write_bundle(&ws_plugins, "gedcom-import");
        write_bundle(&embedded, "gedcom-import");
        write_bundle(&embedded, "gedcom-export");

        let layers = plugin_layers(Some(&workspace), None, &embedded);
        let resolved = resolve_bundles(&layers);
        assert_eq!(resolved.len(), 2, "two distinct ids");
        assert_eq!(
            resolved.get("gedcom-import"),
            Some(&ws_plugins.join("gedcom-import")),
            "the workspace layer wins for a shared id"
        );
        assert_eq!(
            resolved.get("gedcom-export"),
            Some(&embedded.join("gedcom-export")),
            "an id only in the embedded layer resolves there"
        );
        assert_eq!(
            resolve_bundle(&layers, "gedcom-import"),
            Some(ws_plugins.join("gedcom-import")),
            "single-id resolution honours precedence too"
        );
        assert_eq!(
            resolve_bundle(&layers, "nope"),
            None,
            "an unknown id resolves to nothing"
        );
    }

    #[test]
    fn a_directory_without_a_manifest_is_not_a_bundle() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let embedded = tmp.path().join("plugins");
        std::fs::create_dir_all(embedded.join("gedcom-import/i18n")).expect("stale i18n-only dir");
        let layers = plugin_layers(None, None, &embedded);
        assert!(
            resolve_bundles(&layers).is_empty(),
            "a directory without plugin.toml/plugin.wasm is not a bundle"
        );
    }
}
