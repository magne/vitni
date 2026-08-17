//! The `RecordDraft` contract shared by every editable record's buffered draft (PR27).
//!
//! A whole-record editor buffers a draft against the committed `seed`; dirtiness is `draft != seed`
//! (via [`PartialEq`]) and Save is gated on dirty *and* [`RecordDraft::is_valid`]. Create starts from
//! [`Default::default`]. The renderer's `RecordEditState` is generic over this trait so one component
//! drives create, view, and edit for every aggregate (`record-editing.html` §1–§7).

use crate::presentation::RestrictionKind;

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

    /// How the record names itself from what has been typed so far, or `None` while nothing typed
    /// names it — the label an unsaved draft's tab shows, and **the same string the stored record is
    /// labelled with on commit**, so a tab does not rename itself the moment it is saved.
    ///
    /// Free-text fields go through `line_label`, so a whole note or path can never become a tab label.
    ///
    /// `None` is a decision, not an omission: four aggregates are titled by something a draft cannot
    /// know — an Event by its *localized* type (this trait takes no `Localizer`), a Citation by the
    /// source it cites, and a `DnaTest` / `DnaMatch` by person names the draft holds only ids for. Those
    /// return `None` and their tabs keep the localized "New <entity>".
    fn display_label(&self) -> Option<String>;

    /// The restriction set the shared restriction field binds to, or `None` when the record must not
    /// offer the field at all.
    ///
    /// `None` is the create form of the twelve aggregates whose `*ChangeSetRequest` carries no
    /// restrictions: offering the field there would silently drop what the operator toggled, so a
    /// draft with no stored record behind it hides it and the restrictions become editable once the
    /// record exists. `TagDraft` is the exception — `TagChangeSetRequest` does carry them, so its
    /// create form offers the field too.
    fn editable_restrictions(&self) -> Option<&[RestrictionKind]>;

    /// Replaces the draft's restriction set (what the shared restriction field writes on a toggle).
    /// The set rides the record's own Save, so nothing is committed until then.
    fn set_restrictions(&mut self, restrictions: Vec<RestrictionKind>);

    /// Whether the draft differs from its committed `seed` (an unsaved change).
    fn is_dirty_against(&self, seed: &Self) -> bool {
        self != seed
    }
}

/// `restrictions` with `kind` toggled on/off, kept in [`RestrictionKind::all`]'s canonical order
/// regardless of toggle order — so an unchanged set compares equal (`PartialEq`) no matter which
/// restriction was toggled last, keeping the Save-dirty and diff checks accurate.
#[must_use]
pub fn toggled_restrictions(restrictions: &[RestrictionKind], kind: RestrictionKind) -> Vec<RestrictionKind> {
    let mut next = restrictions.to_vec();
    if let Some(position) = next.iter().position(|&existing| existing == kind) {
        next.remove(position);
    } else {
        next.push(kind);
    }
    RestrictionKind::all()
        .into_iter()
        .filter(|k| next.contains(k))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{RestrictionKind, toggled_restrictions};

    #[test]
    fn a_toggle_adds_in_canonical_order_regardless_of_click_order() {
        // Toggle Privacy first, then Confidential; canonical order (Confidential before Privacy)
        // still wins, so the set compares equal however it was clicked together.
        let after_privacy = toggled_restrictions(&[], RestrictionKind::Privacy);
        let after_confidential = toggled_restrictions(&after_privacy, RestrictionKind::Confidential);
        assert_eq!(
            after_confidential,
            vec![RestrictionKind::Confidential, RestrictionKind::Privacy]
        );
    }

    #[test]
    fn a_toggle_removes_an_already_selected_kind() {
        let toggled = toggled_restrictions(&[RestrictionKind::Confidential], RestrictionKind::Confidential);
        assert!(toggled.is_empty());
    }
}
