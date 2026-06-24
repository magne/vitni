//! Media use-cases (ADR 0006): create, set path/checksum, assert date, add attribute/citation,
//! attach note, tag, show, and list.

use std::collections::{BTreeSet, HashMap};

use genealogy_core::citation::CitationView;
use genealogy_core::date::GenealogicalDate;
use genealogy_core::enums::Restriction;
use genealogy_core::ids::{CitationId, HumanId, MediaId, NoteId, TagId};
use genealogy_core::media::MediaView;
use genealogy_core::media::command::{MediaCommand, MediaCommandEnvelope};
use genealogy_core::media_path::MediaPath;
use genealogy_core::provenance::Confidence;
use genealogy_core::text::{Attribute, Url};
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::dto::{AggRef, CitationRef, UsingRecordRef, citation_refs, tag_refs};
use crate::error::AppError;
use crate::event::{DateParts, gregorian_date};
use crate::media_usage::MediaUsage;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a media object (the DTO the CLI renders), carrying its stable id and
/// the joined views the detail tabs render (the cross-aggregate-joins dependency note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSummary {
    /// The user-facing identifier (e.g. `O0001`).
    pub human_id: String,
    /// The stable `MediaId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The media's location rendered for display, if set.
    pub path: Option<String>,
    /// The media's MIME type (e.g. `image/jpeg`), if set.
    pub mime: Option<String>,
    /// The media's checksum, if set.
    pub checksum: Option<String>,
    /// The media's date, if asserted. Structured so the frontend localizes it (ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The recorded attributes (the File card's typed metadata).
    pub attributes: Vec<MediaAttributeRef>,
    /// The citations backing the media's claims (the Citations tab).
    pub citations: Vec<CitationRef>,
    /// The attached notes (the Notes tab).
    pub notes: Vec<AggRef>,
    /// The applied tags (the Tags tab), by name/colour/priority.
    pub tags: Vec<TagRef>,
    /// The records that reference this media (the Overview "Used by" card).
    pub used_by: Vec<UsingRecordRef>,
    /// The media's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// A typed attribute on a media object (the File card's metadata rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAttributeRef {
    /// The attribute name/type.
    pub attribute_type: String,
    /// The attribute value.
    pub value: String,
}

/// What to create a media object with (the auto/override `human_id` and an optional file path).
#[derive(Debug, Clone)]
pub struct NewMedia {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// An optional file path for an initial `SetPath`.
    pub path: Option<String>,
}

/// Creates a media object, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::MediaDomain`] if a domain rule
/// rejects the command, or a workspace/store error.
pub async fn create_media(workspace: &Workspace, session: &Session, new: NewMedia) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_media(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_media_human_id(&workspace.media_id_format()?).await?,
    };

    let media_id = session.new_media_id();
    let aggregate_id = media_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        MediaCommand::CreateMedia {
            media_id,
            human_id: HumanId::new(&human_id),
        },
    )
    .await?;

    if let Some(path) = new.path {
        execute(
            store,
            session,
            &aggregate_id,
            MediaCommand::SetPath {
                media_id,
                path: MediaPath::File(path),
            },
        )
        .await?;
    }

    Ok(human_id)
}

/// Sets (or changes) a media object's file path, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_file_path(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    path: String,
) -> Result<(), AppError> {
    set_media_path(workspace, session, human_id, MediaPath::File(path)).await
}

/// Sets (or changes) a media object's web reference, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_web_path(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    href: String,
) -> Result<(), AppError> {
    let path = MediaPath::Web(Url {
        url_type: None,
        href,
        description: None,
    });
    set_media_path(workspace, session, human_id, path).await
}

/// Sets (or changes) a media object's checksum, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_checksum(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    checksum: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::SetChecksum { media_id, checksum },
    )
    .await
}

/// Sets (or changes) a media object's MIME type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_mime(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    mime: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::SetMime { media_id, mime },
    )
    .await
}

/// Asserts a media object's date, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn assert_media_date(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    parts: DateParts,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::AssertDate {
            media_id,
            date: gregorian_date(parts),
        },
    )
    .await
}

/// Adds a typed attribute to a media object, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn add_media_attribute(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    attribute_type: String,
    value: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::AddAttribute {
            media_id,
            attribute: Attribute {
                attribute_type,
                value,
                citations: Vec::new(),
            },
        },
    )
    .await
}

/// Adds a citation (by its `human_id`) backing a media object's claims.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] / [`AppError::CitationNotFound`] if either is unknown, or a
/// workspace/store error.
pub async fn add_media_citation(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    citation_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::AddCitation { media_id, citation_id },
    )
    .await
}

/// Attaches a note (by note aggregate id) to a media object, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn attach_media_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::AttachNote { media_id, note_id },
    )
    .await
}

/// Applies (or removes) a tag on a media object, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn tag_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        MediaCommand::Untag { media_id, tag_id }
    } else {
        MediaCommand::Tag { media_id, tag_id }
    };
    execute(store, session, &media_id.to_string(), command).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Attaches a note (by its `human_id`) to a media object — the importer-facing wrapper.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_media_note(
    workspace: &Workspace,
    session: &Session,
    media_human_id: &str,
    note_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = use_case::resolve_id(
        store.find_note(note_human_id).await?,
        genealogy_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_media_note(workspace, session, media_human_id, note_id).await
}

/// Loads a single media object's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_media(workspace: &Workspace, human_id: &str) -> Result<Option<MediaSummary>, AppError> {
    let Some(view) = workspace.store().find_media(human_id).await? else {
        return Ok(None);
    };
    let lookups = MediaLookups::load(workspace).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every media object's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_media(workspace: &Workspace) -> Result<Vec<MediaSummary>, AppError> {
    let views = workspace.store().list_media().await?;
    let lookups = MediaLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// The lookups `summarize` needs to join a media object's attachments and back-references to the
/// other projections without a per-row query (the cross-aggregate join lives here — the app/db layer).
struct MediaLookups {
    citations: HashMap<CitationId, CitationRef>,
    notes: HashMap<NoteId, String>,
    tags: HashMap<TagId, TagRef>,
    usage: MediaUsage,
}

impl MediaLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        Ok(Self {
            citations: citation_refs(store).await?,
            notes: use_case::note_human_ids(store).await?,
            tags: tag_refs(store).await?,
            usage: MediaUsage::load(workspace).await?,
        })
    }
}

/// Sets the media path through the store.
async fn set_media_path(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    path: MediaPath,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::SetPath { media_id, path },
    )
    .await
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
/// Sets a media object's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::SetRestrictions { media_id, restrictions },
    )
    .await
}

async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: MediaCommand) -> Result<(), AppError> {
    let envelope = MediaCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_media(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Resolves a `human_id` to its aggregate [`MediaId`], or [`AppError::MediaNotFound`].
async fn resolve_media_id(store: &Store, human_id: &str) -> Result<MediaId, AppError> {
    use_case::resolve_id(store.find_media(human_id).await?, MediaView::media_id, || {
        AppError::MediaNotFound(human_id.to_owned())
    })
}

/// Resolves a citation `human_id` to its aggregate [`CitationId`], or [`AppError::CitationNotFound`].
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<CitationId, AppError> {
    use_case::resolve_id(store.find_citation(human_id).await?, CitationView::citation_id, || {
        AppError::CitationNotFound(human_id.to_owned())
    })
}

/// Renders the media location for display.
fn render_path(path: &MediaPath) -> String {
    match path {
        MediaPath::File(file) => file.clone(),
        MediaPath::Web(url) => url.href.clone(),
    }
}

/// Renders a [`MediaView`] into the frontend DTO, joining its attachments and back-references to the
/// other projections via `lookups`.
fn summarize(view: &MediaView, lookups: &MediaLookups) -> MediaSummary {
    let attributes = view
        .attributes()
        .into_iter()
        .map(|attribute| MediaAttributeRef {
            attribute_type: attribute.attribute_type.clone(),
            value: attribute.value.clone(),
        })
        .collect();
    let citations = view
        .citations()
        .into_iter()
        .filter_map(|id| lookups.citations.get(&id).cloned())
        .collect();
    let notes = view
        .notes()
        .into_iter()
        .filter_map(|id| {
            lookups.notes.get(&id).map(|human_id| AggRef {
                human_id: human_id.clone(),
                id: id.to_string(),
            })
        })
        .collect();
    let tags = view
        .tags()
        .into_iter()
        .filter_map(|id| lookups.tags.get(&id).cloned())
        .collect();
    let used_by = view.media_id().map(|id| lookups.usage.used_by(id)).unwrap_or_default();
    MediaSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id: view.media_id().map(|id| id.to_string()).unwrap_or_default(),
        path: view.path().map(render_path),
        mime: view.mime().map(ToOwned::to_owned),
        checksum: view.checksum().map(ToOwned::to_owned),
        date: view.date().cloned(),
        attributes,
        citations,
        notes,
        tags,
        used_by,
        restrictions: view.restrictions().clone(),
    }
}
