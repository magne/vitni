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
//! place's events, a source's citations — found through the record links index (ADR 0047).
//!
//! A lookup — [`find_similar`], the draft hint, [`assess`] — reads only its target and the candidates
//! the index yields. [`similar_pairs`] and an import's matcher score every record, so they read their
//! kinds whole.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use vitni_core::enums::PlaceType;
use vitni_core::matching::profile::{
    PersonProfile, PlaceProfile, RepositoryProfile, SourceProfile, VitalEvent, VitalKind,
};
use vitni_core::matching::{
    BlockingKeys, DateBasis, MatchAssessment, MatchBand, MatchableKind, Probe, assess_citations, assess_events,
    assess_families, assess_media, assess_notes, assess_persons, assess_places, assess_repositories, assess_sources,
    assess_tags, prefix_end,
};
use vitni_db::{DirtyRecord, KeyedRecord, RecordLink, Store};

use crate::dto::AggRef;
use crate::error::AppError;
use crate::event::{DateParts, gregorian_date};
use crate::matching::Matching;
use crate::person::{PersonNameParts, build_name};
use crate::profile::{Decisions, Profile, Profiles, aggregate_id_of, linking, place_name};
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
/// id; a tag's id), most similar first, at most `limit` of them. A merged target is matched as its
/// cluster's root. The target itself is never among them, nor a merged record, nor a record the user
/// already decided about against it, either way (ADR 0039 §3, §4).
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
    let store = workspace.store();
    let mut matcher = Matcher::by_record(workspace).await?;
    let found = aggregate_id_of(store, kind, target)
        .await?
        .ok_or_else(|| not_found(kind, target))?;
    matcher
        .profiles
        .include(store, kind, std::slice::from_ref(&found))
        .await?;
    let target_id = matcher.profiles.decisions(kind).root(&found);
    let profile = matcher
        .profiles
        .profile(kind, &target_id)
        .ok_or_else(|| not_found(kind, target))?;
    let lookup = Lookup {
        kind,
        profile: &profile,
        min_band,
        limit,
    };
    matcher
        .similar_by_record(workspace, &lookup, |decisions, candidate| {
            candidate == target_id || decisions.exclude(&target_id, candidate)
        })
        .await
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
    let store = workspace.store();
    let resolve = async |human_id: &str| {
        aggregate_id_of(store, kind, human_id)
            .await?
            .ok_or_else(|| not_found(kind, human_id))
    };
    let (left_id, right_id) = (resolve(a).await?, resolve(b).await?);
    let mut profiles = Profiles::default();
    profiles
        .include(store, kind, &[left_id.clone(), right_id.clone()])
        .await?;
    let left = profiles.profile(kind, &left_id).ok_or_else(|| not_found(kind, a))?;
    let right = profiles.profile(kind, &right_id).ok_or_else(|| not_found(kind, b))?;
    assess_pair(&left, &right, matching).ok_or_else(|| not_found(kind, b))
}

/// Every pair of records of `kind` the engine judges at least `min_band` similar, each pair once, the
/// most similar first. A pair the user already decided, either way, is left out (ADR 0039 §3).
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
    let profiles = refreshed_profiles(workspace, &keys, &[kind]).await?;
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
    let decisions = profiles.decisions(kind);
    let mut pairs = Vec::new();
    for (a, b, assessment) in scored.into_iter().flatten() {
        if decisions.exclude(&a, &b) {
            continue;
        }
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

/// A record being entered by hand, before it exists (ADR 0038 §8): what the similar-record hint matches
/// against the stored records of its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftRecord {
    /// A person: the preferred name, and the birth date if one was typed.
    Person {
        /// The name as typed.
        name: PersonNameParts,
        /// The birth date, if typed.
        birth: Option<DateParts>,
    },
    /// A place: its name and type.
    Place {
        /// The name as typed.
        name: String,
        /// The place's type, if chosen.
        place_type: Option<PlaceType>,
    },
    /// A source: its title and author.
    Source {
        /// The title as typed.
        title: String,
        /// The author, if typed.
        author: Option<String>,
    },
    /// A repository: its name.
    Repository {
        /// The name as typed.
        name: String,
    },
}

impl DraftRecord {
    /// The kind of record the draft is.
    #[must_use]
    pub fn kind(&self) -> MatchableKind {
        match self {
            Self::Person { .. } => MatchableKind::Person,
            Self::Place { .. } => MatchableKind::Place,
            Self::Source { .. } => MatchableKind::Source,
            Self::Repository { .. } => MatchableKind::Repository,
        }
    }

    /// Whether nothing that identifies a record has been typed yet: such a draft is like no record.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let blank = |text: &Option<String>| text.as_deref().is_none_or(|text| text.trim().is_empty());
        match self {
            Self::Person { name, birth } => blank(&name.given) && blank(&name.surname) && birth.is_none(),
            Self::Place { name, .. } | Self::Repository { name } => name.trim().is_empty(),
            Self::Source { title, .. } => title.trim().is_empty(),
        }
    }

    fn profile(&self) -> Profile {
        let text = |text: &str| Some(text.trim().to_owned()).filter(|text| !text.is_empty());
        match self {
            Self::Person { name, birth } => Profile::Person(PersonProfile {
                names: vec![build_name(name.clone())],
                vitals: birth
                    .map(|birth| VitalEvent {
                        kind: VitalKind::Birth,
                        date: Some(gregorian_date(birth)),
                        basis: DateBasis::Recorded,
                        place: None,
                    })
                    .into_iter()
                    .collect(),
                ..PersonProfile::default()
            }),
            Self::Place { name, place_type } => Profile::Place(PlaceProfile {
                names: vec![place_name(name.trim())],
                place_type: place_type.clone(),
                ..PlaceProfile::default()
            }),
            Self::Source { title, author } => Profile::Source(SourceProfile {
                title: text(title),
                author: author.as_deref().and_then(text),
                ..SourceProfile::default()
            }),
            Self::Repository { name } => Profile::Repository(RepositoryProfile {
                name: text(name),
                ..RepositoryProfile::default()
            }),
        }
    }
}

/// The stored records of the draft's kind the engine judges at least `min_band` similar to `draft`,
/// most similar first, at most `limit` of them; none for an [empty](DraftRecord::is_empty) draft. A
/// merged record is never among them — its cluster's root stands for it.
///
/// # Errors
///
/// [`AppError::MatchData`] or [`AppError::Config`] if the matching data or settings cannot be loaded,
/// or [`AppError`] on a store failure.
pub async fn find_similar_to_draft(
    workspace: &Workspace,
    draft: &DraftRecord,
    min_band: MatchBand,
    limit: usize,
) -> Result<Vec<SimilarRecord>, AppError> {
    if draft.is_empty() {
        return Ok(Vec::new());
    }
    let profile = draft.profile();
    let lookup = Lookup {
        kind: draft.kind(),
        profile: &profile,
        min_band,
        limit,
    };
    let mut matcher = Matcher::by_record(workspace).await?;
    matcher
        .similar_by_record(workspace, &lookup, |decisions, candidate| {
            decisions.root(candidate) != candidate
        })
        .await
}

/// The engine's assessment of `draft` against the stored record `human_id` of its kind — the same
/// score [`find_similar_to_draft`] gives the pair.
///
/// # Errors
///
/// The kind's `NotFound` error if `human_id` does not exist, [`AppError::MatchData`] or
/// [`AppError::Config`] if the matching data or settings cannot be loaded, or [`AppError`] on a store
/// failure.
pub async fn assess_draft(
    workspace: &Workspace,
    draft: &DraftRecord,
    human_id: &str,
) -> Result<MatchAssessment, AppError> {
    let matching = workspace.matching()?;
    let store = workspace.store();
    let kind = draft.kind();
    let id = aggregate_id_of(store, kind, human_id)
        .await?
        .ok_or_else(|| not_found(kind, human_id))?;
    let mut profiles = Profiles::default();
    profiles.include(store, kind, std::slice::from_ref(&id)).await?;
    let stored = profiles.profile(kind, &id).ok_or_else(|| not_found(kind, human_id))?;
    assess_pair(&draft.profile(), &stored, matching).ok_or_else(|| not_found(kind, human_id))
}

/// The more similar of two assessments first: by band, then by score.
pub(crate) fn rank(x: &MatchAssessment, y: &MatchAssessment) -> std::cmp::Ordering {
    y.band.cmp(&x.band).then_with(|| y.score.total_cmp(&x.score))
}

/// What one lookup asks for: the records of `kind` at least `min_band` similar to `profile`, at most
/// `limit` of them.
pub(crate) struct Lookup<'p> {
    kind: MatchableKind,
    profile: &'p Profile,
    min_band: MatchBand,
    limit: usize,
}

/// Matches a profile against the workspace's records over the index, brought up to date when the
/// matcher is made.
pub(crate) struct Matcher<'a> {
    matching: &'a Matching,
    keys: BlockingKeys<'a>,
    /// The profiles read: of the kinds the matcher was loaded for, or of the records looked up so far.
    pub(crate) profiles: Profiles,
}

impl<'a> Matcher<'a> {
    /// A matcher over `workspace`'s records of `kinds`, read whole — for matching many records that are
    /// not yet in the workspace, an import's staged entities (ADR 0040 §2).
    pub(crate) async fn load(workspace: &'a Workspace, kinds: &[MatchableKind]) -> Result<Self, AppError> {
        let matching = workspace.matching()?;
        let keys = BlockingKeys::new(&matching.data);
        let profiles = refreshed_profiles(workspace, &keys, kinds).await?;
        Ok(Self {
            matching,
            keys,
            profiles,
        })
    }

    /// A matcher that reads only the records a lookup scores — for one lookup at a time.
    async fn by_record(workspace: &'a Workspace) -> Result<Self, AppError> {
        Self::load(workspace, &[]).await
    }

    /// The records of `kind` at least `min_band` similar to `profile`, most similar first, at most
    /// `limit` of them, leaving out `excluded` (aggregate ids) and every merged record. The matcher
    /// must have been [loaded](Self::load) for `kind`.
    pub(crate) async fn similar(
        &self,
        workspace: &Workspace,
        kind: MatchableKind,
        profile: &Profile,
        excluded: &HashSet<String>,
        min_band: MatchBand,
        limit: usize,
    ) -> Result<Vec<SimilarRecord>, AppError> {
        let lookup = Lookup {
            kind,
            profile,
            min_band,
            limit,
        };
        let candidates = self.candidates(workspace, &lookup).await?;
        let decisions = self.profiles.decisions(kind);
        Ok(self.ranked(&lookup, candidates, |candidate| {
            excluded.contains(candidate) || decisions.root(candidate) != candidate
        }))
    }

    /// [`Self::similar`] for a matcher made [by record](Self::by_record): the candidates are read before
    /// they are scored, and `skip` names those the lookup leaves out under the identity decisions.
    async fn similar_by_record(
        &mut self,
        workspace: &Workspace,
        lookup: &Lookup<'_>,
        skip: impl Fn(&Decisions, &str) -> bool,
    ) -> Result<Vec<SimilarRecord>, AppError> {
        let candidates = self.candidates(workspace, lookup).await?;
        self.profiles
            .include(workspace.store(), lookup.kind, &candidates)
            .await?;
        let decisions = self.profiles.decisions(lookup.kind);
        Ok(self.ranked(lookup, candidates, |candidate| skip(&decisions, candidate)))
    }

    /// The records of the lookup's kind sharing a key with its profile.
    async fn candidates(&self, workspace: &Workspace, lookup: &Lookup<'_>) -> Result<Vec<String>, AppError> {
        let probe = Probe::of(&keys_of(&self.keys, lookup.profile));
        Ok(workspace.store().match_candidates(lookup.kind, &probe).await?)
    }

    /// The `candidates` not skipped that are at least the lookup's band similar to its profile, most
    /// similar first, at most its limit of them.
    fn ranked(&self, lookup: &Lookup<'_>, candidates: Vec<String>, skip: impl Fn(&str) -> bool) -> Vec<SimilarRecord> {
        let mut similar = Vec::new();
        for candidate in candidates {
            if skip(&candidate) {
                continue;
            }
            let Some(other) = self.profiles.profile(lookup.kind, &candidate) else {
                continue;
            };
            let Some(assessment) = assess_pair(lookup.profile, &other, self.matching) else {
                continue;
            };
            if assessment.band >= lookup.min_band {
                similar.push(SimilarRecord {
                    record: agg_ref(&self.profiles, lookup.kind, &candidate),
                    assessment,
                });
            }
        }
        similar
            .sort_by(|x, y| rank(&x.assessment, &y.assessment).then_with(|| x.record.human_id.cmp(&y.record.human_id)));
        similar.truncate(lookup.limit);
        similar
    }
}

/// Brings the index up to date, and returns the profiles of `whole` read whole, with those of the records
/// it rekeyed.
async fn refreshed_profiles(
    workspace: &Workspace,
    keys: &BlockingKeys<'_>,
    whole: &[MatchableKind],
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
    let mut profiles = Profiles::load(store, whole).await?;
    if !dirty.is_empty() {
        let mut records = Vec::new();
        for (each, id) in affected(store, &mut profiles, &dirty).await? {
            records.push(keyed(&profiles, keys, each, id));
        }
        store.rekey_matches(&records, &dirty).await?;
    }
    Ok(profiles)
}

/// The dirty records, and every record whose keys carry one of theirs, each read into `profiles`.
async fn affected(
    store: &Store,
    profiles: &mut Profiles,
    dirty: &[DirtyRecord],
) -> Result<BTreeSet<(MatchableKind, String)>, AppError> {
    let of_kind = |kind: MatchableKind| -> Vec<String> {
        dirty
            .iter()
            .filter(|record| record.kind == kind)
            .map(|record| record.aggregate_id.clone())
            .collect()
    };
    let mut events: BTreeSet<String> = of_kind(MatchableKind::Event).into_iter().collect();
    events.extend(linking(store, RecordLink::EventPlace, &of_kind(MatchableKind::Place)).await?);
    let mut persons: BTreeSet<String> = of_kind(MatchableKind::Person).into_iter().collect();
    persons.extend(linking(store, RecordLink::Participation, &Vec::from_iter(events.clone())).await?);
    let persons = Vec::from_iter(persons);
    profiles.include(store, MatchableKind::Person, &persons).await?;
    events.extend(profiles.events_of_persons(&persons));
    let families = linking(store, RecordLink::FamilyPartner, &persons).await?;
    let citations = linking(store, RecordLink::CitationSource, &of_kind(MatchableKind::Source)).await?;

    let mut out: BTreeSet<(MatchableKind, String)> = dirty
        .iter()
        .map(|record| (record.kind, record.aggregate_id.clone()))
        .collect();
    out.extend(persons.into_iter().map(|id| (MatchableKind::Person, id)));
    out.extend(events.into_iter().map(|id| (MatchableKind::Event, id)));
    out.extend(families.into_iter().map(|id| (MatchableKind::Family, id)));
    out.extend(citations.into_iter().map(|id| (MatchableKind::Citation, id)));
    for kind in MatchableKind::ALL {
        let ids: Vec<String> = out
            .iter()
            .filter(|(each, _)| *each == kind)
            .map(|(_, id)| id.clone())
            .collect();
        profiles.include(store, kind, &ids).await?;
    }
    Ok(out)
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
