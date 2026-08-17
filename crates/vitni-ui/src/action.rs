//! The action-label vocabulary: a typed id for every button, panel title, aria-label and tooltip
//! that names a user action, replacing what used to be a `&str` id resolved through a wildcard
//! match. Framework-free — this module holds no rendering, only the closed set of actions and the
//! [`Affordance`] each one carries.
//!
//! [`Localizer::action_label`](crate::i18n::Localizer::action_label) resolves the bare localized
//! text (panel titles, aria-labels, tooltips);
//! [`Localizer::action_button`](crate::i18n::Localizer::action_button) additionally prefixes the
//! glyph an action's [`Affordance`] carries, for a button's visible label.

/// A user-facing action, named once so every caller resolves the same Fluent key the same way —
/// no `&str` id, no wildcard fallback.
///
/// Distinct from [`crate::vocabulary::Action`], the plugin-UI submit-button action (ADR 0022 §5) —
/// same word, unrelated concept; don't merge them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionLabel {
    /// Opens the add-name form.
    AddName,
    /// Opens the add-fact form.
    AddFact,
    /// Opens the add-source form.
    AddSource,
    /// Opens the attach-citation form.
    AttachCitation,
    /// Opens the attach-media form.
    AttachMedia,
    /// Opens the attach-note form.
    AttachNote,
    /// Opens the add-tag form.
    AddTag,
    /// Removes a tag from the record.
    RemoveTag,
    /// Opens the add-association form.
    AddAssociation,
    /// Opens the add-attribute form.
    AddAttribute,
    /// Opens the add-DNA-segment form.
    AddSegment,
    /// Opens the add-shared-ancestor form.
    AddSharedAncestor,
    /// Opens the add-translation form.
    AddTranslation,
    /// Opens the add-haplogroup form.
    AddHaplogroup,
    /// Opens the add-partner form.
    AddPartner,
    /// Opens the add-child form.
    AddChild,
    /// Opens the link-family-event form.
    LinkEvent,
    /// Opens the DNA match comparison tool.
    Compare,
    /// Detaches a citation from its DNA evidence.
    DetachCitation,
    /// Detaches a DNA match from its evidence.
    DetachDnaMatch,
    /// Retracts a collection row's assertion.
    Retract,
    /// Removes a row (a membership that once held).
    Remove,
    /// Unlinks a row.
    Unlink,
    /// Detaches an attachment.
    Detach,
    /// Opens a row's edit form.
    Edit,
    /// Opens a provenance-only citation form for a row.
    Cite,
    /// Opens the add-subject form (Research Note).
    AddSubject,
    /// Opens a new Research Note.
    NewResearchNote,
    /// Confirms a pending choice.
    Confirm,
    /// Rejects a pending choice.
    Reject,
    /// Cancels the open form, discarding its draft.
    Cancel,
    /// The "Saved" confirmation notice.
    Saved,
    /// The "Created" confirmation notice.
    Created,
    /// Dismisses a popover or overlay.
    Dismiss,
    /// Closes a panel or viewer.
    Close,
    /// Commits the open form.
    Save,
    /// Opens the add-address form.
    AddAddress,
    /// Opens the add-URL form.
    AddUrl,
    /// Opens the add-participant form.
    AddParticipant,
    /// Opens the add-enclosing-place form.
    AddEnclosing,
    /// Opens the link-source form.
    LinkSource,
    /// Opens the link-repository form.
    LinkRepository,
    /// Opens a new-citation draft.
    NewCitation,
    /// Opens the add-succession form (Place).
    AddSuccession,
}

/// Every [`ActionLabel`] variant, for exhaustive iteration (e.g. a sweep over every localized label).
pub const ALL: &[ActionLabel] = &[
    ActionLabel::AddName,
    ActionLabel::AddFact,
    ActionLabel::AddSource,
    ActionLabel::AttachCitation,
    ActionLabel::AttachMedia,
    ActionLabel::AttachNote,
    ActionLabel::AddTag,
    ActionLabel::RemoveTag,
    ActionLabel::AddAssociation,
    ActionLabel::AddAttribute,
    ActionLabel::AddSegment,
    ActionLabel::AddSharedAncestor,
    ActionLabel::AddTranslation,
    ActionLabel::AddHaplogroup,
    ActionLabel::AddPartner,
    ActionLabel::AddChild,
    ActionLabel::LinkEvent,
    ActionLabel::Compare,
    ActionLabel::DetachCitation,
    ActionLabel::DetachDnaMatch,
    ActionLabel::Retract,
    ActionLabel::Remove,
    ActionLabel::Unlink,
    ActionLabel::Detach,
    ActionLabel::Edit,
    ActionLabel::Cite,
    ActionLabel::AddSubject,
    ActionLabel::NewResearchNote,
    ActionLabel::Confirm,
    ActionLabel::Reject,
    ActionLabel::Cancel,
    ActionLabel::Saved,
    ActionLabel::Created,
    ActionLabel::Dismiss,
    ActionLabel::Close,
    ActionLabel::Save,
    ActionLabel::AddAddress,
    ActionLabel::AddUrl,
    ActionLabel::AddParticipant,
    ActionLabel::AddEnclosing,
    ActionLabel::LinkSource,
    ActionLabel::LinkRepository,
    ActionLabel::NewCitation,
    ActionLabel::AddSuccession,
];

/// What kind of affordance an [`ActionLabel`] represents, so a button knows whether to prefix its label
/// with a glyph. `Create`/`Attach`/`Link` are reserved for the actions whose Fluent string carries a
/// glyph today (`Cite` carries its own); most add/attach/link actions are `Chrome` — issue #303 tracks
/// giving the rest a `+` uniformly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affordance {
    /// Creates a new record or sub-record — prefixed with `+`.
    Create,
    /// Attaches an existing record — prefixed with `+`.
    Attach,
    /// Links an existing record — prefixed with `+`.
    Link,
    /// Opens a provenance-only citation form — prefixed with `❝`.
    Cite,
    /// A per-row action (edit, retract, remove, unlink, detach) — no prefix.
    Row,
    /// A generic chrome action (save, cancel, confirm, most add/attach/link buttons) — no prefix.
    Chrome,
}

impl ActionLabel {
    /// The [`Affordance`] this action carries, driving the glyph
    /// [`Localizer::action_button`](crate::i18n::Localizer::action_button) prefixes.
    #[must_use]
    pub fn affordance(self) -> Affordance {
        match self {
            Self::AddSegment
            | Self::AddSharedAncestor
            | Self::AddSubject
            | Self::NewResearchNote
            | Self::NewCitation => Affordance::Create,
            Self::Cite => Affordance::Cite,
            Self::Compare
            | Self::DetachCitation
            | Self::DetachDnaMatch
            | Self::Retract
            | Self::Remove
            | Self::Unlink
            | Self::Detach
            | Self::Edit => Affordance::Row,
            Self::AddName
            | Self::AddFact
            | Self::AddSource
            | Self::AttachCitation
            | Self::AttachMedia
            | Self::AttachNote
            | Self::AddTag
            | Self::RemoveTag
            | Self::AddAssociation
            | Self::AddAttribute
            | Self::AddTranslation
            | Self::AddHaplogroup
            | Self::AddPartner
            | Self::AddChild
            | Self::LinkEvent
            | Self::Confirm
            | Self::Reject
            | Self::Cancel
            | Self::Saved
            | Self::Created
            | Self::Dismiss
            | Self::Close
            | Self::Save
            | Self::AddAddress
            | Self::AddUrl
            | Self::AddParticipant
            | Self::AddEnclosing
            | Self::LinkSource
            | Self::LinkRepository
            | Self::AddSuccession => Affordance::Chrome,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ALL, ActionLabel, Affordance};

    /// Pins the Fluent key each [`ActionLabel`] resolves to in
    /// [`Localizer::action_label`](crate::i18n::Localizer::action_label) — kept in sync by hand, since
    /// the Fluent macro there needs a string literal per arm and can't be driven by a shared table.
    /// Every key is unique and `action-`-prefixed except [`ActionLabel::AddSuccession`], which keeps the
    /// pre-existing `place-succession-add` key (never rename a Fluent key a workspace override may
    /// reference).
    fn fluent_key(action: ActionLabel) -> &'static str {
        match action {
            ActionLabel::AddName => "action-add-name",
            ActionLabel::AddFact => "action-add-fact",
            ActionLabel::AddSource => "action-add-source",
            ActionLabel::AttachCitation => "action-attach-citation",
            ActionLabel::AttachMedia => "action-attach-media",
            ActionLabel::AttachNote => "action-attach-note",
            ActionLabel::AddTag => "action-add-tag",
            ActionLabel::RemoveTag => "action-remove-tag",
            ActionLabel::AddAssociation => "action-add-association",
            ActionLabel::AddAttribute => "action-add-attribute",
            ActionLabel::AddSegment => "action-add-segment",
            ActionLabel::AddSharedAncestor => "action-add-shared-ancestor",
            ActionLabel::AddTranslation => "action-add-translation",
            ActionLabel::AddHaplogroup => "action-add-haplogroup",
            ActionLabel::AddPartner => "action-add-partner",
            ActionLabel::AddChild => "action-add-child",
            ActionLabel::LinkEvent => "action-link-event",
            ActionLabel::Compare => "action-compare",
            ActionLabel::DetachCitation => "action-detach-citation",
            ActionLabel::DetachDnaMatch => "action-detach-dna-match",
            ActionLabel::Retract => "action-retract",
            ActionLabel::Remove => "action-remove",
            ActionLabel::Unlink => "action-unlink",
            ActionLabel::Detach => "action-detach",
            ActionLabel::Edit => "action-edit",
            ActionLabel::Cite => "action-cite",
            ActionLabel::AddSubject => "action-add-subject",
            ActionLabel::NewResearchNote => "action-new-research-note",
            ActionLabel::Confirm => "action-confirm",
            ActionLabel::Reject => "action-reject",
            ActionLabel::Cancel => "action-cancel",
            ActionLabel::Saved => "action-saved",
            ActionLabel::Created => "action-created",
            ActionLabel::Dismiss => "action-dismiss",
            ActionLabel::Close => "action-close",
            ActionLabel::Save => "action-save",
            ActionLabel::AddAddress => "action-add-address",
            ActionLabel::AddUrl => "action-add-url",
            ActionLabel::AddParticipant => "action-add-participant",
            ActionLabel::AddEnclosing => "action-add-enclosing",
            ActionLabel::LinkSource => "action-link-source",
            ActionLabel::LinkRepository => "action-link-repository",
            ActionLabel::NewCitation => "action-new-citation",
            ActionLabel::AddSuccession => "place-succession-add",
        }
    }

    #[test]
    fn every_action_key_is_unique_and_action_prefixed() {
        let mut seen = std::collections::HashSet::new();
        for action in ALL {
            let key = fluent_key(*action);
            assert!(seen.insert(key), "{action:?}'s key {key:?} is not unique");
            if *action == ActionLabel::AddSuccession {
                assert_eq!(key, "place-succession-add", "the pre-existing key must not be renamed");
            } else {
                assert!(
                    key.starts_with("action-"),
                    "{action:?}'s key {key:?} is not action-prefixed"
                );
            }
        }
    }

    #[test]
    fn all_lists_every_variant() {
        // Matching every variant here means a new one that ALL forgets is a compile error, not a
        // silently-missing sweep entry.
        fn assert_covered(action: ActionLabel) {
            match action {
                ActionLabel::AddName
                | ActionLabel::AddFact
                | ActionLabel::AddSource
                | ActionLabel::AttachCitation
                | ActionLabel::AttachMedia
                | ActionLabel::AttachNote
                | ActionLabel::AddTag
                | ActionLabel::RemoveTag
                | ActionLabel::AddAssociation
                | ActionLabel::AddAttribute
                | ActionLabel::AddSegment
                | ActionLabel::AddSharedAncestor
                | ActionLabel::AddTranslation
                | ActionLabel::AddHaplogroup
                | ActionLabel::AddPartner
                | ActionLabel::AddChild
                | ActionLabel::LinkEvent
                | ActionLabel::Compare
                | ActionLabel::DetachCitation
                | ActionLabel::DetachDnaMatch
                | ActionLabel::Retract
                | ActionLabel::Remove
                | ActionLabel::Unlink
                | ActionLabel::Detach
                | ActionLabel::Edit
                | ActionLabel::Cite
                | ActionLabel::AddSubject
                | ActionLabel::NewResearchNote
                | ActionLabel::Confirm
                | ActionLabel::Reject
                | ActionLabel::Cancel
                | ActionLabel::Saved
                | ActionLabel::Created
                | ActionLabel::Dismiss
                | ActionLabel::Close
                | ActionLabel::Save
                | ActionLabel::AddAddress
                | ActionLabel::AddUrl
                | ActionLabel::AddParticipant
                | ActionLabel::AddEnclosing
                | ActionLabel::LinkSource
                | ActionLabel::LinkRepository
                | ActionLabel::NewCitation
                | ActionLabel::AddSuccession => {}
            }
        }
        for action in ALL {
            assert_covered(*action);
        }
        let variant_count = 44;
        assert_eq!(ALL.len(), variant_count, "ALL is missing a variant");
    }

    #[test]
    fn affordance_is_exhaustive_and_stable() {
        assert_eq!(ActionLabel::Save.affordance(), Affordance::Chrome);
        assert_eq!(ActionLabel::Cite.affordance(), Affordance::Cite);
        assert_eq!(ActionLabel::AddSegment.affordance(), Affordance::Create);
        assert_eq!(ActionLabel::Retract.affordance(), Affordance::Row);
    }
}
