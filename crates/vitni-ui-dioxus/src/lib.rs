//! `vitni-ui-dioxus` — the Dioxus renderer (ADR 0008): the GUI binary parallel to the CLI.
//!
//! It binds `vitni-ui` view-models to RSX, routes UI events to `vitni-ui` intents, and hosts
//! the vocabulary→widgets interpreter (ADR 0012). It consumes `vitni-app` through `vitni-ui`
//! and drives the plugin host directly. This is the only layer that names `dioxus::` types.
//!
//! Library + binary: the components live in the library so the SSR/interpreter test can render them
//! without a desktop window; `main.rs` is the thin GUI entry point (behind the `desktop` feature).

pub mod app;
pub mod components;
pub mod i18n;
pub mod master_detail;
pub mod media_asset;
pub mod screens;
pub mod services;
pub mod shell;
pub mod vocabulary_render;
