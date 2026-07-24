//! Shared helpers for the xtask commands: running cargo, copying directories, and parsing the
//! `i18n.toml` / `Cargo.toml` manifests both commands discover their work from.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// The baseline locale every other locale must match (the embedded fallback — ADR 0003).
pub const BASELINE_LOCALE: &str = "en";

/// A localization config (`i18n.toml`, ADR 0003): the fallback language and the assets directory the
/// catalogues live under, relative to the file.
#[derive(Deserialize)]
pub struct I18nConfig {
    pub fallback_language: String,
    pub fluent: Fluent,
}

/// The `[fluent]` table of an `i18n.toml`.
#[derive(Deserialize)]
pub struct Fluent {
    pub assets_dir: String,
}

impl I18nConfig {
    /// Loads and parses the `i18n.toml` at `path`.
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }
}

/// The subset of a plugin's `Cargo.toml` the build needs: the package name (drives the artifact file
/// name), the `[lib]` crate type (only `cdylib` crates are built as components), and its dependency
/// table (path deps may ship catalogues to bundle).
#[derive(Deserialize)]
pub struct CargoManifest {
    pub package: Package,
    #[serde(default)]
    pub lib: Option<Lib>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, toml::Value>,
}

/// A `Cargo.toml` `[package]` table: the name (drives the artifact file name), the plugin's own
/// semver (goes into the bundle manifest), and the optional `[package.metadata.genealogy-plugin]`
/// bundle-manifest table (ADR 0014 §2).
#[derive(Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub metadata: Option<PackageMetadata>,
}

/// The `[package.metadata]` table, carrying the plugin bundle-manifest declaration.
#[derive(Deserialize)]
pub struct PackageMetadata {
    #[serde(rename = "genealogy-plugin")]
    pub genealogy_plugin: Option<PluginMetadata>,
}

/// The `[package.metadata.genealogy-plugin]` table (ADR 0014 §2): the declared role, the host-API
/// version the plugin pins, its capability requests, and an optional publisher identity.
#[derive(Deserialize)]
pub struct PluginMetadata {
    pub role: String,
    pub host_api: String,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub publisher: Option<String>,
}

/// A `Cargo.toml` `[lib]` table (only the crate type is read).
#[derive(Deserialize, Default)]
pub struct Lib {
    #[serde(default, rename = "crate-type")]
    pub crate_type: Vec<String>,
}

impl CargoManifest {
    /// Loads and parses the `Cargo.toml` at `path`.
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// The cdylib artifact file stem: the package name with `-` mapped to `_`.
    pub fn artifact_stem(&self) -> String {
        self.package.name.replace('-', "_")
    }

    /// Whether this crate is a plugin **component** (a `cdylib`) rather than a shared library
    /// dependency. Only components are built and copied by `build-plugins`.
    pub fn is_component(&self) -> bool {
        self.lib
            .as_ref()
            .is_some_and(|lib| lib.crate_type.iter().any(|kind| kind == "cdylib"))
    }

    /// The relative paths of every path dependency, sorted by dependency name.
    pub fn path_dependencies(&self) -> Vec<&str> {
        let mut paths = Vec::new();
        for dependency in self.dependencies.values() {
            if let Some(path) = dependency.get("path").and_then(toml::Value::as_str) {
                paths.push(path);
            }
        }
        paths
    }
}

/// The immediate subdirectories of `root`, sorted by name. Returns an empty list if `root` is absent.
pub fn child_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut dirs = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("reading {}", root.display()))? {
        let entry = entry.with_context(|| format!("reading an entry under {}", root.display()))?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

/// Every `.rs` file under `dir` (recursively), sorted for stable output.
pub fn rust_sources(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_rust_sources(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rust_sources(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry.with_context(|| format!("reading an entry under {}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

/// The keys extracted from `fl!(loader, "key", …)` calls in a source text, plus whether any call
/// used a non-literal key (which a static scan cannot validate).
pub struct FlScan {
    pub keys: Vec<String>,
    pub has_dynamic_key: bool,
}

/// Scans source text for `fl!` macro calls and extracts each key — the string literal immediately
/// after the loader argument. A call whose key argument is not a string literal sets
/// `has_dynamic_key` so the caller can surface it instead of silently skipping it.
///
/// This is a lexical heuristic, not a Rust parse: it assumes `fl!` appears only as a macro call and
/// that the loader argument contains no comma (always `self.loader` here).
pub fn scan_fl_keys(text: &str) -> FlScan {
    let mut keys = Vec::new();
    let mut has_dynamic_key = false;
    let bytes = text.as_bytes();
    let mut search_from = 0;
    while let Some(found) = text[search_from..].find("fl!") {
        let after_macro = search_from + found + "fl!".len();
        search_from = after_macro;
        let Some(open) = text[after_macro..].find('(') else {
            break;
        };
        let after_open = after_macro + open + 1;
        let Some(comma) = text[after_open..].find(',') else {
            continue;
        };
        let mut cursor = after_open + comma + 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() || bytes[cursor] != b'"' {
            has_dynamic_key = true;
            continue;
        }
        let key_start = cursor + 1;
        let Some(end) = text[key_start..].find('"') else {
            continue;
        };
        keys.push(text[key_start..key_start + end].to_owned());
    }
    FlScan { keys, has_dynamic_key }
}

/// Whether `key` appears as a quoted string literal (`"key"`) anywhere in `text`.
pub fn key_literal_present(text: &str, key: &str) -> bool {
    text.contains(&format!("\"{key}\""))
}

/// Recursively copies the directory `src` into `dest` (creating `dest`).
pub fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("reading {}", src.display()))? {
        let entry = entry.with_context(|| format!("reading an entry under {}", src.display()))?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target).with_context(|| format!("copying {} to {}", path.display(), target.display()))?;
        }
    }
    Ok(())
}

/// Runs `cargo` with `args`, failing if it exits non-zero.
pub fn run_cargo(args: &[&str]) -> Result<()> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args(args)
        .status()
        .with_context(|| format!("running cargo {}", args.join(" ")))?;
    if !status.success() {
        bail!("cargo {} failed with {status}", args.join(" "));
    }
    Ok(())
}
