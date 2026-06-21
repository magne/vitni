//! Place use-cases (ADR 0006): create, set type, add name, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`PlaceSummary`] (never a `PlaceView`, cqrs-es, or sqlx type). `human_id` is
//! auto-allocated using the workspace's configured format, or validated when supplied (ADR 0005).

use genealogy_core::citation::CitationView;
use genealogy_core::enums::PlaceType;
use genealogy_core::geo::GeoCoordinates;
use genealogy_core::ids::{CitationId, HumanId, MediaId, NoteId, PlaceId, TagId};
use genealogy_core::place::PlaceView;
use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use genealogy_core::place_name::PlaceName;
use genealogy_core::place_ref::PlaceRef;
use genealogy_core::provenance::Confidence;
use genealogy_core::text::MediaRef;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a place (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceSummary {
    /// The user-facing identifier (e.g. `P0001`).
    pub human_id: String,
    /// The place's type. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub place_type: Option<PlaceType>,
    /// The asserted name texts, in assertion order.
    pub names: Vec<String>,
    /// The place's code, if set.
    pub code: Option<String>,
    /// The place's coordinates rendered as `lat,long` degrees, if asserted.
    pub coordinates: Option<String>,
    /// The enclosing places (their aggregate ids), in assertion order.
    pub enclosing: Vec<String>,
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

/// Applies (or removes) a tag on a place, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or a workspace/store error.
pub async fn tag_place(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let command = if remove {
        PlaceCommand::Untag { place_id, tag_id }
    } else {
        PlaceCommand::Tag { place_id, tag_id }
    };
    execute(store, session, &place_id.to_string(), command).await
}

/// Loads a single place's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_place(workspace: &Workspace, human_id: &str) -> Result<Option<PlaceSummary>, AppError> {
    let found = workspace.store().find_place(human_id).await?;
    Ok(found.as_ref().map(summarize))
}

/// Lists every place's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_places(workspace: &Workspace) -> Result<Vec<PlaceSummary>, AppError> {
    let views = workspace.store().list_places().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view));
    }
    Ok(summaries)
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
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

/// Renders a [`PlaceView`] into the frontend DTO.
fn summarize(view: &PlaceView) -> PlaceSummary {
    PlaceSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        place_type: view.place_type().cloned(),
        names: view.names().iter().map(|n| n.text.clone()).collect(),
        code: view.code().map(ToOwned::to_owned),
        coordinates: view.coordinates().map(|c| format!("{},{}", c.latitude, c.longitude)),
        enclosing: view.enclosed_by().iter().map(|e| e.place_id.to_string()).collect(),
    }
}
