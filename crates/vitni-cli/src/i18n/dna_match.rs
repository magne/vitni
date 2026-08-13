use super::{DnaMatchError, DnaMatchSummary, Localizer, MatchStatus, fl};

impl Localizer {
    /// `No DNA matches yet.`
    #[must_use]
    pub fn dna_match_list_empty(&self) -> String {
        fl!(self.loader, "dna-match-list-empty")
    }

    /// One DNA-match line: `X0001  shared: 850.5 cM  predicted: 2nd cousin  status: confirmed  segments: 3`.
    #[must_use]
    pub fn dna_match_summary_line(&self, summary: &DnaMatchSummary) -> String {
        let shared = match &summary.shared_cm {
            Some(shared) => shared.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let predicted = match &summary.predicted_relationship {
            Some(predicted) => predicted.clone(),
            None => fl!(self.loader, "no-value"),
        };
        let status = match summary.status {
            Some(MatchStatus::Confirmed) => fl!(self.loader, "dna-match-status-confirmed"),
            Some(MatchStatus::Rejected) => fl!(self.loader, "dna-match-status-rejected"),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "dna-match-summary",
            id = summary.human_id.clone(),
            shared = shared,
            predicted = predicted,
            status = status,
            segments = summary.segments.len().to_string()
        )
    }

    pub(super) fn dna_match_error(&self, error: &DnaMatchError) -> String {
        match error {
            DnaMatchError::NotFound(id) => fl!(self.loader, "err-dna-match-not-exist", id = id.to_string()),
            DnaMatchError::AlreadyExists(id) => fl!(self.loader, "err-dna-match-exists", id = id.to_string()),
            DnaMatchError::UnknownTest(id) => fl!(self.loader, "err-dna-match-unknown-test", id = id.to_string()),
            DnaMatchError::SameTestBothSides(id) => {
                fl!(self.loader, "err-dna-match-same-test", id = id.to_string())
            }
            DnaMatchError::NegativeSharedCm => fl!(self.loader, "err-dna-match-negative-cm"),
            DnaMatchError::RetractsMissingAssertion(id) | DnaMatchError::SupersedesMissingAssertion(id) => {
                fl!(self.loader, "err-missing-assertion", id = id.to_string())
            }
        }
    }
}
