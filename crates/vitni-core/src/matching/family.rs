//! Family matching (ADR 0038 §2, §4): one marriage from a church book and from a GEDCOM file.
//!
//! A family is compared by its partners, its children and its marriage. Every partner of one family is
//! assessed as a person against every partner of the other, under the cultures the two families select
//! together — a marriage in a Norwegian parish selects Norwegian names for partners who carry no place
//! of their own. The pairs are then chosen one to one, best score first, so partners pair by who they
//! are and not by the order they are listed in, and each chosen pair is one [`Feature::Partner`] term:
//! its weight is the sum of the pair's own feature weights — capped, since a remarriage shares a partner
//! too, so only both partners agreeing make one family — and its assessment is kept in
//! [`MatchAssessment::parts`] as the explanation. Children only ever support a pair. The marriage is
//! compared by its date and place; its participants are the partners, already compared.

use crate::matching::name::Applied;
use crate::matching::profile::VitalKind;
use crate::matching::profile::{FamilyProfile, PersonProfile, Relative};
use crate::matching::relative::{self, CHILDREN};
use crate::matching::select::{Signals, comparison_cultures};
use crate::matching::weights;
use crate::matching::{
    CultureId, Feature, FeatureComparison, FeatureValue, Identity, MatchAssessment, MatchData, MatchSettings, applied,
    compare_dates, compare_places, conclude, estimate, event_estimate, may_be_one, missing, pair_up, person_assessment,
    summary_term,
};

/// Compares two family profiles.
#[must_use]
pub fn assess_families(
    a: &FamilyProfile,
    b: &FamilyProfile,
    data: &MatchData,
    settings: &MatchSettings,
) -> MatchAssessment {
    let cultures = comparison_cultures(&Signals::family(a), &Signals::family(b), data, settings);
    let applied = applied(&cultures, data);
    let (mut features, parts) = compare_partners(a, b, &cultures, &applied, settings);
    let children: (Vec<&Relative>, Vec<&Relative>) = (a.children.iter().collect(), b.children.iter().collect());
    features.push(relative::compare(CHILDREN, &children.0, &children.1, &applied));
    let (marriage_a, marriage_b) = (a.marriage.as_ref(), b.marriage.as_ref());
    let (date_a, date_b) = (
        event_estimate(marriage_a.and_then(|m| m.date.as_ref())),
        event_estimate(marriage_b.and_then(|m| m.date.as_ref())),
    );
    features.push(compare_dates(
        Feature::Marriage,
        date_a.as_ref(),
        date_b.as_ref(),
        weights::EVENT_DATE,
    ));
    features.push(compare_places(
        Feature::MarriagePlace,
        marriage_a.and_then(|m| m.place.as_ref()),
        marriage_b.and_then(|m| m.place.as_ref()),
        &applied,
    ));
    let sides = (
        Identity::of(&a.origins, &a.external_ids),
        Identity::of(&b.origins, &b.external_ids),
    );
    conclude(features, cultures, parts, sides, settings)
}

/// Assesses every pair of partners, chooses the pairs one to one, best score first, and returns one term
/// per chosen pair with its assessment; a single missing term when no pair can be formed.
fn compare_partners(
    a: &FamilyProfile,
    b: &FamilyProfile,
    cultures: &[CultureId],
    applied: &Applied<'_>,
    settings: &MatchSettings,
) -> (Vec<FeatureComparison>, Vec<MatchAssessment>) {
    let mut assessed = Vec::new();
    let mut candidates = Vec::new();
    for (i, x) in a.partners.iter().enumerate() {
        for (j, y) in b.partners.iter().enumerate() {
            let assessment = person_assessment(x, y, cultures.to_vec(), applied, settings);
            if may_be_one(x.sex.as_ref(), y.sex.as_ref()) {
                candidates.push((assessment.score, i, j));
            }
            assessed.push(Some(assessment));
        }
    }
    let (mut terms, mut parts) = (Vec::new(), Vec::new());
    for (_, i, j) in pair_up(candidates) {
        let (Some(x), Some(y)) = (a.partners.get(i), b.partners.get(j)) else {
            continue;
        };
        let Some(part) = assessed.get_mut(i * b.partners.len() + j).and_then(Option::take) else {
            continue;
        };
        terms.push(partner_term(x, y, &part));
        parts.push(part);
    }
    if terms.is_empty() {
        let first = |family: &FamilyProfile| family.partners.first().map(value);
        terms.push(missing(Feature::Partner, first(a), first(b)));
    }
    (terms, parts)
}

/// The term summarising one partner pair's assessment, its support capped at
/// [`weights::PARTNER_SUPPORT`].
fn partner_term(x: &PersonProfile, y: &PersonProfile, part: &MatchAssessment) -> FeatureComparison {
    summary_term(
        Feature::Partner,
        part,
        weights::PARTNER_SUPPORT,
        (Some(value(x)), Some(value(y))),
    )
}

/// A partner as shown beside a feature: the first name, with the birth (or its stand-in).
fn value(partner: &PersonProfile) -> FeatureValue {
    let born = estimate(&partner.vitals, VitalKind::Birth).map(|birth| birth.date.clone());
    relative::shown(&partner.names, born)
}

#[cfg(test)]
mod tests;
