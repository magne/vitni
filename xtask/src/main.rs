//! Repository task runner — `cargo xtask <command>`.
//!
//! Project-local automation that does not belong in a shipped binary. Run through the `cargo xtask`
//! alias defined in `.cargo/config.toml`. Commands:
//!
//! - `i18n-check` — verify every locale catalogue is complete against the English baseline and that
//!   `fl!()` key usage matches the catalogue (ADR 0003).
//! - `build-plugins` — lint + build the WASM plugin components, collecting them in `target/plugins`
//!   (ADR 0007, 0011).

mod build_plugins;
mod i18n_check;
mod util;

use std::env;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("i18n-check") => i18n_check::run(),
        Some("build-plugins") => build_plugins::run(),
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
    println!("  i18n-check     verify locale catalogues are complete and used keys are defined");
    println!("  build-plugins  lint + build the WASM plugin components, collecting them in target/plugins");
}
