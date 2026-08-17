use super::{
    ActionLabel, AddressVm, AttachedRefVm, DetailTab, HistoryEntryVm, Localizer, RecordDraft,
    RepositoryChangeSetRequest, RepositoryEdit, RestrictionKind, RowVm, TagRef, line_label, non_blank,
};

/// One URL recorded on a repository (Repository › URLs tab): the type · href · description plus the
/// `AssertionId` that introduced it — the target a per-row Edit supersedes and a Retract retracts
/// (ADR 0004 §2). The assertion id is never rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryUrlVm {
    /// The URL type (e.g. `website`), if recorded.
    pub url_type: Option<String>,
    /// The URL itself.
    pub href: String,
    /// A description of the URL, if recorded.
    pub description: Option<String>,
    /// The `AssertionId` (a UUID string) that introduced this URL. Never rendered.
    pub assertion_id: String,
}

/// One source held by a repository (Repository › Sources tab): the source, call number, medium, and
/// how many citations cite it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceHeldVm {
    /// The source's user-facing id (e.g. `S0001`).
    pub human_id: String,
    /// The source's stable id (a UUID string) — the navigation key.
    pub id: String,
    /// The source's display title (falls back to the `human_id`).
    pub title: String,
    /// The source's call number / shelf mark in this repository, if recorded.
    pub call_number: Option<String>,
    /// The localized medium label (book, film, electronic, …).
    pub media_type_label: String,
    /// How many citations cite the source.
    pub citation_count: usize,
}

/// A repository's detail view — type/name facts, addresses, URLs, the sources it holds, and the
/// audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryDetail {
    /// The user-facing id (e.g. `R0001`).
    pub human_id: String,
    /// The stable `RepositoryId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: the repository's name (falls back to the `human_id`).
    pub title: String,
    /// The repository's raw name, if set (seeds the whole-record editor's Name field).
    pub name: Option<String>,
    /// The repository's raw type, if set (seeds the whole-record editor's Type select).
    pub repository_type: Option<vitni_app::RepositoryType>,
    /// The localized repository-type label, if set.
    pub type_label: Option<String>,
    /// The recorded postal addresses, each with the `AssertionId` that introduced it.
    pub addresses: Vec<AddressVm>,
    /// The recorded URLs, each with the `AssertionId` that introduced it.
    pub urls: Vec<RepositoryUrlVm>,
    /// The sources held by this repository.
    pub sources: Vec<SourceHeldVm>,
    /// The attached notes, each with its attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRefVm>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The repository's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The repository's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl RepositoryDetail {
    /// Builds a detail view from a [`RepositorySummary`](vitni_app::RepositorySummary), localizing
    /// the type label and medium labels. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &vitni_app::RepositorySummary, loc: &Localizer) -> Self {
        let sources = summary
            .sources
            .iter()
            .map(|held| SourceHeldVm {
                human_id: held.source.human_id.clone(),
                id: held.source.id.clone(),
                title: held.title.clone().unwrap_or_else(|| held.source.human_id.clone()),
                call_number: held.call_number.clone(),
                media_type_label: loc.source_media_type_label(&held.media_type),
                citation_count: held.citation_count,
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title: summary.name.clone().unwrap_or_else(|| summary.human_id.clone()),
            name: summary.name.clone(),
            repository_type: summary.repository_type.clone(),
            type_label: summary.repository_type.as_ref().map(|t| loc.repository_type_label(t)),
            addresses: summary
                .addresses
                .iter()
                .map(|a| AddressVm {
                    address: a.address.clone(),
                    assertion_id: a.assertion_id.clone(),
                })
                .collect(),
            urls: summary
                .urls
                .iter()
                .map(|u| RepositoryUrlVm {
                    url_type: u.url.url_type.clone(),
                    href: u.url.href.clone(),
                    description: u.url.description.clone(),
                    assertion_id: u.assertion_id.clone(),
                })
                .collect(),
            sources,
            notes: summary.notes.iter().map(AttachedRefVm::from_ref).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a generic list row from a [`RepositorySummary`](vitni_app::RepositorySummary): the
/// name, a `type · locality` subtitle, and a per-type avatar.
#[must_use]
pub fn repository_row(summary: &vitni_app::RepositorySummary, loc: &Localizer) -> RowVm {
    let type_label = summary.repository_type.as_ref().map(|t| loc.repository_type_label(t));
    let locality = summary.addresses.first().and_then(|a| a.address.locality.clone());
    let subtitle = match (type_label, locality) {
        (Some(type_label), Some(locality)) => Some(format!("{type_label} · {locality}")),
        (Some(type_label), None) => Some(type_label),
        (None, Some(locality)) => Some(locality),
        (None, None) => None,
    };
    RowVm {
        id: summary.human_id.clone(),
        title: summary.name.clone().unwrap_or_else(|| summary.human_id.clone()),
        subtitle,
        avatar: Some(repository_avatar(summary.repository_type.as_ref())),
        ..RowVm::default()
    }
}

/// The decorative avatar glyph for a repository row, by type (a generic building otherwise).
fn repository_avatar(repository_type: Option<&vitni_app::RepositoryType>) -> String {
    use vitni_app::RepositoryType;
    match repository_type {
        Some(RepositoryType::Church) => "⛪",
        Some(RepositoryType::Cemetery) => "🪦",
        Some(RepositoryType::Library) => "📚",
        Some(RepositoryType::Website) => "🌐",
        _ => "🏛",
    }
    .to_owned()
}

/// The tab strip for a repository's detail: an overview, then the related-item tabs with counts.
#[must_use]
pub fn repository_tabs(detail: &RepositoryDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>, action: Option<ActionLabel>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
        action,
    };
    vec![
        tab("overview", None, None),
        tab("addresses", Some(detail.addresses.len()), Some(ActionLabel::AddAddress)),
        tab("urls", Some(detail.urls.len()), Some(ActionLabel::AddUrl)),
        tab("sources", Some(detail.sources.len()), Some(ActionLabel::LinkSource)),
        tab("notes", Some(detail.notes.len()), Some(ActionLabel::AttachNote)),
        tab("tags", Some(detail.tags.len()), Some(ActionLabel::AddTag)),
        tab("history", None, None),
    ]
}

/// The buffered whole-record draft of a repository (create + edit, one mechanism, `record-editing.html`
/// §2/§6): the editable user-facing id, an optional type, and the name. `existing_human_id` is `None`
/// in create mode and `Some` in edit mode (so Save creates or diffs). Nothing is written until Save.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepositoryDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit) / auto-allocated (create).
    pub human_id: String,
    /// The repository type, if chosen.
    pub repository_type: Option<vitni_app::RepositoryType>,
    /// The repository name.
    pub name: String,
    /// The repository's privacy restrictions (GEDCOM `RESN`); empty is unrestricted. Edit-only — the
    /// change-set request carries none, so a create form does not offer the field.
    pub restrictions: Vec<RestrictionKind>,
}

impl RepositoryDraft {
    /// A fresh empty draft for creating a new repository.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing repository for editing. Records the current `human_id`
    /// so [`Self::edits_against`] diffs (supersedes) rather than creates.
    #[must_use]
    pub fn from_detail(detail: &RepositoryDetail) -> Self {
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            repository_type: detail.repository_type.clone(),
            name: detail.name.clone().unwrap_or_default(),
            restrictions: detail.restrictions.clone(),
        }
    }

    /// Builds the [`RepositoryChangeSetRequest`] the app commits on Save (create mode).
    #[must_use]
    pub fn to_request(&self) -> RepositoryChangeSetRequest {
        RepositoryChangeSetRequest {
            human_id: non_blank(&self.human_id),
            repository_type: self.repository_type.clone(),
            name: non_blank(&self.name),
        }
    }

    /// The per-field edits that carry this draft from its committed `seed` to its current values (edit
    /// mode): one `Set*` per changed scalar, with `SetHumanId` emitted last so the record is only
    /// re-keyed once every other field has committed against its current id (a blank id regenerates) —
    /// the restriction set included, so it too commits against the id the record still has.
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<RepositoryEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.repository_type != seed.repository_type
            && let Some(repository_type) = self.repository_type.clone()
        {
            edits.push(RepositoryEdit::SetType {
                human_id: human_id.clone(),
                repository_type,
            });
        }
        if self.name != seed.name {
            edits.push(RepositoryEdit::SetName {
                human_id: human_id.clone(),
                name: self.name.clone(),
            });
        }
        if self.restrictions != seed.restrictions {
            edits.push(RepositoryEdit::SetRestrictions {
                human_id: human_id.clone(),
                restrictions: self.restrictions.clone(),
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(RepositoryEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }
}

impl RecordDraft for RepositoryDraft {
    type Detail = RepositoryDetail;

    fn from_detail(detail: &RepositoryDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn display_label(&self) -> Option<String> {
        line_label(&self.name)
    }

    fn editable_restrictions(&self) -> Option<&[RestrictionKind]> {
        self.existing_human_id.is_some().then_some(self.restrictions.as_slice())
    }

    fn set_restrictions(&mut self, restrictions: Vec<RestrictionKind>) {
        self.restrictions = restrictions;
    }
}

#[cfg(test)]
mod repository_draft_tests {
    use super::{RecordDraft, RepositoryDetail, RepositoryDraft};
    use crate::navigation::RepositoryEdit;
    use crate::presentation::RestrictionKind;
    use vitni_app::RepositoryType;

    fn seed() -> RepositoryDraft {
        RepositoryDraft {
            existing_human_id: Some("R0001".to_owned()),
            human_id: "R0001".to_owned(),
            repository_type: Some(RepositoryType::Library),
            name: "Public library".to_owned(),
            restrictions: vec![RestrictionKind::Confidential],
        }
    }

    fn detail() -> RepositoryDetail {
        RepositoryDetail {
            human_id: "R0009".to_owned(),
            id: "repository-uuid".to_owned(),
            title: "Archive".to_owned(),
            name: Some("Archive".to_owned()),
            repository_type: Some(RepositoryType::Archive),
            type_label: Some("Archive".to_owned()),
            addresses: Vec::new(),
            urls: Vec::new(),
            sources: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            restrictions: vec![RestrictionKind::Locked],
            history: Vec::new(),
        }
    }

    #[test]
    fn a_changed_restriction_set_yields_one_restriction_edit() {
        let draft = RepositoryDraft {
            restrictions: vec![RestrictionKind::Confidential, RestrictionKind::Privacy],
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(
            &edits[0],
            RepositoryEdit::SetRestrictions { restrictions, .. }
                if restrictions == &[RestrictionKind::Confidential, RestrictionKind::Privacy]
        ));
    }

    #[test]
    fn an_unchanged_restriction_set_yields_no_restriction_edit() {
        let draft = RepositoryDraft {
            name: "National library".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert!(
            !edits
                .iter()
                .any(|edit| matches!(edit, RepositoryEdit::SetRestrictions { .. }))
        );
    }

    #[test]
    fn from_detail_seeds_the_restrictions_and_offers_the_field() {
        let draft = RepositoryDraft::from_detail(&detail());
        assert_eq!(draft.restrictions, vec![RestrictionKind::Locked]);
        assert_eq!(
            draft.editable_restrictions(),
            Some([RestrictionKind::Locked].as_slice()),
            "a stored record offers the restriction field"
        );
    }

    #[test]
    fn a_create_draft_offers_no_restriction_field() {
        assert_eq!(RepositoryDraft::new().editable_restrictions(), None);
    }

    #[test]
    fn to_request_carries_the_type_and_trims_the_name() {
        let draft = RepositoryDraft {
            repository_type: Some(RepositoryType::Library),
            name: "  Public library  ".to_owned(),
            ..RepositoryDraft::new()
        };
        let request = draft.to_request();
        assert_eq!(request.repository_type, Some(RepositoryType::Library));
        assert_eq!(request.name.as_deref(), Some("Public library"));
        assert_eq!(request.human_id, None);
    }

    #[test]
    fn an_unchanged_draft_yields_no_edits() {
        assert!(seed().edits_against(&seed()).is_empty());
    }

    #[test]
    fn each_changed_scalar_yields_exactly_one_edit() {
        let draft = RepositoryDraft {
            name: "National library".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], RepositoryEdit::SetName { name, .. } if name == "National library"));
    }

    #[test]
    fn a_blank_human_id_regenerates() {
        let draft = RepositoryDraft {
            human_id: "   ".to_owned(),
            ..seed()
        };
        let edits = draft.edits_against(&seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], RepositoryEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
    }

    #[test]
    fn seeding_from_a_detail_is_not_dirty_against_itself() {
        let seed = RepositoryDraft::from_detail(&detail());
        assert!(seed.edits_against(&seed).is_empty());
    }
}

#[cfg(test)]
mod repository_display_label_tests {
    use super::{RecordDraft, RepositoryDraft};

    #[test]
    fn the_label_is_the_repository_name() {
        let draft = RepositoryDraft {
            name: "Public library".to_owned(),
            ..RepositoryDraft::new()
        };
        assert_eq!(draft.display_label(), Some("Public library".to_owned()));
    }

    #[test]
    fn a_draft_with_no_name_has_no_label() {
        let draft = RepositoryDraft {
            repository_type: Some(vitni_app::RepositoryType::Library),
            ..RepositoryDraft::new()
        };
        assert_eq!(draft.display_label(), None);
    }
}
