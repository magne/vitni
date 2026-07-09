//! Citation use-cases (ADR 0006): create (against a source), set page, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`CitationSummary`]. Creating a citation resolves the cited source's `human_id`
//! to its id (an [`AppError::SourceNotFound`] if absent); the core then *also* re-checks the source
//! exists against the projection via the aggregate's `Services` resolver, surfacing
//! [`CitationError::UnknownSource`](genealogy_core::citation::CitationError) — the §9 aggregate-tax
//! check (ADR 0004 §3).

use std::collections::{BTreeSet, HashMap};

use genealogy_core::citation::CitationView;
use genealogy_core::citation::command::{CitationCommand, CitationCommandEnvelope};
use genealogy_core::date::GenealogicalDate;
use genealogy_core::enums::Restriction;
use genealogy_core::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, SourceId, TagId};
use genealogy_core::media::MediaView;
use genealogy_core::note::NoteView;
use genealogy_core::provenance::{CitationRef, Confidence, EvidenceAnalysis};
use genealogy_core::source::SourceView;
use genealogy_core::text::{Attribute, MediaRef};
use genealogy_db::Store;
use uuid::Uuid;

use crate::dto::{AggRef, AttachedRef, MediaRefSummary};
use crate::error::AppError;
use crate::event::{DateParts, gregorian_date};
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// An applied tag. The user only ever sees the name, colour, and priority; the `id` is carried for
/// the detach command but is never rendered (data-model §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRef {
    /// The tag's aggregate id (a UUID string) — used to attach/detach; never shown to the user.
    pub id: String,
    /// The tag's name (the user-facing label).
    pub name: String,
    /// The tag's colour (a CSS colour string, e.g. `#e5534b`), if set — drives the chip's dot.
    pub color: Option<String>,
    /// The tag's sort priority, if set.
    pub priority: Option<i32>,
}

/// A typed attribute on a citation, with the `AssertionId` that introduced it — the target a per-row
/// Edit supersedes and a Retract retracts (ADR 0004 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationAttributeRef {
    /// The attribute's type / key.
    pub attribute_type: String,
    /// The attribute's value.
    pub value: String,
    /// The `AssertionId` (a UUID string) that introduced this attribute. Never rendered.
    pub assertion_id: String,
}

/// A frontend-neutral summary of a citation (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationSummary {
    /// The user-facing identifier (e.g. `C0001`).
    pub human_id: String,
    /// The cited source (its `human_id` + stable id), resolved from the projected `SourceId`, for
    /// display and navigation.
    pub source: Option<AggRef>,
    /// The page / locator within the source, if set.
    pub page: Option<String>,
    /// The date of the cited record. Structured so the frontend localizes it (ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The operator's confidence in this citation. Structured so the frontend localizes it.
    pub confidence: Option<Confidence>,
    /// The citation's Evidence Explained analysis (the three axes), if set. Structured so the
    /// frontend can render and localize each axis value.
    pub evidence_analysis: Option<EvidenceAnalysis>,
    /// The recorded attributes, in assertion order, each with the `AssertionId` that introduced it.
    pub attributes: Vec<CitationAttributeRef>,
    /// Media attached to this citation, with stable ids + the attach `AssertionId`, in assertion order.
    pub media: Vec<MediaRefSummary>,
    /// Notes attached to this citation, with the attach `AssertionId` (the Detach target), in
    /// assertion order.
    pub notes: Vec<AttachedRef>,
    /// The tags applied to this citation, by name + colour (never by id — data-model §9).
    pub tags: Vec<TagRef>,
    /// The citation's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// What to create a citation with (the auto/override `human_id`, the cited source, and a page).
#[derive(Debug, Clone)]
pub struct NewCitation {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// The cited source's `human_id` (e.g. `S0001`).
    pub source: String,
    /// An optional page / locator for an initial `SetPage`.
    pub page: Option<String>,
}

/// Creates a citation against a source, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::SourceNotFound`] if the cited
/// source does not exist, [`AppError::CitationDomain`] if a domain rule rejects the command (e.g.
/// `UnknownSource`), or a workspace/store error.
pub async fn create_citation(
    workspace: &Workspace,
    session: &Session,
    new: NewCitation,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_citation(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_citation_human_id(&workspace.citation_id_format()?).await?,
    };

    let source_id = resolve_source_id(store, &new.source).await?;
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;
    let citation_id = session.new_citation_id();
    let aggregate_id = citation_id.to_string();

    execute(
        store,
        session,
        &aggregate_id,
        CitationCommand::CreateCitation {
            citation_id,
            human_id: HumanId::new(&human_id),
            source_id,
        },
        provenance,
        citation_refs,
    )
    .await?;

    if let Some(page) = new.page {
        execute(
            store,
            session,
            &aggregate_id,
            CitationCommand::SetPage { citation_id, page },
            Provenance::default(),
            Vec::new(),
        )
        .await?;
    }

    Ok(human_id)
}

/// Creates a citation against an already-resolved [`SourceId`] with an already-allocated `human_id`,
/// returning its minted [`CitationId`].
///
/// The change-set use-case ([`crate::person_change_set`]) reuses this to create a pending citation
/// and keep the id so several of a person's assertions can cite the one new citation.
///
/// # Errors
///
/// [`AppError::CitationDomain`] on a domain rejection (e.g. `UnknownSource`), or a workspace/store
/// error.
pub(crate) async fn create_citation_returning_id(
    session: &Session,
    store: &Store,
    human_id: &str,
    source_id: SourceId,
    page: Option<String>,
    provenance: Provenance,
) -> Result<CitationId, AppError> {
    let citation_id = session.new_citation_id();
    let aggregate_id = citation_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        CitationCommand::CreateCitation {
            citation_id,
            human_id: HumanId::new(human_id),
            source_id,
        },
        provenance.clone(),
        Vec::new(),
    )
    .await?;
    if let Some(page) = page {
        execute(
            store,
            session,
            &aggregate_id,
            CitationCommand::SetPage { citation_id, page },
            provenance,
            Vec::new(),
        )
        .await?;
    }
    Ok(citation_id)
}

/// Resolves a source `human_id` to its aggregate [`SourceId`] — the crate-internal accessor the
/// change-set use-case reuses to point a pending citation at an existing source.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub(crate) async fn resolve_source_id_public(store: &Store, human_id: &str) -> Result<SourceId, AppError> {
    resolve_source_id(store, human_id).await
}

/// Sets (or changes) an existing citation's page, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or a workspace/store error.
pub async fn set_page(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    page: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    execute_citation_mutation(
        store,
        session,
        citation_id,
        CitationCommand::SetPage { citation_id, page },
        meta,
    )
    .await
}

/// Asserts the date of an existing citation's cited record, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or a workspace/store error.
pub async fn assert_citation_date(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    parts: DateParts,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    execute_citation_mutation(
        store,
        session,
        citation_id,
        CitationCommand::AssertDate {
            citation_id,
            date: gregorian_date(parts),
        },
        meta,
    )
    .await
}

/// Asserts a citation's date from an already-built [`GenealogicalDate`] (the full GEDCOM date
/// grammar, via [`build_genealogical_date`](crate::event::build_genealogical_date)).
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or a workspace/store error.
pub async fn assert_citation_date_value(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    date: GenealogicalDate,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    execute_citation_mutation(
        store,
        session,
        citation_id,
        CitationCommand::AssertDate { citation_id, date },
        meta,
    )
    .await
}

/// Sets (or changes) an existing citation's confidence, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or a workspace/store error.
pub async fn set_citation_confidence(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    confidence: Confidence,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    execute_citation_mutation(
        store,
        session,
        citation_id,
        CitationCommand::SetConfidence {
            citation_id,
            confidence,
        },
        meta,
    )
    .await
}

/// Sets (or changes) an existing citation's evidence analysis, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or a workspace/store error.
pub async fn set_citation_evidence_analysis(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    analysis: EvidenceAnalysis,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    execute_citation_mutation(
        store,
        session,
        citation_id,
        CitationCommand::SetEvidenceAnalysis { citation_id, analysis },
        meta,
    )
    .await
}

/// Adds a typed attribute to a citation, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or a workspace/store error.
pub async fn add_citation_attribute(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    attribute_type: String,
    value: String,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    execute_citation_mutation(
        store,
        session,
        citation_id,
        CitationCommand::AddAttribute {
            citation_id,
            attribute: Attribute {
                attribute_type,
                value,
                citations: Vec::new(),
            },
        },
        meta,
    )
    .await
}

/// Attaches a media object (by its `human_id`) to a citation, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`]/[`AppError::MediaNotFound`] if either record is absent, or a
/// workspace/store error.
pub async fn attach_citation_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    media_human_id: &str,
    caption: Option<String>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    let media_id = resolve_media_id(store, media_human_id).await?;
    execute_citation_mutation(
        store,
        session,
        citation_id,
        CitationCommand::AttachMedia {
            citation_id,
            media: MediaRef {
                media_id,
                crop: None,
                caption,
                citations: Vec::new(),
            },
        },
        meta,
    )
    .await
}

/// Attaches a note (by its `human_id`) to a citation, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`]/[`AppError::NoteNotFound`] if either record is absent, or a
/// workspace/store error.
pub async fn attach_citation_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_human_id: &str,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    let note_id = resolve_note_id(store, note_human_id).await?;
    execute_citation_mutation(
        store,
        session,
        citation_id,
        CitationCommand::AttachNote { citation_id, note_id },
        meta,
    )
    .await
}

/// Applies (or removes) a tag on a citation, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or a workspace/store error.
pub async fn tag_citation(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        CitationCommand::Untag { citation_id, tag_id }
    } else {
        CitationCommand::Tag { citation_id, tag_id }
    };
    execute_citation_mutation(store, session, citation_id, command, meta).await
}

/// Loads a single citation's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_citation(workspace: &Workspace, human_id: &str) -> Result<Option<CitationSummary>, AppError> {
    let store = workspace.store();
    let Some(view) = store.find_citation(human_id).await? else {
        return Ok(None);
    };
    let lookups = Lookups::load(store).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every citation's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_citations(workspace: &Workspace) -> Result<Vec<CitationSummary>, AppError> {
    let store = workspace.store();
    let views = store.list_citations().await?;
    let lookups = Lookups::load(store).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// Sets a citation's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    execute_citation_mutation(
        store,
        session,
        citation_id,
        CitationCommand::SetRestrictions {
            citation_id,
            restrictions,
        },
        meta,
    )
    .await
}

/// Sets (or changes) a citation's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case).
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if the citation is unknown, [`AppError::HumanIdTaken`] if the
/// requested id is already in use, or a workspace/store error.
pub async fn set_citation_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_citation(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_citation_human_id(&workspace.citation_id_format()?).await?,
    };
    execute(
        store,
        session,
        &citation_id.to_string(),
        CitationCommand::SetHumanId {
            citation_id,
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
    command: CitationCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = CitationCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_citation(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create citation mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`CitationCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_citation_mutation(
    store: &Store,
    session: &Session,
    citation_id: CitationId,
    command: CitationCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(citation_id, command, target);
    execute(
        store,
        session,
        &citation_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`CitationCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(citation_id: CitationId, command: CitationCommand, target: Option<AssertionId>) -> CitationCommand {
    match target {
        Some(target) => CitationCommand::SupersedeAssertion {
            citation_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
}

/// Resolves a source `human_id` to its aggregate [`SourceId`], or [`AppError::SourceNotFound`].
async fn resolve_source_id(store: &Store, human_id: &str) -> Result<SourceId, AppError> {
    use_case::resolve_id(store.find_source(human_id).await?, SourceView::source_id, || {
        AppError::SourceNotFound(human_id.to_owned())
    })
}

/// Resolves a citation `human_id` to its aggregate [`CitationId`], or [`AppError::CitationNotFound`].
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<CitationId, AppError> {
    use_case::resolve_id(store.find_citation(human_id).await?, CitationView::citation_id, || {
        AppError::CitationNotFound(human_id.to_owned())
    })
}

/// Resolves a media `human_id` to its aggregate [`MediaId`], or [`AppError::MediaNotFound`].
async fn resolve_media_id(store: &Store, human_id: &str) -> Result<MediaId, AppError> {
    use_case::resolve_id(store.find_media(human_id).await?, MediaView::media_id, || {
        AppError::MediaNotFound(human_id.to_owned())
    })
}

/// Resolves a note `human_id` to its aggregate [`NoteId`], or [`AppError::NoteNotFound`].
async fn resolve_note_id(store: &Store, human_id: &str) -> Result<NoteId, AppError> {
    use_case::resolve_id(store.find_note(human_id).await?, NoteView::note_id, || {
        AppError::NoteNotFound(human_id.to_owned())
    })
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`]. The id is
/// supplied by the caller (resolved from a tag the user picked by name); it is never shown to the user.
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// The lookups `summarize` needs to resolve a citation's cited source and attachments without a
/// per-row query: source/media/notes by `human_id`, and tags by **name** (tags carry no `human_id`
/// and their aggregate id is never surfaced — data-model §9).
struct Lookups {
    sources: HashMap<SourceId, String>,
    media: HashMap<MediaId, String>,
    notes: HashMap<NoteId, String>,
    tags: HashMap<TagId, TagRef>,
}

impl Lookups {
    async fn load(store: &Store) -> Result<Self, AppError> {
        Ok(Self {
            sources: source_human_ids(store).await?,
            media: use_case::media_human_ids(store).await?,
            notes: use_case::note_human_ids(store).await?,
            tags: tag_labels(store).await?,
        })
    }
}

/// Builds a `TagId -> TagRef` lookup from the Tag projection, to render applied tags by name/colour/
/// priority (never by id).
async fn tag_labels(store: &Store) -> Result<HashMap<TagId, TagRef>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_tags().await? {
        if let (Some(id), Some(name)) = (view.tag_id(), view.name()) {
            map.insert(
                id,
                TagRef {
                    id: id.to_string(),
                    name: name.to_owned(),
                    color: view.color().map(ToOwned::to_owned),
                    priority: view.priority(),
                },
            );
        }
    }
    Ok(map)
}

/// Builds a `SourceId -> human_id` lookup from the Source projection, to render the cited source.
async fn source_human_ids(store: &Store) -> Result<HashMap<SourceId, String>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_sources().await? {
        if let (Some(id), Some(human_id)) = (view.source_id(), view.human_id()) {
            map.insert(id, human_id.as_str().to_owned());
        }
    }
    Ok(map)
}

/// Renders a [`CitationView`] into the frontend DTO, resolving the cited source and attachments.
fn summarize(view: &CitationView, lookups: &Lookups) -> CitationSummary {
    let source = view.source_id().and_then(|id| {
        lookups.sources.get(&id).map(|human_id| AggRef {
            human_id: human_id.clone(),
            id: id.to_string(),
        })
    });
    CitationSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        source,
        page: view.page().map(ToOwned::to_owned),
        date: view.date().cloned(),
        confidence: view.confidence().copied(),
        evidence_analysis: view.evidence_analysis().copied(),
        attributes: view
            .attributes_with_assertions()
            .iter()
            .map(|attributed| CitationAttributeRef {
                attribute_type: attributed.value.attribute_type.clone(),
                value: attributed.value.value.clone(),
                assertion_id: attributed.assertion_id.to_string(),
            })
            .collect(),
        media: view
            .media_with_assertions()
            .iter()
            .filter_map(|attributed| {
                let media = &attributed.value;
                lookups.media.get(&media.media_id).map(|human_id| MediaRefSummary {
                    human_id: human_id.clone(),
                    id: media.media_id.to_string(),
                    caption: media.caption.clone(),
                    assertion_id: attributed.assertion_id.to_string(),
                })
            })
            .collect(),
        notes: view
            .notes_with_assertions()
            .iter()
            .filter_map(|attributed| {
                lookups.notes.get(&attributed.value).map(|human_id| AttachedRef {
                    human_id: human_id.clone(),
                    id: attributed.value.to_string(),
                    assertion_id: attributed.assertion_id.to_string(),
                })
            })
            .collect(),
        tags: view
            .tags()
            .into_iter()
            .filter_map(|id| lookups.tags.get(&id).cloned())
            .collect(),
        restrictions: view.restrictions().clone(),
    }
}
