//! The `vitni` binary (ADR 0035): one command over both frontends — the GUI with no arguments, the
//! terminal frontend with any.
//!
//! Both arms are **library calls**, not a spawned child: there is no second process, no argv
//! re-quoting, and the CLI's [`ExitCode`] is returned rather than laundered through a wait status.
//! `main` stays sync because `dioxus::desktop` drives `tao`, which needs the process main thread;
//! `vitni_cli::run` builds and drops its own tokio runtime internally (ADR 0035 §3).
//!
//! Dispatch is argument *presence*, nothing finer, so this crate parses no arguments and cannot drift
//! from the surface it dispatches to: `vitni --workspace demo` reaches clap and gets "subcommand
//! required" (ADR 0035 §2). It also emits no user-facing strings, and so needs no Fluent catalogue
//! (ADR 0003) — the message a build with no renderer prints lives in `vitni-ui-dioxus`.
//!
//! # Licence
//!
//! `AGPL-3.0-or-later` (ADR 0034). Additional permission under GNU AGPL version 3 section 7: if you
//! modify this Program, or any covered work, by combining it with a WebAssembly component that
//! interacts with the Program solely through the versioned `vitni:host-api` WIT world (or any later
//! version of that world), the licensor grants you additional permission to convey the resulting
//! work. Such a component is not required to be licensed under the GNU AGPL.

use std::process::ExitCode;

fn main() -> ExitCode {
    if std::env::args_os().nth(1).is_some() {
        return vitni_cli::run();
    }
    vitni_ui_dioxus::run_desktop();
    ExitCode::SUCCESS
}
