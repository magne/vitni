//! The possible-matches review queue (ADR 0039 §3): every pair the matching engine proposes across the
//! kinds a user can decide.
//!
//! A suggestion is a function of the current data, so the queue is read from the `match_pairs`
//! projection (ADR 0048), refreshed first and leaving out every pair decided either way, narrowed to
//! one kind, a minimum band, or the records one import run created. Every pair is counted; only the
//! strongest a caller lists are assessed again for the terms behind their scores. An import's *Decide
//! later* writes nothing but the records (ADR 0040 §4), so a run's deferred pairs are exactly the pairs
//! its records are in.
//! Deciding a pair ([`decide_match`]) is the kind's own merge or distinguish use-case, after which the
//! pair is gone from the queue.

use std::collections::HashSet;

use vitni_core::ids::ImportRunId;
use vitni_core::matching::{MatchAssessment, MatchBand, MatchableKind};
use vitni_db::MatchPair;

use crate::dto::AggRef;
use crate::error::AppError;
use crate::identity::{IdentityDecision, PairDecision};
use crate::session::Session;
use crate::similar::{assessed_pairs, refresh_pairs};
use crate::workspace::Workspace;

/// A kind of record a user can decide the identity of (ADR 0039 §1): every [`MatchableKind`] but
/// [`Tag`](MatchableKind::Tag), which resolves by its case-folded name and is never proposed as a pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DecidableKind {
    /// A person.
    Person,
    /// A family.
    Family,
    /// An event.
    Event,
    /// A place.
    Place,
    /// A source.
    Source,
    /// A repository.
    Repository,
    /// A citation.
    Citation,
    /// A media object.
    Media,
    /// A note.
    Note,
}

impl DecidableKind {
    /// Every decidable kind, in [`MatchableKind::ALL`] order.
    pub const ALL: [Self; 9] = [
        Self::Person,
        Self::Family,
        Self::Event,
        Self::Place,
        Self::Source,
        Self::Repository,
        Self::Citation,
        Self::Media,
        Self::Note,
    ];

    /// The matching engine's kind.
    #[must_use]
    pub fn matchable(self) -> MatchableKind {
        match self {
            Self::Person => MatchableKind::Person,
            Self::Family => MatchableKind::Family,
            Self::Event => MatchableKind::Event,
            Self::Place => MatchableKind::Place,
            Self::Source => MatchableKind::Source,
            Self::Repository => MatchableKind::Repository,
            Self::Citation => MatchableKind::Citation,
            Self::Media => MatchableKind::Media,
            Self::Note => MatchableKind::Note,
        }
    }

    /// The decidable kind of a matchable one, or `None` for a tag.
    #[must_use]
    pub fn from_matchable(kind: MatchableKind) -> Option<Self> {
        Self::ALL.into_iter().find(|decidable| decidable.matchable() == kind)
    }

    /// The kind's aggregate type name (`person`, `place`, …).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.matchable().as_str()
    }

    /// The decidable kind of an aggregate type name, or `None` for any other name.
    #[must_use]
    pub fn parse(aggregate_type: &str) -> Option<Self> {
        MatchableKind::parse(aggregate_type).and_then(Self::from_matchable)
    }
}

/// Which proposed pairs the queue lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchQueueFilter {
    /// Only pairs with a record this import run created; `None` for every pair.
    pub run: Option<ImportRunId>,
    /// Only pairs of this kind; `None` for every decidable kind.
    pub kind: Option<DecidableKind>,
    /// Only pairs the engine judges at least this similar.
    pub min_band: MatchBand,
}

/// One undecided pair in the queue.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuedMatch {
    /// The kind of both records.
    pub kind: DecidableKind,
    /// The first record, the lower aggregate id.
    pub a: AggRef,
    /// The second record.
    pub b: AggRef,
    /// The engine's assessment of the pair.
    pub assessment: MatchAssessment,
}

/// A user's identity decision about a pair (ADR 0039 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchVerdict {
    /// The two records describe one entity: the second is merged into the first.
    Same,
    /// The two records describe different entities.
    Distinct,
}

/// The undecided pairs a filter admits: how many there are, and the strongest of them assessed.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchQueue {
    /// How many undecided pairs the filter admits.
    pub total: usize,
    /// The strongest of them, the most similar first whatever their kind — every one when no limit was
    /// asked for.
    pub pairs: Vec<QueuedMatch>,
}

/// The undecided pairs `filter` admits: all counted, and the `limit` most similar of them, whatever
/// their kind, listed with the engine's assessment — every one when `limit` is `None`.
///
/// # Errors
///
/// [`AppError::MatchData`] or [`AppError::Config`] if the matching data or settings cannot be loaded,
/// or [`AppError`] on a store failure.
pub async fn match_queue(
    workspace: &Workspace,
    filter: &MatchQueueFilter,
    limit: Option<usize>,
) -> Result<MatchQueue, AppError> {
    Box::pin(refresh_pairs(workspace)).await?;
    let mut kinds = Vec::new();
    for kind in DecidableKind::ALL {
        if filter.kind.is_none_or(|wanted| wanted == kind) {
            kinds.push(kind);
        }
    }
    let (total, listed) = if let Some(run) = filter.run {
        of_run(workspace, &kinds, run, filter.min_band, limit).await?
    } else {
        of_kinds(workspace, &kinds, filter.min_band, limit).await?
    };
    let mut pairs = Vec::with_capacity(listed.len());
    for pair in assessed_pairs(workspace, listed).await? {
        let Some(kind) = DecidableKind::from_matchable(pair.kind) else {
            continue;
        };
        pairs.push(QueuedMatch {
            kind,
            a: pair.a,
            b: pair.b,
            assessment: pair.assessment,
        });
    }
    Ok(MatchQueue { total, pairs })
}

/// How many undecided pairs of `kinds` from `min_band` up there are, and the `limit` strongest of them.
async fn of_kinds(
    workspace: &Workspace,
    kinds: &[DecidableKind],
    min_band: MatchBand,
    limit: Option<usize>,
) -> Result<(usize, Vec<MatchPair>), AppError> {
    let store = workspace.store();
    let matchable: Vec<MatchableKind> = kinds.iter().map(|kind| kind.matchable()).collect();
    let mut total = 0;
    for (kind, count) in store.match_pair_counts(min_band).await? {
        if matchable.contains(&kind) {
            total += count;
        }
    }
    Ok((total, store.match_pairs(&matchable, min_band, limit).await?))
}

/// How many undecided pairs of `kinds` from `min_band` up have a record the run `run` created, and the
/// `limit` strongest of them.
async fn of_run(
    workspace: &Workspace,
    kinds: &[DecidableKind],
    run: ImportRunId,
    min_band: MatchBand,
    limit: Option<usize>,
) -> Result<(usize, Vec<MatchPair>), AppError> {
    let mut created = HashSet::new();
    for kind in kinds {
        created.extend(created_by(workspace, *kind, run).await?);
    }
    let matchable: Vec<MatchableKind> = kinds.iter().map(|kind| kind.matchable()).collect();
    let mut pairs = workspace.store().match_pairs(&matchable, min_band, None).await?;
    pairs.retain(|pair| created.contains(&pair.a) || created.contains(&pair.b));
    let total = pairs.len();
    if let Some(limit) = limit {
        pairs.truncate(limit);
    }
    Ok((total, pairs))
}

/// The aggregate ids of the records of `kind` the run `run` created.
async fn created_by(workspace: &Workspace, kind: DecidableKind, run: ImportRunId) -> Result<HashSet<String>, AppError> {
    let mut created = HashSet::new();
    for (aggregate_id, origin) in workspace.store().created_origins(kind.as_str()).await? {
        if origin.run == run {
            created.insert(aggregate_id);
        }
    }
    Ok(created)
}

/// Calls the decidable kind's own use-case `$fn` (one per kind, listed in [`DecidableKind::ALL`]
/// order) with `$args`, discarding its result.
macro_rules! per_kind {
    ($kind:expr, [$person:path, $family:path, $event:path, $place:path, $source:path, $repository:path,
        $citation:path, $media:path, $note:path], ($($arg:expr),*)) => {
        match $kind {
            DecidableKind::Person => $person($($arg),*).await.map(|_| ()),
            DecidableKind::Family => $family($($arg),*).await.map(|_| ()),
            DecidableKind::Event => $event($($arg),*).await.map(|_| ()),
            DecidableKind::Place => $place($($arg),*).await.map(|_| ()),
            DecidableKind::Source => $source($($arg),*).await.map(|_| ()),
            DecidableKind::Repository => $repository($($arg),*).await.map(|_| ()),
            DecidableKind::Citation => $citation($($arg),*).await.map(|_| ()),
            DecidableKind::Media => $media($($arg),*).await.map(|_| ()),
            DecidableKind::Note => $note($($arg),*).await.map(|_| ()),
        }
    };
}

/// Records the user's decision about the pair `first`, `second` of `kind` (human ids): [`Same`]
/// merges `second` into `first`, [`Distinct`] holds them apart. Either way the pair leaves the queue.
///
/// [`Same`]: MatchVerdict::Same
/// [`Distinct`]: MatchVerdict::Distinct
///
/// # Errors
///
/// The kind's `NotFound` error for an unknown id, its domain rejection (see
/// [`AppError::identity_refusal`]) when the pair is already decided or cannot be merged, or
/// [`AppError`] on a store failure.
pub async fn decide_match(
    workspace: &Workspace,
    session: &Session,
    kind: DecidableKind,
    first: &str,
    second: &str,
    verdict: MatchVerdict,
    decision: IdentityDecision,
) -> Result<(), AppError> {
    match verdict {
        MatchVerdict::Same => per_kind!(
            kind,
            [
                crate::person::merge_persons,
                crate::family::merge_families,
                crate::event::merge_events,
                crate::place::merge_places,
                crate::source::merge_sources,
                crate::repository::merge_repositories,
                crate::citation::merge_citations,
                crate::media::merge_media,
                crate::note::merge_notes
            ],
            (workspace, session, first, second, decision)
        ),
        MatchVerdict::Distinct => per_kind!(
            kind,
            [
                crate::person::distinguish_persons,
                crate::family::distinguish_families,
                crate::event::distinguish_events,
                crate::place::distinguish_places,
                crate::source::distinguish_sources,
                crate::repository::distinguish_repositories,
                crate::citation::distinguish_citations,
                crate::media::distinguish_media,
                crate::note::distinguish_notes
            ],
            (workspace, session, first, second, decision)
        ),
    }
}

/// Retracts the live distinction between the pair's clusters and merges `second` into `first`: the
/// compare view's *Undo "not the same" and merge* (ADR 0039 §4).
///
/// # Errors
///
/// As [`decide_match`].
pub async fn undo_match_distinction_and_merge(
    workspace: &Workspace,
    session: &Session,
    kind: DecidableKind,
    first: &str,
    second: &str,
    decision: IdentityDecision,
) -> Result<(), AppError> {
    per_kind!(
        kind,
        [
            crate::person::undo_distinction_and_merge,
            crate::family::undo_family_distinction_and_merge,
            crate::event::undo_event_distinction_and_merge,
            crate::place::undo_place_distinction_and_merge,
            crate::source::undo_source_distinction_and_merge,
            crate::repository::undo_repository_distinction_and_merge,
            crate::citation::undo_citation_distinction_and_merge,
            crate::media::undo_media_distinction_and_merge,
            crate::note::undo_note_distinction_and_merge
        ],
        (workspace, session, first, second, decision)
    )
}

/// The live decision already taken between the clusters of `first` and `second` of `kind`, if any.
///
/// # Errors
///
/// The kind's `NotFound` error for an unknown id, or [`AppError`] on a store failure.
pub async fn match_pair_decision(
    workspace: &Workspace,
    kind: DecidableKind,
    first: &str,
    second: &str,
) -> Result<Option<PairDecision>, AppError> {
    match kind {
        DecidableKind::Person => crate::person::pair_decision(workspace, first, second).await,
        DecidableKind::Family => crate::family::family_pair_decision(workspace, first, second).await,
        DecidableKind::Event => crate::event::event_pair_decision(workspace, first, second).await,
        DecidableKind::Place => crate::place::place_pair_decision(workspace, first, second).await,
        DecidableKind::Source => crate::source::source_pair_decision(workspace, first, second).await,
        DecidableKind::Repository => crate::repository::repository_pair_decision(workspace, first, second).await,
        DecidableKind::Citation => crate::citation::citation_pair_decision(workspace, first, second).await,
        DecidableKind::Media => crate::media::media_pair_decision(workspace, first, second).await,
        DecidableKind::Note => crate::note::note_pair_decision(workspace, first, second).await,
    }
}

/// Why the decision core refused an identity decision, whatever the kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityRefusal {
    /// The two records carry claims that cannot both be true; the core's reason (domain text).
    Conflict(String),
    /// The pair already holds a live decision, either way.
    Decided,
}

/// Classifies a kind's domain error as an [`IdentityRefusal`].
macro_rules! refusal {
    ($error:expr, $($variant:ident => $module:ident::$Err:ident),+) => {
        $(
            if let AppError::$variant(error) = $error {
                if let vitni_core::$module::$Err::MergeConflict { reason, .. } = error {
                    return Some(IdentityRefusal::Conflict(reason.clone()));
                }
                if let vitni_core::$module::$Err::IdentityDecided { .. } = error {
                    return Some(IdentityRefusal::Decided);
                }
                return None;
            }
        )+
    };
}

impl AppError {
    /// The identity refusal this error is, when a merge or distinction of any decidable kind was
    /// refused by its decision core; `None` for every other error.
    #[must_use]
    pub fn identity_refusal(&self) -> Option<IdentityRefusal> {
        refusal!(
            self,
            Domain => person::PersonError,
            FamilyDomain => family::FamilyError,
            EventDomain => event::EventError,
            PlaceDomain => place::PlaceError,
            SourceDomain => source::SourceError,
            RepositoryDomain => repository::RepositoryError,
            CitationDomain => citation::CitationError,
            MediaDomain => media::MediaError,
            NoteDomain => note::NoteError
        );
        None
    }
}
