//! Dispatch tests for the `vitni` launcher (ADR 0035 §2): any argument reaches the CLI.
//!
//! Every case here is deliberately **config-free** — `--version`, `--help` and a clap usage error all
//! fail or print before a workspace or the global config is touched, so these cannot write to the
//! caller's real `~/.config/vitni/config.toml` the way a `vitni init` would.
//!
//! The no-argument arm opens a window, so it is not testable here: `cargo xtask gui-pass` covers it by
//! spawning this binary with no arguments, and its fixture seeding covers the CLI arm end to end.

use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

/// `--version` is an argument, so it reaches the CLI and prints the workspace version.
#[test]
fn version_flag_reaches_the_cli() {
    Command::cargo_bin("vitni")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

/// A subcommand's help proves the whole clap surface is reachable through the launcher, not just the
/// top-level flags.
#[test]
fn subcommand_help_reaches_the_cli() {
    Command::cargo_bin("vitni")
        .unwrap()
        .args(["person", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"));
}

/// Arguments with no subcommand still reach the CLI, so clap reports the missing subcommand rather
/// than the launcher opening a window (the decision in ADR 0035 §2).
#[test]
fn global_flag_without_a_subcommand_is_a_clap_error() {
    Command::cargo_bin("vitni")
        .unwrap()
        .args(["--workspace", "demo"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"));
}
