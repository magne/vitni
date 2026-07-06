//! Shared change-set primitives (Phase 5): the reference vocabulary a deferred create uses to name
//! not-yet-saved aggregates, and the helper that commits a change-set's pending Source and Citation
//! aggregates in dependency order.
//!
//! A change-set buffers every edit locally and persists nothing until Save (`record-editing.html`
//! §6). On Save the frontend hands the app the *desired* end state; each per-aggregate
//! `commit_<agg>_change_set` validates it up front (before any write) and turns it into the minimal
//! set of commands. A citation created inside the same form is not yet saved, yet several assertions
//! may cite it; the UI names that not-yet-saved target with a [`PlaceholderRef`], and
//! [`commit_pending_sources_and_citations`] mints the real UUID once and resolves every placeholder
//! to it (ADR 0004 §1).
//!
//! # Provenance rule (ADR 0004 §1, PR26)
//!
//! Every change-set carries one operator [`Provenance`] (confidence · rationale · evidence analysis)
//! captured once in the save's provenance block (`record-editing.html` §5b), plus the backing
//! citation `human_id`s. The rule each `commit_<agg>_change_set` follows: resolve the backing
//! citations to `CitationRef`s **before any write** (an unknown id rejects the whole action, so
//! nothing commits), stamp the [`Provenance`] on every emitted command, and ride the backing
//! citations on every non-`Create*` command. The pending Source/Citation aggregates created here are
//! support records for the primary one, so they carry the operator's [`Provenance`] but no backing
//! citations of their own.

use genealogy_core::ids::{CitationId, SourceId};
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::Provenance;
use crate::workspace::Workspace;

/// A placeholder for a not-yet-saved aggregate created inside the same change-set (a pending Source
/// or Citation). The UI mints these locally; [`commit_pending_sources_and_citations`] resolves each
/// to the real UUID it allocates, so later entries and every citing assertion use the persisted id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlaceholderRef(pub String);

/// Which source a pending citation cites: one that already exists, or one created in this set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRefInput {
    /// An existing source, by its `human_id` (e.g. `S0001`).
    Existing(String),
    /// A source created earlier in this same change-set, by its placeholder.
    Pending(PlaceholderRef),
}

/// Which citation an assertion cites: one that already exists, or one created in this set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationRefInput {
    /// An existing citation, by its `human_id` (e.g. `C0001`).
    Existing(String),
    /// A citation created in this same change-set, by its placeholder.
    Pending(PlaceholderRef),
}

/// A new Source to create as part of the change-set (only the title is collected in this slice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSourceEntry {
    /// The placeholder a pending citation references this source by.
    pub placeholder: PlaceholderRef,
    /// The source's title, if the operator gave one.
    pub title: Option<String>,
}

/// A new Citation to create as part of the change-set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCitationEntry {
    /// The placeholder assertions reference this citation by.
    pub placeholder: PlaceholderRef,
    /// The source this citation cites (existing or pending in the same set).
    pub source: SourceRefInput,
    /// The page / locator within the source, if given.
    pub page: Option<String>,
}

/// The ids a change-set minted for its pending Source/Citation placeholders, for resolving
/// intra-set references.
#[derive(Default)]
pub(crate) struct Resolution {
    sources: Vec<(PlaceholderRef, SourceId)>,
    citations: Vec<(PlaceholderRef, CitationId)>,
}

impl Resolution {
    /// The minted [`SourceId`] for a pending source placeholder, if this set created one.
    pub(crate) fn source(&self, placeholder: &PlaceholderRef) -> Option<SourceId> {
        self.sources.iter().find(|(p, _)| p == placeholder).map(|(_, id)| *id)
    }

    /// The minted [`CitationId`] for a pending citation placeholder, if this set created one.
    pub(crate) fn citation(&self, placeholder: &PlaceholderRef) -> Option<CitationId> {
        self.citations.iter().find(|(p, _)| p == placeholder).map(|(_, id)| *id)
    }
}

/// Creates a change-set's new Source and Citation aggregates in dependency order (Source →
/// Citation), stamping each with the operator `provenance`, and returns the minted ids so the
/// primary record's assertions can cite them.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if a pending citation references a source that neither exists nor was
/// created earlier in the set, [`AppError::CitationDomain`]/[`AppError::SourceDomain`] on a domain
/// rejection, or a workspace/store error.
pub(crate) async fn commit_pending_sources_and_citations(
    workspace: &Workspace,
    session: &Session,
    store: &Store,
    new_sources: &[NewSourceEntry],
    new_citations: &[NewCitationEntry],
    provenance: &Provenance,
) -> Result<Resolution, AppError> {
    let mut resolution = Resolution::default();
    for entry in new_sources {
        let human_id = store.next_source_human_id(&workspace.source_id_format()?).await?;
        let source_id = crate::source::create_source_returning_id(
            session,
            store,
            &human_id,
            entry.title.clone(),
            provenance.clone(),
        )
        .await?;
        resolution.sources.push((entry.placeholder.clone(), source_id));
    }
    for entry in new_citations {
        let source_id = match &entry.source {
            SourceRefInput::Existing(human_id) => crate::citation::resolve_source_id_public(store, human_id).await?,
            SourceRefInput::Pending(placeholder) => resolution
                .source(placeholder)
                .ok_or_else(|| AppError::SourceNotFound(placeholder.0.clone()))?,
        };
        let human_id = store.next_citation_human_id(&workspace.citation_id_format()?).await?;
        let citation_id = crate::citation::create_citation_returning_id(
            session,
            store,
            &human_id,
            source_id,
            entry.page.clone(),
            provenance.clone(),
        )
        .await?;
        resolution.citations.push((entry.placeholder.clone(), citation_id));
    }
    Ok(resolution)
}
