//! A bulk import's Plan and Review stages' frontend contract (ADR 0040 §4).
//!
//! Once the guest has submitted every record graph and the run's dataset is decided, the host plans the
//! import and shows the plan to the frontend's [`PlanReviewer`]: commit it, asking about each possible
//! match, commit it leaving every match for later, or write nothing. While reviewing, the frontend
//! answers one pair at a time, or with a bulk answer (ADR 0040 §3). Once a review has asked anything,
//! the plan as the answers leave it is shown once more, to commit or write nothing. The plugin takes no
//! part in either stage.
//!
//! A frontend that answers on its own task takes the [`channel_reviewer`]: each question arrives on a
//! channel as a [`ReviewRequest`] carrying where to send the answer.

use std::fmt;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
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

    /// Shows `summary`, the plan as the review's answers leave it, and resolves with whether to commit
    /// it, or a [`PresentError`] if the frontend could not be reached.
    async fn confirm(&mut self, summary: PlanSummary) -> Result<bool, PresentError>;
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

    async fn confirm(&mut self, _summary: PlanSummary) -> Result<bool, PresentError> {
        Ok(true)
    }
}

/// One question a [`channel_reviewer`] puts to its frontend, with where to send the answer.
#[derive(Debug)]
pub enum ReviewRequest {
    /// Show the plan and say what to do with it.
    Plan {
        /// The plan's counts by kind.
        summary: PlanSummary,
        /// Where the answer goes.
        reply: oneshot::Sender<PlanStep>,
    },
    /// Answer one possible match.
    Match {
        /// The pair.
        question: Box<MatchQuestion>,
        /// Where the answer goes.
        reply: oneshot::Sender<ReviewReply>,
    },
    /// Show the plan as the review's answers leave it and say whether to commit it.
    Confirm {
        /// The reviewed plan's counts by kind and the records it changes.
        summary: PlanSummary,
        /// Where the answer goes: `true` to commit.
        reply: oneshot::Sender<bool>,
    },
}

/// A [`PlanReviewer`] that puts each question on a channel, and the receiving end its frontend answers
/// from. A frontend that stops listening, or drops a reply, fails the import with a [`PresentError`].
#[must_use]
pub fn channel_reviewer() -> (Box<dyn PlanReviewer>, mpsc::Receiver<ReviewRequest>) {
    let (requests, received) = mpsc::channel(1);
    (Box::new(ChannelReviewer { requests }), received)
}

struct ChannelReviewer {
    requests: mpsc::Sender<ReviewRequest>,
}

#[async_trait]
impl PlanReviewer for ChannelReviewer {
    async fn plan(&mut self, summary: PlanSummary) -> Result<PlanStep, PresentError> {
        let (reply, answer) = oneshot::channel();
        self.ask(ReviewRequest::Plan { summary, reply }, answer).await
    }

    async fn review_match(&mut self, question: MatchQuestion) -> Result<ReviewReply, PresentError> {
        let (reply, answer) = oneshot::channel();
        let question = Box::new(question);
        self.ask(ReviewRequest::Match { question, reply }, answer).await
    }

    async fn confirm(&mut self, summary: PlanSummary) -> Result<bool, PresentError> {
        let (reply, answer) = oneshot::channel();
        self.ask(ReviewRequest::Confirm { summary, reply }, answer).await
    }
}

impl ChannelReviewer {
    /// Sends `request` to the frontend and awaits its answer on `answer`.
    async fn ask<T>(&self, request: ReviewRequest, answer: oneshot::Receiver<T>) -> Result<T, PresentError> {
        self.requests
            .send(request)
            .await
            .map_err(|_| PresentError::Backend("the import review is no longer listening".to_owned()))?;
        answer
            .await
            .map_err(|_| PresentError::Backend("the import review dropped its answer".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use vitni_app::{PlanStep, PlanSummary};

    use super::{ReviewRequest, channel_reviewer};

    #[tokio::test]
    async fn a_channel_reviewer_forwards_the_plan_and_returns_the_answer() {
        let (mut reviewer, mut requests) = channel_reviewer();
        let frontend = tokio::spawn(async move {
            let Some(ReviewRequest::Plan { summary, reply }) = requests.recv().await else {
                return None;
            };
            reply.send(PlanStep::Discard).ok()?;
            Some(summary)
        });
        let step = reviewer.plan(PlanSummary::default()).await;
        assert_eq!(step.ok(), Some(PlanStep::Discard));
        assert_eq!(frontend.await.ok().flatten(), Some(PlanSummary::default()));
    }

    #[tokio::test]
    async fn a_channel_reviewer_forwards_the_reviewed_plan_and_returns_whether_to_commit() {
        let (mut reviewer, mut requests) = channel_reviewer();
        let frontend = tokio::spawn(async move {
            let Some(ReviewRequest::Confirm { reply, .. }) = requests.recv().await else {
                return None;
            };
            reply.send(false).ok()
        });
        assert_eq!(reviewer.confirm(PlanSummary::default()).await.ok(), Some(false));
        assert_eq!(frontend.await.ok().flatten(), Some(()));
    }

    #[tokio::test]
    async fn a_frontend_that_stopped_listening_is_an_error() {
        let (mut reviewer, requests) = channel_reviewer();
        drop(requests);
        assert!(reviewer.plan(PlanSummary::default()).await.is_err(), "nobody answers");
    }
}
