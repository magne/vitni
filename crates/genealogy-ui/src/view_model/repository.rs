use super::{
    DetailTab, HistoryEntryVm, Localizer, RepositoryChangeSetRequest, RestrictionKind, RowVm, TagRef, non_blank,
};

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
    /// The localized repository-type label, if set.
    pub type_label: Option<String>,
    /// The recorded postal addresses.
    pub addresses: Vec<genealogy_app::Address>,
    /// The recorded URLs.
    pub urls: Vec<genealogy_app::Url>,
    /// The sources held by this repository.
    pub sources: Vec<SourceHeldVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The repository's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The repository's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl RepositoryDetail {
    /// Builds a detail view from a [`RepositorySummary`](genealogy_app::RepositorySummary), localizing
    /// the type label and medium labels. The History tab starts empty and is filled by the dispatcher.
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::RepositorySummary, loc: &Localizer) -> Self {
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
            type_label: summary.repository_type.as_ref().map(|t| loc.repository_type_label(t)),
            addresses: summary.addresses.clone(),
            urls: summary.urls.clone(),
            sources,
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a generic list row from a [`RepositorySummary`](genealogy_app::RepositorySummary): the
/// name, a `type · locality` subtitle, and a per-type avatar.
#[must_use]
pub fn repository_row(summary: &genealogy_app::RepositorySummary, loc: &Localizer) -> RowVm {
    let type_label = summary.repository_type.as_ref().map(|t| loc.repository_type_label(t));
    let locality = summary.addresses.first().and_then(|a| a.locality.clone());
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
fn repository_avatar(repository_type: Option<&genealogy_app::RepositoryType>) -> String {
    use genealogy_app::RepositoryType;
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
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("addresses", Some(detail.addresses.len())),
        tab("urls", Some(detail.urls.len())),
        tab("sources", Some(detail.sources.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

/// The create form's in-memory draft for a new repository (`record-editing.html` §6): an optional
/// type and name, buffered until Save. Create-only; nothing is written until Save commits a
/// [`RepositoryChangeSetRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepositoryDraft {
    /// The repository type, if chosen.
    pub repository_type: Option<genealogy_app::RepositoryType>,
    /// The repository name.
    pub name: String,
}

impl RepositoryDraft {
    /// A fresh empty draft for creating a new repository.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the operator has entered anything — the Save gate.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.repository_type.is_some() || non_blank(&self.name).is_some()
    }

    /// Builds the [`RepositoryChangeSetRequest`] the app commits on Save.
    #[must_use]
    pub fn to_request(&self) -> RepositoryChangeSetRequest {
        RepositoryChangeSetRequest {
            repository_type: self.repository_type.clone(),
            name: non_blank(&self.name),
        }
    }
}

#[cfg(test)]
mod repository_draft_tests {
    use super::RepositoryDraft;
    use genealogy_app::RepositoryType;

    #[test]
    fn a_fresh_draft_is_not_dirty() {
        assert!(!RepositoryDraft::new().is_dirty());
    }

    #[test]
    fn a_chosen_type_or_a_name_makes_the_draft_dirty() {
        assert!(
            RepositoryDraft {
                repository_type: Some(RepositoryType::Archive),
                ..RepositoryDraft::new()
            }
            .is_dirty()
        );
        assert!(
            RepositoryDraft {
                name: "Archives".to_owned(),
                ..RepositoryDraft::new()
            }
            .is_dirty()
        );
    }

    #[test]
    fn to_request_carries_the_type_and_trims_the_name() {
        let draft = RepositoryDraft {
            repository_type: Some(RepositoryType::Library),
            name: "  Public library  ".to_owned(),
        };
        let request = draft.to_request();
        assert_eq!(request.repository_type, Some(RepositoryType::Library));
        assert_eq!(request.name.as_deref(), Some("Public library"));
    }
}
