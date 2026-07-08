use super::{
    DetailTab, DnaMatchChangeSetRequest, DnaMatchEdit, HistoryEntryVm, Localizer, RecordDraft, RestrictionKind, RowVm,
    TagRef, UsingRecordVm, nav_ref, non_blank,
};

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
    /// The raw confirmation status (`None` = undecided) — seeds the whole-record editor's Status select.
    pub status_kind: Option<genealogy_app::MatchStatus>,
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
                chromosome: s.segment.chromosome.clone(),
                start: s.segment.start.to_string(),
                end: s.segment.end.to_string(),
                centimorgans: s.segment.centimorgans.to_string(),
                snps: s.segment.snps.map(|n| n.to_string()),
                side: loc.chromosome_side_label(s.segment.side),
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
            status_kind: summary.status,
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

/// The create form's in-memory draft for a new DNA match (`record-editing.html` §6): the two tests
/// and provider (required), plus the shared-cM (required) and the optional %-shared, largest-segment,
/// segment-count, and predicted-relationship. Numeric fields are raw text, parsed at the boundary:
/// an unparseable value is **rejected**, never zero-filled (§7). Create-only.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DnaMatchDraft {
    /// The record being edited (its current `human_id`); `None` in create mode.
    pub existing_human_id: Option<String>,
    /// The editable user-facing id; blank ⇒ generated on save (edit mode only — create auto-allocates).
    pub human_id: String,
    /// The match's confirmation status (`None` = undecided) — the only editable scalar on edit.
    pub status: Option<genealogy_app::MatchStatus>,
    /// One side's test `human_id` (required; create-only, locked on edit).
    pub test_a: String,
    /// The other side's test `human_id` (required; create-only, locked on edit).
    pub test_b: String,
    /// The provider the match was observed at, if chosen (create-only, locked on edit).
    pub provider: Option<genealogy_app::DnaProvider>,
    /// Total shared centimorgans, raw text (required, numeric; create-only).
    pub shared_cm: String,
    /// Shared percentage, raw text (optional; create-only).
    pub percent_shared: String,
    /// The largest shared segment's length in cM, raw text (optional; create-only).
    pub largest_segment_cm: String,
    /// The number of shared segments, raw text (optional; create-only).
    pub segment_count: String,
    /// The provider's predicted relationship (optional; create-only).
    pub predicted_relationship: String,
}

impl DnaMatchDraft {
    /// A fresh empty draft for creating a new DNA match.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A draft pre-populated from an existing match for editing. Records the current `human_id` so
    /// [`Self::edits_against`] diffs (supersedes) rather than creates; the only editable scalar is the
    /// confirmation status (the observed totals are locked — they are the provider's observation).
    #[must_use]
    pub fn from_detail(detail: &DnaMatchDetail) -> Self {
        Self {
            existing_human_id: Some(detail.human_id.clone()),
            human_id: detail.human_id.clone(),
            status: detail.status_kind,
            ..Self::default()
        }
    }

    /// The per-field edits carrying this draft from its committed `seed` to its current values (edit
    /// mode): the confirmation status (there is no command to return a match to undecided, so only a
    /// change to confirmed/rejected commits) and, last, `SetHumanId` when the id changed (a blank id
    /// regenerates).
    #[must_use]
    pub fn edits_against(&self, seed: &Self) -> Vec<DnaMatchEdit> {
        let Some(human_id) = seed.existing_human_id.clone() else {
            return Vec::new();
        };
        let mut edits = Vec::new();
        if self.status != seed.status
            && let Some(status) = self.status
        {
            edits.push(DnaMatchEdit::SetStatus {
                human_id: human_id.clone(),
                confirmed: status == genealogy_app::MatchStatus::Confirmed,
            });
        }
        if self.human_id.trim() != seed.human_id {
            edits.push(DnaMatchEdit::SetHumanId {
                human_id,
                new_human_id: non_blank(&self.human_id),
            });
        }
        edits
    }

    /// Whether the shared-cM field is invalid — a non-blank value that does not parse (drives
    /// `aria-invalid` + its field error).
    #[must_use]
    pub fn shared_cm_invalid(&self) -> bool {
        let value = self.shared_cm.trim();
        !value.is_empty() && value.parse::<genealogy_app::Centimorgans>().is_err()
    }

    /// Whether the draft is valid: on edit (the observed totals are locked) it is always valid; on
    /// create the two tests and provider are present, the required shared-cM parses, and every
    /// non-blank optional numeric parses.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.existing_human_id.is_some() || self.to_request().is_some()
    }

    /// Builds the [`DnaMatchChangeSetRequest`] the app commits on Save, or `None` when a required field
    /// is missing or any numeric is unparseable (Save is then a no-op — never zero-filled).
    #[must_use]
    pub fn to_request(&self) -> Option<DnaMatchChangeSetRequest> {
        let test_a = non_blank(&self.test_a)?;
        let test_b = non_blank(&self.test_b)?;
        let provider = self.provider.clone()?;
        let shared_cm = self.shared_cm.trim().parse::<genealogy_app::Centimorgans>().ok()?;
        let percent_shared = match non_blank(&self.percent_shared) {
            None => None,
            Some(text) => Some(text.parse::<genealogy_app::PercentShared>().ok()?),
        };
        let largest_segment_cm = match non_blank(&self.largest_segment_cm) {
            None => genealogy_app::Centimorgans::from_hundredths(0),
            Some(text) => text.parse::<genealogy_app::Centimorgans>().ok()?,
        };
        let segment_count = match non_blank(&self.segment_count) {
            None => 0,
            Some(text) => text.parse::<u32>().ok()?,
        };
        Some(DnaMatchChangeSetRequest {
            test_a,
            test_b,
            provider,
            shared_cm,
            percent_shared,
            largest_segment_cm,
            segment_count,
            predicted_relationship: non_blank(&self.predicted_relationship),
        })
    }
}

impl RecordDraft for DnaMatchDraft {
    type Detail = DnaMatchDetail;

    fn from_detail(detail: &DnaMatchDetail) -> Self {
        Self::from_detail(detail)
    }

    fn is_valid(&self) -> bool {
        Self::is_valid(self)
    }
}

#[cfg(test)]
mod dna_match_draft_tests {
    use super::{DnaMatchDraft, RecordDraft};
    use crate::navigation::DnaMatchEdit;
    use genealogy_app::{DnaProvider, MatchStatus};

    fn seed() -> DnaMatchDraft {
        DnaMatchDraft {
            test_a: "D0001".to_owned(),
            test_b: "D0002".to_owned(),
            provider: Some(DnaProvider::AncestryDna),
            shared_cm: "1200.5".to_owned(),
            ..DnaMatchDraft::new()
        }
    }

    #[test]
    fn a_fresh_draft_is_invalid_and_not_dirty() {
        let draft = DnaMatchDraft::new();
        assert!(!draft.is_valid());
        assert!(!draft.is_dirty_against(&DnaMatchDraft::new()));
        assert!(draft.to_request().is_none());
    }

    fn edit_seed() -> DnaMatchDraft {
        DnaMatchDraft {
            existing_human_id: Some("X0001".to_owned()),
            human_id: "X0001".to_owned(),
            status: None,
            ..DnaMatchDraft::new()
        }
    }

    #[test]
    fn an_edit_draft_is_valid_without_the_locked_observations() {
        assert!(edit_seed().is_valid());
        assert!(edit_seed().edits_against(&edit_seed()).is_empty());
    }

    #[test]
    fn confirming_a_match_yields_one_set_status() {
        let draft = DnaMatchDraft {
            status: Some(MatchStatus::Confirmed),
            ..edit_seed()
        };
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], DnaMatchEdit::SetStatus { confirmed, .. } if *confirmed));
    }

    #[test]
    fn a_blank_id_regenerates() {
        let draft = DnaMatchDraft {
            human_id: String::new(),
            ..edit_seed()
        };
        let edits = draft.edits_against(&edit_seed());
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], DnaMatchEdit::SetHumanId { new_human_id, .. } if new_human_id.is_none()));
    }

    #[test]
    fn a_complete_draft_parses_its_numerics() {
        let request = seed().to_request().expect("valid");
        assert_eq!(request.test_a, "D0001");
        assert_eq!(
            request.segment_count, 0,
            "a blank segment count is not reported (0), not rejected"
        );
    }

    #[test]
    fn an_unparseable_shared_cm_is_rejected_not_zero_filled() {
        let draft = DnaMatchDraft {
            shared_cm: "lots".to_owned(),
            ..seed()
        };
        assert!(draft.shared_cm_invalid());
        assert!(!draft.is_valid());
        assert!(
            draft.to_request().is_none(),
            "an unparseable shared-cM yields no request"
        );
    }

    #[test]
    fn an_unparseable_optional_is_rejected() {
        let draft = DnaMatchDraft {
            segment_count: "many".to_owned(),
            ..seed()
        };
        assert!(
            draft.to_request().is_none(),
            "an unparseable optional blocks the commit"
        );
    }

    #[test]
    fn a_missing_required_field_yields_no_request() {
        let draft = DnaMatchDraft {
            test_b: String::new(),
            ..seed()
        };
        assert!(draft.to_request().is_none());
    }
}
