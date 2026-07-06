use super::{DetailTab, HistoryEntryVm, Localizer, RestrictionKind, RowVm, TagRef, UsingRecordVm, nav_ref};

/// A match this kit produced — one row on the DNA test › Matches tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaTestMatchVm {
    /// The match record, navigable (to the DNA-match detail).
    pub match_ref: UsingRecordVm,
    /// The other test compared against this kit, navigable, if still projected.
    pub compared_test: Option<UsingRecordVm>,
    /// Total shared centimorgans, rendered for display.
    pub shared_cm: Option<String>,
    /// Shared percentage, rendered for display.
    pub percent_shared: Option<String>,
    /// The provider's predicted relationship, if any.
    pub predicted: Option<String>,
}

/// A DNA test's detail view — kit metadata, the anchoring person, haplogroups, the matches it
/// produced, attached notes, tags, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaTestDetail {
    /// The user-facing id (e.g. `D0001`).
    pub human_id: String,
    /// The stable `DnaTestId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: `provider — person` (falls back to the `human_id`).
    pub title: String,
    /// The testing provider's localized label, if set.
    pub provider: Option<String>,
    /// The test type's localized label, if set.
    pub test_type: Option<String>,
    /// The provider's kit id, if set.
    pub kit_id: Option<String>,
    /// The genome build's localized label, if set.
    pub genome_build: Option<String>,
    /// The anchoring person, navigable, if still projected.
    pub person: Option<UsingRecordVm>,
    /// The anchoring person's display name, if resolvable.
    pub person_name: Option<String>,
    /// The recorded haplogroups (the Haplogroups tab).
    pub haplogroups: Vec<String>,
    /// The matches this kit produced (the Matches tab).
    pub matches: Vec<DnaTestMatchVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The test's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The test's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl DnaTestDetail {
    /// Builds a detail view from a [`DnaTestSummary`](genealogy_app::DnaTestSummary).
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::DnaTestSummary, loc: &Localizer) -> Self {
        let provider = summary.provider.as_ref().map(|p| loc.dna_provider_label(p));
        let title = match (provider.clone(), summary.person_name.clone()) {
            (Some(provider), Some(person)) => format!("{provider} — {person}"),
            (Some(provider), None) => provider,
            (None, Some(person)) => person,
            (None, None) => summary.human_id.clone(),
        };
        let person = summary.person.as_ref().map(|p| {
            let label = summary.person_name.clone().unwrap_or_else(|| p.human_id.clone());
            nav_ref(genealogy_app::UsingKind::Person, &p.human_id, &p.id, label, loc)
        });
        let matches = summary
            .matches
            .iter()
            .map(|m| DnaTestMatchVm {
                match_ref: nav_ref(
                    genealogy_app::UsingKind::DnaMatch,
                    &m.dna_match.human_id,
                    &m.dna_match.id,
                    m.dna_match.human_id.clone(),
                    loc,
                ),
                compared_test: m.compared_test.as_ref().map(|t| {
                    nav_ref(
                        genealogy_app::UsingKind::DnaTest,
                        &t.human_id,
                        &t.id,
                        t.human_id.clone(),
                        loc,
                    )
                }),
                shared_cm: m.shared_cm.clone(),
                percent_shared: m.percent_shared.clone(),
                predicted: m.predicted_relationship.clone(),
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title,
            provider,
            test_type: summary.test_type.map(|t| loc.dna_test_type_label(t)),
            kit_id: summary.kit_id.clone(),
            genome_build: summary.genome_build.map(|b| loc.dna_genome_build_label(b)),
            person,
            person_name: summary.person_name.clone(),
            haplogroups: summary.haplogroups.clone(),
            matches,
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a list row from a [`DnaTestSummary`](genealogy_app::DnaTestSummary): the person name (or id),
/// a `provider · type` subtitle, and a 🧬 avatar.
#[must_use]
pub fn dna_test_row(summary: &genealogy_app::DnaTestSummary, loc: &Localizer) -> RowVm {
    let mut parts: Vec<String> = Vec::new();
    if let Some(provider) = &summary.provider {
        parts.push(loc.dna_provider_label(provider));
    }
    if let Some(test_type) = summary.test_type {
        parts.push(loc.dna_test_type_label(test_type));
    }
    let subtitle = (!parts.is_empty()).then(|| parts.join(" · "));
    RowVm {
        id: summary.human_id.clone(),
        title: summary.person_name.clone().unwrap_or_else(|| summary.human_id.clone()),
        subtitle,
        avatar: Some("🧬".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a DNA test's detail: overview, then haplogroups/matches/notes/tags with counts.
#[must_use]
pub fn dna_test_tabs(detail: &DnaTestDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("haplogroups", Some(detail.haplogroups.len())),
        tab("matches", Some(detail.matches.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}
