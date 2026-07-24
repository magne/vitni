//! `build-plugins` — lints and builds every WASM plugin component (ADR 0007, 0011, 0014).
//!
//! Plugins are discovered as the `plugins/*` subdirectories that carry a `Cargo.toml`. Each is built
//! for `wasm32-wasip2` and laid out as an ADR 0014 §2 **bundle directory** under
//! `target/plugins/<id>/`: the component as `plugin.wasm`, the signed `plugin.toml` manifest and its
//! `plugin.sig` detached signature, and any Fluent catalogue it contributes — its own (`i18n.toml`)
//! plus any path dependency that ships one — under `i18n/`. Plugin crates are excluded from the
//! workspace, so this is the only place they are compiled or linted; keep it in CI so the
//! zero-warnings bar holds for guest code too.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use genealogy_plugin_host::signing::{self, PluginManifest};

use crate::util::{self, CargoManifest, I18nConfig, copy_dir, run_cargo};

/// The publisher attributed to a first-party plugin whose manifest declares none.
const DEFAULT_PUBLISHER: &str = "genealogy-project";

/// The target every plugin component is built for (ADR 0007 §1).
const PLUGIN_TARGET: &str = "wasm32-wasip2";
/// Where built plugin bundles are collected for the host to load (ADR 0014 §4, the embedded layer).
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
    manifest: PluginManifest,
    signature_hex: String,
}

/// Runs the `build-plugins` command (see module docs).
pub fn run() -> Result<()> {
    let out_dir = Path::new(PLUGIN_OUT_DIR);
    reset_out_dir(out_dir)?;

    let plugins = discover()?;
    let mut summary = Vec::new();
    for plugin in &plugins {
        let wasm_dest = build_one(plugin, out_dir)?;
        let catalogues = bundle_catalogues(plugin, out_dir)?;
        let (manifest, signature_hex) = emit_bundle(plugin, out_dir, &wasm_dest)?;
        summary.push(Built {
            id: plugin.id.clone(),
            catalogues,
            manifest,
            signature_hex,
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
        let manifest = &built.manifest;
        println!(
            "  {id} -> {id}/{{plugin.toml, plugin.wasm, plugin.sig}} | i18n: {catalogues}",
            id = built.id
        );
        println!(
            "    manifest: v{} publisher={} host-api={} role={} capabilities=[{}]",
            manifest.version,
            manifest.publisher,
            manifest.host_api,
            manifest.role,
            manifest.capabilities.join(", ")
        );
        println!("    signature: {}", built.signature_hex);
    }
    println!(
        "build-plugins: {} component(s) ready in {}",
        plugins.len(),
        out_dir.display()
    );
    Ok(())
}

/// Clears and recreates the output directory so a rebuild never leaves a stale bundle (or a stale
/// flat-layout artifact from before ADR 0014) behind.
fn reset_out_dir(out_dir: &Path) -> Result<()> {
    if out_dir.exists() {
        fs::remove_dir_all(out_dir).with_context(|| format!("clearing {}", out_dir.display()))?;
    }
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))
}

/// The bundle directory for `id` under `out_dir` (ADR 0014 §2): `target/plugins/<id>/`.
fn bundle_dir(out_dir: &Path, id: &str) -> PathBuf {
    out_dir.join(id)
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

/// Lints (clippy, `-D warnings`), builds, and copies one plugin's artifact into its bundle directory
/// `target/plugins/<id>/plugin.wasm`, returning that path.
fn build_one(plugin: &Plugin, out_dir: &Path) -> Result<PathBuf> {
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
    let dir = bundle_dir(out_dir, &plugin.id);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let dest = dir.join("plugin.wasm");
    fs::copy(&artifact, &dest).with_context(|| format!("copying {} to {}", artifact.display(), dest.display()))?;
    println!("build-plugins: {} -> {}", plugin.id, dest.display());
    Ok(dest)
}

/// Emits the bundle manifest + signature into the bundle directory `target/plugins/<id>/` (ADR 0014
/// §2): the `plugin.toml` manifest and the `plugin.sig` ed25519 detached signature over the
/// canonical digest of manifest + component. The emitted signature is verified before it is trusted,
/// so a broken build fails loudly. Returns the manifest and the signature's hex encoding for the
/// summary.
fn emit_bundle(plugin: &Plugin, out_dir: &Path, wasm_dest: &Path) -> Result<(PluginManifest, String)> {
    let manifest = plugin_manifest(plugin)?;
    let manifest_toml =
        toml::to_string(&manifest).with_context(|| format!("serializing manifest for {}", plugin.id))?;
    let wasm_bytes = fs::read(wasm_dest).with_context(|| format!("reading {}", wasm_dest.display()))?;

    let digest = signing::bundle_digest(manifest_toml.as_bytes(), &wasm_bytes);
    let signing_key = signing::resolve_signing_key().context("resolving the plugin signing key")?;
    let signature = signing::sign(&signing_key, &digest);
    signing::verify(&signing_key.verifying_key(), &digest, &signature)
        .with_context(|| format!("self-check: the emitted signature for {} did not verify", plugin.id))?;

    let dir = bundle_dir(out_dir, &plugin.id);
    let manifest_path = dir.join("plugin.toml");
    let sig_path = dir.join("plugin.sig");
    fs::write(&manifest_path, &manifest_toml).with_context(|| format!("writing {}", manifest_path.display()))?;
    let signature_bytes = signing::signature_to_bytes(&signature);
    fs::write(&sig_path, signature_bytes).with_context(|| format!("writing {}", sig_path.display()))?;
    println!(
        "build-plugins: {} -> {} + {}",
        plugin.id,
        manifest_path.display(),
        sig_path.display()
    );

    Ok((manifest, hex_encode(&signature_bytes)))
}

/// Builds the bundle manifest (ADR 0014 §2) from the plugin's `Cargo.toml`: id from the plugin id,
/// version from `[package] version`, and role/host-API/capabilities/publisher from the
/// `[package.metadata.genealogy-plugin]` table.
fn plugin_manifest(plugin: &Plugin) -> Result<PluginManifest> {
    let metadata = plugin
        .manifest
        .package
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.genealogy_plugin.as_ref())
        .with_context(|| {
            format!(
                "plugin {} is missing the [package.metadata.genealogy-plugin] table (ADR 0014 §2)",
                plugin.id
            )
        })?;
    Ok(PluginManifest {
        id: plugin.id.clone(),
        version: plugin.manifest.package.version.clone(),
        publisher: metadata
            .publisher
            .clone()
            .unwrap_or_else(|| DEFAULT_PUBLISHER.to_owned()),
        host_api: metadata.host_api.clone(),
        role: metadata.role.clone(),
        capabilities: metadata.capabilities.clone(),
    })
}

/// Lowercase hex encoding of `bytes` (for the signature line of the build summary).
fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
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
