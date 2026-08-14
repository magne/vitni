//! Repository task runner — `cargo xtask <command>`.
//!
//! Project-local automation that does not belong in a shipped binary. Run through the `cargo xtask`
//! alias defined in `.cargo/config.toml`. Commands:
//!
//! - `i18n-check` — verify every locale catalogue is complete against the English baseline and that
//!   `fl!()` key usage matches the catalogue (ADR 0003).
//! - `build-plugins` — lint + build the WASM plugin components, collecting them in `target/plugins`
//!   (ADR 0007, 0011).
//! - `css-check` — verify the bundled component CSS hardcodes no colour literals (every colour comes
//!   from a `var(--token)` in `tokens.css`).
//! - `input-guard` — verify no RSX form element is rendered outside the guarded behavior-core
//!   primitives (so the typing guard is wired once; fixes "global keys fire inside text controls").
//! - `licence-check` — verify the per-crate licence split holds: every crate declares a licence, and
//!   no permissive crate reaches an `AGPL-3.0-or-later` one (ADR 0034).
//! - `icons` — rasterise the committed SVG icon sources into the installed PNG sizes; `--check`
//!   verifies the committed rasters instead of rewriting them (#326).
//! - `check` — run every static check above (`i18n-check`, `css-check`, `input-guard`,
//!   `licence-check`, `icons --check`) in one pass, reporting all failures rather than stopping at
//!   the first.
//! - `issue-sync` — verify the `docs/issues.md` ↔ GitHub Issues linkage: references well-formed and
//!   unique, every backlog bullet inside an `###` area. `--online` also reconciles against `gh`.
//! - `labels` — reconcile GitHub's issue labels with `.github/labels.toml` (`--apply` to write).
//! - `package` — assemble a Linux release tarball (binaries + signed plugin fleet + launcher) under
//!   `target/dist` (Phase 11 workstream C, ADR 0014 §7).
//! - `gui-pass` — run the real GUI on a headless Xvfb display, drive it with `xdotool`, and assert
//!   over screenshots of what SSR cannot reach (the `MapLibre` canvas, the overlay layer). Scenarios
//!   are TOML files under `crates/vitni-ui-dioxus/tests/gui-pass/`, so adding one needs no rebuild.
//!
//! # Licence
//!
//! `AGPL-3.0-or-later` (ADR 0034). Additional permission under GNU AGPL version 3 section 7: if you
//! modify this Program, or any covered work, by combining it with a WebAssembly component that
//! interacts with the Program solely through the versioned `vitni:host-api` WIT world (or any later
//! version of that world), the licensor grants you additional permission to convey the resulting
//! work. Such a component is not required to be licensed under the GNU AGPL.

mod build_plugins;
mod css_check;
mod gui_pass;
mod i18n_check;
mod icons;
mod input_guard;
mod issue_sync;
mod labels;
mod licence_check;
mod package;
mod util;

use std::env;

use anyhow::{Result, bail};

/// A named static check: its command name and its entry point.
type Check = (&'static str, fn() -> Result<()>);

fn main() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("i18n-check") => i18n_check::run(),
        Some("build-plugins") => build_plugins::run(),
        Some("css-check") => css_check::run(),
        Some("input-guard") => input_guard::run(),
        Some("licence-check") => licence_check::run(),
        Some("icons") => icons::run(&env::args().skip(2).collect::<Vec<String>>()),
        Some("issue-sync") => issue_sync::run(),
        Some("labels") => labels::run(),
        Some("package") => package::run(),
        Some("gui-pass") => gui_pass::run(&env::args().skip(2).collect::<Vec<String>>()),
        Some("check") => check(),
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

/// Runs every static check, reporting all failures (never stopping at the first).
fn check() -> Result<()> {
    let checks: [Check; 6] = [
        ("i18n-check", i18n_check::run),
        ("css-check", css_check::run),
        ("input-guard", input_guard::run),
        ("licence-check", licence_check::run),
        ("icons", icons::check),
        ("issue-sync", issue_sync::run),
    ];
    let mut failed = Vec::new();
    for (name, run) in checks {
        println!("\n=== {name} ===");
        if let Err(error) = run() {
            eprintln!("{name} failed: {error:#}");
            failed.push(name);
        }
    }
    if !failed.is_empty() {
        bail!("check: {} failed ({})", failed.len(), failed.join(", "));
    }
    println!("\ncheck: all static checks passed.");
    Ok(())
}

fn print_usage() {
    println!("usage: cargo xtask <command>");
    println!();
    println!("commands:");
    println!("  i18n-check     verify locale catalogues are complete and used keys are defined");
    println!("  build-plugins  lint + build the WASM plugin components, collecting them in target/plugins");
    println!("  css-check      verify bundled component CSS hardcodes no colour literals");
    println!("  input-guard    verify no RSX form element is rendered outside the input primitives");
    println!("  licence-check  verify no permissive crate reaches AGPL code and every crate declares a licence");
    println!("  icons          rasterise the SVG icon sources into the installed PNG sizes");
    println!("                 [--check]      verify the committed rasters instead of rewriting them");
    println!("  issue-sync     verify the docs/issues.md <-> GitHub Issues linkage (--online to reconcile)");
    println!("  labels         reconcile GitHub labels with .github/labels.toml (--apply to write)");
    println!(
        "  check          run every static check (i18n-check, css-check, input-guard, licence-check, icons, issue-sync)"
    );
    println!("  package        assemble a Linux release tarball (binaries + signed plugins) in target/dist");
    println!("  gui-pass       run GUI scenarios on a headless Xvfb display, asserting over screenshots");
    println!("                 [SCENARIO...]  a name or path under crates/vitni-ui-dioxus/tests/gui-pass");
    println!("                                (default: every scenario there)");
    println!("                 [--reset]      wipe the fixture workspace, isolated home and old shots");
    println!("                 [--keep]       leave Xvfb + the GUI up (attach with x11vnc -display :99)");
    println!("                 [--display :N] drive a different display (default :99)");
    println!("                 [--real-config] use your own config/workspaces instead of the fixture");
    println!("                 [--workspace NAME] open that workspace (implies --real-config)");
}
