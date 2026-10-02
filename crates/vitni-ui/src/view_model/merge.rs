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

/// A blocked decision (Phase 5 PR 30; `matches.html`): the decision core of the pair's kind rejected a
/// merge because the two records carry contradictions that cannot both be true, or rejected a merge or
/// distinction because the pair already holds a live decision — nothing was written.
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
    /// the decision core refused an identity decision of any kind; every other error returns `None` so
    /// the screen keeps its generic toast.
    #[must_use]
    pub fn from_error(error: &vitni_app::AppError, loc: &Localizer) -> Option<Self> {
        match error.identity_refusal()? {
            vitni_app::IdentityRefusal::Conflict(reason) => Some(Self {
                heading: loc.merge_blocked_heading(),
                guidance: loc.merge_blocked_guidance(),
                detail: reason,
            }),
            vitni_app::IdentityRefusal::Decided => Some(Self {
                heading: loc.identity_decided_heading(),
                guidance: loc.identity_decided_guidance(),
                detail: String::new(),
            }),
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
