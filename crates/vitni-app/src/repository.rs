//! Repository use-cases (ADR 0006): create, set type/name, add address/url, attach note, tag,
//! show, and list.
//!
//! Each builds a command + [`AssertionMeta`](vitni_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`RepositorySummary`]. `human_id` is auto-allocated using the workspace's
//! configured format, or validated when supplied (ADR 0005).

use std::collections::{BTreeSet, HashMap};

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
            Provenance::default(),
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

/// Applies (or removes) a tag on a repository, identified by `human_id`.
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
    let command = if remove {
        RepositoryCommand::Untag { repository_id, tag_id }
    } else {
        RepositoryCommand::Tag { repository_id, tag_id }
    };
    execute_repository_mutation(store, session, repository_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Loads a single repository's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_repository(workspace: &Workspace, human_id: &str) -> Result<Option<RepositorySummary>, AppError> {
    let Some(view) = workspace.store().find_repository(human_id).await? else {
        return Ok(None);
    };
    let lookups = RepositoryLookups::load(workspace).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every repository's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_repositories(workspace: &Workspace) -> Result<Vec<RepositorySummary>, AppError> {
    let views = workspace.store().list_repositories().await?;
    let lookups = RepositoryLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// Sets a repository's privacy restrictions (GEDCOM `RESN` — data-model §6).
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
async fn execute(
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
        let mut citation_counts: HashMap<SourceId, usize> = HashMap::new();
        for view in store.list_citations().await? {
            if let Some(source_id) = view.source_id() {
                *citation_counts.entry(source_id).or_default() += 1;
            }
        }
        let mut sources_by_repository: HashMap<RepositoryId, Vec<SourceLinkRef>> = HashMap::new();
        for view in store.list_sources().await? {
            let (Some(source_id), Some(human_id)) = (view.source_id(), view.human_id()) else {
                continue;
            };
            let title = view.title().map(ToOwned::to_owned);
            let citation_count = citation_counts.get(&source_id).copied().unwrap_or_default();
            for repo_ref in view.repositories() {
                sources_by_repository
                    .entry(repo_ref.repository_id)
                    .or_default()
                    .push(SourceLinkRef {
                        source: AggRef {
                            human_id: human_id.as_str().to_owned(),
                            id: source_id.to_string(),
                        },
                        title: title.clone(),
                        call_number: repo_ref.call_number.clone(),
                        media_type: repo_ref.media_type.clone(),
                        citation_count,
                    });
            }
        }
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
                id: attributed.value.to_string(),
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
