//! The deny-by-default capability-grant model (ADR 0011 §2).
//!
//! Every host capability interface is always linked into a plugin, but each implementation checks
//! the instance's [`Grants`] before doing any work. A plugin starts with no grants; the caller opts
//! it into exactly the capabilities it declared.

use std::collections::HashSet;

/// A host capability a plugin may be granted (ADR 0007 §6). Ambient `files`/`net` remain denied by
/// construction (an empty WASI context); the bulk source/sink (ADR 0013) are host-mediated and so
/// have their own grants rather than relying on WASI's filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Read views as frontend-neutral DTOs.
    Query,
    /// Submit domain commands through `genealogy-app` use-cases.
    Commands,
    /// Emit structured log records into host `tracing`.
    Log,
    /// Report progress of a bulk operation to the frontend (ADR 0013).
    Progress,
    /// Read the host-opened import source (ADR 0013).
    ImportSource,
    /// Write the host-resolved export sink (ADR 0013).
    ExportSink,
    /// Perform host-mediated HTTP GETs under a net policy (ADR 0017 §2).
    Net,
    /// Write files into the workspace media library (ADR 0017 §3).
    MediaStore,
    /// Interpret media through a config-declared AI provider (ADR 0017 §4).
    Ai,
    /// Present a payload to the frontend and suspend until the user answers (ADR 0017 §5).
    Present,
}

/// The set of capabilities granted to one plugin instance. Empty by default (deny-by-default).
#[derive(Debug, Clone, Default)]
pub struct Grants {
    granted: HashSet<Capability>,
}

impl Grants {
    /// An empty grant set — every capability is denied.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Grants `capability`, returning the updated set (builder style).
    #[must_use]
    pub fn with(mut self, capability: Capability) -> Self {
        self.granted.insert(capability);
        self
    }

    /// Whether `capability` is granted to this instance.
    #[must_use]
    pub fn allows(&self, capability: Capability) -> bool {
        self.granted.contains(&capability)
    }
}
