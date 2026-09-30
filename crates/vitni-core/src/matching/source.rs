//! Source, repository and citation matching (ADR 0038 §2, §4): one church book, one archive and one
//! entry in it, recorded twice.
//!
//! A source is compared by its title, author and publication and by the repositories holding it; a
//! repository by its name and addresses. Titles, names and addresses are free text, compared by the words
//! they share, so *Ringsaker ministerialbok* and *Ministerialbok for Ringsaker* agree. A copy is published
//! otherwise and held elsewhere, so publication and repository only ever support a pair.
//!
//! A citation is compared by its source, its page and the date of its entry. Its sources are assessed as
//! sources, summarised as one [`Feature::Source`] term — capped, since one source holds many citations —
//! with the pair's assessment kept in [`MatchAssessment::parts`]. A page is compared by its numbers
//! (*s. 45, nr. 12* and *side 45 nr 12* agree, *s. 46* does not), or, when it has none, as free text.

use crate::address::Address;
use crate::matching::name::{Applied, is_number};
use crate::matching::profile::{CitationProfile, RepositoryProfile, SourceProfile};
use crate::matching::select::{Signals, comparison_cultures};
use crate::matching::weights::{self, TEXT_FLOOR, Weights};
use crate::matching::{
    CultureId, Feature, FeatureComparison, FeatureValue, Identity, MatchAssessment, MatchData, MatchSettings, Outcome,
    applied, compare_dates, conclude, event_estimate, grade, missing, summary_term,
};

/// Compares two source profiles.
#[must_use]
pub fn assess_sources(
    a: &SourceProfile,
    b: &SourceProfile,
    data: &MatchData,
    settings: &MatchSettings,
) -> MatchAssessment {
    let cultures = comparison_cultures(&Signals::source(a), &Signals::source(b), data, settings);
    let applied = applied(&cultures, data);
    source_assessment(a, b, cultures, &applied, settings)
}

/// Compares two source profiles under cultures already chosen — a source's own, or a citation's.
fn source_assessment(
    a: &SourceProfile,
    b: &SourceProfile,
    cultures: Vec<CultureId>,
    applied: &Applied<'_>,
    settings: &MatchSettings,
) -> MatchAssessment {
    let text = |feature, x: &Option<String>, y: &Option<String>, table| {
        compare_text(feature, x.as_deref(), y.as_deref(), table, applied)
    };
    let features = vec![
        text(Feature::Title, &a.title, &b.title, weights::TITLE),
        text(Feature::Author, &a.author, &b.author, weights::AUTHOR),
        text(
            Feature::Publication,
            &a.publication,
            &b.publication,
            weights::PUBLICATION,
        ),
        compare_repositories(&a.repositories, &b.repositories, applied),
    ];
    let sides = (Identity::of(&a.origins, &[]), Identity::of(&b.origins, &[]));
    conclude(features, cultures, Vec::new(), sides, settings)
}

/// Compares two repository profiles.
#[must_use]
pub fn assess_repositories(
    a: &RepositoryProfile,
    b: &RepositoryProfile,
    data: &MatchData,
    settings: &MatchSettings,
) -> MatchAssessment {
    let cultures = comparison_cultures(&Signals::repository(a), &Signals::repository(b), data, settings);
    let applied = applied(&cultures, data);
    let features = vec![
        compare_text(
            Feature::Name,
            a.name.as_deref(),
            b.name.as_deref(),
            weights::REPOSITORY_NAME,
            &applied,
        ),
        compare_addresses(&a.addresses, &b.addresses, &applied),
    ];
    let sides = (Identity::of(&a.origins, &[]), Identity::of(&b.origins, &[]));
    conclude(features, cultures, Vec::new(), sides, settings)
}

/// Compares two citation profiles.
#[must_use]
pub fn assess_citations(
    a: &CitationProfile,
    b: &CitationProfile,
    data: &MatchData,
    settings: &MatchSettings,
) -> MatchAssessment {
    let cultures = comparison_cultures(&Signals::citation(a), &Signals::citation(b), data, settings);
    let applied = applied(&cultures, data);
    let title = |source: &SourceProfile| source.title.clone().map(FeatureValue::Text);
    let (left, right) = (a.source.as_ref().and_then(title), b.source.as_ref().and_then(title));
    let mut parts = Vec::new();
    let source = match (&a.source, &b.source) {
        (Some(x), Some(y)) => {
            let part = source_assessment(x, y, cultures.clone(), &applied, settings);
            let term = summary_term(Feature::Source, &part, weights::SOURCE_SUPPORT, (left, right));
            parts.push(part);
            term
        }
        (None, _) | (_, None) => missing(Feature::Source, left, right),
    };
    let (date_a, date_b) = (event_estimate(a.date.as_ref()), event_estimate(b.date.as_ref()));
    let features = vec![
        source,
        compare_pages(a.page.as_deref(), b.page.as_deref(), &applied),
        compare_dates(Feature::Date, date_a.as_ref(), date_b.as_ref(), weights::CITATION_DATE),
    ];
    let sides = (Identity::of(&a.origins, &[]), Identity::of(&b.origins, &[]));
    conclude(features, cultures, parts, sides, settings)
}

/// A term from a similarity graded on `table` from [`TEXT_FLOOR`]. Where `table` holds nothing against
/// a mismatch, a mismatch is missing evidence rather than a disagreement.
pub(super) fn text_term(
    feature: Feature,
    similarity: Option<f64>,
    table: Weights,
    (left, right): (Option<FeatureValue>, Option<FeatureValue>),
) -> FeatureComparison {
    let Some(similarity) = similarity else {
        return missing(feature, left, right);
    };
    let (mut outcome, mut weight) = grade(similarity, TEXT_FLOOR, table);
    if outcome == Outcome::Disagree && table.disagree == 0.0 {
        (outcome, weight) = (Outcome::Missing, 0.0);
    }
    FeatureComparison {
        feature,
        outcome,
        weight,
        left,
        right,
    }
}

/// Compares two free texts by the words they share.
pub(super) fn compare_text(
    feature: Feature,
    a: Option<&str>,
    b: Option<&str>,
    table: Weights,
    applied: &Applied<'_>,
) -> FeatureComparison {
    let similarity = a.zip(b).and_then(|(x, y)| applied.text_similarity(x, y));
    let value = |text: &str| FeatureValue::Text(text.to_owned());
    text_term(feature, similarity, table, (a.map(value), b.map(value)))
}

/// Compares the repositories holding two sources: the best pair, the same repository or one named alike.
fn compare_repositories(
    left: &[RepositoryProfile],
    right: &[RepositoryProfile],
    applied: &Applied<'_>,
) -> FeatureComparison {
    let name = |repository: &RepositoryProfile| repository.name.clone().map(FeatureValue::Text);
    let mut best: Option<(f64, &RepositoryProfile, &RepositoryProfile)> = None;
    for x in left {
        for y in right {
            let similarity = if x.id.is_some() && x.id == y.id {
                Some(1.0)
            } else {
                x.name
                    .as_deref()
                    .zip(y.name.as_deref())
                    .and_then(|(p, q)| applied.text_similarity(p, q))
            };
            if let Some(s) = similarity
                && best.is_none_or(|(so_far, _, _)| s > so_far)
            {
                best = Some((s, x, y));
            }
        }
    }
    let Some((similarity, x, y)) = best else {
        return missing(
            Feature::Repository,
            left.first().and_then(name),
            right.first().and_then(name),
        );
    };
    text_term(
        Feature::Repository,
        Some(similarity),
        weights::REPOSITORY,
        (name(x), name(y)),
    )
}

/// Compares two repositories' addresses: the best pair, as free text.
fn compare_addresses(left: &[Address], right: &[Address], applied: &Applied<'_>) -> FeatureComparison {
    let value = |address: &Address| FeatureValue::Address(address.clone());
    let mut best: Option<(f64, &Address, &Address)> = None;
    for x in left {
        for y in right {
            if let Some(s) = applied.text_similarity(&address_text(x), &address_text(y))
                && best.is_none_or(|(so_far, _, _)| s > so_far)
            {
                best = Some((s, x, y));
            }
        }
    }
    let Some((similarity, x, y)) = best else {
        return missing(Feature::Address, left.first().map(value), right.first().map(value));
    };
    text_term(
        Feature::Address,
        Some(similarity),
        weights::ADDRESS,
        (Some(value(x)), Some(value(y))),
    )
}

/// An address's lines, locality, region, postal code and country as one text.
fn address_text(address: &Address) -> String {
    let parts = [
        &address.locality,
        &address.region,
        &address.postal_code,
        &address.country,
    ];
    let mut text = address.lines.join(" ");
    for part in parts.into_iter().flatten() {
        text.push(' ');
        text.push_str(part);
    }
    text
}

/// Compares two pages by their numbers, or as free text when either has none.
fn compare_pages(a: Option<&str>, b: Option<&str>, applied: &Applied<'_>) -> FeatureComparison {
    let similarity = a.zip(b).and_then(|(x, y)| {
        let (p, q) = (numbers(x, applied), numbers(y, applied));
        if p.is_empty() || q.is_empty() {
            applied.text_similarity(x, y)
        } else if p == q {
            Some(1.0)
        } else {
            Some(0.0)
        }
    });
    let value = |text: &str| FeatureValue::Text(text.to_owned());
    text_term(Feature::Page, similarity, weights::PAGE, (a.map(value), b.map(value)))
}

/// The numbers in a locator, without leading zeros, in ascending order.
fn numbers(locator: &str, applied: &Applied<'_>) -> Vec<String> {
    let mut numbers: Vec<String> = applied
        .tokens(locator)
        .into_iter()
        .filter(|token| is_number(token))
        .map(|token| {
            let trimmed = token.trim_start_matches('0');
            if trimmed.is_empty() {
                "0".to_owned()
            } else {
                trimmed.to_owned()
            }
        })
        .collect();
    numbers.sort();
    numbers
}

#[cfg(test)]
mod tests;
