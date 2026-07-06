//! Shared use-case plumbing: command-outcome mapping and id resolution.
//!
//! The per-aggregate use-cases (`person`, `family`, …) all turn a store outcome into an
//! [`AppError`] and resolve a `human_id` to an aggregate id the same way; those two steps live here
//! so each use-case stays a thin, aggregate-specific wrapper.

use std::collections::HashMap;

use genealogy_core::ids::{AssertionId, MediaId, NoteId};
use genealogy_core::provenance::{CitationRef, Confidence, EvidenceAnalysis};
use genealogy_db::{CommandError, DbError, Store};
use uuid::Uuid;

use crate::error::AppError;

/// The operator's surety in, reason for, and evidence analysis of a single assertion — the
/// per-assertion provenance the frontend supplies (data-model §8). Defaults to
/// [`Confidence::Normal`] with no rationale or analysis, so a caller that does not collect it keeps
/// the previous behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// The operator's surety in this claim.
    pub confidence: Confidence,
    /// Why the claim was made (free text; GENTECH rationale / GEDCOM X change message).
    pub rationale: Option<String>,
    /// The optional Evidence Explained analysis (source · information · evidence) for this claim.
    pub evidence_analysis: Option<EvidenceAnalysis>,
}

impl Default for Provenance {
    fn default() -> Self {
        Self {
            confidence: Confidence::Normal,
            rationale: None,
            evidence_analysis: None,
        }
    }
}

/// The operator-intent inputs a non-create mutation carries: the [`Provenance`], the citation
/// `human_id`s backing the assertion, and — for a correction — the `AssertionId` of the assertion
/// being superseded (ADR 0004 §2). Bundled so a mutation's signature stays within the argument-count
/// lint; every field defaults to the previous behavior (default provenance, no citations, no
/// supersede). A `create_*` never supersedes, so it takes [`Provenance`] + citations flat instead.
#[derive(Debug, Default)]
pub struct MutationMeta<'a> {
    /// The operator's surety, rationale, and evidence analysis.
    pub provenance: Provenance,
    /// Citation `human_id`s recorded in the assertion's `EventContext.citations`.
    pub citations: &'a [String],
    /// The `AssertionId` (a UUID string) this mutation supersedes, if it is a correction.
    pub supersedes: Option<&'a str>,
}

/// Maps a [`CommandError`] to [`AppError`]: a domain rejection becomes the matching `…Domain`
/// variant (via its `From` impl), a store failure becomes [`AppError::Db`]. This keeps the
/// operator's fault (a 4xx) distinct from the system's.
pub(crate) fn map_command_error<E>(error: CommandError<E>) -> AppError
where
    E: std::error::Error,
    AppError: From<E>,
{
    match error {
        CommandError::Rejected(domain) => AppError::from(domain),
        CommandError::Store(db) => AppError::Db(db),
    }
}

/// Loads a `MediaId -> human_id` lookup from the Media projection.
pub(crate) async fn media_human_ids(store: &Store) -> Result<HashMap<MediaId, String>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_media().await? {
        if let (Some(id), Some(human_id)) = (view.media_id(), view.human_id()) {
            map.insert(id, human_id.as_str().to_owned());
        }
    }
    Ok(map)
}

/// Loads a `NoteId -> human_id` lookup from the Note projection.
pub(crate) async fn note_human_ids(store: &Store) -> Result<HashMap<NoteId, String>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_notes().await? {
        if let (Some(id), Some(human_id)) = (view.note_id(), view.human_id()) {
            map.insert(id, human_id.as_str().to_owned());
        }
    }
    Ok(map)
}

/// Resolves citation `human_id`s to the [`CitationRef`]s that back an assertion, linking the
/// provenance envelope to real Citation aggregates (data-model §8). Shared by every aggregate's
/// mutation use-cases.
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if a cited citation `human_id` is unknown.
pub(crate) async fn resolve_citation_refs(store: &Store, human_ids: &[String]) -> Result<Vec<CitationRef>, AppError> {
    let mut refs = Vec::with_capacity(human_ids.len());
    for human_id in human_ids {
        let view = store
            .find_citation(human_id)
            .await?
            .ok_or_else(|| AppError::CitationNotFound(human_id.clone()))?;
        let citation_id = view
            .citation_id()
            .ok_or_else(|| AppError::CitationNotFound(human_id.clone()))?;
        refs.push(CitationRef { citation_id });
    }
    Ok(refs)
}

/// Parses an optional supersede target: `None` stays `None`; a `Some(id)` is parsed to an
/// [`AssertionId`] (a UUID string, as [`crate::history`] parses one). Whether the target assertion
/// actually exists is left to the decision core to reject.
///
/// # Errors
///
/// [`AppError::Db`] with [`DbError::Malformed`] if `supersedes` is not a UUID.
pub(crate) fn parse_supersedes(supersedes: Option<&str>) -> Result<Option<AssertionId>, AppError> {
    match supersedes {
        None => Ok(None),
        Some(id) => Uuid::parse_str(id)
            .map(AssertionId::from_uuid)
            .map(Some)
            .map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}")))),
    }
}

/// Resolves a looked-up view to an aggregate id: `extract` pulls the id from the view, and a missing
/// view (or a view without the id) becomes the `not_found` error. Centralizes the
/// find → extract → not-found pattern every `resolve_<agg>_id` helper repeats.
pub(crate) fn resolve_id<V, Id>(
    found: Option<V>,
    extract: impl FnOnce(&V) -> Option<Id>,
    not_found: impl FnOnce() -> AppError,
) -> Result<Id, AppError> {
    let Some(view) = found else {
        return Err(not_found());
    };
    extract(&view).ok_or_else(not_found)
}
