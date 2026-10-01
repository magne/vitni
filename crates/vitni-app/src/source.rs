//! Source use-cases (ADR 0006): create, set title, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](vitni_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`SourceSummary`]. `human_id` is auto-allocated using the workspace's configured
//! format, or validated when supplied (ADR 0005).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use vitni_core::enums::{Restriction, SourceMediaType};
use vitni_core::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, RepositoryId, SourceId, TagId};
use vitni_core::provenance::EvidenceRef;
use vitni_core::provenance::{EvidenceAnalysis, EvidenceKind, InformationKind, SourceQuality};
use vitni_core::repo_ref::RepoRef;
use vitni_core::repository::RepositoryView;
use vitni_core::source::SourceView;
use vitni_core::source::command::{SourceCommand, SourceCommandEnvelope};
use vitni_core::source::error::SourceError;
use vitni_core::text::{Attribute, MediaRef};
use vitni_db::Store;

use crate::citation::TagRef;
use crate::citation_usage::CitationUsage;
use crate::dto::{
    AggRef, AttachedRef, CitationRef, MediaLookup, MediaRefSummary, RepositoryLinkRef, SourceCitationRef,
    SourceReliability, citation_refs, media_lookups, repository_refs, tag_refs,
};
use crate::error::AppError;
use crate::identity::{self, CitationClusters, IdentityDecision, PairDecision, SourceClusters};
use crate::session::Session;
use crate::use_case::{self, MediaRefInput, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// A typed attribute on a source (the Source › Attributes rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAttributeRef {
    /// The attribute's type / key (e.g. `microfilm series`).
    pub attribute_type: String,
    /// The attribute's value.
    pub value: String,
    /// The `AssertionId` (a UUID string) that introduced this attribute — the target a per-row Edit
    /// supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// A frontend-neutral summary of a source (the DTO the CLI and UI render). References to other
/// aggregates carry their stable ids alongside their `human_id`s (the cross-aggregate-joins note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSummary {
    /// The user-facing identifier (e.g. `S0001`).
    pub human_id: String,
    /// The source's stable `SourceId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The bibliographic title, if set.
    pub title: Option<String>,
    /// The author, if set.
    pub author: Option<String>,
    /// The publication info, if set.
    pub pub_info: Option<String>,
    /// The abbreviation, if set.
    pub abbrev: Option<String>,
    /// The repositories that hold this source, joined to the Repository projection, in assertion order.
    pub repositories: Vec<RepositoryLinkRef>,
    /// The source's attributes, in assertion order.
    pub attributes: Vec<SourceAttributeRef>,
    /// The citations that use this source, joined to the records they back, in `human_id` order.
    pub citations: Vec<SourceCitationRef>,
    /// Media attached to the source, in assertion order.
    pub media: Vec<MediaRefSummary>,
    /// Notes attached to the source, with the attach `AssertionId` (the Detach target), in assertion
    /// order.
    pub notes: Vec<AttachedRef>,
    /// Tags applied to the source, by name + colour (never by id — data-model §9).
    pub tags: Vec<TagRef>,
    /// The reliability synthesis derived from the source's citation set.
    pub reliability: SourceReliability,
    /// The source's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
    /// Every source record merged into this source's cluster, directly or through another member
    /// (ADR 0039 §4), in id order.
    pub merged: Vec<AggRef>,
    /// The `human_id` of the member each member row came from, by the row's `AssertionId`: its edit or
    /// retraction is written to that member's stream (ADR 0039 §5). A row absent here is the
    /// source's own. Never rendered as a key; see [`owner_of`](Self::owner_of).
    pub claim_owners: BTreeMap<String, String>,
}

impl SourceSummary {
    /// The `human_id` of the record that owns the row introduced by `assertion_id`: the member it came
    /// from, or this source for its own rows.
    #[must_use]
    pub fn owner_of(&self, assertion_id: &str) -> &str {
        self.claim_owners
            .get(assertion_id)
            .map_or(self.human_id.as_str(), String::as_str)
    }
}

/// What to create a source with (the auto/override `human_id` and an optional title).
#[derive(Debug, Clone)]
pub struct NewSource {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// An optional title for an initial `SetTitle`.
    pub title: Option<String>,
}

/// Creates a source, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::SourceDomain`] if a domain
/// rule rejects the command, or a workspace/store error.
pub async fn create_source(
    workspace: &Workspace,
    session: &Session,
    new: NewSource,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let follow_up = provenance.follow_up();
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_source(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_source_human_id(&workspace.source_id_format()?).await?,
    };
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let source_id = session.new_source_id();
    let aggregate_id = source_id.to_string();

    execute(
        store,
        session,
        &aggregate_id,
        SourceCommand::CreateSource {
            source_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        citation_refs,
    )
    .await?;

    if let Some(title) = new.title {
        execute(
            store,
            session,
            &aggregate_id,
            SourceCommand::SetTitle { source_id, title },
            follow_up,
            Vec::new(),
        )
        .await?;
    }

    Ok(human_id)
}

/// Creates a source with an already-allocated `human_id`, returning its minted [`SourceId`].
///
/// The change-set use-case ([`crate::person_change_set`]) reuses this to create a pending source and
/// keep the id for later intra-set references; the `human_id` is allocated by the caller before any
/// write, so id allocation and the person's id validation happen together.
///
/// # Errors
///
/// [`AppError::SourceDomain`] on a domain rejection, or a workspace/store error.
pub(crate) async fn create_source_returning_id(
    session: &Session,
    store: &Store,
    human_id: &str,
    title: Option<String>,
    provenance: Provenance,
) -> Result<SourceId, AppError> {
    let source_id = session.new_source_id();
    let aggregate_id = source_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        SourceCommand::CreateSource {
            source_id,
            human_id: HumanId::new(human_id),
        },
        provenance.clone(),
        Vec::new(),
    )
    .await?;
    if let Some(title) = title {
        execute(
            store,
            session,
            &aggregate_id,
            SourceCommand::SetTitle { source_id, title },
            provenance,
            Vec::new(),
        )
        .await?;
    }
    Ok(source_id)
}

/// Sets (or changes) an existing source's title, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_title(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    title: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::SetTitle { source_id, title },
        meta,
    )
    .await
}

/// Sets (or changes) an existing source's author, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_source_author(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    author: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::SetAuthor { source_id, author },
        meta,
    )
    .await
}

/// Sets (or changes) an existing source's publication info, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_source_pub_info(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    pub_info: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::SetPubInfo { source_id, pub_info },
        meta,
    )
    .await
}

/// Sets (or changes) an existing source's abbreviation, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_source_abbrev(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    abbrev: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::SetAbbrev { source_id, abbrev },
        meta,
    )
    .await
}

/// Links a source to a repository (by its `human_id`) that holds it.
///
/// # Errors
///
/// [`AppError::SourceNotFound`]/[`AppError::RepositoryNotFound`] if either is unknown,
/// [`AppError::SourceDomain`] if the repository is not yet projected (`UnknownRepository`), or a
/// workspace/store error.
pub async fn link_source_repository(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    repository_human_id: &str,
    call_number: Option<String>,
    media_type: SourceMediaType,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    let repository_id = resolve_repository_id(store, repository_human_id).await?;
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::LinkRepository {
            source_id,
            repo_ref: RepoRef {
                repository_id,
                call_number,
                media_type,
            },
        },
        meta,
    )
    .await
}

/// Adds a typed attribute to a source, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn add_source_attribute(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    attribute_type: String,
    value: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::AddAttribute {
            source_id,
            attribute: Attribute { attribute_type, value },
        },
        meta,
    )
    .await
}

/// Attaches a media reference (by media aggregate id) to a source, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn attach_source_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    media_id: MediaId,
    input: MediaRefInput,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::AttachMedia {
            source_id,
            media: MediaRef {
                media_id,
                crop: input.crop,
                caption: input.caption,
                citations: Vec::new(),
            },
        },
        meta,
    )
    .await
}

/// Re-edits an existing source media attachment (crop / caption) by the `AssertionId` of the attach
/// assertion — supersedes it with a new `MediaAttached` carrying the same media and citations plus
/// the new crop/caption (the row-Edit correction, ADR 0004 §2).
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, [`AppError::SourceDomain`] if
/// `assertion_id` names no live media attachment, or a workspace/store error.
pub async fn update_source_media_ref(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
    input: MediaRefInput,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let view = store
        .find_source(human_id)
        .await?
        .ok_or_else(|| AppError::SourceNotFound(human_id.to_owned()))?;
    let source_id = resolve_source_id(store, human_id).await?;
    let target = use_case::parse_assertion_id(assertion_id)?;
    let existing = view
        .media_with_assertions()
        .iter()
        .find(|attributed| attributed.assertion_id == target)
        .ok_or(AppError::SourceDomain(SourceError::SupersedesMissingAssertion(target)))?;
    let media = MediaRef {
        media_id: existing.value.media_id,
        crop: input.crop,
        caption: input.caption,
        citations: existing.value.citations.clone(),
    };
    let meta = MutationMeta {
        supersedes: Some(assertion_id),
        ..meta
    };
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::AttachMedia { source_id, media },
        meta,
    )
    .await
}

/// Attaches a note (by note aggregate id) to a source, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn attach_source_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::AttachNote { source_id, note_id },
        meta,
    )
    .await
}

/// Applies (or removes) a tag on a source, identified by `human_id`. A removed tag is untagged on
/// every record of the cluster that holds it (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn tag_source(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    if !remove {
        let command = SourceCommand::Tag { source_id, tag_id };
        return execute_source_mutation(store, session, source_id, command, meta).await;
    }
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    for record in identity::cluster_records(store, source_id).await? {
        let Some(id) = record.source_id() else { continue };
        if id != source_id && record.tags().contains(&tag_id) {
            let command = SourceCommand::Untag { source_id: id, tag_id };
            execute(
                store,
                session,
                &id.to_string(),
                command,
                meta.provenance.clone(),
                citations.clone(),
            )
            .await?;
        }
    }
    let command = SourceCommand::Untag { source_id, tag_id };
    execute_source_mutation(store, session, source_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// The outcome of [`merge_sources`]: the survivor's refreshed summary and the merged source's
/// `human_id`.
///
/// The merge is a same-as link on the survivor (ADR 0039 §1): no record that names the merged source
/// is rewritten. Every reader resolves those references to the cluster's root instead (ADR 0039 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMergeResult {
    /// The survivor's summary after the merge, composed with every record now in its cluster.
    pub survivor: SourceSummary,
    /// The merged source's `human_id` (its own record/stream is untouched).
    pub merged_human_id: String,
}

/// Merges `merged_human_id`'s cluster into `surviving_human_id`'s, recording a same-as link
/// (ADR 0039 §1). Both records resolve to their cluster roots first (§4), and one `SourcesMerged`
/// event is emitted on the surviving root's stream, carrying the decision's provenance and assessment.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if either `human_id` does not resolve; [`AppError::SourceDomain`] with
/// `MergeConflict` if they resolve to the same source, or `IdentityDecided` if the two are already one
/// cluster or a record of one cluster is distinguished from a record of the other; or a store error.
pub async fn merge_sources(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<SourceMergeResult, AppError> {
    let pair = identity::merge::<SourceView>(workspace, session, surviving_human_id, merged_human_id, decision).await?;
    source_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// Undoes every live distinction between the two source clusters, then merges them (ADR 0039 §4).
///
/// # Errors
///
/// As [`merge_sources`], except that a distinction between the clusters no longer refuses the merge.
pub async fn undo_source_distinction_and_merge(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<SourceMergeResult, AppError> {
    let pair = identity::undo_distinction_and_merge::<SourceView>(
        workspace,
        session,
        surviving_human_id,
        merged_human_id,
        decision,
    )
    .await?;
    source_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// The survivor's composed summary after a merge.
async fn source_merge_result(
    workspace: &Workspace,
    survivor_human_id: &str,
    merged_human_id: &str,
) -> Result<SourceMergeResult, AppError> {
    let survivor = show_source(workspace, survivor_human_id)
        .await?
        .ok_or_else(|| AppError::SourceNotFound(survivor_human_id.to_owned()))?;
    Ok(SourceMergeResult {
        survivor,
        merged_human_id: merged_human_id.to_owned(),
    })
}

/// Records that the two sources are different (ADR 0039 §1), so neither cluster is proposed as a
/// duplicate of the other again. One `SourcesDistinguished` event is emitted on the first root's
/// stream; undoing it lifts the decision.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if either `human_id` does not resolve; [`AppError::SourceDomain`] with
/// `DistinctFromItself` if they resolve to the same source, or `IdentityDecided` if the two are already
/// one cluster or already distinguished; or a store error.
pub async fn distinguish_sources(
    workspace: &Workspace,
    session: &Session,
    source_human_id: &str,
    other_human_id: &str,
    decision: IdentityDecision,
) -> Result<(), AppError> {
    identity::distinguish::<SourceView>(workspace, session, source_human_id, other_human_id, decision).await
}

/// The live identity decision between the clusters of two sources, or `None` when the pair is
/// undecided (ADR 0039 §4).
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if either `human_id` does not resolve, or a store error.
pub async fn source_pair_decision(
    workspace: &Workspace,
    first_human_id: &str,
    other_human_id: &str,
) -> Result<Option<PairDecision>, AppError> {
    identity::pair_decision::<SourceView>(workspace, first_human_id, other_human_id).await
}

/// The `human_id` of the record of `human_id`'s source cluster whose stream holds the live assertion
/// `assertion_id` — where an edit or retraction of that row is written (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if `human_id` is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or a store error.
pub async fn source_claim_owner(workspace: &Workspace, human_id: &str, assertion_id: &str) -> Result<String, AppError> {
    identity::claim_owner::<SourceView>(workspace, human_id, assertion_id).await
}

/// Loads a single source's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_source(workspace: &Workspace, human_id: &str) -> Result<Option<SourceSummary>, AppError> {
    let store = workspace.store();
    let Some(source_id) = store.find_source(human_id).await?.and_then(|view| view.source_id()) else {
        return Ok(None);
    };
    let views = identity::cluster_records(store, source_id).await?;
    let lookups = SourceLookups::load(workspace).await?;
    Ok(summarize_cluster(&views.iter().collect::<Vec<_>>(), &lookups))
}

/// Lists every source's summary, ordered by `human_id`. A merged record is listed once, as its cluster's
/// root (ADR 0039 §5).
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_sources(workspace: &Workspace) -> Result<Vec<SourceSummary>, AppError> {
    let store = workspace.store();
    let views = store.list_sources().await?;
    let clusters = SourceClusters::load(store).await?;
    let lookups = SourceLookups::load(workspace).await?;
    let mut summaries = Vec::with_capacity(views.len());
    for cluster in identity::group_clusters(&views, &clusters) {
        summaries.extend(summarize_cluster(&cluster, &lookups));
    }
    Ok(summaries)
}

/// Sets a source's privacy restrictions (GEDCOM `RESN` — data-model §6). A merged member restricted
/// beyond the new set is narrowed to it first, so the cluster reads with exactly `restrictions`
/// (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    for record in identity::cluster_records(store, source_id).await? {
        let Some(id) = record.source_id() else { continue };
        if id == source_id || record.restrictions().is_subset(&restrictions) {
            continue;
        }
        let narrowed = record.restrictions().intersection(&restrictions).copied().collect();
        let command = SourceCommand::SetRestrictions {
            source_id: id,
            restrictions: narrowed,
        };
        execute(
            store,
            session,
            &id.to_string(),
            command,
            meta.provenance.clone(),
            citations.clone(),
        )
        .await?;
    }
    execute_source_mutation(
        store,
        session,
        source_id,
        SourceCommand::SetRestrictions {
            source_id,
            restrictions,
        },
        meta,
    )
    .await
}

/// Sets (or changes) a source's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case).
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if the source is unknown, [`AppError::HumanIdTaken`] if the requested
/// id is already in use, or a workspace/store error.
pub async fn set_source_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_source(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_source_human_id(&workspace.source_id_format()?).await?,
    };
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::SetHumanId {
            source_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        Vec::new(),
    )
    .await?;
    Ok(human_id)
}

/// Executes one command through the store, stamping the operator `provenance` and backing
/// `citations`, and mapping the command outcome to [`AppError`].
pub(crate) async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: SourceCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = SourceCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    let Some(envelope) = crate::origin_gate::gate(store, session, aggregate_id, envelope).await? else {
        return Ok(());
    };
    store
        .execute_source(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create source mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`SourceCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_source_mutation(
    store: &Store,
    session: &Session,
    source_id: SourceId,
    command: SourceCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(source_id, command, target);
    execute(
        store,
        session,
        &source_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`SourceCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(source_id: SourceId, command: SourceCommand, target: Option<AssertionId>) -> SourceCommand {
    match target {
        Some(target) => SourceCommand::SupersedeAssertion {
            source_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
}

/// Resolves a `human_id` to its aggregate [`SourceId`], or [`AppError::SourceNotFound`].
async fn resolve_source_id(store: &Store, human_id: &str) -> Result<SourceId, AppError> {
    use_case::resolve_id(store.find_source(human_id).await?, SourceView::source_id, || {
        AppError::SourceNotFound(human_id.to_owned())
    })
}

/// Resolves a repository `human_id` to its aggregate [`RepositoryId`], or
/// [`AppError::RepositoryNotFound`].
async fn resolve_repository_id(store: &Store, human_id: &str) -> Result<RepositoryId, AppError> {
    use_case::resolve_id(
        store.find_repository(human_id).await?,
        RepositoryView::repository_id,
        || AppError::RepositoryNotFound(human_id.to_owned()),
    )
}

/// The lookups `summarize` needs to join a source's repository links, the citations that use it, and
/// its attachments to the other projections without a per-row query (the join lives in this layer).
struct SourceLookups {
    repositories: HashMap<RepositoryId, (RepositoryId, String, Option<String>)>,
    citations: HashMap<CitationId, CitationRef>,
    citations_by_source: HashMap<SourceId, Vec<CitationId>>,
    media: HashMap<MediaId, MediaLookup>,
    notes: HashMap<NoteId, use_case::NoteLookup>,
    tags: HashMap<TagId, TagRef>,
    usage: CitationUsage,
}

impl SourceLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        // A merged citation is listed once, as its root, and a citation of a merged source under the
        // source's root (ADR 0039 §5).
        let citation_clusters = CitationClusters::load(store).await?;
        let mut citations_by_source: HashMap<SourceId, Vec<CitationId>> = HashMap::new();
        for view in store.list_citations().await? {
            if let (Some(citation_id), Some(source_id)) = (view.citation_id(), view.source_id())
                && !citation_clusters.is_member(citation_id)
            {
                citations_by_source.entry(source_id).or_default().push(citation_id);
            }
        }
        SourceClusters::load(store).await?.fold(&mut citations_by_source);
        Ok(Self {
            repositories: repository_refs(store).await?,
            citations: citation_refs(store).await?,
            citations_by_source,
            media: media_lookups(store).await?,
            notes: use_case::note_lookups(store).await?,
            tags: tag_refs(store).await?,
            usage: CitationUsage::load(workspace).await?,
        })
    }
}

/// Renders a [`SourceView`] into the frontend DTO, joining its repository links, the citations that
/// use it (and the records they back), and its attachments via `lookups`.
fn summarize(view: &SourceView, lookups: &SourceLookups) -> SourceSummary {
    let repositories = view
        .repositories_with_assertions()
        .iter()
        .map(|attributed| {
            let asserted = &attributed.value;
            let repo_ref = &asserted.value;
            let info = lookups.repositories.get(&repo_ref.repository_id);
            RepositoryLinkRef {
                repository: info.map(|(root, human_id, _)| AggRef {
                    human_id: human_id.clone(),
                    id: root.to_string(),
                }),
                name: info.and_then(|(_, _, name)| name.clone()),
                call_number: repo_ref.call_number.clone(),
                media_type: repo_ref.media_type.clone(),
                confidence: asserted.confidence,
                source_count: asserted.citation_ids().count(),
                assertion_id: attributed.assertion_id.to_string(),
            }
        })
        .collect();
    let attributes = view
        .attributes_with_assertions()
        .iter()
        .map(|attributed| SourceAttributeRef {
            attribute_type: attributed.value.attribute_type.clone(),
            value: attributed.value.value.clone(),
            assertion_id: attributed.assertion_id.to_string(),
        })
        .collect();
    let source_id = view.source_id();
    let citation_ids = source_id
        .and_then(|id| lookups.citations_by_source.get(&id))
        .cloned()
        .unwrap_or_default();
    let citations: Vec<SourceCitationRef> = citation_ids
        .iter()
        .filter_map(|id| {
            lookups.citations.get(id).map(|citation| SourceCitationRef {
                citation: citation.clone(),
                backers: lookups.usage.backers(*id),
            })
        })
        .collect();
    let reliability = reliability(&citations);
    let media = view
        .media_with_assertions()
        .iter()
        .filter_map(|attributed| {
            let media = &attributed.value;
            lookups.media.get(&media.media_id).map(|lookup| MediaRefSummary {
                human_id: lookup.human_id.clone(),
                id: lookup.id.clone(),
                caption: media.caption.clone(),
                crop: media.crop,
                path: lookup.path.clone(),
                mime: lookup.mime.clone(),
                assertion_id: attributed.assertion_id.to_string(),
            })
        })
        .collect();
    let notes = view
        .notes_with_assertions()
        .iter()
        .filter_map(|attributed| {
            lookups.notes.get(&attributed.value).map(|note| AttachedRef {
                human_id: note.human_id.clone(),
                id: note.id.clone(),
                note_type: note.note_type.clone(),
                text: note.text.clone(),
                language: note.language.clone(),
                assertion_id: attributed.assertion_id.to_string(),
            })
        })
        .collect();
    let tags = view
        .tags()
        .into_iter()
        .filter_map(|id| lookups.tags.get(&id).cloned())
        .collect();
    SourceSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id: source_id.map(|id| id.to_string()).unwrap_or_default(),
        title: view.title().map(ToOwned::to_owned),
        author: view.author().map(ToOwned::to_owned),
        pub_info: view.pub_info().map(ToOwned::to_owned),
        abbrev: view.abbrev().map(ToOwned::to_owned),
        repositories,
        attributes,
        citations,
        media,
        notes,
        tags,
        reliability,
        restrictions: view.restrictions().clone(),
        merged: Vec::new(),
        claim_owners: BTreeMap::new(),
    }
}

/// Aggregates the reliability synthesis from a source's citation set: the modal surety, the modal
/// Evidence Explained analysis (per axis), and how many citations + distinct records use the source.
fn reliability(citations: &[SourceCitationRef]) -> SourceReliability {
    let typical_surety = mode(citations.iter().filter_map(|c| c.citation.confidence));
    let evidence = modal_evidence(citations);
    let mut records: BTreeSet<String> = BTreeSet::new();
    for citation in citations {
        for backer in &citation.backers {
            records.insert(backer.id.clone());
        }
    }
    SourceReliability {
        typical_surety,
        evidence,
        citation_count: citations.len(),
        record_count: records.len(),
    }
}

/// The most frequent value in `values` (ties resolved by first-seen), or `None` if empty.
fn mode<T: Copy + PartialEq>(values: impl Iterator<Item = T>) -> Option<T> {
    let mut counts: Vec<(T, usize)> = Vec::new();
    for value in values {
        if let Some(entry) = counts.iter_mut().find(|(v, _)| *v == value) {
            entry.1 += 1;
        } else {
            counts.push((value, 1));
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(v, _)| v)
}

/// Builds the modal [`EvidenceAnalysis`] across a source's citations: the most common value on each
/// of the three Evidence Explained axes, or `None` if no citation carries an analysis.
fn modal_evidence(citations: &[SourceCitationRef]) -> Option<EvidenceAnalysis> {
    let analyses: Vec<EvidenceAnalysis> = citations.iter().filter_map(|c| c.citation.analysis).collect();
    if analyses.is_empty() {
        return None;
    }
    let source: Option<SourceQuality> = mode(analyses.iter().map(|a| a.source));
    let information: Option<InformationKind> = mode(analyses.iter().map(|a| a.information));
    let evidence: Option<EvidenceKind> = mode(analyses.iter().map(|a| a.evidence));
    Some(EvidenceAnalysis {
        source: source?,
        information: information?,
        evidence: evidence?,
    })
}

/// Attaches a media object (by its `human_id`) to a source — the importer-facing wrapper that
/// resolves the media `human_id` to its id, so a bulk importer never handles UUIDs.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] / [`AppError::MediaNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_source_media(
    workspace: &Workspace,
    session: &Session,
    source_human_id: &str,
    media_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = use_case::resolve_id(
        store.find_media(media_human_id).await?,
        vitni_core::media::MediaView::media_id,
        || AppError::MediaNotFound(media_human_id.to_owned()),
    )?;
    attach_source_media(
        workspace,
        session,
        source_human_id,
        media_id,
        MediaRefInput::default(),
        MutationMeta::default(),
    )
    .await
}

/// Attaches a note (by its `human_id`) to a source — the importer-facing wrapper.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_source_note(
    workspace: &Workspace,
    session: &Session,
    source_human_id: &str,
    note_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = use_case::resolve_id(
        store.find_note(note_human_id).await?,
        vitni_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_source_note(workspace, session, source_human_id, note_id, MutationMeta::default()).await
}

/// Summarises a cluster — its root first, then its members — as one source (ADR 0039 §5): the root's
/// summary with every member's rows appended, each member-owned row recorded in `claim_owners`.
/// `None` for an empty slice.
fn summarize_cluster(views: &[&SourceView], lookups: &SourceLookups) -> Option<SourceSummary> {
    let (root, members) = views.split_first()?;
    let mut summary = summarize(root, lookups);
    for view in members {
        let member = summarize(view, lookups);
        summary.merged.push(AggRef {
            human_id: member.human_id.clone(),
            id: view.source_id().map(|id| id.to_string()).unwrap_or_default(),
        });
        adopt(&mut summary, member);
    }
    summary.merged.sort_by(|x, y| x.id.cmp(&y.id));
    Some(summary)
}

/// Appends a member's rows to its root's summary, recording the member each row came from, and fills
/// what the root lacks from the member.
fn adopt(root: &mut SourceSummary, member: SourceSummary) {
    let owner = member.human_id.clone();
    let mut owned: Vec<String> = Vec::new();
    owned.extend(member.repositories.iter().map(|row| row.assertion_id.clone()));
    owned.extend(member.attributes.iter().map(|row| row.assertion_id.clone()));
    owned.extend(member.media.iter().map(|row| row.assertion_id.clone()));
    owned.extend(member.notes.iter().map(|row| row.assertion_id.clone()));
    for assertion_id in owned {
        root.claim_owners.insert(assertion_id, owner.clone());
    }
    root.title = root.title.take().or(member.title);
    root.author = root.author.take().or(member.author);
    root.pub_info = root.pub_info.take().or(member.pub_info);
    root.abbrev = root.abbrev.take().or(member.abbrev);
    root.repositories.extend(member.repositories);
    root.attributes.extend(member.attributes);
    for citation in member.citations {
        if !root
            .citations
            .iter()
            .any(|held| held.citation.id == citation.citation.id)
        {
            root.citations.push(citation);
        }
    }
    root.reliability = reliability(&root.citations);
    root.media.extend(member.media);
    root.notes.extend(member.notes);
    for tag in member.tags {
        if !root.tags.iter().any(|held| held.id == tag.id) {
            root.tags.push(tag);
        }
    }
    root.restrictions.extend(member.restrictions);
}

#[cfg(test)]
mod tests {
    use super::{NewSource, create_source, link_source_repository, list_sources, show_source};
    use crate::citation::{NewCitation, create_citation};
    use crate::config::{AppDefaults, IdFormats, OperatorConfig, WorkspaceDefaults};
    use crate::dto::{CitingContext, CitingKind};
    use crate::error::AppError;
    use crate::person::{NewPerson, add_person_citation, create_person};
    use crate::repository::{NewRepository, create_repository, show_repository};
    use crate::session::Session;
    use crate::use_case::{MutationMeta, Provenance};
    use crate::workspace::Workspace;
    use tempfile::TempDir;
    use uuid::Uuid;
    use vitni_core::enums::{EvidenceLevel, SourceMediaType};
    use vitni_core::ids::AgentId;
    use vitni_core::provenance::{Agent, AgentKind};

    fn operator() -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
            email: None,
        }
    }

    fn defaults() -> WorkspaceDefaults {
        WorkspaceDefaults {
            id_formats: IdFormats {
                person: "I%04d".to_owned(),
                family: "F%04d".to_owned(),
                place: "P%04d".to_owned(),
                source: "S%04d".to_owned(),
                citation: "C%04d".to_owned(),
                event: "E%04d".to_owned(),
                dna_test: "D%04d".to_owned(),
                dna_match: "X%04d".to_owned(),
                repository: "R%04d".to_owned(),
                note: "N%04d".to_owned(),
                media: "O%04d".to_owned(),
                research_note: "A%04d".to_owned(),
            },
            ..Default::default()
        }
    }

    fn session() -> Session {
        Session::new(Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
        })
    }

    async fn setup() -> (Workspace, Session, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
        let workspace = Workspace::open(&ws, &operator(), &defaults()).await.expect("open");
        (workspace, session(), dir)
    }

    #[tokio::test]
    async fn a_linked_repository_is_joined_with_name_and_call_number() {
        let (workspace, session, _dir) = setup().await;
        let repo = create_repository(
            &workspace,
            &session,
            NewRepository {
                human_id: None,
                name: Some("National Archives".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("repo");
        let source = create_source(
            &workspace,
            &session,
            NewSource {
                human_id: None,
                title: Some("1850 Census".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("source");
        link_source_repository(
            &workspace,
            &session,
            &source,
            &repo,
            Some("M432, roll 552".to_owned()),
            SourceMediaType::Film,
            MutationMeta::default(),
        )
        .await
        .expect("link");

        let summary = show_source(&workspace, &source).await.expect("show").expect("source");
        assert!(!summary.id.is_empty(), "the stable id is surfaced");
        assert_eq!(summary.repositories.len(), 1);
        let link = &summary.repositories[0];
        assert_eq!(link.name.as_deref(), Some("National Archives"));
        assert_eq!(link.call_number.as_deref(), Some("M432, roll 552"));
        assert_eq!(link.media_type, SourceMediaType::Film);
        assert_eq!(
            link.repository.as_ref().map(|r| r.human_id.as_str()),
            Some(repo.as_str())
        );
    }

    #[tokio::test]
    async fn citations_using_a_source_resolve_their_backing_records() {
        let (workspace, session, _dir) = setup().await;
        let source = create_source(
            &workspace,
            &session,
            NewSource {
                human_id: None,
                title: Some("Parish register".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("source");
        let citation = create_citation(
            &workspace,
            &session,
            NewCitation {
                human_id: None,
                source: source.clone(),
                page: Some("p. 14".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("citation");
        let person = create_person(
            &workspace,
            &session,
            NewPerson {
                human_id: None,
                name: None,
                evidence_level: EvidenceLevel::Conclusion,
                external_ids: Vec::new(),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("person");
        add_person_citation(&workspace, &session, &person, &citation, MutationMeta::default())
            .await
            .expect("attach citation");

        let summary = show_source(&workspace, &source).await.expect("show").expect("source");
        assert_eq!(summary.citations.len(), 1, "the citation using the source is listed");
        let row = &summary.citations[0];
        assert_eq!(row.citation.page.as_deref(), Some("p. 14"));
        assert_eq!(row.backers.len(), 1, "the citing person is found by the reverse index");
        assert_eq!(row.backers[0].kind, CitingKind::Person);
        assert_eq!(row.backers[0].human_id, person);
        assert!(matches!(row.backers[0].context, CitingContext::Record));
        assert_eq!(summary.reliability.citation_count, 1);
        assert_eq!(summary.reliability.record_count, 1);
    }

    #[tokio::test]
    async fn a_repository_lists_the_sources_it_holds_with_citation_counts() {
        let (workspace, session, _dir) = setup().await;
        let repo = create_repository(
            &workspace,
            &session,
            NewRepository {
                human_id: None,
                name: Some("Trinity Church".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("repo");
        let source = create_source(
            &workspace,
            &session,
            NewSource {
                human_id: None,
                title: Some("Marriage register".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("source");
        link_source_repository(
            &workspace,
            &session,
            &source,
            &repo,
            None,
            SourceMediaType::Book,
            MutationMeta::default(),
        )
        .await
        .expect("link");
        create_citation(
            &workspace,
            &session,
            NewCitation {
                human_id: None,
                source: source.clone(),
                page: None,
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("citation");

        let summary = show_repository(&workspace, &repo).await.expect("show").expect("repo");
        assert_eq!(summary.sources.len(), 1, "the held source is listed");
        assert_eq!(summary.sources[0].source.human_id, source);
        assert_eq!(summary.sources[0].title.as_deref(), Some("Marriage register"));
        assert_eq!(summary.sources[0].citation_count, 1);

        // also unused-source case: ensure list_sources joins without error
        let sources = list_sources(&workspace).await.expect("list");
        assert_eq!(sources.len(), 1);
    }

    #[tokio::test]
    async fn projected_notes_and_tags_appear_on_the_summary() {
        let (workspace, session, _dir) = setup().await;
        let source = create_source(
            &workspace,
            &session,
            NewSource {
                human_id: None,
                title: None,
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("source");
        let summary = show_source(&workspace, &source).await.expect("show").expect("source");
        assert!(summary.notes.is_empty());
        assert!(summary.tags.is_empty());
        assert!(summary.media.is_empty());
        assert_eq!(summary.reliability.citation_count, 0);
    }

    async fn bare_source(workspace: &Workspace, session: &Session) -> String {
        create_source(
            workspace,
            session,
            NewSource {
                human_id: None,
                title: Some("Register".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("source")
    }

    #[tokio::test]
    async fn setting_a_specific_human_id_round_trips_through_show() {
        let (workspace, session, _dir) = setup().await;
        let source = bare_source(&workspace, &session).await;

        let new_id = super::set_source_human_id(
            &workspace,
            &session,
            &source,
            Some("S0555".to_owned()),
            Provenance::default(),
        )
        .await
        .expect("rename");
        assert_eq!(new_id, "S0555");

        assert!(show_source(&workspace, &source).await.expect("show").is_none());
        let renamed = show_source(&workspace, "S0555").await.expect("show").expect("source");
        assert_eq!(renamed.human_id, "S0555");
    }

    #[tokio::test]
    async fn a_blank_human_id_regenerates_from_the_configured_format() {
        let (workspace, session, _dir) = setup().await;
        let source = create_source(
            &workspace,
            &session,
            NewSource {
                human_id: Some("S9000".to_owned()),
                title: Some("Register".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("source");

        let new_id = super::set_source_human_id(&workspace, &session, &source, None, Provenance::default())
            .await
            .expect("regenerate");
        assert_eq!(
            new_id, "S9001",
            "the next id from the S%04d format follows the existing max"
        );
    }

    #[tokio::test]
    async fn renaming_onto_an_existing_id_is_rejected() {
        let (workspace, session, _dir) = setup().await;
        let first = bare_source(&workspace, &session).await;
        let second = bare_source(&workspace, &session).await;

        let taken = super::set_source_human_id(
            &workspace,
            &session,
            &second,
            Some(first.clone()),
            Provenance::default(),
        )
        .await;
        assert!(matches!(taken, Err(AppError::HumanIdTaken(id)) if id == first));
    }
}
