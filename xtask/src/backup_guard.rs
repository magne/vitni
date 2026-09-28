//! `cargo xtask backup-guard` — the 1.0 backup-format freeze cannot be forgotten (ADR 0041 §4).
//!
//! Before 1.0 a restore reads the current backup format and the two before it, through the
//! temporary upgraders in `backup::upgrade::pre_release`. At 1.0 the format in force is frozen as
//! v1 and that module is deleted (#392). This check fails while the module still exists at a
//! workspace version of 1.0.0 or later.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// The temporary pre-1.0 upgrader module.
const PRE_RELEASE: &str = "crates/vitni-app/src/backup/upgrade/pre_release.rs";

/// Runs the check from the repository root.
pub fn run() -> Result<()> {
    let text = fs::read_to_string("Cargo.toml").context("reading the root Cargo.toml")?;
    let root: toml::Value = toml::from_str(&text).context("parsing the root Cargo.toml")?;
    let version = root
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .context("the root Cargo.toml declares no [workspace.package] version")?;
    let pre_release_exists = Path::new(PRE_RELEASE).exists();
    if must_freeze(version, pre_release_exists)? {
        eprintln!("  error: the workspace is at {version}, but {PRE_RELEASE} still exists");
        bail!(
            "freeze backup format v1 before 1.0: alias the current 0.K format as v1, delete {PRE_RELEASE} and \
             its older v0.* fixtures (ADR 0041 §4, #392)"
        );
    }
    println!(
        "backup-guard: ok ({version}, pre-release upgraders {})",
        presence(pre_release_exists)
    );
    Ok(())
}

fn presence(exists: bool) -> &'static str {
    if exists { "present" } else { "removed" }
}

/// Whether `version` has reached 1.0 while the pre-release upgraders are still present.
fn must_freeze(version: &str, pre_release_exists: bool) -> Result<bool> {
    let major: u64 = version
        .split('.')
        .next()
        .and_then(|major| major.parse().ok())
        .with_context(|| format!("{version:?} is not a semantic version"))?;
    Ok(major >= 1 && pre_release_exists)
}

#[cfg(test)]
mod tests {
    use super::must_freeze;

    #[test]
    fn pre_release_versions_keep_the_upgraders() {
        for version in ["0.1.0", "0.10.3", "0.99.99-rc.1"] {
            assert!(!must_freeze(version, true).unwrap_or(true), "{version}");
        }
    }

    #[test]
    fn one_point_zero_with_the_upgraders_still_present_fails() {
        for version in ["1.0.0", "1.0.0-rc.1", "2.3.4"] {
            assert!(must_freeze(version, true).unwrap_or(false), "{version}");
        }
    }

    #[test]
    fn one_point_zero_after_the_freeze_passes() {
        assert!(!must_freeze("1.0.0", false).unwrap_or(true));
    }

    #[test]
    fn a_malformed_version_is_an_error() {
        assert!(must_freeze("banana", true).is_err());
    }
}
