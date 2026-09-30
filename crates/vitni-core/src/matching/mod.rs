//! Record matching: explainable Fellegi–Sunter scoring (ADR 0038).
//!
//! [`assess_persons`] compares two [`PersonProfile`]s and returns a [`MatchAssessment`];
//! [`assess_families`] and [`assess_events`] do the same for families and events. Each
//! comparator grades one feature — names, sex, birth and death dates, their places, occupations, and the
//! relatives: fathers, mothers, partners, children and a patronymic read against the candidate father —
//! as `Agree`, `Partial`, `Disagree`, `Missing` or `Conflict`, with a log-likelihood weight. The weights sum to the
//! match weight, which maps to a score in `0..1` and a [`MatchBand`]. Every term is kept, so the
//! features *are* the explanation.
//!
//! A family is compared by its partners — each pair scored as persons, summarised as one term, with the
//! pair's own assessment kept in [`MatchAssessment::parts`] — its children and its marriage. An event is
//! compared by its type, date and place, its principals pair by pair, and its other participants.
//!
//! The other kinds have one function each: [`assess_places`] (names, type, where the place lies —
//! a farm and its parish are partial — and coordinates), [`assess_sources`], [`assess_repositories`],
//! [`assess_citations`] (its source summarised as one term, like a family's partner, and its page),
//! [`assess_media`] (the checksum, exactly), [`assess_notes`] (the normalized text) and [`assess_tags`]
//! (the case-folded name, which alone makes a tag pair deterministic).
//!
//! Comparators are graded, never binary: a near miss lowers the score but keeps the candidate, only an
//! implausible distance disagrees, and only a logical impossibility (a different asserted sex, a death
//! before the other's birth, two different items of one source record) is a conflict, which caps the
//! score. [`MatchBand::Deterministic`] is never
//! earned by a score: only a shared record origin or external id establishes identity (§6).
//!
//! The name rules are data — [`pack`]s selected per comparison from places, dates, data languages and
//! lineage ([`select`]). The module is pure: it reads no files and no configuration. The app layer
//! reads any workspace packs and hands them in through [`MatchData::layered`].

pub mod date;
mod event;
mod family;
pub mod keys;
mod media;
mod name;
mod note;
pub mod pack;
mod place;
pub mod profile;
mod relative;
pub mod select;
mod source;
mod tag;
mod weights;

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use crate::enums::Sex;
use crate::matching::date::{DayInterval, SLIP_SIMILARITY, Tolerance, interval, is_clerical_slip, similarity};
use crate::matching::name::{Applied, Rules};
use crate::matching::pack::{CulturePack, CulturePacks, PackError, PackSource};
use crate::matching::profile::{PersonProfile, PlaceProfile, VitalEvent, VitalKind};
use crate::matching::relative::{compare_patronymic, compare_relatives};
use crate::matching::select::{RegionTable, Signals, comparison_cultures};
use crate::matching::weights::Weights;
use crate::origin::RecordOrigin;
use crate::text::ExternalId;

pub use crate::matching::date::DateBasis;
pub use crate::matching::event::assess_events;
pub use crate::matching::family::assess_families;
pub use crate::matching::keys::{BlockingKeys, MatchableKind, Probe, prefix_end};
pub use crate::matching::media::assess_media;
pub use crate::matching::note::assess_notes;
pub use crate::matching::place::assess_places;
pub use crate::matching::source::{assess_citations, assess_repositories, assess_sources};
pub use crate::matching::tag::assess_tags;

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
pub const ENGINE_VERSION: EngineVersion = EngineVersion(3);

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
    /// The fathers.
    Father,
    /// The mothers.
    Mother,
    /// The partners.
    Partners,
    /// The children.
    Children,
    /// One side's patronymic against the other side's father.
    Patronymic,
    /// The occupations.
    Occupation,
    /// Two different items of one source record, which are by construction different records.
    Record,
    /// A family's partner, scored by the pair's own person assessment.
    Partner,
    /// A family's marriage date.
    Marriage,
    /// A family's marriage place.
    MarriagePlace,
    /// An event's type.
    EventType,
    /// An event's date.
    Date,
    /// An event's place.
    Place,
    /// A principal of an event — its primary participant, a husband, wife, spouse, groom or bride.
    Principal,
    /// An event's other participants: witnesses, godparents, parents, clergy.
    Participants,
    /// A place's names.
    PlaceName,
    /// A place's type.
    PlaceType,
    /// Where a place lies: the same place, one enclosing the other, or the same enclosing place.
    Enclosure,
    /// A place's coordinates.
    Coordinates,
    /// A source's title.
    Title,
    /// A source's author.
    Author,
    /// A source's publication information.
    Publication,
    /// The repositories holding a source.
    Repository,
    /// A repository's or a tag's name.
    Name,
    /// A repository's addresses.
    Address,
    /// A citation's source, scored by the pair's own source assessment.
    Source,
    /// A citation's page or locator.
    Page,
    /// A media object's checksum.
    Checksum,
    /// A media object's file name.
    Path,
    /// A note's text.
    Text,
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
    /// A date not of a vital event: a marriage's, or any event's.
    When(crate::date::GenealogicalDate),
    /// An event's type.
    EventType(crate::enums::EventType),
    /// A sex.
    Sex(Sex),
    /// A relative's name, with their birth date.
    Relative {
        /// The relative's name, as written.
        name: String,
        /// The relative's birth date, if known.
        born: Option<crate::date::GenealogicalDate>,
    },
    /// Free text, such as an occupation.
    Text(String),
    /// The source record an item was imported from.
    Origin(RecordOrigin),
    /// A place's type.
    PlaceType(crate::enums::PlaceType),
    /// A place's coordinates.
    Coordinates(crate::geo::GeoCoordinates),
    /// A repository's address.
    Address(crate::address::Address),
    /// A media object's location.
    Path(crate::media_path::MediaPath),
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
    /// The assessments a term summarises — a family's partner pairs, in the order of their
    /// [`Feature::Partner`] terms, or a citation's source pair behind its [`Feature::Source`] term;
    /// empty for every other kind.
    pub parts: Vec<MatchAssessment>,
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
    /// The packs' rules normalized per culture set, built on first use.
    rules: RulesCache,
}

impl MatchData {
    /// The data of `packs` and `regions`.
    #[must_use]
    pub fn new(packs: CulturePacks, regions: RegionTable) -> Self {
        Self {
            packs,
            regions,
            rules: RulesCache::default(),
        }
    }

    /// The packs and region table shipped with the engine.
    pub fn embedded() -> Result<Self, PackError> {
        Ok(Self::new(CulturePacks::embedded()?, RegionTable::embedded()?))
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
        Ok(Self::new(self.packs.layered(packs)?, regions))
    }
}

/// The normalized rules of each culture set a comparison has applied, so a set's packs are normalized
/// once rather than per comparison. A cache: it is empty when cloned and never tells two data apart.
#[derive(Default)]
struct RulesCache(RwLock<HashMap<Vec<CultureId>, Arc<Rules>>>);

impl RulesCache {
    /// The rules of `cultures`, built by `build` the first time they are asked for. Comparisons on
    /// several threads share the cache, and only a miss takes the write lock.
    fn get(&self, cultures: &[CultureId], build: impl FnOnce() -> Rules) -> Arc<Rules> {
        if let Some(rules) = self.0.read().unwrap_or_else(PoisonError::into_inner).get(cultures) {
            return Arc::clone(rules);
        }
        let mut cache = self.0.write().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(cache.entry(cultures.to_vec()).or_insert_with(|| Arc::new(build())))
    }
}

impl Clone for RulesCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for RulesCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for RulesCache {}

impl std::fmt::Debug for RulesCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RulesCache")
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
    let cultures = comparison_cultures(&Signals::person(a), &Signals::person(b), data, settings);
    let applied = applied(&cultures, data);
    person_assessment(a, b, cultures, &applied, settings)
}

/// The packs of `cultures`, ready to apply, their rules normalized once per culture set.
fn applied<'d>(cultures: &[CultureId], data: &'d MatchData) -> Applied<'d> {
    let packs: Vec<&CulturePack> = cultures.iter().filter_map(|id| data.packs.get(id)).collect();
    let rules = data.rules.get(cultures, || Rules::of(&packs));
    Applied::with_rules(packs, rules)
}

/// Compares two person profiles under cultures already chosen — a person's own, or a family's.
fn person_assessment(
    a: &PersonProfile,
    b: &PersonProfile,
    cultures: Vec<CultureId>,
    applied: &Applied<'_>,
    settings: &MatchSettings,
) -> MatchAssessment {
    let (birth_a, birth_b) = (
        estimate(&a.vitals, VitalKind::Birth),
        estimate(&b.vitals, VitalKind::Birth),
    );
    let (death_a, death_b) = (
        estimate(&a.vitals, VitalKind::Death),
        estimate(&b.vitals, VitalKind::Death),
    );
    let surname = compare_surname(a, b, applied);
    let patronymic = match surname.outcome {
        Outcome::Agree | Outcome::Partial(_) => missing(Feature::Patronymic, None, None),
        Outcome::Disagree | Outcome::Missing | Outcome::Conflict => compare_patronymic(a, b, applied),
    };
    let mut features = vec![
        compare_given(a, b, applied),
        surname,
        compare_sex(a.sex.as_ref(), b.sex.as_ref()),
        compare_dates(Feature::Birth, birth_a.as_ref(), birth_b.as_ref(), weights::BIRTH),
        compare_dates(Feature::Death, death_a.as_ref(), death_b.as_ref(), weights::DEATH),
        compare_places(
            Feature::BirthPlace,
            place_of(a, VitalKind::Birth),
            place_of(b, VitalKind::Birth),
            applied,
        ),
        compare_places(
            Feature::DeathPlace,
            place_of(a, VitalKind::Death),
            place_of(b, VitalKind::Death),
            applied,
        ),
    ];
    features.extend(compare_relatives(a, b, applied));
    features.push(patronymic);
    features.push(compare_occupations(a, b, applied));
    features.extend(lifespan_conflict(death_a.as_ref(), birth_b.as_ref()));
    features.extend(lifespan_conflict(death_b.as_ref(), birth_a.as_ref()));
    let sides = (
        Identity::of(&a.origins, &a.external_ids),
        Identity::of(&b.origins, &b.external_ids),
    );
    conclude(features, cultures, Vec::new(), sides, settings)
}

/// Sums the features into a score and band. Identity established between the two `sides` makes the band
/// deterministic; otherwise two items of one record add a conflict.
fn conclude(
    mut features: Vec<FeatureComparison>,
    cultures: Vec<CultureId>,
    parts: Vec<MatchAssessment>,
    (left, right): (Identity<'_>, Identity<'_>),
    settings: &MatchSettings,
) -> MatchAssessment {
    let identified = left.shared_with(right);
    if !identified {
        features.extend(left.same_record_conflict(right));
    }
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
        parts,
        engine: ENGINE_VERSION,
    }
}

/// The term summarising the assessment of a pair a record is compared through — a family's partners, a
/// citation's sources: a conflict if the pair has one, agreement when it is probable (or established),
/// disagreement when unlikely, and partial in between. Its weight is the pair's summed feature weights,
/// its support capped at `cap`; a pair that is someone (or something) else is not capped.
fn summary_term(
    feature: Feature,
    part: &MatchAssessment,
    cap: f64,
    (left, right): (Option<FeatureValue>, Option<FeatureValue>),
) -> FeatureComparison {
    let mut weight = 0.0;
    let mut conflict = false;
    for term in &part.features {
        weight += term.weight;
        conflict |= term.outcome == Outcome::Conflict;
    }
    let outcome = if conflict {
        Outcome::Conflict
    } else {
        match part.band {
            MatchBand::Probable | MatchBand::Deterministic => Outcome::Agree,
            MatchBand::Possible => Outcome::Partial(part.score),
            MatchBand::Unlikely => Outcome::Disagree,
        }
    };
    FeatureComparison {
        feature,
        outcome,
        weight: weight.min(cap),
        left,
        right,
    }
}

/// What establishes a record's identity: the origins of the assertions that created it and its external
/// ids.
#[derive(Debug, Clone, Copy)]
struct Identity<'p> {
    origins: &'p [RecordOrigin],
    external_ids: &'p [ExternalId],
}

impl<'p> Identity<'p> {
    fn of(origins: &'p [RecordOrigin], external_ids: &'p [ExternalId]) -> Self {
        Self { origins, external_ids }
    }

    /// Whether the two records share a record origin or an external id — identity already established.
    fn shared_with(self, other: Self) -> bool {
        let same_origin = self.origins.iter().any(|x| {
            other
                .origins
                .iter()
                .any(|y| x.dataset == y.dataset && x.record == y.record && x.item == y.item)
        });
        let same_external = self.external_ids.iter().any(|x| {
            other
                .external_ids
                .iter()
                .any(|y| x.authority == y.authority && x.value == y.value)
        });
        same_origin || same_external
    }

    /// A conflict when the two records were created from different items of one source record — two
    /// members of one household, say — which are by construction different records (ADR 0037 §1).
    fn same_record_conflict(self, other: Self) -> Option<FeatureComparison> {
        for x in self.origins {
            for y in other.origins {
                if x.dataset == y.dataset && x.record == y.record && x.item != y.item {
                    return Some(FeatureComparison {
                        feature: Feature::Record,
                        outcome: Outcome::Conflict,
                        weight: weights::CONFLICT,
                        left: Some(FeatureValue::Origin(x.clone())),
                        right: Some(FeatureValue::Origin(y.clone())),
                    });
                }
            }
        }
        None
    }
}

/// Compares the occupations: one in common, after normalization, is weak support; a different one is
/// no evidence, because occupations change over a life.
fn compare_occupations(a: &PersonProfile, b: &PersonProfile, applied: &Applied<'_>) -> FeatureComparison {
    let text = |occupation: &String| FeatureValue::Text(occupation.clone());
    for x in &a.occupations {
        let normalized = applied.tokens(x);
        if normalized.is_empty() {
            continue;
        }
        if let Some(y) = b.occupations.iter().find(|y| applied.tokens(y) == normalized) {
            return FeatureComparison {
                feature: Feature::Occupation,
                outcome: Outcome::Agree,
                weight: weights::OCCUPATION_AGREE,
                left: Some(text(x)),
                right: Some(text(y)),
            };
        }
    }
    missing(
        Feature::Occupation,
        a.occupations.first().map(text),
        b.occupations.first().map(text),
    )
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

/// Pairs left items with right items, most similar first, each item at most once. A candidate is
/// `(similarity, left index, right index)`; the pairs come back in the order they were chosen.
fn pair_up(mut candidates: Vec<(f64, usize, usize)>) -> Vec<(f64, usize, usize)> {
    candidates.sort_by(|x, y| y.0.total_cmp(&x.0));
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let mut pairs = Vec::new();
    for (similarity, i, j) in candidates {
        if left.contains(&i) || right.contains(&j) {
            continue;
        }
        left.push(i);
        right.push(j);
        pairs.push((similarity, i, j));
    }
    pairs
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

/// Whether two records of these sexes can be one person: not when their asserted sexes differ.
fn may_be_one(x: Option<&Sex>, y: Option<&Sex>) -> bool {
    match (asserted(x), asserted(y)) {
        (Some(s), Some(t)) => s == t,
        (None, _) | (_, None) => true,
    }
}

/// A side's estimate of a date: of a birth or death, the event itself when dated, else its stand-in
/// (baptism for birth, burial for death), whose interval reaches back by the typical offset; of any other
/// event, its recorded date.
#[derive(Debug, Clone)]
struct Estimate<'p> {
    /// The vital event the date is of, or `None` for any other event's date.
    kind: Option<VitalKind>,
    date: &'p crate::date::GenealogicalDate,
    interval: DayInterval,
    tolerance: Tolerance,
}

/// How far a baptism may follow the birth it stands in for.
const BAPTISM_OFFSET_DAYS: i32 = 365;

/// How far a burial may follow the death it stands in for.
const BURIAL_OFFSET_DAYS: i32 = 30;

/// The estimate of `kind` (birth or death) among `vitals`.
fn estimate(vitals: &[VitalEvent], kind: VitalKind) -> Option<Estimate<'_>> {
    let (proxy, offset) = match kind {
        VitalKind::Birth | VitalKind::Baptism => (VitalKind::Baptism, BAPTISM_OFFSET_DAYS),
        VitalKind::Death | VitalKind::Burial => (VitalKind::Burial, BURIAL_OFFSET_DAYS),
    };
    let dated = |wanted: VitalKind| {
        vitals.iter().filter(move |v| v.kind == wanted).find_map(|vital| {
            let date = vital.date.as_ref()?;
            Some(Estimate {
                kind: Some(vital.kind),
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

impl Estimate<'_> {
    /// The date as shown beside a feature: with its vital kind, when it is of one.
    fn value(&self) -> FeatureValue {
        let date = self.date.clone();
        match self.kind {
            Some(kind) => FeatureValue::Date { kind, date },
            None => FeatureValue::When(date),
        }
    }
}

/// The estimate of an event's recorded date.
fn event_estimate(date: Option<&crate::date::GenealogicalDate>) -> Option<Estimate<'_>> {
    let date = date?;
    Some(Estimate {
        kind: None,
        date,
        interval: interval(date)?,
        tolerance: Tolerance::of(date, DateBasis::Recorded),
    })
}

/// Compares two date estimates on the decay curve of the more lenient tolerance.
fn compare_dates(
    feature: Feature,
    a: Option<&Estimate<'_>>,
    b: Option<&Estimate<'_>>,
    weights: Weights,
) -> FeatureComparison {
    let (left, right) = (a.map(Estimate::value), b.map(Estimate::value));
    let (Some(a), Some(b)) = (a, b) else {
        return missing(feature, left, right);
    };
    let mut s = similarity(a.interval.gap(b.interval), a.tolerance.max(b.tolerance));
    if a.kind == b.kind && is_clerical_slip(a.date, b.date) {
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
        left: Some(death.value()),
        right: Some(birth.value()),
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
