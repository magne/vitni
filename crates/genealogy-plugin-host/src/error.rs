//! Errors surfaced by the plugin host.

use thiserror::Error;

/// A failure loading, instantiating, or running a plugin component.
#[derive(Debug, Error)]
pub enum PluginError {
    /// The Wasmtime engine could not be configured or a component failed to load/instantiate.
    #[error("plugin runtime error: {0}")]
    Runtime(String),

    /// The guest exhausted its resource budget (fuel or memory) and was stopped (ADR 0011 §4).
    #[error("plugin exceeded its resource limit: {0}")]
    ResourceLimit(String),

    /// The guest ran to completion but returned an error from its entry point.
    #[error("plugin reported an error: {0}")]
    Guest(String),
}
