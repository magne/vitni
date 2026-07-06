use super::{DetailTab, HistoryEntryVm, Localizer, RestrictionKind, RowVm, TagRef, UsingRecordVm, nav_ref};

/// One matching segment on the DNA match › Segments tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaSegmentVm {
    /// The chromosome (`1`..=`22` or `X`).
    pub chromosome: String,
    /// The start position (base pairs), rendered.
    pub start: String,
    /// The end position (base pairs), rendered.
    pub end: String,
    /// The segment length in centimorgans, rendered.
    pub centimorgans: String,
    /// The matching-SNP count, rendered, if known.
    pub snps: Option<String>,
    /// The localized parental-side (phasing) label.
    pub side: String,
}

/// One inferred shared ancestor on the DNA match › Shared ancestors tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedAncestorVm {
    /// The inferred ancestor, navigable to the Person record, if identified.
    pub person: Option<UsingRecordVm>,
    /// The free-text note describing the shared ancestry, if any.
    pub note: Option<String>,
}

/// A DNA match's detail view — the two compared tests, the observed shared-DNA totals, the inferred
/// relationship, segments, shared ancestors, notes, tags, and the audit history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaMatchDetail {
    /// The user-facing id (e.g. `X0001`).
    pub human_id: String,
    /// The stable `DnaMatchId` (a UUID string) — the navigation/join key.
    pub id: String,
    /// The header title: `person A ⟷ person B` (falls back to the test ids or the `human_id`).
    pub title: String,
    /// One side's test, navigable, if still projected.
    pub test_a: Option<UsingRecordVm>,
    /// The other side's test, navigable, if still projected.
    pub test_b: Option<UsingRecordVm>,
    /// The provider the match was observed at, localized, if set.
    pub provider: Option<String>,
    /// Total shared centimorgans, rendered.
    pub shared_cm: Option<String>,
    /// Shared percentage, rendered.
    pub percent_shared: Option<String>,
    /// The largest shared segment's length, rendered.
    pub largest_segment_cm: Option<String>,
    /// The provider's predicted relationship, if any (the inferred-relationship conclusion).
    pub predicted_relationship: Option<String>,
    /// The localized confirmation-status label.
    pub status: String,
    /// The recorded shared segments (the Segments tab).
    pub segments: Vec<DnaSegmentVm>,
    /// The inferred shared ancestors (the Shared ancestors tab).
    pub shared_ancestors: Vec<SharedAncestorVm>,
    /// The `human_id`s of attached notes.
    pub notes: Vec<String>,
    /// The applied tags, by name + colour (never by id).
    pub tags: Vec<TagRef>,
    /// The match's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// The match's change log, newest first (History tab); filled by the dispatcher.
    pub history: Vec<HistoryEntryVm>,
}

impl DnaMatchDetail {
    /// Builds a detail view from a [`DnaMatchSummary`](genealogy_app::DnaMatchSummary).
    #[must_use]
    pub fn from_summary(summary: &genealogy_app::DnaMatchSummary, loc: &Localizer) -> Self {
        let test_ref = |test: &Option<genealogy_app::AggRef>, person: &Option<String>| {
            test.as_ref().map(|t| {
                let label = person.clone().unwrap_or_else(|| t.human_id.clone());
                nav_ref(genealogy_app::UsingKind::DnaTest, &t.human_id, &t.id, label, loc)
            })
        };
        let test_a = test_ref(&summary.test_a, &summary.test_a_person);
        let test_b = test_ref(&summary.test_b, &summary.test_b_person);
        let title = match (summary.test_a_person.clone(), summary.test_b_person.clone()) {
            (Some(a), Some(b)) => format!("{a} ⟷ {b}"),
            _ => summary.human_id.clone(),
        };
        let segments = summary
            .segments
            .iter()
            .map(|s| DnaSegmentVm {
                chromosome: s.chromosome.clone(),
                start: s.start.to_string(),
                end: s.end.to_string(),
                centimorgans: s.centimorgans.to_string(),
                snps: s.snps.map(|n| n.to_string()),
                side: loc.chromosome_side_label(s.side),
            })
            .collect();
        let shared_ancestors = summary
            .shared_ancestors
            .iter()
            .map(|a| SharedAncestorVm {
                person: a.person.as_ref().map(|p| {
                    let label = a.person_name.clone().unwrap_or_else(|| p.human_id.clone());
                    nav_ref(genealogy_app::UsingKind::Person, &p.human_id, &p.id, label, loc)
                }),
                note: a.note.clone(),
            })
            .collect();
        Self {
            human_id: summary.human_id.clone(),
            id: summary.id.clone(),
            title,
            test_a,
            test_b,
            provider: summary.provider.as_ref().map(|p| loc.dna_provider_label(p)),
            shared_cm: summary.shared_cm.clone(),
            percent_shared: summary.percent_shared.clone(),
            largest_segment_cm: summary.largest_segment_cm.clone(),
            predicted_relationship: summary.predicted_relationship.clone(),
            status: loc.match_status_label(summary.status),
            segments,
            shared_ancestors,
            notes: summary.notes.iter().map(|note| note.human_id.clone()).collect(),
            tags: summary.tags.clone(),
            restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
            history: Vec::new(),
        }
    }
}

/// Builds a list row from a [`DnaMatchSummary`](genealogy_app::DnaMatchSummary): `A ⟷ B`, a
/// `shared cM · predicted` subtitle, and a 🔗 avatar.
#[must_use]
pub fn dna_match_row(summary: &genealogy_app::DnaMatchSummary, loc: &Localizer) -> RowVm {
    let title = match (summary.test_a_person.clone(), summary.test_b_person.clone()) {
        (Some(a), Some(b)) => format!("{a} ⟷ {b}"),
        _ => summary.human_id.clone(),
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(shared) = &summary.shared_cm {
        parts.push(format!("{shared} {}", loc.field_label("centimorgans")));
    }
    if let Some(predicted) = &summary.predicted_relationship {
        parts.push(predicted.clone());
    }
    let subtitle = (!parts.is_empty()).then(|| parts.join(" · "));
    RowVm {
        id: summary.human_id.clone(),
        title,
        subtitle,
        avatar: Some("🔗".to_owned()),
        ..RowVm::default()
    }
}

/// The tab strip for a DNA match's detail: overview, then segments/ancestors/notes/tags with counts.
#[must_use]
pub fn dna_match_tabs(detail: &DnaMatchDetail, loc: &Localizer) -> Vec<DetailTab> {
    let tab = |id: &'static str, count: Option<usize>| DetailTab {
        id,
        label: loc.tab_label(id),
        count,
    };
    vec![
        tab("overview", None),
        tab("segments", Some(detail.segments.len())),
        tab("ancestors", Some(detail.shared_ancestors.len())),
        tab("notes", Some(detail.notes.len())),
        tab("tags", Some(detail.tags.len())),
        tab("history", None),
    ]
}

// ---------------------------------------------------------------------------------------------------
// Pedigree tool (PR 18): ancestor/descendant charts + the kinship calculator
// ---------------------------------------------------------------------------------------------------
