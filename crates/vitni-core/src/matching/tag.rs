//! Tag matching (ADR 0038 §2, §6): a tag is its name.
//!
//! Two tags of one name, compared without surrounding space and case, are one tag: that is identity
//! already established, so the band is [`MatchBand::Deterministic`] — the one kind where a compared
//! value, not an origin or external id, establishes it. Any other name is another tag. No name culture
//! applies.

use crate::matching::profile::TagProfile;
use crate::matching::weights;
use crate::matching::{
    Feature, FeatureComparison, FeatureValue, Identity, MatchAssessment, MatchBand, MatchSettings, Outcome, conclude,
};

/// Compares two tag profiles.
#[must_use]
pub fn assess_tags(a: &TagProfile, b: &TagProfile, settings: &MatchSettings) -> MatchAssessment {
    let same = a.name.trim().to_lowercase() == b.name.trim().to_lowercase();
    let (outcome, weight) = if same {
        (Outcome::Agree, weights::TAG_NAME.agree)
    } else {
        (Outcome::Disagree, weights::TAG_NAME.disagree)
    };
    let name = FeatureComparison {
        feature: Feature::Name,
        outcome,
        weight,
        left: Some(FeatureValue::Name(a.name.clone())),
        right: Some(FeatureValue::Name(b.name.clone())),
    };
    let nobody = || Identity::of(&[], &[]);
    let mut assessment = conclude(vec![name], Vec::new(), Vec::new(), (nobody(), nobody()), settings);
    if same {
        assessment.band = MatchBand::Deterministic;
    }
    assessment
}

#[cfg(test)]
mod tests {
    use crate::matching::profile::TagProfile;
    use crate::matching::tests::feature;
    use crate::matching::{Feature, MatchAssessment, MatchBand, MatchSettings, Outcome, assess_tags};

    fn assess(a: &str, b: &str) -> MatchAssessment {
        let tag = |name: &str| TagProfile { name: name.to_owned() };
        assess_tags(&tag(a), &tag(b), &MatchSettings::default())
    }

    #[test]
    fn one_name_in_another_case_is_one_tag() {
        let assessment = assess("Brick Wall", " brick wall ");
        assert_eq!(assessment.band, MatchBand::Deterministic, "{assessment:#?}");
        assert_eq!(feature(&assessment, Feature::Name).outcome, Outcome::Agree);
        assert!(assessment.cultures.is_empty(), "{:?}", assessment.cultures);
    }

    #[test]
    fn a_non_ascii_name_is_case_folded_too() {
        assert_eq!(assess("Åsen-slekta", "ÅSEN-SLEKTA").band, MatchBand::Deterministic);
        assert_eq!(
            assess("Åsen", "Asen").band,
            MatchBand::Unlikely,
            "case, not diacritics, is folded"
        );
    }

    #[test]
    fn another_name_is_another_tag() {
        let assessment = assess("Brick Wall", "Brick Walls");
        assert_eq!(feature(&assessment, Feature::Name).outcome, Outcome::Disagree);
        assert_eq!(assessment.band, MatchBand::Unlikely);
    }
}
