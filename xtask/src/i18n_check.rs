//! `i18n-check` — verifies the locale catalogues discovered across the workspace (ADR 0003).
//!
//! Catalogues are discovered by the presence of an `i18n.toml` under `crates/*` / `plugins/*`. For
//! each, every non-baseline locale must define every message the baseline (`en`) does, every key
//! used via `fl!()` in the crate's source must exist in the baseline, and unused baseline keys are
//! reported. Diagnostics are collected per crate and printed as a block; the command scans every
//! catalogue and only then exits non-zero if any error was recorded.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::util::{self, BASELINE_LOCALE, I18nConfig, key_literal_present, rust_sources, scan_fl_keys};

/// Warnings and errors collected for one crate/catalogue, printed together after its scan.
struct Diagnostics {
    label: String,
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl Diagnostics {
    fn new(label: String) -> Self {
        Self {
            label,
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn warn(&mut self, message: String) {
        self.warnings.push(message);
    }

    fn error(&mut self, message: String) {
        self.errors.push(message);
    }

    fn report(&self) {
        if self.warnings.is_empty() && self.errors.is_empty() {
            println!("  ok");
            return;
        }
        for warning in &self.warnings {
            println!("  warning: {warning}");
        }
        for error in &self.errors {
            println!("  error: {error}");
        }
    }
}

/// Runs the `i18n-check` command (see module docs).
pub fn run() -> Result<()> {
    let mut roots = util::child_dirs(Path::new("crates"))?;
    roots.extend(util::child_dirs(Path::new("plugins"))?);

    let mut error_count = 0usize;
    for dir in roots {
        let config_path = dir.join("i18n.toml");
        let mut diagnostics = Diagnostics::new(dir.display().to_string());

        if config_path.exists() {
            println!("i18n-check: {}", diagnostics.label);
            check_catalogue(&dir, &config_path, &mut diagnostics)?;
        } else if dir.join("i18n").is_dir() {
            println!("i18n-check: {}", diagnostics.label);
            diagnostics.error("has an `i18n/` directory but no `i18n.toml` to declare it".to_owned());
        } else {
            continue;
        }

        diagnostics.report();
        error_count += diagnostics.errors.len();
    }

    if error_count > 0 {
        bail!("i18n-check found {error_count} error(s) (see above)");
    }
    println!("i18n-check: all catalogues complete and consistent against `{BASELINE_LOCALE}`.");
    Ok(())
}

/// Checks one catalogue: fallback language, locale completeness, and `fl!()` key usage.
fn check_catalogue(dir: &Path, config_path: &Path, diagnostics: &mut Diagnostics) -> Result<()> {
    let config = match I18nConfig::load(config_path) {
        Ok(config) => config,
        Err(error) => {
            diagnostics.error(format!("invalid i18n.toml: {error:#}"));
            return Ok(());
        }
    };

    if config.fallback_language != BASELINE_LOCALE {
        diagnostics.error(format!(
            "fallback_language is `{}`, expected `{BASELINE_LOCALE}`",
            config.fallback_language
        ));
        return Ok(());
    }

    let root = dir.join(&config.fluent.assets_dir);
    let baseline = locale_keys(&root.join(BASELINE_LOCALE))?;
    if baseline.is_empty() {
        diagnostics.error(format!(
            "no baseline (`{BASELINE_LOCALE}`) messages found under {}",
            root.display()
        ));
        return Ok(());
    }

    check_locales(&root, &baseline, diagnostics)?;
    check_usage(dir, &baseline, diagnostics)?;
    Ok(())
}

/// Records an error for every non-baseline locale missing baseline keys.
fn check_locales(root: &Path, baseline: &BTreeSet<String>, diagnostics: &mut Diagnostics) -> Result<()> {
    for locale in locale_dirs(root)? {
        if locale == BASELINE_LOCALE {
            continue;
        }
        let keys = locale_keys(&root.join(&locale))?;
        let missing: Vec<String> = baseline.difference(&keys).cloned().collect();
        if missing.is_empty() {
            println!("  [{locale}]: complete");
        } else {
            diagnostics.error(format!("[{locale}] missing {}: {}", missing.len(), missing.join(", ")));
        }
    }
    Ok(())
}

/// Validates `fl!()` key usage in the crate's `src/`: keys used but undefined are errors, and
/// baseline keys never referenced are warnings. Non-literal `fl!()` keys are warned about because a
/// static scan cannot validate them.
fn check_usage(dir: &Path, baseline: &BTreeSet<String>, diagnostics: &mut Diagnostics) -> Result<()> {
    let src = dir.join("src");
    let mut used = BTreeSet::new();
    let mut combined = String::new();
    for file in rust_sources(&src)? {
        let text = fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        let scan = scan_fl_keys(&text);
        for key in scan.keys {
            used.insert(key);
        }
        if scan.has_dynamic_key {
            diagnostics.warn(format!("non-literal fl! key in {} — cannot validate", file.display()));
        }
        combined.push_str(&text);
        combined.push('\n');
    }

    for key in &used {
        if !baseline.contains(key) {
            diagnostics.error(format!(
                "key `{key}` used in code but absent from the baseline catalogue"
            ));
        }
    }
    for key in baseline {
        if !key_literal_present(&combined, key) {
            diagnostics.warn(format!("key `{key}` defined but never referenced in src/"));
        }
    }
    Ok(())
}

/// The locale subdirectory names under `root`, sorted.
fn locale_dirs(root: &Path) -> Result<Vec<String>> {
    let mut locales = Vec::new();
    for dir in util::child_dirs(root)? {
        if let Some(name) = dir.file_name().and_then(|name| name.to_str()) {
            locales.push(name.to_owned());
        }
    }
    Ok(locales)
}

/// The union of message keys across every `.ftl` file in a single locale directory.
fn locale_keys(dir: &Path) -> Result<BTreeSet<String>> {
    let mut keys = BTreeSet::new();
    if !dir.is_dir() {
        return Ok(keys);
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry.with_context(|| format!("reading an entry under {}", dir.display()))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "ftl") {
            keys.extend(message_keys(&path)?);
        }
    }
    Ok(keys)
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
