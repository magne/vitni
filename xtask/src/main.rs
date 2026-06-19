//! Repository task runner — `cargo xtask <command>`.
//!
//! Project-local automation that does not belong in a shipped binary. Run through the `cargo xtask`
//! alias defined in `.cargo/config.toml`. Commands:
//!
//! - `i18n-check` — verify every non-baseline locale catalogue has every message the English
//!   baseline defines, so translations don't silently drift as strings grow (ADR 0003).

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The baseline locale every other locale must match (the embedded fallback — ADR 0003).
const BASELINE_LOCALE: &str = "en";

/// A localized crate's catalogue: the `i18n/` root and the per-locale catalogue file name.
struct Catalogue {
    dir: &'static str,
    file: &'static str,
}

/// Every catalogue `i18n-check` verifies. Each frontend crate that resolves strings via `fl!()`
/// (ADR 0003) carries its own catalogue; plugins that contribute UI carry one too (ADR 0012 §5).
const CATALOGUES: &[Catalogue] = &[
    Catalogue {
        dir: "crates/genealogy-cli/i18n",
        file: "genealogy-cli.ftl",
    },
    Catalogue {
        dir: "crates/genealogy-ui/i18n",
        file: "genealogy-ui.ftl",
    },
    Catalogue {
        dir: "crates/genealogy-ui-dioxus/i18n",
        file: "genealogy-ui-dioxus.ftl",
    },
    Catalogue {
        dir: "plugins/ui-panel/i18n",
        file: "ui-panel.ftl",
    },
];

/// The target every plugin component is built for (ADR 0007 §1).
const PLUGIN_TARGET: &str = "wasm32-wasip2";
/// Where built plugin components are collected for the host to load (ADR 0011 §6).
const PLUGIN_OUT_DIR: &str = "target/plugins";

/// A WASM plugin component crate: its manifest, the wasm file it produces, and the id the host
/// loads it under.
struct Plugin {
    manifest: &'static str,
    artifact: &'static str,
    id: &'static str,
}

/// Every plugin component built by `build-plugins`.
const PLUGINS: &[Plugin] = &[
    Plugin {
        manifest: "plugins/_fixture/Cargo.toml",
        artifact: "plugins/_fixture/target/wasm32-wasip2/release/genealogy_fixture_plugin.wasm",
        id: "fixture",
    },
    Plugin {
        manifest: "plugins/gedcom-import/Cargo.toml",
        artifact: "plugins/gedcom-import/target/wasm32-wasip2/release/genealogy_gedcom_import.wasm",
        id: "gedcom-import",
    },
    Plugin {
        manifest: "plugins/gedcom-export/Cargo.toml",
        artifact: "plugins/gedcom-export/target/wasm32-wasip2/release/genealogy_gedcom_export.wasm",
        id: "gedcom-export",
    },
    Plugin {
        manifest: "plugins/ui-panel/Cargo.toml",
        artifact: "plugins/ui-panel/target/wasm32-wasip2/release/genealogy_ui_panel_plugin.wasm",
        id: "ui-panel",
    },
];

fn main() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("i18n-check") => i18n_check(),
        Some("build-plugins") => build_plugins(),
        Some(other) => {
            print_usage();
            bail!("unknown xtask command: {other}");
        }
        None => {
            print_usage();
            bail!("no xtask command given");
        }
    }
}

fn print_usage() {
    println!("usage: cargo xtask <command>");
    println!();
    println!("commands:");
    println!("  i18n-check     verify locale catalogues are complete against the baseline");
    println!("  build-plugins  lint + build the WASM plugin components, collecting them in target/plugins");
}

/// Lints (clippy, `-D warnings`) and builds each plugin component for `wasm32-wasip2`, then copies
/// the artifacts into `target/plugins/<id>.wasm` for the host to load (ADR 0011 §6).
///
/// Plugin crates are excluded from the workspace, so this is the only place they are compiled or
/// linted — keep it in CI so zero-warnings holds for guest code too.
fn build_plugins() -> Result<()> {
    let out_dir = Path::new(PLUGIN_OUT_DIR);
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    for plugin in PLUGINS {
        println!("build-plugins: linting {}", plugin.id);
        run_cargo(&[
            "clippy",
            "--manifest-path",
            plugin.manifest,
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
            plugin.manifest,
            "--release",
            "--target",
            PLUGIN_TARGET,
        ])?;

        let dest: PathBuf = out_dir.join(format!("{}.wasm", plugin.id));
        fs::copy(plugin.artifact, &dest)
            .with_context(|| format!("copying {} to {}", plugin.artifact, dest.display()))?;
        println!("build-plugins: {} -> {}", plugin.id, dest.display());

        // A plugin that contributes UI ships a Fluent catalogue the frontend resolves its form
        // label ids against (ADR 0012 §5); collect it next to the component as `<id>/i18n`.
        let manifest_dir = Path::new(plugin.manifest).parent().unwrap_or_else(|| Path::new("."));
        let i18n_src = manifest_dir.join("i18n");
        if i18n_src.is_dir() {
            let i18n_dest = out_dir.join(plugin.id).join("i18n");
            copy_dir(&i18n_src, &i18n_dest)?;
            println!("build-plugins: {} i18n -> {}", plugin.id, i18n_dest.display());
        }
    }

    println!(
        "build-plugins: {} component(s) ready in {}",
        PLUGINS.len(),
        out_dir.display()
    );
    Ok(())
}

/// Recursively copies the directory `src` into `dest` (creating `dest`), used to collect a plugin's
/// `i18n/` catalogue beside its built component.
fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
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
fn run_cargo(args: &[&str]) -> Result<()> {
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

/// Checks that, for every catalogue, each non-baseline locale defines every message key the baseline
/// does, exiting with an error (and a per-locale list of the missing keys) when any locale drifted.
fn i18n_check() -> Result<()> {
    let mut gaps: Vec<(String, String, Vec<String>)> = Vec::new();
    for catalogue in CATALOGUES {
        let root = Path::new(catalogue.dir);
        let baseline = message_keys(&root.join(BASELINE_LOCALE).join(catalogue.file))?;
        for locale in locale_dirs(root)? {
            if locale == BASELINE_LOCALE {
                continue;
            }
            let keys = message_keys(&root.join(&locale).join(catalogue.file))?;
            let missing: Vec<String> = baseline.difference(&keys).cloned().collect();
            if !missing.is_empty() {
                gaps.push((catalogue.dir.to_owned(), locale, missing));
            }
        }
    }

    if gaps.is_empty() {
        println!("i18n-check: all locale catalogues are complete against `{BASELINE_LOCALE}`.");
        return Ok(());
    }

    println!("i18n-check: locale catalogues are missing keys defined in `{BASELINE_LOCALE}`:");
    for (dir, locale, missing) in &gaps {
        println!("  {dir} [{locale}]: {} missing", missing.len());
        for key in missing {
            println!("    - {key}");
        }
    }
    bail!("incomplete locale catalogues (see above)");
}

/// Returns the locale subdirectory names under `root`.
fn locale_dirs(root: &Path) -> Result<Vec<String>> {
    let mut locales = Vec::new();
    let entries = fs::read_dir(root).with_context(|| format!("reading catalogue root {}", root.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("reading an entry under {}", root.display()))?;
        if entry.path().is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            locales.push(name.to_owned());
        }
    }
    locales.sort();
    Ok(locales)
}

/// Parses a Fluent catalogue, returning its message keys.
///
/// A message is a top-level `key = …` line; comments (`#`), term definitions (`-key`), and
/// continuation/indented lines are not messages and are skipped. This is a lightweight scan, not a
/// full Fluent parse — sufficient for the completeness check.
fn message_keys(path: &Path) -> Result<BTreeSet<String>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading catalogue {}", path.display()))?;
    let mut keys = BTreeSet::new();
    for line in text.lines() {
        // Messages start in column 0 (no leading whitespace) and are not comments or terms.
        if line.starts_with(|c: char| c.is_whitespace()) || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            keys.insert(key.to_owned());
        }
    }
    Ok(keys)
}
