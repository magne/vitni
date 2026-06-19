//! [`CitationState`] — the folded aggregate state used by the decision core.

use serde::{Deserialize, Serialize};

use crate::ids::{CitationId, HumanId, SourceId};

/// The folded state of a Citation aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationState {
    /// Whether `CitationCreated` has been seen.
    pub exists: bool,
    /// The citation's id (set on creation).
    pub citation_id: Option<CitationId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The source this citation points into (set on creation).
    pub source_id: Option<SourceId>,
    /// The page / locator within the source (last writer wins).
    pub page: Option<String>,
}
