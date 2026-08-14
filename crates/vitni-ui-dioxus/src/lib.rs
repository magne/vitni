//! `vitni-ui-dioxus` — the Dioxus renderer (ADR 0008): the GUI binary parallel to the CLI.
//!
//! It binds `vitni-ui` view-models to RSX, routes UI events to `vitni-ui` intents, and hosts
//! the vocabulary→widgets interpreter (ADR 0012). It consumes `vitni-app` through `vitni-ui`
//! and drives the plugin host directly. This is the only layer that names `dioxus::` types.
//!
//! Library only (ADR 0035 §1): the components live here so the SSR/interpreter test can render them
//! without a desktop window, and [`run_desktop`] — the window/theme entry point, behind the `desktop`
//! feature — is what the `vitni` launcher calls. Run the GUI with `cargo run -p vitni`.
//!
//! # Licence
//!
//! `AGPL-3.0-or-later` (ADR 0034). Additional permission under GNU AGPL version 3 section 7: if you
//! modify this Program, or any covered work, by combining it with a WebAssembly component that
//! interacts with the Program solely through the versioned `vitni:host-api` WIT world (or any later
//! version of that world), the licensor grants you additional permission to convey the resulting
//! work. Such a component is not required to be licensed under the GNU AGPL.

pub mod app;
pub mod components;
mod desktop;
pub mod i18n;
pub mod master_detail;
pub mod media_asset;
pub mod screens;
pub mod services;
pub mod shell;
pub mod vocabulary_render;

pub use desktop::run_desktop;
