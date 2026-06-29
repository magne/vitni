//! Place use-cases (ADR 0006): create, set type, add name, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`PlaceSummary`] (never a `PlaceView`, cqrs-es, or sqlx type). `human_id` is
//! auto-allocated using the workspace's configured format, or validated when supplied (ADR 0005).

use std::collections::{BTreeSet, HashMap};

use genealogy_core::citation::CitationView;
use genealogy_core::date::GenealogicalDate;
use genealogy_core::enums::{PlaceType, Restriction};
use genealogy_core::geo::GeoCoordinates;
use genealogy_core::ids::{CitationId, HumanId, MediaId, NoteId, PlaceId, TagId};
use genealogy_core::place::PlaceView;
use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use genealogy_core::place_name::PlaceName;
use genealogy_core::place_ref::PlaceRef;
use genealogy_core::provenance::Confidence;
use genealogy_core::text::MediaRef;
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::dto::{AggRef, CitationRef, MediaRefSummary, citation_refs, media_refs, tag_refs};
use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// An asserted place name, with its language/date metadata and the assertion's provenance (the
/// evidence-first cue for the Names tab — data-model §14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceNameRef {
    /// The name text.
    pub text: String,
    /// The BCP-47 language tag the name is in, if recorded.
    pub language: Option<String>,
    /// The date the name was in use (structured so the frontend localizes it — ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The operator's surety in the name assertion.
    pub confidence: Confidence,
    /// How many citations back the name assertion.
    pub source_count: usize,
}

/// An enclosing place, joined to the place projection: its name/type for display, the stable id for
/// navigation, the dated link, and the assertion's surety (the jurisdiction chain — data-model §14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceEnclosingRef {
    /// The enclosing place's user-facing identifier (e.g. `P0001`).
    pub human_id: String,
    /// The enclosing place's stable `PlaceId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The enclosing place's primary name, if resolved.
    pub name: Option<String>,
    /// The enclosing place's type, if resolved.
    pub place_type: Option<PlaceType>,
    /// The date the enclosing relationship was valid (structured so the frontend localizes it).
    pub date: Option<GenealogicalDate>,
    /// The operator's surety in the enclosing-by assertion.
    pub confidence: Confidence,
}

/// A frontend-neutral summary of a place (the DTO the CLI renders). References to other aggregates
/// carry their stable ids alongside their `human_id`s (the cross-aggregate-joins dependency note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceSummary {
    /// The user-facing identifier (e.g. `P0001`).
    pub human_id: String,
    /// The place's stable `PlaceId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The place's type. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub place_type: Option<PlaceType>,
    /// The operator's surety in the place type, if set.
    pub place_type_confidence: Option<Confidence>,
    /// The asserted names, in assertion order, each with its language/date + provenance.
    pub names: Vec<PlaceNameRef>,
    /// The place's code, if set.
    pub code: Option<String>,
    /// The operator's surety in the code, if set.
    pub code_confidence: Option<Confidence>,
    /// The place's coordinates rendered as `lat,long` degrees, if asserted.
    pub coordinates: Option<String>,
    /// The operator's surety in the coordinates, if asserted.
    pub coordinates_confidence: Option<Confidence>,
    /// The coordinate assertion's citations, joined to the source projection — the evidence behind
    /// the coordinates, for the provenance popover.
    pub coordinate_citations: Vec<CitationRef>,
    /// The enclosing places (the jurisdiction chain), joined to the place projection.
    pub enclosing: Vec<PlaceEnclosingRef>,
    /// Citations backing the place's claims, joined to the citation/source projection.
    pub citations: Vec<CitationRef>,
    /// Media attached to the place, in assertion order.
    pub media: Vec<MediaRefSummary>,
    /// Notes attached to the place, in assertion order.
    pub notes: Vec<AggRef>,
    /// Tags applied to the place, by name + colour (never by id — data-model §9).
    pub tags: Vec<TagRef>,
    /// The place's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// What to create a place with (the auto/override `human_id`, its type, and an optional first name).
#[derive(Debug, Clone)]
pub struct NewPlace {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// The place's type.
    pub place_type: PlaceType,
    /// An optional name text for an initial `AssertName`.
    pub name: Option<String>,
}

/// Creates a place, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::PlaceDomain`] if a domain rule
/// rejects the command, or a workspace/store error.
pub async fn create_place(workspace: &Workspace, session: &Session, new: NewPlace) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_place(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_place_human_id(&workspace.place_id_format()?).await?,
    };

    let place_id = session.new_place_id();
    let aggregate_id = place_id.to_string();

    execute(
        store,
        session,
        &aggregate_id,
        PlaceCommand::CreatePlace {
            place_id,
            human_id: HumanId::new(&human_id),
            place_type: new.place_type,
        },
    )
    .await?;

    if let Some(text) = new.name {
        execute(
            store,
            session,
            &aggregate_id,
            PlaceCommand::AssertName {
                place_id,
                name: place_name(text),
            },
        )
        .await?;
    }

    Ok(human_id)
}

/// Sets (or changes) an existing place's type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn set_place_type(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    place_type: PlaceType,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::SetPlaceType { place_id, place_type },
    )
    .await
}

/// Asserts an additional name on an existing place, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, [`AppError::PlaceDomain`] if the name is
/// empty, or a workspace/store error.
pub async fn add_place_name(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    name: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::AssertName {
            place_id,
            name: place_name(name),
        },
    )
    .await
}

/// Asserts that a place is enclosed by another place, identified by their `human_id`s.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if either place is unknown, [`AppError::PlaceDomain`] if the
/// enclosing place is not yet projected (`UnknownPlace`), or a workspace/store error.
pub async fn assert_place_enclosed_by(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    enclosing_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let enclosing_id = resolve_place_id(store, enclosing_human_id).await?;
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::AssertEnclosedBy {
            place_id,
            enclosed_by: PlaceRef {
                place_id: enclosing_id,
                date: None,
            },
        },
    )
    .await
}

/// Asserts a place's geographic coordinates, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn assert_place_coordinates(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    coordinates: GeoCoordinates,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::AssertCoordinates { place_id, coordinates },
    )
    .await
}

/// Sets (or changes) a place's code, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, [`AppError::PlaceDomain`] if the code is
/// empty, or a workspace/store error.
pub async fn set_place_code(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    code: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::SetCode { place_id, code },
    )
    .await
}

/// Adds a citation (by its `human_id`) backing a place's claims.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`]/[`AppError::CitationNotFound`] if either is unknown, or a
/// workspace/store error.
pub async fn add_place_citation(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    citation_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::AddCitation { place_id, citation_id },
    )
    .await
}

/// Attaches a media reference (by media aggregate id) to a place, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn attach_place_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    media_id: MediaId,
    caption: Option<String>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::AttachMedia {
            place_id,
            media: MediaRef {
                media_id,
                crop: None,
                caption,
                citations: Vec::new(),
            },
        },
    )
    .await
}

/// Attaches a note (by note aggregate id) to a place, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn attach_place_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::AttachNote { place_id, note_id },
    )
    .await
}

/// Attaches a media object (by its `human_id`) to a place — the frontend/importer-facing wrapper
/// that resolves the media `human_id` to its id, so a caller never handles UUIDs.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] / [`AppError::MediaNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_place_media(
    workspace: &Workspace,
    session: &Session,
    place_human_id: &str,
    media_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = use_case::resolve_id(
        store.find_media(media_human_id).await?,
        genealogy_core::media::MediaView::media_id,
        || AppError::MediaNotFound(media_human_id.to_owned()),
    )?;
    attach_place_media(workspace, session, place_human_id, media_id, None).await
}

/// Attaches a note (by its `human_id`) to a place — the frontend/importer-facing wrapper.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_place_note(
    workspace: &Workspace,
    session: &Session,
    place_human_id: &str,
    note_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = use_case::resolve_id(
        store.find_note(note_human_id).await?,
        genealogy_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_place_note(workspace, session, place_human_id, note_id).await
}

/// Applies (or removes) a tag on a place, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn tag_place(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        PlaceCommand::Untag { place_id, tag_id }
    } else {
        PlaceCommand::Tag { place_id, tag_id }
    };
    execute(store, session, &place_id.to_string(), command).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Loads a single place's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_place(workspace: &Workspace, human_id: &str) -> Result<Option<PlaceSummary>, AppError> {
    let Some(view) = workspace.store().find_place(human_id).await? else {
        return Ok(None);
    };
    let lookups = PlaceLookups::load(workspace).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every place's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_places(workspace: &Workspace) -> Result<Vec<PlaceSummary>, AppError> {
    let views = workspace.store().list_places().await?;
    let lookups = PlaceLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// An enclosing place joined to the Place projection: the `human_id`, primary name, and type.
struct PlaceInfo {
    human_id: String,
    name: Option<String>,
    place_type: Option<PlaceType>,
}

/// The lookups `summarize` needs to join a place's enclosing chain and attachments to the other
/// projections without a per-row query (the cross-aggregate join lives here — the app/db layer).
struct PlaceLookups {
    places: HashMap<PlaceId, PlaceInfo>,
    citations: HashMap<CitationId, CitationRef>,
    media: HashMap<MediaId, (String, String)>,
    notes: HashMap<NoteId, String>,
    tags: HashMap<TagId, TagRef>,
}

impl PlaceLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        let mut places = HashMap::new();
        for view in store.list_places().await? {
            if let (Some(id), Some(human_id)) = (view.place_id(), view.human_id()) {
                places.insert(
                    id,
                    PlaceInfo {
                        human_id: human_id.as_str().to_owned(),
                        name: view.names().first().map(|n| n.text.clone()),
                        place_type: view.place_type().cloned(),
                    },
                );
            }
        }
        Ok(Self {
            places,
            citations: citation_refs(store).await?,
            media: media_refs(store).await?,
            notes: use_case::note_human_ids(store).await?,
            tags: tag_refs(store).await?,
        })
    }
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
/// Sets a place's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    execute(
        store,
        session,
        &place_id.to_string(),
        PlaceCommand::SetRestrictions { place_id, restrictions },
    )
    .await
}

async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: PlaceCommand) -> Result<(), AppError> {
    let envelope = PlaceCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_place(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Resolves a `human_id` to its aggregate [`PlaceId`], or [`AppError::PlaceNotFound`].
async fn resolve_place_id(store: &Store, human_id: &str) -> Result<PlaceId, AppError> {
    use_case::resolve_id(store.find_place(human_id).await?, PlaceView::place_id, || {
        AppError::PlaceNotFound(human_id.to_owned())
    })
}

/// Resolves a citation `human_id` to its aggregate [`CitationId`], or [`AppError::CitationNotFound`].
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<CitationId, AppError> {
    use_case::resolve_id(store.find_citation(human_id).await?, CitationView::citation_id, || {
        AppError::CitationNotFound(human_id.to_owned())
    })
}

/// Builds a [`PlaceName`] from plain text (language/date are not collected by the CLI yet).
fn place_name(text: String) -> PlaceName {
    PlaceName {
        text,
        language: None,
        date: None,
    }
}

/// Renders a [`PlaceView`] into the frontend DTO, joining the enclosing chain and attachments to the
/// other projections via `lookups`.
fn summarize(view: &PlaceView, lookups: &PlaceLookups) -> PlaceSummary {
    let names = view
        .asserted_names()
        .into_iter()
        .map(|asserted| PlaceNameRef {
            text: asserted.value.text.clone(),
            language: asserted.value.language.as_ref().map(|l| l.as_str().to_owned()),
            date: asserted.value.date.clone(),
            confidence: asserted.confidence,
            source_count: asserted.citations.len(),
        })
        .collect();
    let enclosing = view
        .asserted_enclosed_by()
        .into_iter()
        .map(|asserted| {
            let info = lookups.places.get(&asserted.value.place_id);
            PlaceEnclosingRef {
                human_id: info.map_or_else(|| asserted.value.place_id.to_string(), |i| i.human_id.clone()),
                id: asserted.value.place_id.to_string(),
                name: info.and_then(|i| i.name.clone()),
                place_type: info.and_then(|i| i.place_type.clone()),
                date: asserted.value.date.clone(),
                confidence: asserted.confidence,
            }
        })
        .collect();
    let citations = view
        .citations()
        .into_iter()
        .filter_map(|id| lookups.citations.get(&id).cloned())
        .collect();
    let media = view
        .media()
        .into_iter()
        .filter_map(|media| {
            lookups
                .media
                .get(&media.media_id)
                .map(|(human_id, id)| MediaRefSummary {
                    human_id: human_id.clone(),
                    id: id.clone(),
                    caption: media.caption.clone(),
                })
        })
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
    PlaceSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id: view.place_id().map(|id| id.to_string()).unwrap_or_default(),
        place_type: view.place_type().cloned(),
        place_type_confidence: view.asserted_place_type().map(|a| a.confidence),
        names,
        code: view.code().map(ToOwned::to_owned),
        code_confidence: view.asserted_code().map(|a| a.confidence),
        coordinates: view.coordinates().map(|c| format!("{},{}", c.latitude, c.longitude)),
        coordinates_confidence: view.asserted_coordinates().map(|a| a.confidence),
        coordinate_citations: view.asserted_coordinates().map_or_else(Vec::new, |a| {
            a.citations
                .iter()
                .filter_map(|id| lookups.citations.get(id).cloned())
                .collect()
        }),
        enclosing,
        citations,
        media,
        notes,
        tags,
        restrictions: view.restrictions().clone(),
    }
}
