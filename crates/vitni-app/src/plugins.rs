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
//! 3. **Embedded** — the sanctioned first-party fleet shipped with the binary
//!    ([`resolve_embedded_plugins_dir`] gives the lookup order; dev: `target/plugins`).
//!
//! This module owns only the *layering* — which directories participate, in what precedence, and the
//! id-keyed merge. Inspecting and verifying a bundle's component (Wasmtime, trust roots) stays in
//! `vitni-plugin-host`, which sits **above** this crate (so this crate cannot depend on it): a
//! frontend resolves an id → bundle directory here, then hands the directory to the host's
//! `load_bundle`/`discover_bundle` to inspect and classify it.

use std::collections::BTreeMap;
use std::ffi::OsStr;
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

/// The embedded plugin layer (ADR 0014 §4) for this process: [`resolve_embedded_plugins_dir`] over
/// `$VITNI_PLUGIN_DIR` and the running binary's path. The only place those two inputs are read.
///
/// A binary path that cannot be determined skips the installed locations (logged at `debug`), leaving
/// the override or the dev fallback.
#[must_use]
pub fn embedded_plugins_dir() -> PathBuf {
    let env_override = std::env::var_os("VITNI_PLUGIN_DIR");
    let exe = match std::env::current_exe() {
        Ok(exe) => Some(exe),
        Err(error) => {
            tracing::debug!(%error, "cannot locate the running binary; skipping installed plugin fleet locations");
            None
        }
    };
    resolve_embedded_plugins_dir(env_override.as_deref(), exe.as_deref())
}

/// Resolves the embedded plugin layer (ADR 0014 §4) from its two inputs; first match wins:
///
/// 1. `env_override` (`$VITNI_PLUGIN_DIR`) when non-empty, as given — an `AppImage`'s `AppRun` sets it.
/// 2. `<exe_dir>/plugins`, if a directory — the release tarball ships the fleet beside the binaries.
/// 3. `<exe_dir>/../lib/vitni/plugins`, if a directory — a `.deb` installs `/usr/bin/vitni` and
///    `/usr/lib/vitni/plugins`.
/// 4. `target/plugins` in this source tree, independent of the working directory (the dev default;
///    `cargo xtask build-plugins` fills it).
#[must_use]
pub fn resolve_embedded_plugins_dir(env_override: Option<&OsStr>, exe: Option<&Path>) -> PathBuf {
    if let Some(dir) = env_override.filter(|value| !value.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(exe_dir) = exe.and_then(Path::parent) {
        let beside = exe_dir.join("plugins");
        if beside.is_dir() {
            return beside;
        }
        if let Some(prefix) = exe_dir.parent() {
            let lib = prefix.join("lib/vitni/plugins");
            if lib.is_dir() {
                return lib;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/plugins")
}

/// Whether `dir` is a plugin bundle: a directory holding both the manifest and the component.
fn is_bundle(dir: &Path) -> bool {
    dir.join(BUNDLE_MANIFEST).is_file() && dir.join(BUNDLE_COMPONENT).is_file()
}

#[cfg(test)]
mod tests {
    use super::{plugin_layers, resolve_bundle, resolve_bundles, resolve_embedded_plugins_dir};
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    /// An install root `<tmp>/` with the binary at `<tmp>/bin/vitni`; returns the exe path.
    fn exe_in(root: &Path) -> PathBuf {
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).expect("bin dir");
        bin.join("vitni")
    }

    fn assert_dev_fallback(resolved: &Path) {
        assert!(
            resolved.is_absolute(),
            "the dev fallback is source-tree-absolute: {resolved:?}"
        );
        assert!(
            resolved.ends_with("target/plugins"),
            "the dev fallback is target/plugins: {resolved:?}"
        );
    }

    #[test]
    fn the_env_override_wins_even_over_a_fleet_beside_the_binary() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = exe_in(tmp.path());
        std::fs::create_dir_all(tmp.path().join("bin/plugins")).expect("beside-binary fleet");
        let resolved = resolve_embedded_plugins_dir(Some(OsStr::new("/opt/fleet")), Some(&exe));
        assert_eq!(resolved, PathBuf::from("/opt/fleet"));
    }

    #[test]
    fn an_empty_env_override_is_treated_as_unset() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = exe_in(tmp.path());
        let beside = tmp.path().join("bin/plugins");
        std::fs::create_dir_all(&beside).expect("beside-binary fleet");
        assert_eq!(resolve_embedded_plugins_dir(Some(OsStr::new("")), Some(&exe)), beside);
    }

    #[test]
    fn a_fleet_beside_the_binary_is_chosen() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = exe_in(tmp.path());
        let beside = tmp.path().join("bin/plugins");
        std::fs::create_dir_all(&beside).expect("beside-binary fleet");
        assert_eq!(resolve_embedded_plugins_dir(None, Some(&exe)), beside);
    }

    #[test]
    fn the_lib_fleet_is_chosen_when_only_it_exists() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = exe_in(tmp.path());
        let lib = tmp.path().join("lib/vitni/plugins");
        std::fs::create_dir_all(&lib).expect("lib fleet");
        assert_eq!(resolve_embedded_plugins_dir(None, Some(&exe)), lib);
    }

    #[test]
    fn the_fleet_beside_the_binary_beats_the_lib_fleet() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = exe_in(tmp.path());
        let beside = tmp.path().join("bin/plugins");
        std::fs::create_dir_all(&beside).expect("beside-binary fleet");
        std::fs::create_dir_all(tmp.path().join("lib/vitni/plugins")).expect("lib fleet");
        assert_eq!(resolve_embedded_plugins_dir(None, Some(&exe)), beside);
    }

    #[test]
    fn no_installed_fleet_falls_back_to_the_source_tree() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = exe_in(tmp.path());
        assert_dev_fallback(&resolve_embedded_plugins_dir(None, Some(&exe)));
    }

    #[test]
    fn an_unknown_exe_falls_back_to_the_source_tree() {
        assert_dev_fallback(&resolve_embedded_plugins_dir(None, None));
    }

    #[test]
    fn a_file_named_plugins_beside_the_binary_is_not_a_fleet() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = exe_in(tmp.path());
        std::fs::write(tmp.path().join("bin/plugins"), b"not a directory").expect("plugins file");
        assert_dev_fallback(&resolve_embedded_plugins_dir(None, Some(&exe)));
    }

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
