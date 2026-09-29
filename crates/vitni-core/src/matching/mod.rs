//! Record matching: explainable Fellegi–Sunter scoring (ADR 0038).
//!
//! [`assess_persons`] compares two [`PersonProfile`]s and returns a [`MatchAssessment`]. Each
//! comparator grades one feature — names, sex, birth and death dates, their places — as `Agree`,
//! `Partial`, `Disagree`, `Missing` or `Conflict`, with a log-likelihood weight. The weights sum to the
//! match weight, which maps to a score in `0..1` and a [`MatchBand`]. Every term is kept, so the
//! features *are* the explanation.
//!
//! Comparators are graded, never binary: a near miss lowers the score but keeps the candidate, only an
//! implausible distance disagrees, and only a logical impossibility (a different asserted sex, a death
//! before the other's birth) is a conflict, which caps the score. [`MatchBand::Deterministic`] is never
//! earned by a score: only a shared record origin or external id establishes identity (§6).
//!
//! The name rules are data — [`pack`]s selected per comparison from places, dates, data languages and
//! lineage ([`select`]). The module is pure: it reads no files and no configuration. The app layer
//! reads any workspace packs and hands them in through [`MatchData::layered`].

pub mod date;
mod name;
pub mod pack;
mod place;
pub mod profile;
pub mod select;
mod weights;

use crate::enums::Sex;
use crate::matching::date::{DayInterval, SLIP_SIMILARITY, Tolerance, interval, is_clerical_slip, similarity};
use crate::matching::name::Applied;
use crate::matching::pack::{CulturePacks, PackError, PackSource};
use crate::matching::profile::{PersonProfile, PlaceProfile, VitalEvent, VitalKind};
use crate::matching::select::{RegionTable, comparison_cultures};
use crate::matching::weights::Weights;

pub use crate::matching::date::DateBasis;

/// The id of a name-culture pack (`no`, `en`, `pl-en`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CultureId(String);

impl CultureId {
    /// Wraps a pack id.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The version of the scoring engine, recorded on every assessment so a stored one can be told apart
/// from a rescoring by a later engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EngineVersion(pub u32);

/// The engine version of this build. Bump it when a comparator, weight or shipped pack changes scores.
pub const ENGINE_VERSION: EngineVersion = EngineVersion(1);

/// How sure the engine is that two records describe one individual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchBand {
    /// Below the possible threshold: not shown.
    Unlikely,
    /// Above the possible threshold.
    Possible,
    /// Above the probable threshold.
    Probable,
    /// Identity already established by a shared record origin or external id — never by a score.
    Deterministic,
}

/// One feature's comparison outcome.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    /// The values agree after normalization.
    Agree,
    /// The values are close; the similarity is in `0..1`.
    Partial(f64),
    /// The values are implausibly far apart.
    Disagree,
    /// One side, or both, has no value.
    Missing,
    /// The values are logically impossible for one individual.
    Conflict,
}

/// A compared feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// The given names.
    GivenName,
    /// The surnames.
    Surname,
    /// The asserted sex.
    Sex,
    /// The birth date, or a baptism standing in for it.
    Birth,
    /// The death date, or a burial standing in for it.
    Death,
    /// The birth (or baptism) place.
    BirthPlace,
    /// The death (or burial) place.
    DeathPlace,
    /// One side's death against the other's birth.
    Lifespan,
}

/// A value shown beside a feature, as the record states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureValue {
    /// A name, as written.
    Name(String),
    /// A vital event's date.
    Date {
        /// Which event the date is of.
        kind: VitalKind,
        /// The date.
        date: crate::date::GenealogicalDate,
    },
    /// A place's name.
    Place(String),
    /// A sex.
    Sex(Sex),
}

/// One term of the match weight: the feature, its outcome and weight, and the values compared.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureComparison {
    /// The feature compared.
    pub feature: Feature,
    /// How the values compared.
    pub outcome: Outcome,
    /// The log₂ weight this term adds to the match weight.
    pub weight: f64,
    /// The left record's value.
    pub left: Option<FeatureValue>,
    /// The right record's value.
    pub right: Option<FeatureValue>,
}

/// The engine's verdict on a pair of records, with its evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchAssessment {
    /// The probability-like score in `0..=1`.
    pub score: f64,
    /// The band the score (or established identity) falls in.
    pub band: MatchBand,
    /// Every term of the match weight.
    pub features: Vec<FeatureComparison>,
    /// The name-culture packs applied, `universal` first.
    pub cultures: Vec<CultureId>,
    /// The engine that produced the assessment.
    pub engine: EngineVersion,
}

/// The thresholds and default cultures a comparison uses — the `[matching]` configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchSettings {
    /// The cultures a side with no place, language or lineage signal gets.
    pub default_cultures: Vec<CultureId>,
    /// The score at or above which a pair is [`MatchBand::Probable`].
    pub probable: f64,
    /// The score at or above which a pair is [`MatchBand::Possible`].
    pub possible: f64,
}

impl Default for MatchSettings {
    fn default() -> Self {
        Self {
            default_cultures: Vec::new(),
            probable: 0.95,
            possible: 0.5,
        }
    }
}

/// The installed name-culture packs and region table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchData {
    /// The installed packs.
    pub packs: CulturePacks,
    /// The region table.
    pub regions: RegionTable,
}

impl MatchData {
    /// The packs and region table shipped with the engine.
    pub fn embedded() -> Result<Self, PackError> {
        Ok(Self {
            packs: CulturePacks::embedded()?,
            regions: RegionTable::embedded()?,
        })
    }

    /// This data with `packs` layered over its packs, and `regions`, when given, replacing its table.
    pub fn layered(
        self,
        packs: impl IntoIterator<Item = PackSource>,
        regions: Option<&PackSource>,
    ) -> Result<Self, PackError> {
        let regions = match regions {
            Some(source) => RegionTable::from_source(source)?,
            None => self.regions,
        };
        Ok(Self {
            packs: self.packs.layered(packs)?,
            regions,
        })
    }
}

/// Compares two person profiles.
#[must_use]
pub fn assess_persons(
    a: &PersonProfile,
    b: &PersonProfile,
    data: &MatchData,
    settings: &MatchSettings,
) -> MatchAssessment {
    let cultures = comparison_cultures(a, b, data, settings);
    let applied = Applied::new(cultures.iter().filter_map(|id| data.packs.get(id)).collect());
    let (birth_a, birth_b) = (estimate(a, VitalKind::Birth), estimate(b, VitalKind::Birth));
    let (death_a, death_b) = (estimate(a, VitalKind::Death), estimate(b, VitalKind::Death));
    let mut features = vec![
        compare_given(a, b, &applied),
        compare_surname(a, b, &applied),
        compare_sex(a.sex.as_ref(), b.sex.as_ref()),
        compare_dates(Feature::Birth, birth_a.as_ref(), birth_b.as_ref(), weights::BIRTH),
        compare_dates(Feature::Death, death_a.as_ref(), death_b.as_ref(), weights::DEATH),
        compare_places(
            Feature::BirthPlace,
            place_of(a, VitalKind::Birth),
            place_of(b, VitalKind::Birth),
            &applied,
        ),
        compare_places(
            Feature::DeathPlace,
            place_of(a, VitalKind::Death),
            place_of(b, VitalKind::Death),
            &applied,
        ),
    ];
    features.extend(lifespan_conflict(death_a.as_ref(), birth_b.as_ref()));
    features.extend(lifespan_conflict(death_b.as_ref(), birth_a.as_ref()));
    let identified = shares_identity(a, b);
    conclude(features, cultures, identified, settings)
}

/// Sums the features into a score and band.
fn conclude(
    features: Vec<FeatureComparison>,
    cultures: Vec<CultureId>,
    identified: bool,
    settings: &MatchSettings,
) -> MatchAssessment {
    let mut match_weight = weights::PRIOR;
    let mut conflict = false;
    for feature in &features {
        match_weight += feature.weight;
        conflict |= feature.outcome == Outcome::Conflict;
    }
    let mut score = 1.0 / (1.0 + (-match_weight).exp2());
    if conflict {
        score = score.min(weights::CONFLICT_CAP);
    }
    let band = if identified {
        MatchBand::Deterministic
    } else if score >= settings.probable {
        MatchBand::Probable
    } else if score >= settings.possible {
        MatchBand::Possible
    } else {
        MatchBand::Unlikely
    };
    MatchAssessment {
        score,
        band,
        features,
        cultures,
        engine: ENGINE_VERSION,
    }
}

/// Whether the two profiles share a record origin or an external id — identity already established.
fn shares_identity(a: &PersonProfile, b: &PersonProfile) -> bool {
    let same_origin = a.origins.iter().any(|x| {
        b.origins
            .iter()
            .any(|y| x.dataset == y.dataset && x.record == y.record && x.item == y.item)
    });
    let same_external = a.external_ids.iter().any(|x| {
        b.external_ids
            .iter()
            .any(|y| x.authority == y.authority && x.value == y.value)
    });
    same_origin || same_external
}

/// A graded similarity turned into an outcome and weight: at `1` it agrees, below `floor` (or at `0`)
/// it disagrees, and in between it is partial, weighted along the way from `floor` to `1`.
fn grade(similarity: f64, floor: f64, weights: Weights) -> (Outcome, f64) {
    if similarity >= 1.0 {
        (Outcome::Agree, weights.agree)
    } else if similarity < floor || similarity <= 0.0 {
        (Outcome::Disagree, weights.disagree)
    } else {
        (
            Outcome::Partial(similarity),
            weights.at((similarity - floor) / (1.0 - floor)),
        )
    }
}

/// A feature neither side can be compared on.
fn missing(feature: Feature, left: Option<FeatureValue>, right: Option<FeatureValue>) -> FeatureComparison {
    FeatureComparison {
        feature,
        outcome: Outcome::Missing,
        weight: 0.0,
        left,
        right,
    }
}

/// The best-matching pair among `left` × `right` values, by `similarity`.
fn best_pair<'v>(
    left: &[&'v str],
    right: &[&'v str],
    similarity: impl Fn(&str, &str) -> Option<f64>,
) -> Option<(f64, &'v str, &'v str)> {
    let mut best: Option<(f64, &str, &str)> = None;
    for x in left {
        for y in right {
            let Some(s) = similarity(x, y) else {
                continue;
            };
            if best.is_none_or(|(so_far, _, _)| s > so_far) {
                best = Some((s, x, y));
            }
        }
    }
    best
}

/// Compares every given name of `a` with every given name of `b`.
fn compare_given(a: &PersonProfile, b: &PersonProfile, applied: &Applied<'_>) -> FeatureComparison {
    let given = |p: &PersonProfile| -> Vec<String> { p.names.iter().filter_map(|n| n.given.clone()).collect() };
    let (left, right) = (given(a), given(b));
    let (left, right): (Vec<&str>, Vec<&str>) = (
        left.iter().map(String::as_str).collect(),
        right.iter().map(String::as_str).collect(),
    );
    let Some((similarity, best_left, best_right)) = best_pair(&left, &right, |x, y| applied.given_similarity(x, y))
    else {
        return missing(Feature::GivenName, first_name(&left), first_name(&right));
    };
    let (outcome, weight) = grade(similarity, weights::NAME_FLOOR, weights::GIVEN_NAME);
    let (left, right) = (
        Some(FeatureValue::Name(best_left.to_owned())),
        Some(FeatureValue::Name(best_right.to_owned())),
    );
    FeatureComparison {
        feature: Feature::GivenName,
        outcome,
        weight,
        left,
        right,
    }
}

/// Compares every surname of `a` with every surname of `b`. Where a culture's surnames are patronymic
/// or residence names, a mismatch is missing evidence rather than a disagreement.
fn compare_surname(a: &PersonProfile, b: &PersonProfile, applied: &Applied<'_>) -> FeatureComparison {
    let surnames = |p: &PersonProfile| -> Vec<String> {
        p.names
            .iter()
            .flat_map(|n| n.surnames.iter().map(|s| s.surname.clone()))
            .collect()
    };
    let (left, right) = (surnames(a), surnames(b));
    let (left, right): (Vec<&str>, Vec<&str>) = (
        left.iter().map(String::as_str).collect(),
        right.iter().map(String::as_str).collect(),
    );
    let Some((similarity, best_left, best_right)) = best_pair(&left, &right, |x, y| applied.surname_similarity(x, y))
    else {
        return missing(Feature::Surname, first_name(&left), first_name(&right));
    };
    let weak = applied.surnames_are_weak();
    let table = if weak { weights::WEAK_SURNAME } else { weights::SURNAME };
    let (mut outcome, mut weight) = grade(similarity, weights::NAME_FLOOR, table);
    if weak && outcome == Outcome::Disagree {
        (outcome, weight) = (Outcome::Missing, 0.0);
    }
    let (left, right) = (
        Some(FeatureValue::Name(best_left.to_owned())),
        Some(FeatureValue::Name(best_right.to_owned())),
    );
    FeatureComparison {
        feature: Feature::Surname,
        outcome,
        weight,
        left,
        right,
    }
}

/// The first of some names, shown beside a feature that could not be compared.
fn first_name(names: &[&str]) -> Option<FeatureValue> {
    names.first().map(|name| FeatureValue::Name((*name).to_owned()))
}

/// Compares the asserted sexes: male against female is a conflict; anything unknown is missing.
fn compare_sex(a: Option<&Sex>, b: Option<&Sex>) -> FeatureComparison {
    let (left, right) = (a.cloned().map(FeatureValue::Sex), b.cloned().map(FeatureValue::Sex));
    let (outcome, weight) = match (asserted(a), asserted(b)) {
        (Some(x), Some(y)) if x == y => (Outcome::Agree, weights::SEX_AGREE),
        (Some(_), Some(_)) => (Outcome::Conflict, weights::CONFLICT),
        (None, _) | (_, None) => (Outcome::Missing, 0.0),
    };
    FeatureComparison {
        feature: Feature::Sex,
        outcome,
        weight,
        left,
        right,
    }
}

/// A male or female sex as a comparable value; any other sex is not compared.
fn asserted(sex: Option<&Sex>) -> Option<Sex> {
    match sex {
        Some(Sex::Male) => Some(Sex::Male),
        Some(Sex::Female) => Some(Sex::Female),
        Some(Sex::Unknown | Sex::Intersex | Sex::Other(_)) | None => None,
    }
}

/// A side's estimate of a birth or death: the event itself when dated, else its stand-in (baptism for
/// birth, burial for death), whose interval reaches back by the typical offset.
#[derive(Debug, Clone)]
struct Estimate<'p> {
    vital: &'p VitalEvent,
    date: &'p crate::date::GenealogicalDate,
    interval: DayInterval,
    tolerance: Tolerance,
}

/// How far a baptism may follow the birth it stands in for.
const BAPTISM_OFFSET_DAYS: i32 = 365;

/// How far a burial may follow the death it stands in for.
const BURIAL_OFFSET_DAYS: i32 = 30;

/// The estimate of `kind` (birth or death) for `profile`.
fn estimate(profile: &PersonProfile, kind: VitalKind) -> Option<Estimate<'_>> {
    let (proxy, offset) = match kind {
        VitalKind::Birth | VitalKind::Baptism => (VitalKind::Baptism, BAPTISM_OFFSET_DAYS),
        VitalKind::Death | VitalKind::Burial => (VitalKind::Burial, BURIAL_OFFSET_DAYS),
    };
    let dated = |wanted: VitalKind| {
        profile
            .vitals
            .iter()
            .filter(move |v| v.kind == wanted)
            .find_map(|vital| {
                let date = vital.date.as_ref()?;
                Some(Estimate {
                    vital,
                    date,
                    interval: interval(date)?,
                    tolerance: Tolerance::of(date, vital.basis),
                })
            })
    };
    dated(kind).or_else(|| {
        dated(proxy).map(|estimate| Estimate {
            interval: estimate.interval.widened(offset, 0),
            ..estimate
        })
    })
}

/// Compares two date estimates on the decay curve of the more lenient tolerance.
fn compare_dates(
    feature: Feature,
    a: Option<&Estimate<'_>>,
    b: Option<&Estimate<'_>>,
    weights: Weights,
) -> FeatureComparison {
    let value = |e: &Estimate<'_>| FeatureValue::Date {
        kind: e.vital.kind,
        date: e.date.clone(),
    };
    let (left, right) = (a.map(value), b.map(value));
    let (Some(a), Some(b)) = (a, b) else {
        return missing(feature, left, right);
    };
    let mut s = similarity(a.interval.gap(b.interval), a.tolerance.max(b.tolerance));
    if a.vital.kind == b.vital.kind && is_clerical_slip(a.date, b.date) {
        s = s.max(SLIP_SIMILARITY);
    }
    let (outcome, weight) = grade(s, 0.0, weights);
    FeatureComparison {
        feature,
        outcome,
        weight,
        left,
        right,
    }
}

/// A conflict when `death` is too far before `birth` for one individual, allowing each date's slack.
fn lifespan_conflict(death: Option<&Estimate<'_>>, birth: Option<&Estimate<'_>>) -> Option<FeatureComparison> {
    let (death, birth) = (death?, birth?);
    let slack = death.tolerance.max(birth.tolerance).slack_days();
    if birth.interval.lo - death.interval.hi <= slack {
        return None;
    }
    Some(FeatureComparison {
        feature: Feature::Lifespan,
        outcome: Outcome::Conflict,
        weight: weights::CONFLICT,
        left: Some(FeatureValue::Date {
            kind: death.vital.kind,
            date: death.date.clone(),
        }),
        right: Some(FeatureValue::Date {
            kind: birth.vital.kind,
            date: birth.date.clone(),
        }),
    })
}

/// The place of a side's birth or death, falling back to its stand-in's.
fn place_of(profile: &PersonProfile, kind: VitalKind) -> Option<&PlaceProfile> {
    let proxy = match kind {
        VitalKind::Birth | VitalKind::Baptism => VitalKind::Baptism,
        VitalKind::Death | VitalKind::Burial => VitalKind::Burial,
    };
    let placed = |wanted: VitalKind| {
        profile
            .vitals
            .iter()
            .filter(move |v| v.kind == wanted)
            .find_map(|v| v.place.as_ref())
    };
    placed(kind).or_else(|| placed(proxy))
}

/// Compares two places.
fn compare_places(
    feature: Feature,
    a: Option<&PlaceProfile>,
    b: Option<&PlaceProfile>,
    applied: &Applied<'_>,
) -> FeatureComparison {
    let value = |p: &PlaceProfile| p.names.first().map(|name| FeatureValue::Place(name.text.clone()));
    let (left, right) = (a.and_then(value), b.and_then(value));
    let Some(graded) = a.zip(b).and_then(|(a, b)| place::compare(a, b, applied)) else {
        return missing(feature, left, right);
    };
    let (outcome, weight) = grade(graded.similarity, graded.floor, weights::PLACE);
    FeatureComparison {
        feature,
        outcome,
        weight,
        left,
        right,
    }
}

#[cfg(test)]
mod tests;
