//! The `vitni-cli` binary: the terminal frontend, installable with no webview dependency.
//!
//! Everything it does lives in the library ([`vitni_cli::run`]), which the `vitni` launcher calls too
//! (ADR 0035).

use std::process::ExitCode;

fn main() -> ExitCode {
    vitni_cli::run()
}
