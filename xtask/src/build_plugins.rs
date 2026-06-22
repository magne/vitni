//! `build-plugins` — lints and builds every WASM plugin component (ADR 0007, 0011).
//!
//! Plugins are discovered as the `plugins/*` subdirectories that carry a `Cargo.toml`. Each is built
//! for `wasm32-wasip2`, its artifact copied to `target/plugins/<id>.wasm`, and any Fluent catalogue
//! it contributes — its own (`i18n.toml`) plus any path dependency that ships one — copied beside it.
//! Plugin crates are excluded from the workspace, so this is the only place they are compiled or
//! linted; keep it in CI so the zero-warnings bar holds for guest code too.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::util::{self, CargoManifest, I18nConfig, copy_dir, run_cargo};

/// The target every plugin component is built for (ADR 0007 §1).
const PLUGIN_TARGET: &str = "wasm32-wasip2";
/// Where built plugin components are collected for the host to load (ADR 0011 §6).
const PLUGIN_OUT_DIR: &str = "target/plugins";

/// A discovered plugin component crate.
struct Plugin {
    /// The id the host loads it under: the directory name with any leading `_` stripped.
    id: String,
    dir: PathBuf,
    manifest: CargoManifest,
}

impl Plugin {
    fn manifest_path(&self) -> PathBuf {
        self.dir.join("Cargo.toml")
    }

    /// The built artifact path under the plugin's own target directory.
    fn artifact(&self) -> PathBuf {
        self.dir
            .join("target")
            .join(PLUGIN_TARGET)
            .join("release")
            .join(format!("{}.wasm", self.manifest.artifact_stem()))
    }
}

/// What was bundled for one plugin, for the closing summary.
struct Built {
    id: String,
    catalogues: Vec<String>,
}

/// Runs the `build-plugins` command (see module docs).
pub fn run() -> Result<()> {
    let out_dir = Path::new(PLUGIN_OUT_DIR);
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let plugins = discover()?;
    let mut summary = Vec::new();
    for plugin in &plugins {
        build_one(plugin, out_dir)?;
        let catalogues = bundle_catalogues(plugin, out_dir)?;
        summary.push(Built {
            id: plugin.id.clone(),
            catalogues,
        });
    }

    println!();
    println!("build-plugins: summary");
    for built in &summary {
        let catalogues = if built.catalogues.is_empty() {
            "(no catalogue)".to_owned()
        } else {
            built.catalogues.join(", ")
        };
        println!("  {} -> {}.wasm | i18n: {}", built.id, built.id, catalogues);
    }
    println!(
        "build-plugins: {} component(s) ready in {}",
        plugins.len(),
        out_dir.display()
    );
    Ok(())
}

/// Discovers the plugin crates under `plugins/` (those with a `Cargo.toml`).
fn discover() -> Result<Vec<Plugin>> {
    let mut plugins = Vec::new();
    for dir in util::child_dirs(Path::new("plugins"))? {
        let manifest_path = dir.join("Cargo.toml");
        if !manifest_path.exists() {
            continue;
        }
        let id = dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .trim_start_matches('_')
            .to_owned();
        let manifest = CargoManifest::load(&manifest_path)?;
        // Shared library crates (e.g. `plugin-api`) are dependencies of the components, not
        // components themselves; cargo builds them transitively. Skip them here.
        if !manifest.is_component() {
            println!("build-plugins: skipping {id} (shared library, not a component)");
            continue;
        }
        plugins.push(Plugin { id, dir, manifest });
    }
    Ok(plugins)
}

/// Lints (clippy, `-D warnings`), builds, and copies one plugin's artifact to `target/plugins`.
fn build_one(plugin: &Plugin, out_dir: &Path) -> Result<()> {
    let manifest_path = plugin.manifest_path();
    let manifest = manifest_path.to_string_lossy();

    println!("build-plugins: linting {}", plugin.id);
    run_cargo(&[
        "clippy",
        "--manifest-path",
        &manifest,
        "--release",
        "--target",
        PLUGIN_TARGET,
        "--",
        "-D",
        "warnings",
    ])?;

    println!("build-plugins: building {}", plugin.id);
    run_cargo(&[
        "build",
        "--manifest-path",
        &manifest,
        "--release",
        "--target",
        PLUGIN_TARGET,
    ])?;

    let artifact = plugin.artifact();
    let dest = out_dir.join(format!("{}.wasm", plugin.id));
    fs::copy(&artifact, &dest).with_context(|| format!("copying {} to {}", artifact.display(), dest.display()))?;
    println!("build-plugins: {} -> {}", plugin.id, dest.display());
    Ok(())
}

/// Copies the catalogues the plugin contributes — its own and any path dependency's — into
/// `target/plugins/<id>/i18n`, returning a label per bundled catalogue for the summary.
fn bundle_catalogues(plugin: &Plugin, out_dir: &Path) -> Result<Vec<String>> {
    let mut bundled = Vec::new();
    let dest = out_dir.join(&plugin.id).join("i18n");

    if let Some(assets) = catalogue_assets_dir(&plugin.dir)? {
        copy_dir(&assets, &dest)?;
        println!("build-plugins: {} i18n (own) -> {}", plugin.id, dest.display());
        bundled.push("own".to_owned());
    }

    for path in plugin.manifest.path_dependencies() {
        let dep_dir = plugin.dir.join(path);
        if let Some(assets) = catalogue_assets_dir(&dep_dir)? {
            copy_dir(&assets, &dest)?;
            let name = dep_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
                .to_owned();
            println!("build-plugins: {} i18n ({name}) -> {}", plugin.id, dest.display());
            bundled.push(name);
        }
    }

    Ok(bundled)
}

/// The catalogue assets directory declared by a crate's `i18n.toml`, or `None` if it has none.
fn catalogue_assets_dir(dir: &Path) -> Result<Option<PathBuf>> {
    let config_path = dir.join("i18n.toml");
    if !config_path.exists() {
        return Ok(None);
    }
    let config = I18nConfig::load(&config_path)?;
    Ok(Some(dir.join(config.fluent.assets_dir)))
}
