//! Record matching's entry points (ADR 0038 §7, §8): [`find_similar`], [`assess`] and
//! [`similar_pairs`], over the `match_keys` blocking index.
//!
//! Every consumer — the duplicate check, import planning, pickers, *Find similar* — asks through these,
//! so one pair gets one score and one explanation everywhere. Candidates come from the index: only
//! records sharing a loose key with the target are scored ([`vitni_core::matching::keys`]).
//!
//! The index is kept current here, before each lookup. When it was built under other packs or keying
//! rules — or never, or before a projection rebuild — it is built from scratch. Otherwise only the
//! records committed to since they were last keyed are rekeyed, with the records whose profiles carry
//! theirs: an event's principals (their birth decade is its date), a person's events and families, a
//! place's events, a source's citations.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use vitni_core::matching::{
    BlockingKeys, MatchAssessment, MatchBand, MatchableKind, Probe, assess_citations, assess_events, assess_families,
    assess_media, assess_notes, assess_persons, assess_places, assess_repositories, assess_sources, assess_tags,
    prefix_end,
};
use vitni_db::{DirtyRecord, KeyedRecord};

use crate::dto::AggRef;
use crate::error::AppError;
use crate::matching::Matching;
use crate::profile::{Profile, Profiles};
use crate::workspace::Workspace;

/// A record similar to the target, with the engine's assessment of the pair.
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarRecord {
    /// The similar record.
    pub record: AggRef,
    /// The assessment of the target against it.
    pub assessment: MatchAssessment,
}

/// Two records of one kind the engine judged similar.
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarPair {
    /// The first record, the lower aggregate id.
    pub a: AggRef,
    /// The second record.
    pub b: AggRef,
    /// The assessment of the pair.
    pub assessment: MatchAssessment,
}

/// The records of `kind` the engine judges at least `min_band` similar to the record `target` (a human
/// id; a tag's id), most similar first, at most `limit` of them. The target itself is never among them.
///
/// # Errors
///
/// The kind's `NotFound` error if `target` does not exist, [`AppError::MatchData`] or
/// [`AppError::Config`] if the matching data or settings cannot be loaded, or [`AppError`] on a store
/// failure.
pub async fn find_similar(
    workspace: &Workspace,
    kind: MatchableKind,
    target: &str,
    min_band: MatchBand,
    limit: usize,
) -> Result<Vec<SimilarRecord>, AppError> {
    let matching = workspace.matching()?;
    let keys = BlockingKeys::new(&matching.data);
    let profiles = refreshed_profiles(workspace, &keys, kind).await?;
    let target_id = profiles
        .aggregate_id_of(kind, target)
        .ok_or_else(|| not_found(kind, target))?;
    let target_profile = profiles
        .profile(kind, &target_id)
        .ok_or_else(|| not_found(kind, target))?;
    let probe = Probe::of(&keys_of(&keys, &target_profile));
    let mut similar = Vec::new();
    for candidate in workspace.store().match_candidates(kind, &probe).await? {
        if candidate == target_id {
            continue;
        }
        let Some(profile) = profiles.profile(kind, &candidate) else {
            continue;
        };
        let Some(assessment) = assess_pair(&target_profile, &profile, matching) else {
            continue;
        };
        if assessment.band >= min_band {
            similar.push(SimilarRecord {
                record: agg_ref(&profiles, kind, &candidate),
                assessment,
            });
        }
    }
    similar.sort_by(|x, y| rank(&x.assessment, &y.assessment).then_with(|| x.record.human_id.cmp(&y.record.human_id)));
    similar.truncate(limit);
    Ok(similar)
}

/// The engine's assessment of the records `a` and `b` of `kind` (human ids; a tag's id).
///
/// # Errors
///
/// The kind's `NotFound` error if either does not exist, [`AppError::MatchData`] or
/// [`AppError::Config`] if the matching data or settings cannot be loaded, or [`AppError`] on a store
/// failure.
pub async fn assess(workspace: &Workspace, kind: MatchableKind, a: &str, b: &str) -> Result<MatchAssessment, AppError> {
    let matching = workspace.matching()?;
    let profiles = Profiles::load(workspace.store(), &[kind]).await?;
    let profile_of = |human_id: &str| {
        profiles
            .aggregate_id_of(kind, human_id)
            .and_then(|id| profiles.profile(kind, &id))
            .ok_or_else(|| not_found(kind, human_id))
    };
    let (left, right) = (profile_of(a)?, profile_of(b)?);
    assess_pair(&left, &right, matching).ok_or_else(|| not_found(kind, b))
}

/// Every pair of records of `kind` the engine judges at least `min_band` similar, each pair once, the
/// most similar first.
///
/// # Errors
///
/// [`AppError::MatchData`] or [`AppError::Config`] if the matching data or settings cannot be loaded,
/// or [`AppError`] on a store failure.
pub async fn similar_pairs(
    workspace: &Workspace,
    kind: MatchableKind,
    min_band: MatchBand,
) -> Result<Vec<SimilarPair>, AppError> {
    let matching = workspace.matching()?;
    let keys = BlockingKeys::new(&matching.data);
    let profiles = refreshed_profiles(workspace, &keys, kind).await?;
    let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_record: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (aggregate_id, key) in workspace.store().match_keys_of_kind(kind).await? {
        by_key.entry(key.clone()).or_default().push(aggregate_id.clone());
        by_record.entry(aggregate_id).or_default().push(key);
    }
    let mut built = BTreeMap::new();
    for id in by_record.keys() {
        if let Some(profile) = profiles.profile(kind, id) {
            built.insert(id.clone(), profile);
        }
    }
    let records: Vec<(&String, &Vec<String>)> = by_record.iter().collect();
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let chunk = records.len().div_ceil(threads).max(1);
    let scored: Vec<Vec<(String, String, MatchAssessment)>> = std::thread::scope(|scope| {
        let workers: Vec<_> = records
            .chunks(chunk)
            .map(|part| scope.spawn(|| pairs_from(part, &by_key, &built, matching, min_band)))
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
            .collect()
    });
    let mut pairs = Vec::new();
    for (a, b, assessment) in scored.into_iter().flatten() {
        let (a, b) = (agg_ref(&profiles, kind, &a), agg_ref(&profiles, kind, &b));
        pairs.push(SimilarPair { a, b, assessment });
    }
    pairs.sort_by(|x, y| rank(&x.assessment, &y.assessment).then_with(|| (&x.a.id, &x.b.id).cmp(&(&y.a.id, &y.b.id))));
    Ok(pairs)
}

/// The pairs each record of `records` forms, at least `min_band` similar, with the candidates its keys
/// meet that sort after it — so every pair is scored once across all the parts.
fn pairs_from(
    records: &[(&String, &Vec<String>)],
    by_key: &BTreeMap<String, Vec<String>>,
    built: &BTreeMap<String, Profile>,
    matching: &Matching,
    min_band: MatchBand,
) -> Vec<(String, String, MatchAssessment)> {
    let mut pairs = Vec::new();
    for (id, record_keys) in records {
        let Some(profile) = built.get(*id) else {
            continue;
        };
        for other in candidates(&Probe::of(record_keys), by_key) {
            if other.as_str() <= id.as_str() {
                continue;
            }
            let Some(assessment) = built.get(&other).and_then(|o| assess_pair(profile, o, matching)) else {
                continue;
            };
            if assessment.band >= min_band {
                pairs.push(((*id).clone(), other, assessment));
            }
        }
    }
    pairs
}

/// The records in `by_key` holding a key `probe` meets.
fn candidates(probe: &Probe, by_key: &BTreeMap<String, Vec<String>>) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for key in &probe.exact {
        found.extend(by_key.get(key).into_iter().flatten().cloned());
    }
    for prefix in &probe.prefixes {
        let end = prefix_end(prefix);
        for (_, ids) in by_key.range::<str, _>((
            std::ops::Bound::Included(prefix.as_str()),
            std::ops::Bound::Excluded(end.as_str()),
        )) {
            found.extend(ids.iter().cloned());
        }
    }
    found
}

/// The more similar of two assessments first: by band, then by score.
fn rank(x: &MatchAssessment, y: &MatchAssessment) -> std::cmp::Ordering {
    y.band.cmp(&x.band).then_with(|| y.score.total_cmp(&x.score))
}

/// Brings the index up to date, and returns the profiles of `kind` — read once for both.
async fn refreshed_profiles(
    workspace: &Workspace,
    keys: &BlockingKeys<'_>,
    kind: MatchableKind,
) -> Result<Profiles, AppError> {
    let store = workspace.store();
    let dirty = store.match_dirty().await?;
    if store.match_keys_fingerprint().await?.as_deref() != Some(keys.fingerprint()) {
        let profiles = Profiles::load(store, &MatchableKind::ALL).await?;
        let mut records = Vec::new();
        for each in MatchableKind::ALL {
            for id in profiles.ids(each) {
                records.push(keyed(&profiles, keys, each, id));
            }
        }
        store.reset_match_keys(keys.fingerprint(), &records, &dirty).await?;
        return Ok(profiles);
    }
    let mut kinds = vec![kind];
    for record in &dirty {
        kinds.extend(affected_kinds(record.kind));
    }
    kinds.sort_unstable();
    kinds.dedup();
    let profiles = Profiles::load(store, &kinds).await?;
    if !dirty.is_empty() {
        let mut records = Vec::new();
        for (each, id) in affected(&profiles, &dirty) {
            records.push(keyed(&profiles, keys, each, id));
        }
        store.rekey_matches(&records, &dirty).await?;
    }
    Ok(profiles)
}

/// The kinds whose profiles a commit to a record of `kind` can change.
fn affected_kinds(kind: MatchableKind) -> Vec<MatchableKind> {
    use MatchableKind::{Citation, Event, Family, Media, Note, Person, Place, Repository, Source, Tag};
    match kind {
        Person | Family | Event | Place => vec![Person, Family, Event, Place],
        Source => vec![Source, Citation],
        Repository | Citation | Media | Note | Tag => vec![kind],
    }
}

/// The dirty records, and every record whose keys carry one of theirs.
fn affected(profiles: &Profiles, dirty: &[DirtyRecord]) -> BTreeSet<(MatchableKind, String)> {
    let of_kind = |kind: MatchableKind| -> HashSet<String> {
        dirty
            .iter()
            .filter(|record| record.kind == kind)
            .map(|record| record.aggregate_id.clone())
            .collect()
    };
    let mut out: BTreeSet<(MatchableKind, String)> = dirty
        .iter()
        .map(|record| (record.kind, record.aggregate_id.clone()))
        .collect();
    let mut events = of_kind(MatchableKind::Event);
    events.extend(profiles.events_at(&of_kind(MatchableKind::Place)));
    let mut persons = of_kind(MatchableKind::Person);
    persons.extend(profiles.participants_of(&events));
    let (person_events, families) = profiles.dependents_of_persons(&persons);
    events.extend(person_events);
    out.extend(persons.into_iter().map(|id| (MatchableKind::Person, id)));
    out.extend(events.into_iter().map(|id| (MatchableKind::Event, id)));
    out.extend(families.into_iter().map(|id| (MatchableKind::Family, id)));
    let citations = profiles.citations_of(&of_kind(MatchableKind::Source));
    out.extend(citations.into_iter().map(|id| (MatchableKind::Citation, id)));
    out
}

/// The index row of the record `aggregate_id`; no keys when its profile cannot be built.
fn keyed(profiles: &Profiles, keys: &BlockingKeys<'_>, kind: MatchableKind, aggregate_id: String) -> KeyedRecord {
    let record_keys = profiles
        .profile(kind, &aggregate_id)
        .map(|profile| keys_of(keys, &profile))
        .unwrap_or_default();
    KeyedRecord {
        kind,
        aggregate_id,
        keys: record_keys,
    }
}

/// A profile's blocking keys.
fn keys_of(keys: &BlockingKeys<'_>, profile: &Profile) -> Vec<String> {
    match profile {
        Profile::Person(p) => keys.person(p),
        Profile::Family(p) => keys.family(p),
        Profile::Event(p) => keys.event(p),
        Profile::Place(p) => keys.place(p),
        Profile::Source(p) => keys.source(p),
        Profile::Repository(p) => keys.repository(p),
        Profile::Citation(p) => keys.citation(p),
        Profile::Media(p) => keys.media(p),
        Profile::Note(p) => keys.note(p),
        Profile::Tag(p) => keys.tag(p),
    }
}

/// The assessment of two profiles, or `None` when they are of different kinds.
fn assess_pair(a: &Profile, b: &Profile, matching: &Matching) -> Option<MatchAssessment> {
    let (data, settings) = (&matching.data, &matching.settings);
    let assessment = match a {
        Profile::Person(a) => {
            let Profile::Person(b) = b else { return None };
            assess_persons(a, b, data, settings)
        }
        Profile::Family(a) => {
            let Profile::Family(b) = b else { return None };
            assess_families(a, b, data, settings)
        }
        Profile::Event(a) => {
            let Profile::Event(b) = b else { return None };
            assess_events(a, b, data, settings)
        }
        Profile::Place(a) => {
            let Profile::Place(b) = b else { return None };
            assess_places(a, b, data, settings)
        }
        Profile::Source(a) => {
            let Profile::Source(b) = b else { return None };
            assess_sources(a, b, data, settings)
        }
        Profile::Repository(a) => {
            let Profile::Repository(b) = b else { return None };
            assess_repositories(a, b, data, settings)
        }
        Profile::Citation(a) => {
            let Profile::Citation(b) = b else { return None };
            assess_citations(a, b, data, settings)
        }
        Profile::Media(a) => {
            let Profile::Media(b) = b else { return None };
            assess_media(a, b, settings)
        }
        Profile::Note(a) => {
            let Profile::Note(b) = b else { return None };
            assess_notes(a, b, data, settings)
        }
        Profile::Tag(a) => {
            let Profile::Tag(b) = b else { return None };
            assess_tags(a, b, settings)
        }
    };
    Some(assessment)
}

/// A record as a result names it: its human id (a tag's name) and aggregate id.
fn agg_ref(profiles: &Profiles, kind: MatchableKind, aggregate_id: &str) -> AggRef {
    AggRef {
        human_id: profiles.human_id(kind, aggregate_id).unwrap_or_default(),
        id: aggregate_id.to_owned(),
    }
}

/// The kind's not-found error for `id`.
fn not_found(kind: MatchableKind, id: &str) -> AppError {
    let id = id.to_owned();
    match kind {
        MatchableKind::Person => AppError::PersonNotFound(id),
        MatchableKind::Family => AppError::FamilyNotFound(id),
        MatchableKind::Event => AppError::EventNotFound(id),
        MatchableKind::Place => AppError::PlaceNotFound(id),
        MatchableKind::Source => AppError::SourceNotFound(id),
        MatchableKind::Repository => AppError::RepositoryNotFound(id),
        MatchableKind::Citation => AppError::CitationNotFound(id),
        MatchableKind::Media => AppError::MediaNotFound(id),
        MatchableKind::Note => AppError::NoteNotFound(id),
        MatchableKind::Tag => AppError::TagNotFound(id),
    }
}
