//! Repository task runner — `cargo xtask <command>`.
//!
//! Project-local automation that does not belong in a shipped binary (catalogue checks,
//! maintenance routines). Run through the `cargo xtask` alias defined in `.cargo/config.toml`.
//! Subcommands are added as the project needs them; the first is `i18n-check` (locale catalogue
//! completeness), landing with the localization work.

use std::env;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    print_usage();
    if let Some(other) = env::args().nth(1) {
        bail!("unknown xtask command: {other}");
    }
    bail!("no xtask command given");
}

fn print_usage() {
    println!("usage: cargo xtask <command>");
    println!();
    println!("commands:");
    println!("  (none yet — subcommands land with the work that needs them)");
}
