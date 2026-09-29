//! `cargo xtask fmt` — format the workspace and every `plugins/*` crate; `--check` verifies instead.
//!
//! The plugin crates are excluded from the workspace (they build only for `wasm32-wasip2`), so
//! `cargo fmt --all` never reaches them. Each is formatted through its own manifest, under the same
//! root `rustfmt.toml`.

use std::path::Path;

use anyhow::{Result, bail};

use crate::util;

/// Formats (or with `--check`, verifies) the workspace, then each plugin crate, reporting every
/// unformatted crate rather than stopping at the first.
pub fn run(args: &[String]) -> Result<()> {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => bail!("usage: cargo xtask fmt [--check]"),
    };
    let mode: &[&str] = if check { &["--check"] } else { &[] };

    let mut failed = Vec::new();
    if util::run_cargo(&[&["fmt", "--all"], mode].concat()).is_err() {
        failed.push("workspace".to_owned());
    }
    for dir in util::child_dirs(Path::new("plugins"))? {
        let manifest = dir.join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        let manifest = manifest.display().to_string();
        if util::run_cargo(&[&["fmt", "--manifest-path", manifest.as_str()], mode].concat()).is_err() {
            failed.push(dir.display().to_string());
        }
    }
    if !failed.is_empty() {
        bail!(
            "fmt: {} not formatted ({}); run `cargo xtask fmt`",
            failed.len(),
            failed.join(", ")
        );
    }
    println!("fmt: workspace and plugins formatted.");
    Ok(())
}
