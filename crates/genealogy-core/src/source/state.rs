//! [`SourceState`] — the folded aggregate state used by the decision core.

use serde::{Deserialize, Serialize};

use crate::ids::{HumanId, SourceId};

/// The folded state of a Source aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceState {
    /// Whether `SourceCreated` has been seen.
    pub exists: bool,
    /// The source's id (set on creation).
    pub source_id: Option<SourceId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The bibliographic title (last writer wins).
    pub title: Option<String>,
}
