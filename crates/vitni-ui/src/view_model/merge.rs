use super::{Localizer, PedigreeNodeVm, pedigree_node_vm};

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
    /// Builds the view-model from the app's [`RelationshipResult`](vitni_app::RelationshipResult),
    /// localizing the kinship into a display sentence.
    #[must_use]
    pub fn build(result: &vitni_app::RelationshipResult, loc: &Localizer) -> Self {
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

/// A blocked decision (Phase 5 PR 30; `merge.html:181-188`): the decision core rejected `MergePersons`
/// with [`PersonError::MergeConflict`](vitni_app::PersonError) because the two records carry
/// contradictions that cannot both be true, or rejected a merge or distinction with
/// `IdentityDecided` because the pair already holds a live decision — nothing was written.
///
/// `heading` and `guidance` are localized chrome; `detail` is the core's own reason string
/// (developer/domain text, English), surfaced verbatim so the operator sees what contradicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeBlockedVm {
    /// The localized "Merge blocked" heading.
    pub heading: String,
    /// The localized "resolve the contradiction first" guidance.
    pub guidance: String,
    /// The core's reason the merge was refused (not localized — domain text); empty when the
    /// guidance says it all.
    pub detail: String,
}

impl MergeBlockedVm {
    /// Builds the blocked view-model from an [`AppError`](vitni_app::AppError) if — and only if —
    /// it is a merge conflict; every other error returns `None` so the screen keeps its generic toast.
    #[must_use]
    pub fn from_error(error: &vitni_app::AppError, loc: &Localizer) -> Option<Self> {
        let vitni_app::AppError::Domain(person_error) = error else {
            return None;
        };
        match person_error {
            vitni_app::PersonError::MergeConflict { reason, .. } => Some(Self {
                heading: loc.merge_blocked_heading(),
                guidance: loc.merge_blocked_guidance(),
                detail: reason.clone(),
            }),
            vitni_app::PersonError::IdentityDecided { .. } => Some(Self {
                heading: loc.identity_decided_heading(),
                guidance: loc.identity_decided_guidance(),
                detail: String::new(),
            }),
            vitni_app::PersonError::NotFound(_)
            | vitni_app::PersonError::AlreadyExists(_)
            | vitni_app::PersonError::EmptyName
            | vitni_app::PersonError::RetractsMissingAssertion(_)
            | vitni_app::PersonError::SupersedesMissingAssertion(_)
            | vitni_app::PersonError::InvalidDate(_)
            | vitni_app::PersonError::SelfAssociation(_)
            | vitni_app::PersonError::DistinctFromItself(_) => None,
        }
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
    /// Classifies an [`AppError`](vitni_app::AppError): a merge conflict becomes [`Blocked`],
    /// everything else becomes [`Other`] with the localized error line.
    ///
    /// [`Blocked`]: MergeFailure::Blocked
    /// [`Other`]: MergeFailure::Other
    #[must_use]
    pub fn from_error(error: &vitni_app::AppError, loc: &Localizer) -> Self {
        match MergeBlockedVm::from_error(error, loc) {
            Some(blocked) => Self::Blocked(blocked),
            None => Self::Other(loc.error(error)),
        }
    }
}

/// One possible-duplicate pair (the Compare/merge screen): the two persons, the matching engine's band
/// (already localized), and its score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateCandidateVm {
    /// The first person.
    pub a: PedigreeNodeVm,
    /// The second person.
    pub b: PedigreeNodeVm,
    /// The already-localized band the engine put the pair in (probable, possible).
    pub reason: String,
    /// The matching engine's score as a whole percentage (`0..=100`, higher = more likely one
    /// person). This is *not* an operator-asserted surety — it must never be rendered as the 5-level
    /// assertion Confidence; the screen shows it as a plain `{score}%` badge (`merge.html`).
    pub score: u8,
    /// The already-localized reasons behind the score, the strongest first.
    pub reasons: Vec<String>,
}

impl DuplicateCandidateVm {
    /// Builds the view-model from the engine's [`SimilarPair`](vitni_app::SimilarPair), localizing its
    /// band and reasons and rounding its `0..1` score to a percentage.
    #[must_use]
    pub fn build(pair: &vitni_app::SimilarPair, loc: &Localizer) -> Self {
        let percent = (pair.assessment.score * 100.0).round().clamp(0.0, 100.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to 0..=100 just above"
        )]
        let score = percent as u8;
        Self {
            a: node_ref(&pair.a),
            b: node_ref(&pair.b),
            reason: loc.match_band(pair.assessment.band),
            score,
            reasons: loc.match_reasons(&pair.assessment.evidence()),
        }
    }
}

/// Builds a bare [`PedigreeNodeVm`] from an [`AggRef`](vitni_app::AggRef) — no vitals/confidence,
/// just the id + display fallback the duplicates table and merge picker need for navigation.
fn node_ref(agg: &vitni_app::AggRef) -> PedigreeNodeVm {
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

/// The result of a completed merge (Phase 5 PR 19): the refreshed survivor, the merged person's id,
/// and an accurate — not fabricated — summary of what changed.
///
/// `summary` deliberately never claims relationships were "re-pointed": `PersonsMerged` only records
/// a same-as link on the survivor (data-model §9); Family/Association/Participation records that name
/// the merged person are left exactly as they were, and read as the survivor (ADR 0039 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResultVm {
    /// The survivor's `human_id` (unchanged by the merge).
    pub survivor_human_id: String,
    /// The merged person's `human_id` (their own record is untouched and now reads as the survivor).
    pub merged_human_id: String,
    /// The already-localized outcome summary.
    pub summary: String,
}

impl MergeResultVm {
    /// Builds the view-model from the app's [`MergeResult`](vitni_app::MergeResult).
    #[must_use]
    pub fn build(result: &vitni_app::MergeResult, loc: &Localizer) -> Self {
        Self {
            survivor_human_id: result.survivor.human_id.clone(),
            merged_human_id: result.merged_human_id.clone(),
            summary: loc.merge_result_summary(&result.merged_human_id, &result.survivor.human_id),
        }
    }
}
