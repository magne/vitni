//! [`TagState`] — the folded aggregate state used by the decision core.
//!
//! Tags are the one aggregate with no assertion chain: every setter is last-writer-wins, so the
//! state carries no `Attributed`/`live_assertions` bookkeeping (data-model §9, §10).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::enums::Restriction;
use crate::ids::TagId;

/// The folded state of a Tag aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagState {
    /// Whether `TagCreated` has been seen.
    pub exists: bool,
    /// The tag's id (set on creation).
    pub tag_id: Option<TagId>,
    /// The tag's name (last writer wins).
    pub name: Option<String>,
    /// The tag's colour (last writer wins).
    pub color: Option<String>,
    /// The tag's sort priority (last writer wins).
    pub priority: Option<i32>,
    /// The tag's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
}
