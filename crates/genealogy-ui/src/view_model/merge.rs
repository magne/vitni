use super::{Localizer, PedigreeNodeVm, RestrictionKind, pedigree_node_vm};

/// The kinship calculator's view-model: the two people, each with their evidence-free node vm, and
/// the localized relationship summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipVm {
    /// The first person.
    pub person_a: PedigreeNodeVm,
    /// The second person.
    pub person_b: PedigreeNodeVm,
    /// The already-localized relationship description, or the "not found" message.
    pub summary: String,
}

impl RelationshipVm {
    /// Builds the view-model from the app's [`RelationshipResult`](genealogy_app::RelationshipResult),
    /// localizing the kinship into a display sentence.
    #[must_use]
    pub fn build(result: &genealogy_app::RelationshipResult, loc: &Localizer) -> Self {
        let person_a = pedigree_node_vm(&result.person_a, None, 0, false, loc);
        let person_b = pedigree_node_vm(&result.person_b, None, 0, false, loc);
        let summary = match &result.kinship {
            Some(kinship) => loc.kinship_summary(&person_a.name, &person_b.name, kinship),
            None => loc.kinship_not_found(),
        };
        Self {
            person_a,
            person_b,
            summary,
        }
    }
}

/// A blocked merge (Phase 5 PR 30; `merge.html:181-188`): the decision core rejected `MergePersons`
/// with [`PersonError::MergeConflict`](genealogy_app::PersonError) because the two records carry
/// contradictions that cannot both be true — nothing was written.
///
/// `heading` and `guidance` are localized chrome; `detail` is the core's own reason string
/// (developer/domain text, English), surfaced verbatim so the operator sees what contradicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeBlockedVm {
    /// The localized "Merge blocked" heading.
    pub heading: String,
    /// The localized "resolve the contradiction first" guidance.
    pub guidance: String,
    /// The core's reason the merge was refused (not localized — domain text).
    pub detail: String,
}

impl MergeBlockedVm {
    /// Builds the blocked view-model from an [`AppError`](genealogy_app::AppError) if — and only if —
    /// it is a merge conflict; every other error returns `None` so the screen keeps its generic toast.
    #[must_use]
    pub fn from_error(error: &genealogy_app::AppError, loc: &Localizer) -> Option<Self> {
        let genealogy_app::AppError::Domain(person_error) = error else {
            return None;
        };
        let genealogy_app::PersonError::MergeConflict { reason, .. } = person_error else {
            return None;
        };
        Some(Self {
            heading: loc.merge_blocked_heading(),
            guidance: loc.merge_blocked_guidance(),
            detail: reason.clone(),
        })
    }
}

/// Why a merge dispatch failed: a resolvable contradiction the operator must clear first
/// ([`MergeBlockedVm`]), or any other failure the screen shows as a plain localized toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeFailure {
    /// The core rejected the merge with a conflict — render the blocked card.
    Blocked(MergeBlockedVm),
    /// Any other failure (not found, workspace/store) — render the localized message as a toast.
    Other(String),
}

impl MergeFailure {
    /// Classifies an [`AppError`](genealogy_app::AppError): a merge conflict becomes [`Blocked`],
    /// everything else becomes [`Other`] with the localized error line.
    ///
    /// [`Blocked`]: MergeFailure::Blocked
    /// [`Other`]: MergeFailure::Other
    #[must_use]
    pub fn from_error(error: &genealogy_app::AppError, loc: &Localizer) -> Self {
        match MergeBlockedVm::from_error(error, loc) {
            Some(blocked) => Self::Blocked(blocked),
            None => Self::Other(loc.error(error)),
        }
    }
}

/// One flagged possible-duplicate pair (Phase 5 PR 19's Compare/merge screen): the two persons, why
/// they were flagged (already localized), and the duplicate-detector's match score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateCandidateVm {
    /// The first person.
    pub a: PedigreeNodeVm,
    /// The second person.
    pub b: PedigreeNodeVm,
    /// The already-localized reason the pair was flagged.
    pub reason: String,
    /// The duplicate-detector's raw match score (`0..=100`, higher = more likely a duplicate). This
    /// is *not* an operator-asserted surety — it must never be rendered as the 5-level assertion
    /// Confidence; the screen shows it as a plain `{score}%` badge (PR 30, `merge.html:24`).
    pub score: u8,
}

impl DuplicateCandidateVm {
    /// Builds the view-model from an app [`DuplicateCandidate`](genealogy_app::DuplicateCandidate),
    /// localizing the match reason and carrying its raw `0..=100` score through verbatim.
    #[must_use]
    pub fn build(candidate: &genealogy_app::DuplicateCandidate, loc: &Localizer) -> Self {
        Self {
            a: node_ref(&candidate.a),
            b: node_ref(&candidate.b),
            reason: loc.duplicate_match_reason(&candidate.kind),
            score: candidate.score,
        }
    }
}

/// Builds a bare [`PedigreeNodeVm`] from an [`AggRef`](genealogy_app::AggRef) — no vitals/confidence,
/// just the id + display fallback the duplicates table and merge picker need for navigation.
fn node_ref(agg: &genealogy_app::AggRef) -> PedigreeNodeVm {
    PedigreeNodeVm {
        human_id: agg.human_id.clone(),
        name: agg.human_id.clone(),
        vitals: None,
        confidence: None,
        confidence_label: None,
        source_count: 0,
        restrictions: Vec::new(),
        has_more: false,
    }
}

/// One field row in the merge compare grid: the field's label and each side's current value.
///
/// Carries only display data — no per-field "chosen" mutation exists in the core (a `MergePersons`
/// command is one atomic same-as event, not a field-by-field reconciliation). The radios the screen
/// renders over these rows are read-only context for the operator's decision, not a granular-apply
/// mechanism (see [`MergeCompareVm`] doc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeFieldRowVm {
    /// The field's already-localized label (e.g. "Name", "Birth").
    pub label: String,
    /// The survivor's current value for this field, or `None` if unrecorded.
    pub survivor_value: Option<String>,
    /// The merged person's current value for this field, or `None` if unrecorded.
    pub merged_value: Option<String>,
}

/// The Compare/merge wizard's view-model (Phase 5 PR 19): the two people's headline info and a
/// field-by-field grid of their current values.
///
/// The per-field rows are informational only. The core's `MergePersons` command has no concept of
/// selecting individual field values from the persona onto the survivor — merging is a single atomic
/// same-as event (data-model §9). The screen's "Merge" action always performs that one atomic call;
/// nothing here drives which fields end up where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeCompareVm {
    /// The surviving person (keeps their id and `human_id`).
    pub survivor: PedigreeNodeVm,
    /// The person who would become a persona of the survivor.
    pub merged: PedigreeNodeVm,
    /// The field-by-field comparison rows.
    pub fields: Vec<MergeFieldRowVm>,
}

impl MergeCompareVm {
    /// Builds the view-model from the two persons' summaries, comparing name/birth/death/occupation
    /// (the fields the mockup shows) — only fields the summaries actually carry, no fabricated rows.
    #[must_use]
    pub fn build(
        survivor: &genealogy_app::PersonSummary,
        merged: &genealogy_app::PersonSummary,
        loc: &Localizer,
    ) -> Self {
        let fields = vec![
            MergeFieldRowVm {
                label: loc.merge_field_name(),
                survivor_value: survivor.display_name.clone(),
                merged_value: merged.display_name.clone(),
            },
            year_row(loc.merge_field_birth(), survivor.birth_year(), merged.birth_year()),
            year_row(loc.merge_field_death(), survivor.death_year(), merged.death_year()),
            fact_row(
                loc.merge_field_occupation(),
                survivor,
                merged,
                &genealogy_app::FactType::Occupation,
            ),
        ];
        Self {
            survivor: summary_node_ref(survivor),
            merged: summary_node_ref(merged),
            fields,
        }
    }
}

/// Builds a [`MergeFieldRowVm`] comparing both persons' vital year (birth/death) — derived from
/// their Primary participation in a dated Event (ADR 0021 §2), not from a Fact.
fn year_row(label: String, survivor: Option<i32>, merged: Option<i32>) -> MergeFieldRowVm {
    MergeFieldRowVm {
        label,
        survivor_value: survivor.map(|year| year.to_string()),
        merged_value: merged.map(|year| year.to_string()),
    }
}

/// Builds a [`MergeFieldRowVm`] comparing both persons' first-asserted fact of `fact_type`.
fn fact_row(
    label: String,
    survivor: &genealogy_app::PersonSummary,
    merged: &genealogy_app::PersonSummary,
    fact_type: &genealogy_app::FactType,
) -> MergeFieldRowVm {
    MergeFieldRowVm {
        label,
        survivor_value: fact_value(survivor, fact_type),
        merged_value: fact_value(merged, fact_type),
    }
}

/// The display value of a person's first-asserted fact of `fact_type`, if any.
fn fact_value(summary: &genealogy_app::PersonSummary, fact_type: &genealogy_app::FactType) -> Option<String> {
    let asserted = summary.facts.iter().find(|f| f.fact.fact_type == *fact_type)?;
    let value = asserted.fact.value.clone();
    let year = asserted
        .fact
        .date
        .as_ref()
        .map(|date| date.sort_value / 10_000)
        .filter(|year| *year != 0)
        .map(|year| year.to_string());
    match (value, year) {
        (Some(value), Some(year)) => Some(format!("{value} ({year})")),
        (Some(value), None) => Some(value),
        (None, Some(year)) => Some(year),
        (None, None) => None,
    }
}

/// Builds a bare [`PedigreeNodeVm`] from a [`PersonSummary`](genealogy_app::PersonSummary) for the
/// merge wizard's header row (name only — the wizard's field grid carries the vitals).
fn summary_node_ref(summary: &genealogy_app::PersonSummary) -> PedigreeNodeVm {
    PedigreeNodeVm {
        human_id: summary.human_id.clone(),
        name: summary.display_name.clone().unwrap_or_else(|| summary.human_id.clone()),
        vitals: None,
        confidence: None,
        confidence_label: None,
        source_count: 0,
        restrictions: summary.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
        has_more: false,
    }
}

/// The result of a completed merge (Phase 5 PR 19): the refreshed survivor, the merged person's id,
/// and an accurate — not fabricated — summary of what changed.
///
/// `summary` deliberately never claims relationships were "re-pointed": `PersonsMerged` only records
/// a same-as link on the survivor (data-model §9); Family/Association/Participation records that name
/// the merged person are left exactly as they were. `still_referenced` counts how many such records
/// still name the merged person's id, worded as still-linked, not re-pointed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResultVm {
    /// The survivor's `human_id` (unchanged by the merge).
    pub survivor_human_id: String,
    /// The merged person's `human_id` (their own record is untouched and still resolvable).
    pub merged_human_id: String,
    /// The already-localized outcome summary.
    pub summary: String,
}

impl MergeResultVm {
    /// Builds the view-model from the app's [`MergeResult`](genealogy_app::MergeResult).
    #[must_use]
    pub fn build(result: &genealogy_app::MergeResult, loc: &Localizer) -> Self {
        Self {
            survivor_human_id: result.survivor.human_id.clone(),
            merged_human_id: result.merged_human_id.clone(),
            summary: loc.merge_result_summary(
                &result.merged_human_id,
                &result.survivor.human_id,
                result.still_referenced,
            ),
        }
    }
}
