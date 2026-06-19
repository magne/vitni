//! The deny-by-default capability-grant model (ADR 0011 §2).
//!
//! Every host capability interface is always linked into a plugin, but each implementation checks
//! the instance's [`Grants`] before doing any work. A plugin starts with no grants; the caller opts
//! it into exactly the capabilities it declared.

use std::collections::HashSet;

/// A host capability a plugin may be granted (ADR 0007 §6). `files`/`net` are denied by
/// construction in the spike (an empty WASI context) and so have no variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Read views as frontend-neutral DTOs.
    Query,
    /// Submit domain commands through `genealogy-app` use-cases.
    Commands,
    /// Emit structured log records into host `tracing`.
    Log,
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
