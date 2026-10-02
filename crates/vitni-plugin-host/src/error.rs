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

    /// The guest's records could not be written to the workspace (ADR 0040 §5). What was written before
    /// the failure stays, and a re-run finishes it.
    #[error("the import could not be written: {0}")]
    Commit(String),

    /// A present plugin-bundle signature was malformed, or verified against no trusted key — a
    /// present-but-unverifiable signature fails closed (ADR 0014 §3), distinct from an absent
    /// signature (unsigned, untrusted-but-loadable).
    #[error("plugin signature error: {0}")]
    Signature(String),

    /// The import's dataset could not be resolved (ADR 0037 §3): the operator named an unknown or
    /// ambiguous one, or none while the file's proposal had candidates.
    #[error("the import's dataset: {0}")]
    Dataset(vitni_app::DatasetError),
}
