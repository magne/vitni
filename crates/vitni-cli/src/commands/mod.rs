//! Per-aggregate command modules: each owns its clap subcommand enum and the handler that runs it
//! against an open [`Workspace`](vitni_app::Workspace). `main` opens the workspace once and
//! dispatches here, keeping each aggregate's surface in its own file as the model grows.

pub mod citation;
pub mod dna_match;
pub mod dna_test;
pub mod event;
pub mod family;
pub mod io;
pub mod media;
pub mod note;
pub mod person;
pub mod place;
pub mod plugin;
pub mod repository;
pub mod research_note;
pub mod source;
pub mod tag;
