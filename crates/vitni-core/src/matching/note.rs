//! Note matching (ADR 0038 §2): one transcription or remark recorded twice.
//!
//! A note is compared by its normalized text — the words it shares with the other, whatever their
//! formatting and punctuation — so an identical note agrees and an edited one agrees in part. The
//! language of its text selects the name cultures that normalize it.

use crate::matching::profile::NoteProfile;
use crate::matching::select::{Signals, comparison_cultures};
use crate::matching::source::compare_text;
use crate::matching::weights;
use crate::matching::{Feature, Identity, MatchAssessment, MatchData, MatchSettings, applied, conclude};

/// Compares two note profiles.
#[must_use]
pub fn assess_notes(a: &NoteProfile, b: &NoteProfile, data: &MatchData, settings: &MatchSettings) -> MatchAssessment {
    let cultures = comparison_cultures(&Signals::note(a), &Signals::note(b), data, settings);
    let applied = applied(&cultures, data);
    let features = vec![compare_text(
        Feature::Text,
        a.text.as_deref(),
        b.text.as_deref(),
        weights::NOTE_TEXT,
        &applied,
    )];
    let sides = (Identity::of(&a.origins, &[]), Identity::of(&b.origins, &[]));
    conclude(features, cultures, Vec::new(), sides, settings)
}

#[cfg(test)]
mod tests {
    use crate::matching::profile::NoteProfile;
    use crate::matching::tests::{DATA, feature, household};
    use crate::matching::{CultureId, Feature, MatchAssessment, MatchBand, MatchSettings, Outcome, assess_notes};
    use crate::name::LanguageTag;

    fn assess(a: &NoteProfile, b: &NoteProfile) -> MatchAssessment {
        assess_notes(a, b, &DATA, &MatchSettings::default())
    }

    fn note(text: &str) -> NoteProfile {
        NoteProfile {
            text: Some(text.to_owned()),
            ..NoteProfile::default()
        }
    }

    const REMARK: &str = "Ole Olsen flyttet fra Haugen til Amerika i 1882 med kone og tre barn.";

    #[test]
    fn one_note_formatted_two_ways_is_probable() {
        let reformatted = "**Ole Olsen** flyttet fra *Haugen* til Amerika i 1882 — med kone og tre barn";
        let assessment = assess(&note(REMARK), &note(reformatted));
        assert_eq!(feature(&assessment, Feature::Text).outcome, Outcome::Agree);
        assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    }

    #[test]
    fn an_edited_note_agrees_in_part() {
        let edited = "Ole Olsen flyttet fra Haugen til Amerika i 1882 med kone og fire barn.";
        let text = feature(&assess(&note(REMARK), &note(edited)), Feature::Text).clone();
        assert!(matches!(text.outcome, Outcome::Partial(_)), "{text:?}");
        assert!(text.weight > 0.0, "{text:?}");
    }

    #[test]
    fn another_note_disagrees_and_an_empty_one_is_missing() {
        let other = assess(&note(REMARK), &note("Kirkeboken for 1850 er skadet av vann."));
        assert_eq!(feature(&other, Feature::Text).outcome, Outcome::Disagree);
        assert_eq!(other.band, MatchBand::Unlikely);
        let empty = assess(&note(REMARK), &NoteProfile::default());
        assert_eq!(feature(&empty, Feature::Text).outcome, Outcome::Missing);
    }

    #[test]
    fn the_texts_language_selects_its_culture() {
        let norwegian = NoteProfile {
            language: Some(LanguageTag::new("nb")),
            ..note(REMARK)
        };
        let cultures: Vec<String> = assess(&norwegian, &note(REMARK))
            .cultures
            .iter()
            .map(|id: &CultureId| id.as_str().to_owned())
            .collect();
        assert_eq!(cultures, ["universal", "no"]);
    }

    #[test]
    fn a_shared_origin_is_deterministic() {
        let (mut a, mut b) = (note(REMARK), note("anything"));
        a.origins.push(household(Some("note:1")));
        b.origins.push(household(Some("note:1")));
        assert_eq!(assess(&a, &b).band, MatchBand::Deterministic);
    }
}
