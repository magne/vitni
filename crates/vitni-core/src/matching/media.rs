//! Media matching (ADR 0038 §2): one scan or photograph recorded twice.
//!
//! A media object is compared by its checksum, exactly — equal bytes are one file, other bytes another —
//! and by its file name as weak evidence, since a file is renamed and one name is reused for other files.
//! The aggregate records no description, so none is compared. No name culture applies.

use crate::matching::name::fold;
use crate::matching::profile::MediaProfile;
use crate::matching::weights;
use crate::matching::{
    Feature, FeatureComparison, FeatureValue, Identity, MatchAssessment, MatchSettings, Outcome, conclude, missing,
};
use crate::media_path::MediaPath;

/// Compares two media profiles.
#[must_use]
pub fn assess_media(a: &MediaProfile, b: &MediaProfile, settings: &MatchSettings) -> MatchAssessment {
    let features = vec![
        compare_checksums(a.checksum.as_deref(), b.checksum.as_deref()),
        compare_paths(a.path.as_ref(), b.path.as_ref()),
    ];
    let sides = (Identity::of(&a.origins, &[]), Identity::of(&b.origins, &[]));
    conclude(features, Vec::new(), Vec::new(), sides, settings)
}

/// Compares two checksums exactly, ignoring surrounding space and the case of hex digits.
fn compare_checksums(a: Option<&str>, b: Option<&str>) -> FeatureComparison {
    let value = |checksum: &str| FeatureValue::Text(checksum.to_owned());
    let (left, right) = (a.map(value), b.map(value));
    let key = |checksum: &str| checksum.trim().to_ascii_lowercase();
    let (Some(x), Some(y)) = (
        a.map(key).filter(|x| !x.is_empty()),
        b.map(key).filter(|y| !y.is_empty()),
    ) else {
        return missing(Feature::Checksum, left, right);
    };
    let (outcome, weight) = if x == y {
        (Outcome::Agree, weights::CHECKSUM.agree)
    } else {
        (Outcome::Disagree, weights::CHECKSUM.disagree)
    };
    FeatureComparison {
        feature: Feature::Checksum,
        outcome,
        weight,
        left,
        right,
    }
}

/// Compares two locations by their folded file names; a different name is no evidence.
fn compare_paths(a: Option<&MediaPath>, b: Option<&MediaPath>) -> FeatureComparison {
    let (left, right) = (a.cloned().map(FeatureValue::Path), b.cloned().map(FeatureValue::Path));
    match (a.and_then(file_name), b.and_then(file_name)) {
        (Some(x), Some(y)) if x == y => FeatureComparison {
            feature: Feature::Path,
            outcome: Outcome::Agree,
            weight: weights::FILE_NAME_AGREE,
            left,
            right,
        },
        (Some(_) | None, _) => missing(Feature::Path, left, right),
    }
}

/// The folded last segment of a location, without a web reference's query or fragment.
fn file_name(path: &MediaPath) -> Option<String> {
    let text = match path {
        MediaPath::File(file) => file.as_str(),
        MediaPath::Web(url) => url.href.split(['?', '#']).next().unwrap_or_default(),
    };
    let name = text.rsplit(['/', '\\']).next()?.trim();
    (!name.is_empty()).then(|| fold(name))
}

#[cfg(test)]
mod tests {
    use crate::matching::profile::MediaProfile;
    use crate::matching::tests::{feature, household};
    use crate::matching::{Feature, MatchAssessment, MatchBand, MatchSettings, Outcome, assess_media};
    use crate::media_path::MediaPath;
    use crate::text::Url;

    fn assess(a: &MediaProfile, b: &MediaProfile) -> MatchAssessment {
        assess_media(a, b, &MatchSettings::default())
    }

    fn file(path: &str, checksum: Option<&str>) -> MediaProfile {
        MediaProfile {
            checksum: checksum.map(ToOwned::to_owned),
            path: Some(MediaPath::File(path.to_owned())),
            origins: Vec::new(),
        }
    }

    #[test]
    fn one_checksum_is_one_file_whatever_its_name() {
        let a = file("media/portretter/ole.jpg", Some("9f86d081884c7d65"));
        let b = file("C:\\Bilder\\scan-0042.jpg", Some(" 9F86D081884C7D65 "));
        let assessment = assess(&a, &b);
        assert_eq!(feature(&assessment, Feature::Checksum).outcome, Outcome::Agree);
        assert_eq!(feature(&assessment, Feature::Path).outcome, Outcome::Missing);
        assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
        assert!(assessment.cultures.is_empty(), "{:?}", assessment.cultures);
    }

    #[test]
    fn another_checksum_is_another_file_even_under_one_name() {
        let a = file("media/ole.jpg", Some("9f86d081884c7d65"));
        let b = file("media/ole.jpg", Some("60303ae22b998861"));
        let assessment = assess(&a, &b);
        assert_eq!(feature(&assessment, Feature::Checksum).outcome, Outcome::Disagree);
        assert_eq!(feature(&assessment, Feature::Path).outcome, Outcome::Agree);
        assert_eq!(assessment.band, MatchBand::Unlikely, "{assessment:#?}");
    }

    #[test]
    fn a_file_name_alone_is_weak_evidence() {
        let assessment = assess(&file("media/Ole.JPG", None), &file("old/ole.jpg", Some("  ")));
        assert_eq!(feature(&assessment, Feature::Checksum).outcome, Outcome::Missing);
        let path = feature(&assessment, Feature::Path);
        assert_eq!(path.outcome, Outcome::Agree);
        assert!(path.weight > 0.0);
        assert_eq!(assessment.band, MatchBand::Unlikely, "{assessment:#?}");
    }

    #[test]
    fn a_web_reference_is_named_by_its_last_segment() {
        let web = MediaProfile {
            path: Some(MediaPath::Web(Url {
                url_type: None,
                href: "https://example.org/scans/ole.jpg?size=2#top".to_owned(),
                description: None,
            })),
            ..MediaProfile::default()
        };
        let assessment = assess(&web, &file("media/ole.jpg", None));
        assert_eq!(feature(&assessment, Feature::Path).outcome, Outcome::Agree);
    }

    #[test]
    fn a_shared_origin_is_deterministic() {
        let (mut a, mut b) = (file("a.jpg", None), file("b.jpg", None));
        a.origins.push(household(Some("media:1")));
        b.origins.push(household(Some("media:1")));
        assert_eq!(assess(&a, &b).band, MatchBand::Deterministic);
    }
}
