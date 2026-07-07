//! The `RecordDraft` contract shared by every editable record's buffered draft (PR27).
//!
//! A whole-record editor buffers a draft against the committed `seed`; dirtiness is `draft != seed`
//! (via [`PartialEq`]) and Save is gated on dirty *and* [`RecordDraft::is_valid`]. Create starts from
//! [`Default::default`]. The renderer's `RecordEditState` is generic over this trait so one component
//! drives create, view, and edit for every aggregate (`record-editing.html` §1–§7).

/// A buffered, editable draft of one record's scalar fields.
///
/// Implementors keep the raw field values a form binds to, plus enough identity to know whether Save
/// creates a new record or edits an existing one. Dirtiness and validity are the two gates the shared
/// editor reads: the Save action is enabled only when the draft both differs from its seed and is
/// valid.
pub trait RecordDraft: Clone + PartialEq + Default + 'static {
    /// The detail view-model an edit draft is seeded from.
    type Detail;

    /// Seeds an edit draft from the record's current detail. The draft records the record's identity,
    /// so a subsequent Save edits (supersedes) rather than creates.
    fn from_detail(detail: &Self::Detail) -> Self;

    /// Whether every field is present and valid — the Save gate (together with dirtiness).
    fn is_valid(&self) -> bool;

    /// Whether the draft differs from its committed `seed` (an unsaved change).
    fn is_dirty_against(&self, seed: &Self) -> bool {
        self != seed
    }
}
