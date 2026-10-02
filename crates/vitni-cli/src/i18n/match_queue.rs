//! The possible-matches review queue's output (ADR 0039 §3).

use vitni_app::{DecidableKind, MatchAssessment, MatchBand, MatchVerdict, Outcome, PairDecision, QueuedMatch};

use super::{Localizer, fl};

impl Localizer {
    /// `No possible matches.`
    #[must_use]
    pub fn match_list_empty(&self) -> String {
        fl!(self.loader, "match-list-empty")
    }

    /// One queued pair: its kind as `match same` takes it, both ids, the band and the score.
    #[must_use]
    pub fn match_line(&self, queued: &QueuedMatch) -> String {
        fl!(
            self.loader,
            "match-line",
            kind = queued.kind.as_str(),
            a = queued.a.human_id.clone(),
            b = queued.b.human_id.clone(),
            band = self.match_band(queued.assessment.band),
            score = queued.assessment.evidence().percent()
        )
    }

    /// The comparison of `first` and `second`: a heading, one line per compared feature, and the
    /// decision already taken between them, if any.
    #[must_use]
    pub fn match_show(
        &self,
        kind: DecidableKind,
        first: &str,
        second: &str,
        assessment: &MatchAssessment,
        earlier: Option<PairDecision>,
    ) -> Vec<String> {
        let mut lines = vec![fl!(
            self.loader,
            "match-show-heading",
            kind = self.import_review_kind(kind.matchable()),
            a = first,
            b = second,
            band = self.match_band(assessment.band),
            score = assessment.evidence().percent()
        )];
        for feature in &assessment.features {
            if let Outcome::Missing = feature.outcome {
                continue;
            }
            lines.push(fl!(
                self.loader,
                "import-review-row",
                feature = self.import_review_feature(feature.feature),
                outcome = self.import_review_outcome(feature.outcome)
            ));
        }
        if let Some(decision) = earlier {
            let decision = match decision {
                PairDecision::SameCluster => "same",
                PairDecision::Distinct => "distinct",
            };
            lines.push(fl!(self.loader, "match-show-decided", decision = decision));
        }
        lines
    }

    /// The confirmation of a decision about `first` and `second`.
    #[must_use]
    pub fn match_decided(&self, verdict: MatchVerdict, first: &str, second: &str) -> String {
        match verdict {
            MatchVerdict::Same => fl!(self.loader, "match-decided-same", first = first, second = second),
            MatchVerdict::Distinct => fl!(self.loader, "match-decided-distinct", first = first, second = second),
        }
    }

    fn match_band(&self, band: MatchBand) -> String {
        let band = match band {
            MatchBand::Deterministic => "deterministic",
            MatchBand::Probable => "probable",
            MatchBand::Possible | MatchBand::Unlikely => "possible",
        };
        fl!(self.loader, "import-review-band", band = band)
    }
}
