//! A bulk import's Plan and Review stages' frontend contract (ADR 0040 §4).
//!
//! Once the guest has submitted every record graph and the run's dataset is decided, the host plans the
//! import and shows the plan to the frontend's [`PlanReviewer`]: commit it, asking about each possible
//! match, commit it leaving every match for later, or write nothing. While reviewing, the frontend
//! answers one pair at a time, or with a bulk answer (ADR 0040 §3). The plugin takes no part in either
//! stage.

use std::fmt;

use async_trait::async_trait;
use vitni_app::{MatchQuestion, PlanStep, PlanSummary, ReviewReply};

use crate::present::PresentError;

/// A frontend that shows a bulk import's plan and puts its possible matches to the user.
#[async_trait]
pub trait PlanReviewer: Send {
    /// Shows `summary` and resolves with what to do with the plan, or a [`PresentError`] if the
    /// frontend could not be reached.
    async fn plan(&mut self, summary: PlanSummary) -> Result<PlanStep, PresentError>;

    /// Puts one possible match to the user and resolves with their reply, or a [`PresentError`] if the
    /// frontend could not be reached.
    async fn review_match(&mut self, question: MatchQuestion) -> Result<ReviewReply, PresentError>;
}

impl fmt::Debug for dyn PlanReviewer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PlanReviewer")
    }
}

/// A [`PlanReviewer`] for an import no one watches: it commits every plan, leaving each possible
/// match for later.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeferMatches;

#[async_trait]
impl PlanReviewer for DeferMatches {
    async fn plan(&mut self, _summary: PlanSummary) -> Result<PlanStep, PresentError> {
        Ok(PlanStep::DeferMatches)
    }

    async fn review_match(&mut self, _question: MatchQuestion) -> Result<ReviewReply, PresentError> {
        Ok(ReviewReply::DeferRest)
    }
}
