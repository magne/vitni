//! Event matching (ADR 0038 §2, §4): two records of one marriage, baptism or census.
//!
//! An event is compared by its type, its date and its place, and by the people taking part. Its
//! principals — the primary participant, a husband, wife, spouse, groom or bride — are paired one to one,
//! most similar first and never across an asserted sex, and each pair is a term: an event with another
//! principal is another event. The other participants — witnesses, godparents, parents, clergy — only
//! ever support a pair, because sources list different ones. A different type is a conflict, except that
//! a christening is a baptism; a custom type agrees only with the same custom type, and is otherwise no
//! evidence.

use crate::enums::{EventType, ParticipantRole};
use crate::matching::name::{Applied, fold};
use crate::matching::profile::{EventProfile, Relative};
use crate::matching::relative::{self, Kind, Mismatch};
use crate::matching::select::{Signals, comparison_cultures};
use crate::matching::weights;
use crate::matching::{
    Feature, FeatureComparison, FeatureValue, Identity, MatchAssessment, MatchData, MatchSettings, Outcome, applied,
    compare_dates, compare_places, conclude, event_estimate, grade, may_be_one, missing, pair_up,
};

/// The participants who are not principals.
const OTHERS: Kind = Kind {
    feature: Feature::Participants,
    weights: weights::PARTICIPANT,
    mismatch: Mismatch::NoEvidence,
};

/// Compares two event profiles.
#[must_use]
pub fn assess_events(
    a: &EventProfile,
    b: &EventProfile,
    data: &MatchData,
    settings: &MatchSettings,
) -> MatchAssessment {
    let cultures = comparison_cultures(&Signals::event(a), &Signals::event(b), data, settings);
    let applied = applied(&cultures, data);
    let (date_a, date_b) = (event_estimate(a.date.as_ref()), event_estimate(b.date.as_ref()));
    let mut features = vec![
        compare_types(a.event_type.as_ref(), b.event_type.as_ref()),
        compare_dates(Feature::Date, date_a.as_ref(), date_b.as_ref(), weights::EVENT_DATE),
        compare_places(Feature::Place, a.place.as_ref(), b.place.as_ref(), &applied),
    ];
    let (principals_a, others_a) = participants(a);
    let (principals_b, others_b) = participants(b);
    features.extend(compare_principals(&principals_a, &principals_b, &applied));
    features.push(relative::compare(OTHERS, &others_a, &others_b, &applied));
    let sides = (Identity::of(&a.origins, &[]), Identity::of(&b.origins, &[]));
    conclude(features, cultures, Vec::new(), sides, settings)
}

/// An event type as compared: a christening is a baptism, and a custom type is its folded text.
#[derive(Debug, PartialEq, Eq)]
enum TypeKey<'t> {
    Known(&'t EventType),
    Custom(String),
}

impl<'t> TypeKey<'t> {
    fn of(event_type: &'t EventType) -> Self {
        if let EventType::Custom(text) = event_type {
            Self::Custom(fold(text.trim()))
        } else if *event_type == EventType::Christening {
            Self::Known(&EventType::Baptism)
        } else {
            Self::Known(event_type)
        }
    }
}

/// Compares two event types.
fn compare_types(a: Option<&EventType>, b: Option<&EventType>) -> FeatureComparison {
    let value = |event_type: &EventType| FeatureValue::EventType(event_type.clone());
    let (left, right) = (a.map(value), b.map(value));
    let (Some(x), Some(y)) = (a, b) else {
        return missing(Feature::EventType, left, right);
    };
    let (outcome, weight) = match (TypeKey::of(x), TypeKey::of(y)) {
        (x, y) if x == y => (Outcome::Agree, weights::EVENT_TYPE_AGREE),
        (TypeKey::Known(_), TypeKey::Known(_)) => (Outcome::Conflict, weights::CONFLICT),
        (TypeKey::Custom(_), _) | (_, TypeKey::Custom(_)) => (Outcome::Missing, 0.0),
    };
    FeatureComparison {
        feature: Feature::EventType,
        outcome,
        weight,
        left,
        right,
    }
}

/// Whether a role is one of an event's principals.
pub(crate) fn is_principal(role: &ParticipantRole) -> bool {
    match role {
        ParticipantRole::Primary
        | ParticipantRole::Husband
        | ParticipantRole::Wife
        | ParticipantRole::Spouse
        | ParticipantRole::Bride
        | ParticipantRole::Groom => true,
        ParticipantRole::Witness
        | ParticipantRole::Officiator
        | ParticipantRole::Clergy
        | ParticipantRole::Father
        | ParticipantRole::Mother
        | ParticipantRole::Parent
        | ParticipantRole::Child
        | ParticipantRole::Godparent
        | ParticipantRole::Friend
        | ParticipantRole::Neighbour
        | ParticipantRole::Multiple
        | ParticipantRole::Custom(_) => false,
    }
}

/// An event's principals, then its other participants.
fn participants(event: &EventProfile) -> (Vec<&Relative>, Vec<&Relative>) {
    let (mut principals, mut others) = (Vec::new(), Vec::new());
    for participant in &event.participants {
        if is_principal(&participant.role) {
            principals.push(&participant.person);
        } else {
            others.push(&participant.person);
        }
    }
    (principals, others)
}

/// Pairs the principals and grades each pair; a single missing term when none can be paired.
fn compare_principals(left: &[&Relative], right: &[&Relative], applied: &Applied<'_>) -> Vec<FeatureComparison> {
    let mut candidates = Vec::new();
    for (i, x) in left.iter().enumerate() {
        for (j, y) in right.iter().enumerate() {
            if !may_be_one(x.sex.as_ref(), y.sex.as_ref()) {
                continue;
            }
            if let Some(similarity) = relative::similarity(x, y, applied) {
                candidates.push((similarity, i, j));
            }
        }
    }
    let mut terms = Vec::new();
    for (similarity, i, j) in pair_up(candidates) {
        let (Some(x), Some(y)) = (left.get(i), right.get(j)) else {
            continue;
        };
        let (outcome, weight) = grade(similarity, 0.0, weights::PRINCIPAL);
        terms.push(FeatureComparison {
            feature: Feature::Principal,
            outcome,
            weight,
            left: Some(relative::value(x)),
            right: Some(relative::value(y)),
        });
    }
    if terms.is_empty() {
        let first = |side: &[&Relative]| side.first().map(|r| relative::value(r));
        let mut term = missing(Feature::Principal, first(left), first(right));
        if sexed_apart(left, right) {
            term.outcome = Outcome::Disagree;
            term.weight = weights::PRINCIPAL.disagree;
        }
        terms.push(term);
    }
    terms
}

/// Whether both events state principals and none of one can be any of the other's by asserted sex: the
/// baptism of a boy and of a girl are two events.
fn sexed_apart(left: &[&Relative], right: &[&Relative]) -> bool {
    !left.is_empty()
        && !right.is_empty()
        && left
            .iter()
            .all(|x| right.iter().all(|y| !may_be_one(x.sex.as_ref(), y.sex.as_ref())))
}

#[cfg(test)]
mod tests;
