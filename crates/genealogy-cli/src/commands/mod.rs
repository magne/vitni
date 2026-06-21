//! Per-aggregate command modules: each owns its clap subcommand enum and the handler that runs it
//! against an open [`Workspace`](genealogy_app::Workspace). `main` opens the workspace once and
//! dispatches here, keeping each aggregate's surface in its own file as the model grows.

pub mod citation;
pub mod dna_test;
pub mod event;
pub mod family;
pub mod person;
pub mod place;
pub mod source;
