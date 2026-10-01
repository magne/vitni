//! Repository use-cases (ADR 0006): create, set type/name, add address/url, attach note, tag,
//! show, and list.
//!
//! Each builds a command + [`AssertionMeta`](vitni_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`RepositorySummary`]. `human_id` is auto-allocated using the workspace's
//! configured format, or validated when supplied (ADR 0005).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use vitni_core::address::Address;
use vitni_core::enums::{RepositoryType, Restriction};
use vitni_core::ids::{AssertionId, HumanId, NoteId, RepositoryId, SourceId, TagId};
use vitni_core::provenance::EvidenceRef;
use vitni_core::repository::RepositoryView;
use vitni_core::repository::command::{RepositoryCommand, RepositoryCommandEnvelope};
use vitni_core::text::Url;
use vitni_db::Store;

use crate::citation::TagRef;
use crate::dto::{AggRef, AttachedRef, SourceLinkRef, tag_refs};
use crate::error::AppError;
use crate::identity::{self, IdentityDecision, PairDecision, RepositoryClusters};
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// An address recorded on a repository, with the `AssertionId` that introduced it — the target a
/// per-card Edit supersedes and a Retract retracts (ADR 0004 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryAddressRef {
    /// The postal address (street · locality · region · …).
    pub address: Address,
    /// The `AssertionId` (a UUID string) that introduced this address. Never rendered.
    pub assertion_id: String,
}

/// A URL recorded on a repository, with the `AssertionId` that introduced it — the target a per-row
/// Edit supersedes and a Retract retracts (ADR 0004 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryUrlRef {
    /// The URL (type · href · description).
    pub url: Url,
    /// The `AssertionId` (a UUID string) that introduced this URL. Never rendered.
    pub assertion_id: String,
}

/// A frontend-neutral summary of a repository (the DTO the CLI and UI render). References to held
/// sources carry their stable ids alongside their `human_id`s (the cross-aggregate-joins note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySummary {
    /// The user-facing identifier (e.g. `R0001`).
    pub human_id: String,
    /// The repository's stable `RepositoryId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The repository's type. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub repository_type: Option<RepositoryType>,
    /// The repository's name, if set.
    pub name: Option<String>,
    /// The recorded postal addresses, in assertion order, each with the `AssertionId` that
    /// introduced it.
    pub addresses: Vec<RepositoryAddressRef>,
    /// The recorded URLs, in assertion order, each with the `AssertionId` that introduced it.
    pub urls: Vec<RepositoryUrlRef>,
    /// The sources held by this repository, joined to the Source projection, in `human_id` order.
    pub sources: Vec<SourceLinkRef>,
    /// Notes attached to the repository, with the attach `AssertionId` (the Detach target), in
    /// assertion order.
    pub notes: Vec<AttachedRef>,
    /// Tags applied to the repository, by name + colour (never by id — data-model §9).
    pub tags: Vec<TagRef>,
    /// The repository's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
    /// Every repository record merged into this repository's cluster, directly or through another member
    /// (ADR 0039 §4), in id order.
    pub merged: Vec<AggRef>,
    /// The `human_id` of the member each member row came from, by the row's `AssertionId`: its edit or
    /// retraction is written to that member's stream (ADR 0039 §5). A row absent here is the
    /// repository's own. Never rendered as a key; see [`owner_of`](Self::owner_of).
    pub claim_owners: BTreeMap<String, String>,
}

impl RepositorySummary {
    /// The `human_id` of the record that owns the row introduced by `assertion_id`: the member it came
    /// from, or this repository for its own rows.
    #[must_use]
    pub fn owner_of(&self, assertion_id: &str) -> &str {
        self.claim_owners
            .get(assertion_id)
            .map_or(self.human_id.as_str(), String::as_str)
    }
}

/// What to create a repository with (the auto/override `human_id` and an optional name).
#[derive(Debug, Clone)]
pub struct NewRepository {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// An optional name for an initial `SetName`.
    pub name: Option<String>,
}

/// Creates a repository, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::RepositoryDomain`] if a
/// domain rule rejects the command, or a workspace/store error.
pub async fn create_repository(
    workspace: &Workspace,
    session: &Session,
    new: NewRepository,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let follow_up = provenance.follow_up();
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_repository(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => {
            store
                .next_repository_human_id(&workspace.repository_id_format()?)
                .await?
        }
    };
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let repository_id = session.new_repository_id();
    let aggregate_id = repository_id.to_string();

    execute(
        store,
        session,
        &aggregate_id,
        RepositoryCommand::CreateRepository {
            repository_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        citation_refs,
    )
    .await?;

    if let Some(name) = new.name {
        execute(
            store,
            session,
            &aggregate_id,
            RepositoryCommand::SetName { repository_id, name },
            follow_up,
            Vec::new(),
        )
        .await?;
    }

    Ok(human_id)
}

/// Sets (or changes) a repository's type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn set_repository_type(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    repository_type: RepositoryType,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute_repository_mutation(
        store,
        session,
        repository_id,
        RepositoryCommand::SetRepositoryType {
            repository_id,
            repository_type,
        },
        meta,
    )
    .await
}

/// Sets (or changes) a repository's name, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, [`AppError::RepositoryDomain`] if
/// the name is empty, or a workspace/store error.
pub async fn set_repository_name(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    name: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute_repository_mutation(
        store,
        session,
        repository_id,
        RepositoryCommand::SetName { repository_id, name },
        meta,
    )
    .await
}

/// Adds a postal address to a repository, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn add_repository_address(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    address: Address,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute_repository_mutation(
        store,
        session,
        repository_id,
        RepositoryCommand::AddAddress { repository_id, address },
        meta,
    )
    .await
}

/// Adds a URL to a repository, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn add_repository_url(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    url: Url,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute_repository_mutation(
        store,
        session,
        repository_id,
        RepositoryCommand::AddUrl { repository_id, url },
        meta,
    )
    .await
}

/// Attaches a note (by note aggregate id) to a repository, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn attach_repository_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute_repository_mutation(
        store,
        session,
        repository_id,
        RepositoryCommand::AttachNote { repository_id, note_id },
        meta,
    )
    .await
}

/// Applies (or removes) a tag on a repository, identified by `human_id`. A removed tag is untagged on
/// every record of the cluster that holds it (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn tag_repository(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    if !remove {
        let command = RepositoryCommand::Tag { repository_id, tag_id };
        return execute_repository_mutation(store, session, repository_id, command, meta).await;
    }
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    for record in identity::cluster_records(store, repository_id).await? {
        let Some(id) = record.repository_id() else { continue };
        if id != repository_id && record.tags().contains(&tag_id) {
            let command = RepositoryCommand::Untag {
                repository_id: id,
                tag_id,
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
    }
    let command = RepositoryCommand::Untag { repository_id, tag_id };
    execute_repository_mutation(store, session, repository_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// The outcome of [`merge_repositories`]: the survivor's refreshed summary and the merged repository's
/// `human_id`.
///
/// The merge is a same-as link on the survivor (ADR 0039 §1): no record that names the merged repository
/// is rewritten. Every reader resolves those references to the cluster's root instead (ADR 0039 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryMergeResult {
    /// The survivor's summary after the merge, composed with every record now in its cluster.
    pub survivor: RepositorySummary,
    /// The merged repository's `human_id` (its own record/stream is untouched).
    pub merged_human_id: String,
}

/// Merges `merged_human_id`'s cluster into `surviving_human_id`'s, recording a same-as link
/// (ADR 0039 §1). Both records resolve to their cluster roots first (§4), and one `RepositoriesMerged`
/// event is emitted on the surviving root's stream, carrying the decision's provenance and assessment.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if either `human_id` does not resolve; [`AppError::RepositoryDomain`] with
/// `MergeConflict` if they resolve to the same repository, or `IdentityDecided` if the two are already one
/// cluster or a record of one cluster is distinguished from a record of the other; or a store error.
pub async fn merge_repositories(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<RepositoryMergeResult, AppError> {
    let pair =
        identity::merge::<RepositoryView>(workspace, session, surviving_human_id, merged_human_id, decision).await?;
    repository_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// Undoes every live distinction between the two repository clusters, then merges them (ADR 0039 §4).
///
/// # Errors
///
/// As [`merge_repositories`], except that a distinction between the clusters no longer refuses the merge.
pub async fn undo_repository_distinction_and_merge(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<RepositoryMergeResult, AppError> {
    let pair = identity::undo_distinction_and_merge::<RepositoryView>(
        workspace,
        session,
        surviving_human_id,
        merged_human_id,
        decision,
    )
    .await?;
    repository_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// The survivor's composed summary after a merge.
async fn repository_merge_result(
    workspace: &Workspace,
    survivor_human_id: &str,
    merged_human_id: &str,
) -> Result<RepositoryMergeResult, AppError> {
    let survivor = show_repository(workspace, survivor_human_id)
        .await?
        .ok_or_else(|| AppError::RepositoryNotFound(survivor_human_id.to_owned()))?;
    Ok(RepositoryMergeResult {
        survivor,
        merged_human_id: merged_human_id.to_owned(),
    })
}

/// Records that the two repositories are different (ADR 0039 §1), so neither cluster is proposed as a
/// duplicate of the other again. One `RepositoriesDistinguished` event is emitted on the first root's
/// stream; undoing it lifts the decision.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if either `human_id` does not resolve; [`AppError::RepositoryDomain`] with
/// `DistinctFromItself` if they resolve to the same repository, or `IdentityDecided` if the two are already
/// one cluster or already distinguished; or a store error.
pub async fn distinguish_repositories(
    workspace: &Workspace,
    session: &Session,
    repository_human_id: &str,
    other_human_id: &str,
    decision: IdentityDecision,
) -> Result<(), AppError> {
    identity::distinguish::<RepositoryView>(workspace, session, repository_human_id, other_human_id, decision).await
}

/// The live identity decision between the clusters of two repositories, or `None` when the pair is
/// undecided (ADR 0039 §4).
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if either `human_id` does not resolve, or a store error.
pub async fn repository_pair_decision(
    workspace: &Workspace,
    first_human_id: &str,
    other_human_id: &str,
) -> Result<Option<PairDecision>, AppError> {
    identity::pair_decision::<RepositoryView>(workspace, first_human_id, other_human_id).await
}

/// The `human_id` of the record of `human_id`'s repository cluster whose stream holds the live assertion
/// `assertion_id` — where an edit or retraction of that row is written (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if `human_id` is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or a store error.
pub async fn repository_claim_owner(
    workspace: &Workspace,
    human_id: &str,
    assertion_id: &str,
) -> Result<String, AppError> {
    identity::claim_owner::<RepositoryView>(workspace, human_id, assertion_id).await
}

/// Loads a single repository's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_repository(workspace: &Workspace, human_id: &str) -> Result<Option<RepositorySummary>, AppError> {
    let store = workspace.store();
    let Some(repository_id) = store
        .find_repository(human_id)
        .await?
        .and_then(|view| view.repository_id())
    else {
        return Ok(None);
    };
    let views = identity::cluster_records(store, repository_id).await?;
    let lookups = RepositoryLookups::load(workspace).await?;
    Ok(summarize_cluster(&views.iter().collect::<Vec<_>>(), &lookups))
}

/// Lists every repository's summary, ordered by `human_id`. A merged record is listed once, as its cluster's
/// root (ADR 0039 §5).
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_repositories(workspace: &Workspace) -> Result<Vec<RepositorySummary>, AppError> {
    let store = workspace.store();
    let views = store.list_repositories().await?;
    let clusters = RepositoryClusters::load(store).await?;
    let lookups = RepositoryLookups::load(workspace).await?;
    let mut summaries = Vec::with_capacity(views.len());
    for cluster in identity::group_clusters(&views, &clusters) {
        summaries.extend(summarize_cluster(&cluster, &lookups));
    }
    Ok(summaries)
}

/// Sets a repository's privacy restrictions (GEDCOM `RESN` — data-model §6). A merged member restricted
/// beyond the new set is narrowed to it first, so the cluster reads with exactly `restrictions`
/// (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    for record in identity::cluster_records(store, repository_id).await? {
        let Some(id) = record.repository_id() else { continue };
        if id == repository_id || record.restrictions().is_subset(&restrictions) {
            continue;
        }
        let narrowed = record.restrictions().intersection(&restrictions).copied().collect();
        let command = RepositoryCommand::SetRestrictions {
            repository_id: id,
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
    execute_repository_mutation(
        store,
        session,
        repository_id,
        RepositoryCommand::SetRestrictions {
            repository_id,
            restrictions,
        },
        meta,
    )
    .await
}

/// Sets (or changes) a repository's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case).
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if the repository is unknown, [`AppError::HumanIdTaken`] if the
/// requested id is already in use, or a workspace/store error.
pub async fn set_repository_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_repository(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => {
            store
                .next_repository_human_id(&workspace.repository_id_format()?)
                .await?
        }
    };
    execute(
        store,
        session,
        &repository_id.to_string(),
        RepositoryCommand::SetHumanId {
            repository_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        Vec::new(),
    )
    .await?;
    Ok(human_id)
}

/// Executes one command through the store, stamping it with `provenance` (the operator's surety and
/// rationale) and `citations` (`EventContext.citations` — data-model §8), and maps the outcome to
/// [`AppError`].
pub(crate) async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: RepositoryCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = RepositoryCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    let Some(envelope) = crate::origin_gate::gate(store, session, aggregate_id, envelope).await? else {
        return Ok(());
    };
    store
        .execute_repository(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create repository mutation, applying the operator-intent [`MutationMeta`]:
/// resolves the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`RepositoryCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004
/// §2).
async fn execute_repository_mutation(
    store: &Store,
    session: &Session,
    repository_id: RepositoryId,
    command: RepositoryCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(repository_id, command, target);
    execute(
        store,
        session,
        &repository_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`RepositoryCommand::SupersedeAssertion`] against `target` when superseding,
/// or returns it unchanged for a plain assertion.
fn superseded(
    repository_id: RepositoryId,
    command: RepositoryCommand,
    target: Option<AssertionId>,
) -> RepositoryCommand {
    match target {
        Some(target) => RepositoryCommand::SupersedeAssertion {
            repository_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
}

/// Resolves a `human_id` to its aggregate [`RepositoryId`], or [`AppError::RepositoryNotFound`].
async fn resolve_repository_id(store: &Store, human_id: &str) -> Result<RepositoryId, AppError> {
    use_case::resolve_id(
        store.find_repository(human_id).await?,
        RepositoryView::repository_id,
        || AppError::RepositoryNotFound(human_id.to_owned()),
    )
}

/// The lookups `summarize` needs to join a repository's held sources and its note/tag attachments to
/// the other projections without a per-row query (the join lives in this layer).
struct RepositoryLookups {
    sources_by_repository: HashMap<RepositoryId, Vec<SourceLinkRef>>,
    notes: HashMap<NoteId, use_case::NoteLookup>,
    tags: HashMap<TagId, TagRef>,
}

impl RepositoryLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        // A source held by a merged repository is listed under the repository's root, and a merged
        // source names its own root (ADR 0039 §5). Its citation count is its cluster's, each merged
        // citation counted once, as the Source › Citations tab lists them.
        let sources = crate::identity::SourceReferences::load(store).await?;
        let citation_clusters = crate::identity::CitationClusters::load(store).await?;
        let mut citation_counts: HashMap<SourceId, usize> = HashMap::new();
        for view in store.list_citations().await? {
            if view.citation_id().is_some_and(|id| citation_clusters.is_member(id)) {
                continue;
            }
            if let Some(source_id) = view.source_id() {
                let (root, _) = sources.resolve(source_id);
                *citation_counts.entry(root).or_default() += 1;
            }
        }
        let mut sources_by_repository: HashMap<RepositoryId, Vec<SourceLinkRef>> = HashMap::new();
        for view in store.list_sources().await? {
            let Some(source_id) = view.source_id() else {
                continue;
            };
            let (source_id, human_id) = sources.resolve(source_id);
            let title = view.title().map(ToOwned::to_owned);
            let citation_count = citation_counts.get(&source_id).copied().unwrap_or_default();
            for repo_ref in view.repositories() {
                sources_by_repository
                    .entry(repo_ref.repository_id)
                    .or_default()
                    .push(SourceLinkRef {
                        source: AggRef {
                            human_id: human_id.clone(),
                            id: source_id.to_string(),
                        },
                        title: title.clone(),
                        call_number: repo_ref.call_number.clone(),
                        media_type: repo_ref.media_type.clone(),
                        citation_count,
                    });
            }
        }
        RepositoryClusters::load(store).await?.fold(&mut sources_by_repository);
        Ok(Self {
            sources_by_repository,
            notes: use_case::note_lookups(store).await?,
            tags: tag_refs(store).await?,
        })
    }
}

/// Renders a [`RepositoryView`] into the frontend DTO, joining the sources it holds and its
/// note/tag attachments via `lookups`.
fn summarize(view: &RepositoryView, lookups: &RepositoryLookups) -> RepositorySummary {
    let repository_id = view.repository_id();
    let sources = repository_id
        .and_then(|id| lookups.sources_by_repository.get(&id))
        .cloned()
        .unwrap_or_default();
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
    let addresses = view
        .addresses_with_assertions()
        .iter()
        .map(|attributed| RepositoryAddressRef {
            address: attributed.value.clone(),
            assertion_id: attributed.assertion_id.to_string(),
        })
        .collect();
    let urls = view
        .urls_with_assertions()
        .iter()
        .map(|attributed| RepositoryUrlRef {
            url: attributed.value.clone(),
            assertion_id: attributed.assertion_id.to_string(),
        })
        .collect();
    let tags = view
        .tags()
        .into_iter()
        .filter_map(|id| lookups.tags.get(&id).cloned())
        .collect();
    RepositorySummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id: repository_id.map(|id| id.to_string()).unwrap_or_default(),
        repository_type: view.repository_type().cloned(),
        name: view.name().map(ToOwned::to_owned),
        addresses,
        urls,
        sources,
        notes,
        tags,
        restrictions: view.restrictions().clone(),
        merged: Vec::new(),
        claim_owners: BTreeMap::new(),
    }
}

/// Attaches a note (by its `human_id`) to a repository — the importer-facing wrapper.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_repository_note(
    workspace: &Workspace,
    session: &Session,
    repository_human_id: &str,
    note_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = use_case::resolve_id(
        store.find_note(note_human_id).await?,
        vitni_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_repository_note(
        workspace,
        session,
        repository_human_id,
        note_id,
        MutationMeta::default(),
    )
    .await
}

/// Summarises a cluster — its root first, then its members — as one repository (ADR 0039 §5): the root's
/// summary with every member's rows appended, each member-owned row recorded in `claim_owners`.
/// `None` for an empty slice.
fn summarize_cluster(views: &[&RepositoryView], lookups: &RepositoryLookups) -> Option<RepositorySummary> {
    let (root, members) = views.split_first()?;
    let mut summary = summarize(root, lookups);
    for view in members {
        let member = summarize(view, lookups);
        summary.merged.push(AggRef {
            human_id: member.human_id.clone(),
            id: view.repository_id().map(|id| id.to_string()).unwrap_or_default(),
        });
        adopt(&mut summary, member);
    }
    summary.merged.sort_by(|x, y| x.id.cmp(&y.id));
    Some(summary)
}

/// Appends a member's rows to its root's summary, recording the member each row came from, and fills
/// what the root lacks from the member.
fn adopt(root: &mut RepositorySummary, member: RepositorySummary) {
    let owner = member.human_id.clone();
    let mut owned: Vec<String> = Vec::new();
    owned.extend(member.addresses.iter().map(|row| row.assertion_id.clone()));
    owned.extend(member.urls.iter().map(|row| row.assertion_id.clone()));
    owned.extend(member.notes.iter().map(|row| row.assertion_id.clone()));
    for assertion_id in owned {
        root.claim_owners.insert(assertion_id, owner.clone());
    }
    root.repository_type = root.repository_type.take().or(member.repository_type);
    root.name = root.name.take().or(member.name);
    root.addresses.extend(member.addresses);
    root.urls.extend(member.urls);
    for source in member.sources {
        if !root.sources.iter().any(|held| held.source.id == source.source.id) {
            root.sources.push(source);
        }
    }
    root.notes.extend(member.notes);
    for tag in member.tags {
        if !root.tags.iter().any(|held| held.id == tag.id) {
            root.tags.push(tag);
        }
    }
    root.restrictions.extend(member.restrictions);
}
