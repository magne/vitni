use super::{
    DetailTab, HistoryEntryVm, Localizer, RecordDraft, RestrictionKind, RowVm, TagChangeSetRequest, UsingRecordVm,
    line_label, using_record_vm,
};

/// One object-type group on the Tag Usage tab: the localized kind, the count, and a few examples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagUsageGroupVm {
    /// The localized object-type label (the row's first cell).
    pub kind_label: String,
    /// How many records of this kind carry the tag.
    pub count: usize,
    /// The first few carrying records, navigable.
    pub examples: Vec<UsingRecordVm>,
}

/// A tag's detail view — its name, colour, priority, the records that carry it grouped by type, and
/// the audit history. The tag's UUID is the join key but is never rendered (data-model §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDetail {
    /// The stable `TagId` (a UUID string) — the navigation/join key, never rendered.
    pub id: String,
    /// The header title: the tag name (falls back to a placeholder).
    pub title: String,
    /// The tag's name, if set (carried for the edit form).
    pub name: Option<String>,
    /// The tag's colour (a CSS hex string), if set.
    pub color: Option<String>,
    /// The tag's sort priority, if set.
    pub priority: Option<i32>,
    /// How many records carry this tag in total (the header subtitle).
    pub total: usize,
    /// The records carrying this tag, grouped by object type (the Usage tab).
    pub usage: Vec<TagUsageGroupVm>,
    /// The tag's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The tag's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl TagDetail {
    /// Builds a detail view from a [`TagSummary`](vitni_app::TagSummary).
    #[must_use]
    pub fn from_summary(summary: &vitni_app::TagSummary, loc: &Localizer) -> Self {
        let total = summary.usage_count;
        let usage = summary
            .usage
            .iter()
            .map(|group| TagUsageGroupVm {
                kind_label: loc.using_kind_label(group.kind),
                count: group.count,
                examples: group.examples.iter().map(|u| using_record_vm(u, loc)).collect(),
            })
            .collect();
        Self {
            id: summary.id.clone(),
            title: summary.name.clone().unwrap_or_else(|| loc.display_name(None)),
            name: summary.name.clone(),
            color: summary.color.clone(),
            priority: summary.priority,
            total,
            usage,
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// The default sort priority a fresh tag draft seeds (data-model §9; the mockup's `priority 1`).
pub const DEFAULT_TAG_PRIORITY: i32 = 1;

/// The default colour a fresh tag draft seeds (a CSS hex string — the mockup's neutral swatch).
pub const DEFAULT_TAG_COLOR: &str = "#1A2129";

/// The buffered state of the directly-editable tag record (create + edit, one mechanism). The Dioxus
/// tag Overview binds its Name / Priority / Colour inputs to these fields; nothing is persisted until
/// Save, when [`Self::to_request`] turns the buffer into a [`TagChangeSetRequest`]. Cancel drops it.
///
/// One value serves both modes: [`Self::new`] seeds the create defaults (empty name, priority 1,
/// colour `#1A2129`), [`Self::from_detail`] is pre-populated (edit) and records the tag's id in
/// `existing_id`. Priority is held as the raw text the number spinner emits, so an in-progress empty
/// or non-numeric entry is representable (and flagged invalid) rather than silently coerced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDraft {
    /// `Some` in edit mode (the tag being edited); `None` in create mode.
    pub existing_id: Option<String>,
    /// The tag's name (required, non-empty).
    pub name: String,
    /// The tag's sort priority, as the raw spinner text (required; must parse to an `i32`).
    pub priority: String,
    /// The tag's colour, a CSS hex string (required, non-empty).
    pub color: String,
    /// The tag's desired privacy restrictions (GEDCOM `RESN`); empty is unrestricted.
    pub restrictions: Vec<RestrictionKind>,
}

impl TagDraft {
    /// A fresh draft for creating a new tag: empty name, the default priority, and the default colour.
    #[must_use]
    pub fn new() -> Self {
        Self {
            existing_id: None,
            name: String::new(),
            priority: DEFAULT_TAG_PRIORITY.to_string(),
            color: DEFAULT_TAG_COLOR.to_owned(),
            restrictions: Vec::new(),
        }
    }

    /// A draft pre-populated from an existing tag for editing. Records the id so the commit edits
    /// (diffs) rather than creates; seeds each unset field with its create default.
    #[must_use]
    pub fn from_detail(detail: &TagDetail) -> Self {
        Self {
            existing_id: Some(detail.id.clone()),
            name: detail.name.clone().unwrap_or_default(),
            priority: detail
                .priority
                .map_or_else(|| DEFAULT_TAG_PRIORITY.to_string(), |p| p.to_string()),
            color: detail.color.clone().unwrap_or_else(|| DEFAULT_TAG_COLOR.to_owned()),
            restrictions: detail.restrictions.clone(),
        }
    }

    /// This draft's restrictions with `kind` toggled on/off, kept in [`RestrictionKind::all`]'s
    /// canonical order regardless of toggle order — so an unchanged set compares equal (`PartialEq`)
    /// no matter which restriction was toggled last, keeping the Save-dirty and diff checks accurate.
    #[must_use]
    pub fn toggle_restriction(&self, kind: RestrictionKind) -> Vec<RestrictionKind> {
        let mut next = self.restrictions.clone();
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

    /// The priority parsed from the spinner text, or `None` when empty / non-numeric (invalid).
    #[must_use]
    pub fn parsed_priority(&self) -> Option<i32> {
        self.priority.trim().parse::<i32>().ok()
    }

    /// Whether the name is missing (empty / whitespace) — the name field's validation state.
    #[must_use]
    pub fn name_missing(&self) -> bool {
        self.name.trim().is_empty()
    }

    /// Whether the colour is missing (empty / whitespace) — the colour field's validation state.
    #[must_use]
    pub fn color_missing(&self) -> bool {
        self.color.trim().is_empty()
    }

    /// Whether the priority text fails to parse to an `i32` — the priority field's validation state.
    #[must_use]
    pub fn priority_invalid(&self) -> bool {
        self.parsed_priority().is_none()
    }

    /// Whether every field is present and valid (name non-empty, priority a number, colour non-empty)
    /// — the Save gate.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && self.parsed_priority().is_some() && !self.color.trim().is_empty()
    }

    /// Builds the [`TagChangeSetRequest`] the app commits on Save, or `None` when the draft is
    /// invalid (so Save is a no-op rather than committing a partial tag).
    #[must_use]
    pub fn to_request(&self) -> Option<TagChangeSetRequest> {
        let priority = self.parsed_priority()?;
        if self.name.trim().is_empty() || self.color.trim().is_empty() {
            return None;
        }
        Some(TagChangeSetRequest {
            existing_id: self.existing_id.clone(),
            name: self.name.trim().to_owned(),
            priority,
            color: self.color.trim().to_owned(),
            restrictions: self.restrictions.clone(),
        })
    }
}

impl Default for TagDraft {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordDraft for TagDraft {
    type Detail = TagDetail;

    fn from_detail(detail: &TagDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        Self::is_valid(self)
    }

    fn display_label(&self) -> Option<String> {
        line_label(&self.name)
    }
}

/// Builds a list row from a [`TagSummary`](vitni_app::TagSummary): the name, a `priority N · X
/// objects` subtitle, a colour-dot avatar, and the `#hex` colour as the trailing id label. The
/// navigation key is the tag's stable id (a UUID, never rendered — data-model §9).
#[must_use]
pub fn tag_row(summary: &vitni_app::TagSummary, loc: &Localizer) -> RowVm {
    let priority = summary.priority.unwrap_or(DEFAULT_TAG_PRIORITY);
    RowVm {
        id: summary.id.clone(),
        title: summary.name.clone().unwrap_or_else(|| loc.display_name(None)),
        subtitle: Some(loc.tag_row_subtitle(priority, summary.usage_count)),
        avatar: None,
        // A tag with no colour still gets a (neutral) dot so the avatar column is consistent, and an
        // empty id label — never the UUID, which `display_id` would otherwise fall back to (§9).
        dot_color: Some(summary.color.clone().unwrap_or_else(|| DEFAULT_TAG_COLOR.to_owned())),
        id_label: Some(summary.color.clone().unwrap_or_default()),
    }
}

/// The tab strip for a tag's detail: overview, usage (with the total count), and history.
#[must_use]
pub fn tag_tabs(detail: &TagDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("usage", Some(detail.total)),
        tab("history", None),
    ]
}

#[cfg(test)]
mod tag_draft_tests {
    use super::TagDraft;
    use crate::presentation::RestrictionKind;

    fn seed() -> TagDraft {
        TagDraft {
            existing_id: Some("9f3a8c12-4e7b-7a05-9f1e-2c9d8b7a6453".to_owned()),
            name: "Direct ancestor".to_owned(),
            priority: "1".to_owned(),
            color: "#e5534b".to_owned(),
            restrictions: vec![RestrictionKind::Confidential],
        }
    }

    #[test]
    fn to_request_carries_restrictions() {
        let draft = TagDraft {
            restrictions: vec![RestrictionKind::Confidential, RestrictionKind::Privacy],
            ..seed()
        };
        let request = draft.to_request().expect("valid draft");
        assert_eq!(
            request.restrictions,
            vec![RestrictionKind::Confidential, RestrictionKind::Privacy]
        );
    }

    #[test]
    fn a_fresh_create_draft_has_no_restrictions() {
        assert!(TagDraft::new().restrictions.is_empty());
    }

    #[test]
    fn toggle_restriction_adds_in_canonical_order_regardless_of_click_order() {
        let draft = TagDraft {
            restrictions: Vec::new(),
            ..seed()
        };
        // Toggle Privacy first, then Confidential; canonical order (Confidential before Privacy)
        // should still win, not click order.
        let after_privacy = draft.toggle_restriction(RestrictionKind::Privacy);
        let draft = TagDraft {
            restrictions: after_privacy,
            ..draft
        };
        let after_confidential = draft.toggle_restriction(RestrictionKind::Confidential);
        assert_eq!(
            after_confidential,
            vec![RestrictionKind::Confidential, RestrictionKind::Privacy]
        );
    }

    #[test]
    fn toggle_restriction_removes_an_already_selected_kind() {
        let draft = seed();
        let toggled = draft.toggle_restriction(RestrictionKind::Confidential);
        assert!(toggled.is_empty());
    }
}

#[cfg(test)]
mod tag_display_label_tests {
    use super::{RecordDraft, TagDraft};

    #[test]
    fn the_label_is_the_tag_name() {
        let draft = TagDraft {
            name: "Direct ancestor".to_owned(),
            ..TagDraft::new()
        };
        assert_eq!(draft.display_label(), Some("Direct ancestor".to_owned()));
    }

    #[test]
    fn a_draft_with_no_name_has_no_label() {
        let draft = TagDraft {
            priority: "7".to_owned(),
            ..TagDraft::new()
        };
        assert_eq!(draft.display_label(), None);
    }
}
