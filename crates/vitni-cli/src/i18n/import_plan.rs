//! The import plan and the interactive review of its possible matches (ADR 0040 §4).

use vitni_app::{KindCounts, MatchBand, MatchQuestion, MatchableKind, Outcome};

use super::{Localizer, fl};

impl Localizer {
    /// `Plan for tree.ged:`.
    #[must_use]
    pub fn import_plan_heading(&self, source: &str) -> String {
        fl!(self.loader, "import-plan-heading", source = source)
    }

    /// One kind's line of the plan: `persons: 2 new, 1 with possible matches`.
    #[must_use]
    pub fn import_plan_row(&self, row: &KindCounts) -> String {
        let counts = row.counts;
        let parts: Vec<String> = [
            (counts.new, fl!(self.loader, "import-plan-new", count = counts.new)),
            (
                counts.unchanged,
                fl!(self.loader, "import-plan-unchanged", count = counts.unchanged),
            ),
            (
                counts.updated,
                fl!(self.loader, "import-plan-updated", count = counts.updated),
            ),
            (
                counts.linked,
                fl!(self.loader, "import-plan-linked", count = counts.linked),
            ),
            (
                counts.candidates,
                fl!(self.loader, "import-plan-candidates", count = counts.candidates),
            ),
            (
                counts.withheld,
                fl!(self.loader, "import-plan-withheld", count = counts.withheld),
            ),
        ]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(_, part)| part)
        .collect();
        let kind = fl!(self.loader, "import-plan-kind", kind = row.kind.as_str());
        fl!(self.loader, "import-plan-row", kind = kind, counts = parts.join(", "))
    }

    /// The plan's closing line when it writes nothing.
    #[must_use]
    pub fn import_plan_all_unchanged(&self) -> String {
        fl!(self.loader, "import-plan-all-unchanged")
    }

    /// The plan's closing line when the file holds no records.
    #[must_use]
    pub fn import_plan_empty(&self) -> String {
        fl!(self.loader, "import-plan-empty")
    }

    /// How to review the `count` records with possible matches a printed plan shows.
    #[must_use]
    pub fn import_plan_review_hint(&self, count: u32) -> String {
        fl!(self.loader, "import-plan-review-hint", count = count)
    }

    /// The lines that put one possible match to the operator: where it stands in the review, both
    /// records, and how each compared feature came out.
    #[must_use]
    pub fn import_review_question(&self, question: &MatchQuestion) -> Vec<String> {
        let kind = self.import_review_kind(question.kind);
        let band = match question.assessment.band {
            MatchBand::Probable | MatchBand::Deterministic => "probable",
            MatchBand::Possible | MatchBand::Unlikely => "possible",
        };
        let score = format!("{:.0}", question.assessment.score * 100.0);
        let record = question
            .incoming_origin
            .as_ref()
            .map_or_else(String::new, |origin| origin.record.clone());
        let mut lines = vec![
            fl!(
                self.loader,
                "import-review-position",
                position = question.position,
                total = question.total,
                kind = kind,
                band = fl!(self.loader, "import-review-band", band = band),
                score = score
            ),
            fl!(
                self.loader,
                "import-review-stored",
                label = question.candidate_label.clone(),
                id = question.candidate.human_id.clone()
            ),
            fl!(
                self.loader,
                "import-review-incoming",
                label = question.incoming_label.clone(),
                record = record
            ),
        ];
        for feature in &question.assessment.features {
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
        lines
    }

    /// The answer prompt for `question`, offering the bulk *Same* when it has a probable group.
    #[must_use]
    pub fn import_review_prompt(&self, question: &MatchQuestion) -> String {
        match question.group {
            Some(group) => fl!(
                self.loader,
                "import-review-prompt-group",
                count = group.remaining,
                kind = self.import_review_kind(question.kind)
            ),
            None => fl!(self.loader, "import-review-prompt"),
        }
    }

    pub(super) fn import_review_kind(&self, kind: MatchableKind) -> String {
        fl!(self.loader, "import-review-kind", kind = kind.as_str())
    }

    pub(super) fn import_review_outcome(&self, outcome: Outcome) -> String {
        match outcome {
            Outcome::Agree => fl!(self.loader, "import-review-outcome-agree"),
            Outcome::Partial(_) => fl!(self.loader, "import-review-outcome-partial"),
            Outcome::Disagree => fl!(self.loader, "import-review-outcome-disagree"),
            Outcome::Missing => fl!(self.loader, "import-review-outcome-missing"),
            Outcome::Conflict => fl!(self.loader, "import-review-outcome-conflict"),
        }
    }

    pub(super) fn import_review_feature(&self, feature: vitni_app::Feature) -> String {
        use vitni_app::Feature;
        match feature {
            Feature::GivenName => fl!(self.loader, "import-review-feature-given-name"),
            Feature::Surname => fl!(self.loader, "import-review-feature-surname"),
            Feature::Sex => fl!(self.loader, "import-review-feature-sex"),
            Feature::Birth => fl!(self.loader, "import-review-feature-birth"),
            Feature::Death => fl!(self.loader, "import-review-feature-death"),
            Feature::BirthPlace => fl!(self.loader, "import-review-feature-birth-place"),
            Feature::DeathPlace => fl!(self.loader, "import-review-feature-death-place"),
            Feature::Lifespan => fl!(self.loader, "import-review-feature-lifespan"),
            Feature::Father => fl!(self.loader, "import-review-feature-father"),
            Feature::Mother => fl!(self.loader, "import-review-feature-mother"),
            Feature::Partners => fl!(self.loader, "import-review-feature-partners"),
            Feature::Children => fl!(self.loader, "import-review-feature-children"),
            Feature::Patronymic => fl!(self.loader, "import-review-feature-patronymic"),
            Feature::Occupation => fl!(self.loader, "import-review-feature-occupation"),
            Feature::Record => fl!(self.loader, "import-review-feature-record"),
            Feature::Partner => fl!(self.loader, "import-review-feature-partner"),
            Feature::Marriage => fl!(self.loader, "import-review-feature-marriage"),
            Feature::MarriagePlace => fl!(self.loader, "import-review-feature-marriage-place"),
            Feature::EventType => fl!(self.loader, "import-review-feature-event-type"),
            Feature::Date => fl!(self.loader, "import-review-feature-date"),
            Feature::Place => fl!(self.loader, "import-review-feature-place"),
            Feature::Principal => fl!(self.loader, "import-review-feature-principal"),
            Feature::Participants => fl!(self.loader, "import-review-feature-participants"),
            Feature::PlaceName => fl!(self.loader, "import-review-feature-place-name"),
            Feature::PlaceType => fl!(self.loader, "import-review-feature-place-type"),
            Feature::Enclosure => fl!(self.loader, "import-review-feature-enclosure"),
            Feature::Coordinates => fl!(self.loader, "import-review-feature-coordinates"),
            Feature::Title => fl!(self.loader, "import-review-feature-title"),
            Feature::Author => fl!(self.loader, "import-review-feature-author"),
            Feature::Publication => fl!(self.loader, "import-review-feature-publication"),
            Feature::Repository => fl!(self.loader, "import-review-feature-repository"),
            Feature::Name => fl!(self.loader, "import-review-feature-name"),
            Feature::Address => fl!(self.loader, "import-review-feature-address"),
            Feature::Source => fl!(self.loader, "import-review-feature-source"),
            Feature::Page => fl!(self.loader, "import-review-feature-page"),
            Feature::Checksum => fl!(self.loader, "import-review-feature-checksum"),
            Feature::Path => fl!(self.loader, "import-review-feature-path"),
            Feature::Text => fl!(self.loader, "import-review-feature-text"),
        }
    }
}
