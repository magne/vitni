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
    /// Submit domain commands through `vitni-app` use-cases.
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

impl Capability {
    /// The canonical short interface name (`log`, `query`, `commands`, `progress`, `import-source`,
    /// `export-sink`, `net`, `media-store`, `ai`, `present`) — the string used in a `plugin.toml`
    /// `capabilities` entry, in discovery, and in the persisted per-plugin approved-grant set
    /// (ADR 0014 §5). The single source of truth for the enum→name direction.
    #[must_use]
    pub const fn interface_name(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Commands => "commands",
            Self::Log => "log",
            Self::Progress => "progress",
            Self::ImportSource => "import-source",
            Self::ExportSink => "export-sink",
            Self::Net => "net",
            Self::MediaStore => "media-store",
            Self::Ai => "ai",
            Self::Present => "present",
        }
    }

    /// The [`Capability`] a canonical short interface name denotes, or `None` for an unknown name.
    /// The inverse of [`Self::interface_name`] and the single source of truth for the name→enum
    /// direction (both discovery's manifest cross-check and the grant resolver share it).
    #[must_use]
    pub fn from_interface_name(name: &str) -> Option<Self> {
        match name {
            "query" => Some(Self::Query),
            "commands" => Some(Self::Commands),
            "log" => Some(Self::Log),
            "progress" => Some(Self::Progress),
            "import-source" => Some(Self::ImportSource),
            "export-sink" => Some(Self::ExportSink),
            "net" => Some(Self::Net),
            "media-store" => Some(Self::MediaStore),
            "ai" => Some(Self::Ai),
            "present" => Some(Self::Present),
            _ => None,
        }
    }
}

/// The set of capabilities granted to one plugin instance. Empty by default (deny-by-default).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
