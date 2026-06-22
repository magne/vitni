//! Event use-cases (ADR 0006): create, set type, assert date, link place, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`EventSummary`]. `link_place` resolves the place's `human_id` to its id (an
//! [`AppError::PlaceNotFound`] if absent); the core then re-checks it against the Place projection
//! via the aggregate's `Services` resolver, surfacing
//! [`EventError::UnknownPlace`](genealogy_core::event::EventError) — the §9 aggregate-tax check
//! (ADR 0004 §3).

use std::collections::HashMap;

use genealogy_core::citation::CitationView;
use genealogy_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use genealogy_core::enums::{EventType, ParticipantRole};
use genealogy_core::event::EventView;
use genealogy_core::event::command::{EventCommand, EventCommandEnvelope};
use genealogy_core::ids::{CitationId, EventId, HumanId, MediaId, NoteId, PersonId, PlaceId, TagId};
use genealogy_core::person::PersonView;
use genealogy_core::place::PlaceView;
use genealogy_core::provenance::Confidence;
use genealogy_core::text::MediaRef;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of an event (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSummary {
    /// The user-facing identifier (e.g. `E0001`).
    pub human_id: String,
    /// The kind of event. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub event_type: Option<EventType>,
    /// When the event occurred. Structured so the frontend localizes it (ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The linked place's `human_id`, resolved from the projected `PlaceId`.
    pub place: Option<String>,
    /// The event's free-text description, if set.
    pub description: Option<String>,
    /// The number of recorded participants.
    pub participant_count: usize,
}

/// What to create an event with (the auto/override `human_id` and its type).
#[derive(Debug, Clone)]
pub struct NewEvent {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// The kind of event.
    pub event_type: EventType,
    /// Whether the event is private (Gramps' universal privacy flag).
    pub private: bool,
}

/// A partial Gregorian date the CLI collects (year is required; month/day optional).
#[derive(Debug, Clone, Copy)]
pub struct DateParts {
    /// The year (negative for BCE).
    pub year: i32,
    /// The month, 1–12, if known.
    pub month: Option<u8>,
    /// The day, 1–31, if known.
    pub day: Option<u8>,
}

/// Creates an event, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::EventDomain`] if a domain
/// rule rejects the command, or a workspace/store error.
pub async fn create_event(workspace: &Workspace, session: &Session, new: NewEvent) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_event(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_event_human_id(&workspace.event_id_format()?).await?,
    };

    let event_id = session.new_event_id();
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::CreateEvent {
            event_id,
            human_id: HumanId::new(&human_id),
            event_type: new.event_type,
            private: new.private,
        },
    )
    .await?;
    Ok(human_id)
}

/// Sets (or changes) an existing event's type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or a workspace/store error.
pub async fn set_event_type(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    event_type: EventType,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::SetEventType { event_id, event_type },
    )
    .await
}

/// Asserts when an event occurred, from partial Gregorian `parts`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or a workspace/store error.
pub async fn assert_event_date(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    parts: DateParts,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    let date = gregorian_date(parts);
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::AssertDate { event_id, date },
    )
    .await
}

/// Links an event to the place it occurred, both identified by `human_id`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] / [`AppError::PlaceNotFound`] if either does not exist,
/// [`AppError::EventDomain`] if the core rejects the link (`UnknownPlace`), or a workspace/store
/// error.
pub async fn link_place(
    workspace: &Workspace,
    session: &Session,
    event_human_id: &str,
    place_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, event_human_id).await?;
    let place_id = resolve_place_id(store, place_human_id).await?;
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::LinkPlace { event_id, place_id },
    )
    .await
}

/// Sets (or changes) an event's free-text description, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or a workspace/store error.
pub async fn set_event_description(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    description: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::SetDescription { event_id, description },
    )
    .await
}

/// Adds (or removes) a participant role on an event; the participant is identified by `human_id`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] / [`AppError::PersonNotFound`] if either is unknown, or a
/// workspace/store error.
pub async fn set_participant_role(
    workspace: &Workspace,
    session: &Session,
    event_human_id: &str,
    participant_human_id: &str,
    role: ParticipantRole,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, event_human_id).await?;
    let participant_id = resolve_person_id(store, participant_human_id).await?;
    let command = if remove {
        EventCommand::RemoveParticipantRole {
            event_id,
            participant_id,
            role,
        }
    } else {
        EventCommand::AddParticipantRole {
            event_id,
            participant_id,
            role,
        }
    };
    execute(store, session, &event_id.to_string(), command).await
}

/// Adds a citation (by its `human_id`) backing an event's claims.
///
/// # Errors
///
/// [`AppError::EventNotFound`] / [`AppError::CitationNotFound`] if either is unknown, or a
/// workspace/store error.
pub async fn add_event_citation(
    workspace: &Workspace,
    session: &Session,
    event_human_id: &str,
    citation_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, event_human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::AddCitation { event_id, citation_id },
    )
    .await
}

/// Attaches a media reference (by media aggregate id) to an event, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or a workspace/store error.
pub async fn attach_event_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    media_id: MediaId,
    caption: Option<String>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::AttachMedia {
            event_id,
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

/// Attaches a note (by note aggregate id) to an event, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or a workspace/store error.
pub async fn attach_event_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::AttachNote { event_id, note_id },
    )
    .await
}

/// Applies (or removes) a tag on an event, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or a workspace/store error.
pub async fn tag_event(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    let command = if remove {
        EventCommand::Untag { event_id, tag_id }
    } else {
        EventCommand::Tag { event_id, tag_id }
    };
    execute(store, session, &event_id.to_string(), command).await
}

/// Loads a single event's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_event(workspace: &Workspace, human_id: &str) -> Result<Option<EventSummary>, AppError> {
    let store = workspace.store();
    let Some(view) = store.find_event(human_id).await? else {
        return Ok(None);
    };
    let places = place_human_ids(store).await?;
    Ok(Some(summarize(&view, &places)))
}

/// Lists every event's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_events(workspace: &Workspace) -> Result<Vec<EventSummary>, AppError> {
    let store = workspace.store();
    let views = store.list_events().await?;
    let places = place_human_ids(store).await?;
    Ok(views.iter().map(|view| summarize(view, &places)).collect())
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: EventCommand) -> Result<(), AppError> {
    let envelope = EventCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_event(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Resolves an event `human_id` to its aggregate [`EventId`], or [`AppError::EventNotFound`].
async fn resolve_event_id(store: &Store, human_id: &str) -> Result<EventId, AppError> {
    use_case::resolve_id(store.find_event(human_id).await?, EventView::event_id, || {
        AppError::EventNotFound(human_id.to_owned())
    })
}

/// Resolves a place `human_id` to its aggregate [`PlaceId`], or [`AppError::PlaceNotFound`].
async fn resolve_place_id(store: &Store, human_id: &str) -> Result<PlaceId, AppError> {
    use_case::resolve_id(store.find_place(human_id).await?, PlaceView::place_id, || {
        AppError::PlaceNotFound(human_id.to_owned())
    })
}

/// Resolves a person `human_id` to its aggregate [`PersonId`], or [`AppError::PersonNotFound`].
async fn resolve_person_id(store: &Store, human_id: &str) -> Result<PersonId, AppError> {
    use_case::resolve_id(store.find_person(human_id).await?, PersonView::person_id, || {
        AppError::PersonNotFound(human_id.to_owned())
    })
}

/// Resolves a citation `human_id` to its aggregate [`CitationId`], or [`AppError::CitationNotFound`].
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<CitationId, AppError> {
    use_case::resolve_id(store.find_citation(human_id).await?, CitationView::citation_id, || {
        AppError::CitationNotFound(human_id.to_owned())
    })
}

/// Builds a `PlaceId -> human_id` lookup from the Place projection, to render the linked place.
async fn place_human_ids(store: &Store) -> Result<HashMap<PlaceId, String>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_places().await? {
        if let (Some(id), Some(human_id)) = (view.place_id(), view.human_id()) {
            map.insert(id, human_id.as_str().to_owned());
        }
    }
    Ok(map)
}

/// Builds an exact Gregorian [`GenealogicalDate`] from `parts`, computing the integer sort key the
/// model stores (data-model §7.1). Month/day default to 0 in the key when unknown.
pub(crate) fn gregorian_date(parts: DateParts) -> GenealogicalDate {
    let month = parts.month.unwrap_or(0);
    let day = parts.day.unwrap_or(0);
    let sort_value = i64::from(parts.year) * 10_000 + i64::from(month) * 100 + i64::from(day);
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(parts.year),
            month: parts.month,
            day: parts.day,
        })),
        time: None,
        new_year_begins: None,
        sort_value,
        original_text: None,
    }
}

/// Renders an [`EventView`] into the frontend DTO, resolving the linked place's `human_id`.
fn summarize(view: &EventView, places: &HashMap<PlaceId, String>) -> EventSummary {
    let place = view.place_id().and_then(|id| places.get(&id).cloned());
    EventSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        event_type: view.event_type().cloned(),
        date: view.date().cloned(),
        place,
        description: view.description().map(ToOwned::to_owned),
        participant_count: view.participants().len(),
    }
}
