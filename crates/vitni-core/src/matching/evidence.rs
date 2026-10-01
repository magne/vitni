//! The assessment an identity decision was made on (ADR 0039 §2): [`MatchEvidence`], a fixed-point
//! snapshot of a [`MatchAssessment`] an event payload can carry.
//!
//! Event payloads derive `Eq` and must encode stably (ADR 0004 §4), which an `f64` cannot, so the score,
//! the weights and a partial outcome's similarity are kept in basis points (1/10 000). The compared
//! values and a family's or citation's part assessments are not kept: the snapshot records what the
//! engine concluded and why, not the records it read.

use serde::{Deserialize, Serialize};

use crate::matching::{CultureId, EngineVersion, Feature, MatchAssessment, MatchBand, Outcome};

/// Basis points in one.
const BASIS: f64 = 10_000.0;

/// The engine's assessment of a pair as the user was shown it, recorded on their decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchEvidence {
    /// The score in basis points, `0..=10000`.
    pub score_bp: u16,
    /// The band the score (or established identity) fell in.
    pub band: MatchBand,
    /// The engine that produced the assessment.
    pub engine: EngineVersion,
    /// The name-culture packs applied, `universal` first.
    pub cultures: Vec<CultureId>,
    /// Every term of the match weight.
    pub features: Vec<FeatureEvidence>,
}

/// One term of a recorded assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureEvidence {
    /// The feature compared.
    pub feature: Feature,
    /// How the values compared.
    pub outcome: OutcomeEvidence,
    /// The term's log₂ weight in basis points.
    pub weight_bp: i32,
}

/// A recorded [`Outcome`], its partial similarity in basis points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "similarity_bp")]
pub enum OutcomeEvidence {
    /// The values agreed after normalization.
    Agree,
    /// The values were close; the similarity in basis points, `0..10000`.
    Partial(u16),
    /// The values were implausibly far apart.
    Disagree,
    /// One side, or both, had no value.
    Missing,
    /// The values were logically impossible for one individual.
    Conflict,
}

impl MatchAssessment {
    /// The fixed-point snapshot of this assessment, for recording on an identity decision.
    #[must_use]
    pub fn evidence(&self) -> MatchEvidence {
        let mut features = Vec::with_capacity(self.features.len());
        for term in &self.features {
            features.push(FeatureEvidence {
                feature: term.feature,
                outcome: OutcomeEvidence::from(term.outcome),
                weight_bp: weight_bp(term.weight),
            });
        }
        MatchEvidence {
            score_bp: unit_bp(self.score),
            band: self.band,
            engine: self.engine,
            cultures: self.cultures.clone(),
            features,
        }
    }
}

impl MatchEvidence {
    /// The score as a whole percentage, rounded.
    #[must_use]
    pub fn percent(&self) -> u8 {
        let percent = (u32::from(self.score_bp) + 50) / 100;
        u8::try_from(percent).unwrap_or(100)
    }
}

impl From<Outcome> for OutcomeEvidence {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Agree => Self::Agree,
            Outcome::Partial(similarity) => Self::Partial(unit_bp(similarity)),
            Outcome::Disagree => Self::Disagree,
            Outcome::Missing => Self::Missing,
            Outcome::Conflict => Self::Conflict,
        }
    }
}

/// A value in `0..=1` in basis points, rounded; out-of-range values clamp.
fn unit_bp(value: f64) -> u16 {
    let clamped = (value * BASIS).round().clamp(0.0, BASIS);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=10000 above, which fits a u16"
    )]
    let bp = clamped as u16;
    bp
}

/// A log₂ weight in basis points, rounded; a weight beyond the `i32` range saturates.
fn weight_bp(weight: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a float-to-int `as` saturates, and engine weights are a few dozen bits at most"
    )]
    let bp = (weight * BASIS).round() as i32;
    bp
}

#[cfg(test)]
mod tests {
    use super::{FeatureEvidence, MatchEvidence, OutcomeEvidence};
    use crate::matching::{
        CultureId, ENGINE_VERSION, EngineVersion, Feature, FeatureComparison, MatchAssessment, MatchBand, Outcome,
    };

    fn term(feature: Feature, outcome: Outcome, weight: f64) -> FeatureComparison {
        FeatureComparison {
            feature,
            outcome,
            weight,
            left: None,
            right: None,
        }
    }

    fn assessment(score: f64, features: Vec<FeatureComparison>) -> MatchAssessment {
        MatchAssessment {
            score,
            band: MatchBand::Probable,
            features,
            cultures: vec![CultureId::new("universal"), CultureId::new("no")],
            parts: Vec::new(),
            engine: ENGINE_VERSION,
        }
    }

    #[test]
    fn the_score_is_kept_in_basis_points_rounded_and_clamped() {
        assert_eq!(assessment(0.874_94, Vec::new()).evidence().score_bp, 8749);
        assert_eq!(assessment(0.874_95, Vec::new()).evidence().score_bp, 8750);
        assert_eq!(assessment(1.0, Vec::new()).evidence().score_bp, 10_000);
        assert_eq!(assessment(0.0, Vec::new()).evidence().score_bp, 0);
        assert_eq!(assessment(1.2, Vec::new()).evidence().score_bp, 10_000);
        assert_eq!(assessment(-0.1, Vec::new()).evidence().score_bp, 0);
    }

    #[test]
    fn every_term_is_kept_with_its_weight_and_outcome_in_basis_points() {
        let evidence = assessment(
            0.9,
            vec![
                term(Feature::GivenName, Outcome::Partial(0.912_34), 2.5),
                term(Feature::Sex, Outcome::Conflict, -12.345_67),
                term(Feature::Surname, Outcome::Missing, 0.0),
            ],
        )
        .evidence();
        assert_eq!(
            evidence.features,
            vec![
                FeatureEvidence {
                    feature: Feature::GivenName,
                    outcome: OutcomeEvidence::Partial(9123),
                    weight_bp: 25_000,
                },
                FeatureEvidence {
                    feature: Feature::Sex,
                    outcome: OutcomeEvidence::Conflict,
                    weight_bp: -123_457,
                },
                FeatureEvidence {
                    feature: Feature::Surname,
                    outcome: OutcomeEvidence::Missing,
                    weight_bp: 0,
                },
            ]
        );
    }

    #[test]
    fn the_band_engine_and_cultures_are_kept() {
        let evidence = assessment(0.97, Vec::new()).evidence();
        assert_eq!(evidence.band, MatchBand::Probable);
        assert_eq!(evidence.engine, ENGINE_VERSION);
        assert_eq!(
            evidence.cultures,
            vec![CultureId::new("universal"), CultureId::new("no")]
        );
    }

    #[test]
    fn the_percentage_rounds_the_basis_points() {
        let mut evidence = assessment(0.0, Vec::new()).evidence();
        for (score_bp, percent) in [(0, 0), (8749, 87), (8750, 88), (9949, 99), (9950, 100), (10_000, 100)] {
            evidence.score_bp = score_bp;
            assert_eq!(evidence.percent(), percent, "{score_bp} bp");
        }
    }

    #[test]
    fn evidence_round_trips_through_json() {
        let evidence = MatchEvidence {
            score_bp: 9712,
            band: MatchBand::Possible,
            engine: EngineVersion(4),
            cultures: vec![CultureId::new("universal")],
            features: vec![FeatureEvidence {
                feature: Feature::Birth,
                outcome: OutcomeEvidence::Partial(5000),
                weight_bp: -1500,
            }],
        };
        let json = serde_json::to_string(&evidence).unwrap();
        assert_eq!(serde_json::from_str::<MatchEvidence>(&json).unwrap(), evidence);
    }
}
