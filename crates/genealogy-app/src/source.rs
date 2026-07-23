//! Source use-cases (ADR 0006): create, set title, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`SourceSummary`]. `human_id` is auto-allocated using the workspace's configured
//! format, or validated when supplied (ADR 0005).

use std::collections::{BTreeSet, HashMap};

use genealogy_core::enums::{Restriction, SourceMediaType};
use genealogy_core::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, RepositoryId, SourceId, TagId};
use genealogy_core::provenance::EvidenceRef;
use genealogy_core::provenance::{EvidenceAnalysis, EvidenceKind, InformationKind, SourceQuality};
use genealogy_core::repo_ref::RepoRef;
use genealogy_core::repository::RepositoryView;
use genealogy_core::source::SourceView;
use genealogy_core::source::command::{SourceCommand, SourceCommandEnvelope};
use genealogy_core::source::error::SourceError;
use genealogy_core::text::{Attribute, MediaRef};
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::citation_usage::CitationUsage;
use crate::dto::{
    AggRef, AttachedRef, CitationRef, MediaLookup, MediaRefSummary, RepositoryLinkRef, SourceCitationRef,
    SourceReliability, citation_refs, media_lookups, repository_refs, tag_refs,
};
use crate::error::AppError;
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
            Provenance::default(),
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

/// Applies (or removes) a tag on a source, identified by `human_id`.
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
    let command = if remove {
        SourceCommand::Untag { source_id, tag_id }
    } else {
        SourceCommand::Tag { source_id, tag_id }
    };
    execute_source_mutation(store, session, source_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Loads a single source's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_source(workspace: &Workspace, human_id: &str) -> Result<Option<SourceSummary>, AppError> {
    let Some(view) = workspace.store().find_source(human_id).await? else {
        return Ok(None);
    };
    let lookups = SourceLookups::load(workspace).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every source's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_sources(workspace: &Workspace) -> Result<Vec<SourceSummary>, AppError> {
    let views = workspace.store().list_sources().await?;
    let lookups = SourceLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// Sets a source's privacy restrictions (GEDCOM `RESN` — data-model §6).
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
async fn execute(
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
    repositories: HashMap<RepositoryId, (String, Option<String>)>,
    citations: HashMap<CitationId, CitationRef>,
    citations_by_source: HashMap<SourceId, Vec<CitationId>>,
    media: HashMap<MediaId, MediaLookup>,
    notes: HashMap<NoteId, String>,
    tags: HashMap<TagId, TagRef>,
    usage: CitationUsage,
}

impl SourceLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        let mut citations_by_source: HashMap<SourceId, Vec<CitationId>> = HashMap::new();
        for view in store.list_citations().await? {
            if let (Some(citation_id), Some(source_id)) = (view.citation_id(), view.source_id()) {
                citations_by_source.entry(source_id).or_default().push(citation_id);
            }
        }
        Ok(Self {
            repositories: repository_refs(store).await?,
            citations: citation_refs(store).await?,
            citations_by_source,
            media: media_lookups(store).await?,
            notes: use_case::note_human_ids(store).await?,
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
                repository: info.map(|(human_id, _)| AggRef {
                    human_id: human_id.clone(),
                    id: repo_ref.repository_id.to_string(),
                }),
                name: info.and_then(|(_, name)| name.clone()),
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
            lookups.notes.get(&attributed.value).map(|human_id| AttachedRef {
                human_id: human_id.clone(),
                id: attributed.value.to_string(),
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
        genealogy_core::media::MediaView::media_id,
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
        genealogy_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_source_note(workspace, session, source_human_id, note_id, MutationMeta::default()).await
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
    use genealogy_core::enums::{EvidenceLevel, SourceMediaType};
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind};
    use tempfile::TempDir;
    use uuid::Uuid;

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
