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
use std::path::Path;

use anyhow::{Context, Result, bail};

/// The CLI catalogue root, relative to the workspace (where `cargo xtask` runs).
const CLI_I18N_DIR: &str = "crates/genealogy-cli/i18n";
/// The baseline locale every other locale must match (the embedded fallback — ADR 0003).
const BASELINE_LOCALE: &str = "en";
/// The catalogue file name within each locale directory.
const CATALOGUE_FILE: &str = "genealogy-cli.ftl";

fn main() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("i18n-check") => i18n_check(),
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
    println!("  i18n-check    verify locale catalogues are complete against the baseline");
}

/// Checks that every non-baseline locale defines every message key the baseline does, exiting with
/// an error (and a per-locale list of the missing keys) when any locale has drifted.
fn i18n_check() -> Result<()> {
    let root = Path::new(CLI_I18N_DIR);
    let baseline_path = root.join(BASELINE_LOCALE).join(CATALOGUE_FILE);
    let baseline = message_keys(&baseline_path)?;

    let mut gaps: Vec<(String, Vec<String>)> = Vec::new();
    for locale in locale_dirs(root)? {
        if locale == BASELINE_LOCALE {
            continue;
        }
        let path = root.join(&locale).join(CATALOGUE_FILE);
        let keys = message_keys(&path)?;
        let missing: Vec<String> = baseline.difference(&keys).cloned().collect();
        if !missing.is_empty() {
            gaps.push((locale, missing));
        }
    }

    if gaps.is_empty() {
        println!("i18n-check: all locale catalogues are complete against `{BASELINE_LOCALE}`.");
        return Ok(());
    }

    println!("i18n-check: locale catalogues are missing keys defined in `{BASELINE_LOCALE}`:");
    for (locale, missing) in &gaps {
        println!("  {locale}: {} missing", missing.len());
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
