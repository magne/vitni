//! Relatives as evidence (ADR 0038 §4): the "family match" that separates two *Ole Olsen*s born the same
//! year, and the patronymic read against the candidate father.
//!
//! Two relatives are as similar as their given names, limited by their birth dates when both are dated.
//! A father or mother both records state and who cannot be one person disagrees. Partners and children
//! only ever support a pair, because sources list different subsets of them. A patronymic is checked
//! against the other record's father only where its own record states none: where both do, the fathers
//! are compared directly.

use crate::enums::Sex;
use crate::matching::date::similarity as date_similarity;
use crate::matching::name::Applied;
use crate::matching::profile::{PersonProfile, Relative, VitalKind};
use crate::matching::weights::{self, Weights};
use crate::matching::{Estimate, Feature, FeatureComparison, FeatureValue, Outcome, estimate, grade, missing};

/// The most an undated relative's name alone can agree: a name is shared by many.
const UNDATED_SIMILARITY: f64 = 0.9;

/// A relative's first name as shown beside a feature, with its birth date.
fn value(relative: &Relative) -> FeatureValue {
    let name = relative.names.first().map_or_else(String::new, |name| {
        let mut parts: Vec<&str> = name.given.iter().map(String::as_str).collect();
        parts.extend(name.surnames.iter().map(|s| s.surname.as_str()));
        parts.join(" ")
    });
    FeatureValue::Relative {
        name,
        born: relative.birth.as_ref().and_then(|birth| birth.date.clone()),
    }
}

/// A relative's birth estimate, a baptism standing in for it.
fn birth(relative: &Relative) -> Option<Estimate<'_>> {
    estimate(relative.birth.as_slice(), VitalKind::Birth)
}

/// How similar two relatives are, or `None` when their given names cannot be compared.
fn similarity(a: &Relative, b: &Relative, applied: &Applied<'_>) -> Option<f64> {
    let mut best: Option<f64> = None;
    for x in a.names.iter().filter_map(|n| n.given.as_deref()) {
        for y in b.names.iter().filter_map(|n| n.given.as_deref()) {
            if let Some(s) = applied.given_similarity(x, y) {
                best = Some(best.map_or(s, |so_far: f64| so_far.max(s)));
            }
        }
    }
    let named = best?;
    if named < weights::NAME_FLOOR {
        return Some(0.0);
    }
    let dated = match (birth(a), birth(b)) {
        (Some(x), Some(y)) => date_similarity(x.interval.gap(y.interval), x.tolerance.max(y.tolerance)),
        (Some(_) | None, None) | (None, Some(_)) => UNDATED_SIMILARITY,
    };
    Some(named.min(dated))
}

/// What it means when no relative of one record can be any of the other's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mismatch {
    /// The records disagree: each states the one relative of this kind a person has.
    Disagrees,
    /// No evidence: sources list different subsets of these relatives.
    NoEvidence,
}

/// One kind of relative: the feature it is scored as, its weights, and what a mismatch means.
#[derive(Debug, Clone, Copy)]
struct Kind {
    feature: Feature,
    weights: Weights,
    mismatch: Mismatch,
}

/// Compares one kind of relative by its best-matching pair.
fn compare(kind: Kind, left: &[&Relative], right: &[&Relative], applied: &Applied<'_>) -> FeatureComparison {
    let feature = kind.feature;
    let mut best: Option<(f64, &Relative, &Relative)> = None;
    for x in left {
        for y in right {
            let Some(similar) = similarity(x, y, applied) else {
                continue;
            };
            if best.is_none_or(|(so_far, _, _)| similar > so_far) {
                best = Some((similar, x, y));
            }
        }
    }
    let Some((similar, x, y)) = best else {
        return missing(feature, left.first().map(|r| value(r)), right.first().map(|r| value(r)));
    };
    let (left, right) = (Some(value(x)), Some(value(y)));
    let (outcome, weight) = grade(similar, 0.0, kind.weights);
    if outcome == Outcome::Disagree && kind.mismatch == Mismatch::NoEvidence {
        return missing(feature, left, right);
    }
    FeatureComparison {
        feature,
        outcome,
        weight,
        left,
        right,
    }
}

/// The parents of `profile` of one sex.
fn parents<'p>(profile: &'p PersonProfile, sex: &Sex) -> Vec<&'p Relative> {
    profile.parents.iter().filter(|p| p.sex.as_ref() == Some(sex)).collect()
}

/// Compares the fathers, the mothers, the partners and the children.
pub(crate) fn compare_relatives<'p>(
    a: &'p PersonProfile,
    b: &'p PersonProfile,
    applied: &Applied<'_>,
) -> Vec<FeatureComparison> {
    let parent = |feature| Kind {
        feature,
        weights: weights::PARENT,
        mismatch: Mismatch::Disagrees,
    };
    let (partners, children) = (
        Kind {
            feature: Feature::Partners,
            weights: weights::PARTNER,
            mismatch: Mismatch::NoEvidence,
        },
        Kind {
            feature: Feature::Children,
            weights: weights::CHILD,
            mismatch: Mismatch::NoEvidence,
        },
    );
    let everyone = |relatives: &'p [Relative]| -> Vec<&'p Relative> { relatives.iter().collect() };
    vec![
        compare(
            parent(Feature::Father),
            &parents(a, &Sex::Male),
            &parents(b, &Sex::Male),
            applied,
        ),
        compare(
            parent(Feature::Mother),
            &parents(a, &Sex::Female),
            &parents(b, &Sex::Female),
            applied,
        ),
        compare(partners, &everyone(&a.partners), &everyone(&b.partners), applied),
        compare(children, &everyone(&a.children), &everyone(&b.children), applied),
    ]
}

/// Checks a patronymic against the candidate father: `a`'s surnames against `b`'s fathers when only `b`
/// states one, or the other way round.
pub(crate) fn compare_patronymic(a: &PersonProfile, b: &PersonProfile, applied: &Applied<'_>) -> FeatureComparison {
    let (fathers_a, fathers_b) = (parents(a, &Sex::Male), parents(b, &Sex::Male));
    let checked = match (fathers_a.is_empty(), fathers_b.is_empty()) {
        (true, false) => patronymic(a, &fathers_b, applied)
            .map(|(outcome, name, father)| (outcome, FeatureValue::Name(name), value(father))),
        (false, true) => patronymic(b, &fathers_a, applied)
            .map(|(outcome, name, father)| (outcome, value(father), FeatureValue::Name(name))),
        (true, true) | (false, false) => None,
    };
    let Some((outcome, left, right)) = checked else {
        return missing(Feature::Patronymic, None, None);
    };
    let weight = match outcome {
        Outcome::Agree => weights::PATRONYMIC.agree,
        Outcome::Partial(_) | Outcome::Disagree | Outcome::Missing | Outcome::Conflict => weights::PATRONYMIC.disagree,
    };
    FeatureComparison {
        feature: Feature::Patronymic,
        outcome,
        weight,
        left: Some(left),
        right: Some(right),
    }
}

/// Whether some patronymic surname of `child` is formed from some given name of `fathers`: `Agree` with
/// the pair that fits, else `Disagree` with the first pair tried, or `None` when there is no patronymic
/// or no father's given name.
fn patronymic<'r>(
    child: &PersonProfile,
    fathers: &[&'r Relative],
    applied: &Applied<'_>,
) -> Option<(Outcome, String, &'r Relative)> {
    let mut first: Option<(String, &Relative)> = None;
    for surname in child.names.iter().flat_map(|n| n.surnames.iter().map(|s| &s.surname)) {
        let Some(stem) = applied.surname_patronymic_key(surname) else {
            continue;
        };
        for father in fathers {
            for given in father.names.iter().filter_map(|n| n.given.as_deref()) {
                if applied.father_patronymic_keys(given).contains(&stem) {
                    return Some((Outcome::Agree, surname.clone(), father));
                }
                first.get_or_insert_with(|| (surname.clone(), father));
            }
        }
    }
    first.map(|(surname, father)| (Outcome::Disagree, surname, father))
}
