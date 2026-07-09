//! Event use-cases (ADR 0006): create, set type, assert date, link place, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`EventSummary`]. `link_place` resolves the place's `human_id` to its id (an
//! [`AppError::PlaceNotFound`] if absent); the core then re-checks it against the Place projection
//! via the aggregate's `Services` resolver, surfacing
//! [`EventError::UnknownPlace`](genealogy_core::event::EventError) — the §9 aggregate-tax check
//! (ADR 0004 §3).

use std::collections::{BTreeSet, HashMap};

use genealogy_core::address::Address;
use genealogy_core::citation::CitationView;
use genealogy_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use genealogy_core::enums::{EventType, ParticipantRole, Restriction};
use genealogy_core::event::EventView;
use genealogy_core::event::command::{EventCommand, EventCommandEnvelope};
use genealogy_core::ids::{AssertionId, CitationId, EventId, HumanId, MediaId, NoteId, PersonId, PlaceId, TagId};
use genealogy_core::person::PersonView;
use genealogy_core::place::PlaceView;
use genealogy_core::provenance::{CitationRef as ProvCitationRef, Confidence};
use genealogy_core::text::MediaRef;
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::dto::{AttachedRef, CitationRef, MediaRefSummary, citation_refs, tag_refs};
use crate::error::AppError;
use crate::person::list_persons;
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// An event participant, joined to the person projection: their name + stable id for navigation, the
/// role they played, and the assertion's surety + source count (the evidence-first cue).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantRef {
    /// The participant's user-facing identifier (e.g. `I0001`).
    pub human_id: String,
    /// The participant's stable `PersonId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The participant's display name, if resolved.
    pub name: Option<String>,
    /// The participant's role in the event.
    pub role: ParticipantRole,
    /// The operator's surety in the participation assertion.
    pub confidence: Confidence,
    /// How many citations back the participation assertion.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this participation — the target a per-row
    /// Edit supersedes and a Remove retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// The place an event occurred, joined to the place projection: its primary name for display and the
/// stable id for navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceRefSummary {
    /// The place's user-facing identifier (e.g. `P0001`).
    pub human_id: String,
    /// The place's stable `PlaceId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The place's primary name, if resolved.
    pub name: Option<String>,
}

/// A frontend-neutral summary of an event (the DTO the CLI renders). References to other aggregates
/// carry their stable ids alongside their `human_id`s (the cross-aggregate-joins dependency note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSummary {
    /// The user-facing identifier (e.g. `E0001`).
    pub human_id: String,
    /// The event's stable `EventId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The kind of event. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub event_type: Option<EventType>,
    /// The operator's surety in the event type, if asserted.
    pub event_type_confidence: Option<Confidence>,
    /// When the event occurred. Structured so the frontend localizes it (ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The operator's surety in the date, and how many citations back it.
    pub date_confidence: Option<Confidence>,
    /// How many citations back the date assertion.
    pub date_source_count: usize,
    /// The date assertion's citations, joined to the source projection — the evidence behind the
    /// date, for the provenance popover.
    pub date_citations: Vec<CitationRef>,
    /// The linked place, joined to the place projection (name + stable id).
    pub place: Option<PlaceRefSummary>,
    /// The operator's surety in the place link, if linked.
    pub place_confidence: Option<Confidence>,
    /// The event's free-text description, if set.
    pub description: Option<String>,
    /// The event's postal addresses (a residence/census `ADDR` — data-model §7, §17).
    pub addresses: Vec<Address>,
    /// The event's participants, joined to the person projection, in assertion order.
    pub participants: Vec<ParticipantRef>,
    /// Citations backing the event's claims, joined to the citation/source projection.
    pub citations: Vec<CitationRef>,
    /// Media attached to the event, in assertion order.
    pub media: Vec<MediaRefSummary>,
    /// Notes attached to the event, with the attach `AssertionId` (the Detach target), in assertion
    /// order.
    pub notes: Vec<AttachedRef>,
    /// Tags applied to the event, by name + colour (never by id — data-model §9).
    pub tags: Vec<TagRef>,
    /// The event's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// What to create an event with (the auto/override `human_id` and its type).
#[derive(Debug, Clone)]
pub struct NewEvent {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// The kind of event.
    pub event_type: EventType,
}

/// A partial Gregorian date the CLI collects (year is required; month/day optional).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateParts {
    /// The year (negative for BCE).
    pub year: i32,
    /// The month, 1–12, if known.
    pub month: Option<u8>,
    /// The day, 1–31, if known.
    pub day: Option<u8>,
}

/// The structured inputs to a full [`GenealogicalDate`] an importer parses from a GEDCOM `DATE`
/// (the calendar, quality, modifier, dual-dating month, and verbatim phrase — data-model §7.1).
/// The `sort_value` is derived by [`build_genealogical_date`], not supplied.
#[derive(Debug, Clone)]
pub struct DateInput {
    /// The calendar the date is expressed in.
    pub calendar: Calendar,
    /// The reliability of the date.
    pub quality: DateQuality,
    /// The date itself (structured modifier, or free text when unparseable).
    pub body: GenealogicalDateBody,
    /// Month in which the year begins, for dual / old-style dating (e.g. 1735/6).
    pub new_year_begins: Option<u8>,
    /// The verbatim source text, retained even when unparseable (GEDCOM 7 date phrase).
    pub original_text: Option<String>,
}

/// Creates an event, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::EventDomain`] if a domain
/// rule rejects the command, or a workspace/store error.
pub async fn create_event(
    workspace: &Workspace,
    session: &Session,
    new: NewEvent,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
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
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let event_id = session.new_event_id();
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::CreateEvent {
            event_id,
            human_id: HumanId::new(&human_id),
            event_type: new.event_type,
        },
        provenance,
        citation_refs,
    )
    .await?;
    Ok(human_id)
}

/// Sets an event's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::SetRestrictions { event_id, restrictions },
        meta,
    )
    .await
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::SetEventType { event_id, event_type },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    let date = gregorian_date(parts);
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::AssertDate { event_id, date },
        meta,
    )
    .await
}

/// Asserts when an event occurred from an already-built [`GenealogicalDate`] (the full GEDCOM date
/// grammar an importer parses, via [`build_genealogical_date`]).
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or a workspace/store error.
pub async fn assert_event_date_value(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    date: GenealogicalDate,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::AssertDate { event_id, date },
        meta,
    )
    .await
}

/// Adds a postal address to an event (a residence or census address — data-model §7, §17).
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or a workspace/store error.
pub async fn assert_event_address(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    address: Address,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::AddAddress { event_id, address },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, event_human_id).await?;
    let place_id = resolve_place_id(store, place_human_id).await?;
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::LinkPlace { event_id, place_id },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::SetDescription { event_id, description },
        meta,
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
    meta: MutationMeta<'_>,
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
    execute_event_mutation(store, session, event_id, command, meta).await
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, event_human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::AddCitation { event_id, citation_id },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::AttachMedia {
            event_id,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    execute_event_mutation(
        store,
        session,
        event_id,
        EventCommand::AttachNote { event_id, note_id },
        meta,
    )
    .await
}

/// Attaches a media object (by its `human_id`) to an event — the importer-facing wrapper that
/// resolves the media `human_id` to its id, so a bulk importer never handles UUIDs.
///
/// # Errors
///
/// [`AppError::EventNotFound`] / [`AppError::MediaNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_event_media(
    workspace: &Workspace,
    session: &Session,
    event_human_id: &str,
    media_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = use_case::resolve_id(
        store.find_media(media_human_id).await?,
        genealogy_core::media::MediaView::media_id,
        || AppError::MediaNotFound(media_human_id.to_owned()),
    )?;
    attach_event_media(
        workspace,
        session,
        event_human_id,
        media_id,
        None,
        MutationMeta::default(),
    )
    .await
}

/// Attaches a note (by its `human_id`) to an event — the importer-facing wrapper.
///
/// # Errors
///
/// [`AppError::EventNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_event_note(
    workspace: &Workspace,
    session: &Session,
    event_human_id: &str,
    note_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = use_case::resolve_id(
        store.find_note(note_human_id).await?,
        genealogy_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_event_note(workspace, session, event_human_id, note_id, MutationMeta::default()).await
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
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        EventCommand::Untag { event_id, tag_id }
    } else {
        EventCommand::Tag { event_id, tag_id }
    };
    execute_event_mutation(store, session, event_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
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
    let lookups = EventLookups::load(workspace).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every event's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_events(workspace: &Workspace) -> Result<Vec<EventSummary>, AppError> {
    let store = workspace.store();
    let views = store.list_events().await?;
    let lookups = EventLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// A person joined to the Person projection: the `human_id` and display name, for participant rows.
struct PersonInfo {
    human_id: String,
    name: Option<String>,
}

/// A place joined to the Place projection: the `human_id` and primary name, for the linked-place row.
struct PlaceInfo {
    human_id: String,
    name: Option<String>,
}

/// The lookups `summarize` needs to join an event's participants, linked place, and attachments to
/// the other projections without a per-row query (the cross-aggregate join lives here).
struct EventLookups {
    persons: HashMap<PersonId, PersonInfo>,
    places: HashMap<PlaceId, PlaceInfo>,
    citations: HashMap<CitationId, CitationRef>,
    media: HashMap<MediaId, (String, String)>,
    notes: HashMap<NoteId, String>,
    tags: HashMap<TagId, TagRef>,
}

impl EventLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        let person_ids: HashMap<String, PersonId> = store
            .list_persons()
            .await?
            .iter()
            .filter_map(|p| Some((p.human_id()?.to_string(), p.person_id()?)))
            .collect();
        let mut persons = HashMap::new();
        for summary in list_persons(workspace).await? {
            if let Some(id) = person_ids.get(&summary.human_id) {
                persons.insert(
                    *id,
                    PersonInfo {
                        human_id: summary.human_id.clone(),
                        name: summary.display_name.clone(),
                    },
                );
            }
        }
        let mut places = HashMap::new();
        for view in store.list_places().await? {
            if let (Some(id), Some(human_id)) = (view.place_id(), view.human_id()) {
                places.insert(
                    id,
                    PlaceInfo {
                        human_id: human_id.as_str().to_owned(),
                        name: view.names().first().map(|n| n.text.clone()),
                    },
                );
            }
        }
        Ok(Self {
            persons,
            places,
            citations: citation_refs(store).await?,
            media: crate::dto::media_refs(store).await?,
            notes: use_case::note_human_ids(store).await?,
            tags: tag_refs(store).await?,
        })
    }
}

/// Sets (or changes) an event's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case).
///
/// # Errors
///
/// [`AppError::EventNotFound`] if the event is unknown, [`AppError::HumanIdTaken`] if the requested
/// id is already in use, or a workspace/store error.
pub async fn set_event_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_event(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_event_human_id(&workspace.event_id_format()?).await?,
    };
    execute(
        store,
        session,
        &event_id.to_string(),
        EventCommand::SetHumanId {
            event_id,
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
    command: EventCommand,
    provenance: Provenance,
    citations: Vec<ProvCitationRef>,
) -> Result<(), AppError> {
    let envelope = EventCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_event(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create event mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in an
/// [`EventCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_event_mutation(
    store: &Store,
    session: &Session,
    event_id: EventId,
    command: EventCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(event_id, command, target);
    execute(
        store,
        session,
        &event_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in an [`EventCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(event_id: EventId, command: EventCommand, target: Option<AssertionId>) -> EventCommand {
    match target {
        Some(target) => EventCommand::SupersedeAssertion {
            event_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
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

/// Builds an exact Gregorian [`GenealogicalDate`] from `parts`, computing the integer sort key the
/// model stores (data-model §7.1). Month/day default to 0 in the key when unknown.
pub(crate) fn gregorian_date(parts: DateParts) -> GenealogicalDate {
    let point = DatePoint {
        year: Some(parts.year),
        month: parts.month,
        day: parts.day,
    };
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(point)),
        time: None,
        new_year_begins: None,
        sort_value: sort_value_of(&point),
        original_text: None,
    }
}

/// Builds a full [`GenealogicalDate`] from a parsed [`DateInput`], computing the sort key from the
/// representative point (the single point, or a range/span **start**; `0` for free text). Calendar
/// is recorded but does not change the (approximate) numeric ordering — acceptable for sorting, as
/// in Gramps (data-model §7.1).
#[must_use]
pub fn build_genealogical_date(input: DateInput) -> GenealogicalDate {
    let sort_value = match &input.body {
        GenealogicalDateBody::Structured(modifier) => sort_value_of(representative_point(modifier)),
        GenealogicalDateBody::TextOnly { .. } => 0,
    };
    GenealogicalDate {
        calendar: input.calendar,
        quality: input.quality,
        modifier: input.body,
        time: None,
        new_year_begins: input.new_year_begins,
        sort_value,
        original_text: input.original_text,
    }
}

/// The point a [`DateModifier`] sorts by: the single point, or the start of a range/span.
fn representative_point(modifier: &DateModifier) -> &DatePoint {
    match modifier {
        DateModifier::None(point)
        | DateModifier::Before(point)
        | DateModifier::After(point)
        | DateModifier::About(point)
        | DateModifier::From(point)
        | DateModifier::To(point)
        | DateModifier::Interpreted { date: point, .. } => point,
        DateModifier::Range { start, .. } | DateModifier::Span { start, .. } => start,
    }
}

/// The integer sort key for a (possibly partial) point: `year*10000 + month*100 + day`, unknown
/// components contributing 0.
fn sort_value_of(point: &DatePoint) -> i64 {
    let year = point.year.unwrap_or(0);
    let month = point.month.unwrap_or(0);
    let day = point.day.unwrap_or(0);
    i64::from(year) * 10_000 + i64::from(month) * 100 + i64::from(day)
}

/// Renders an [`EventView`] into the frontend DTO, joining participants, the linked place, and the
/// attachments to the other projections via `lookups`.
fn summarize(view: &EventView, lookups: &EventLookups) -> EventSummary {
    let place = view.asserted_place().map(|asserted| {
        let info = lookups.places.get(&asserted.value);
        PlaceRefSummary {
            human_id: info.map_or_else(|| asserted.value.to_string(), |i| i.human_id.clone()),
            id: asserted.value.to_string(),
            name: info.and_then(|i| i.name.clone()),
        }
    });
    let participants = view
        .participants_with_assertions()
        .iter()
        .map(|attributed| {
            let asserted = &attributed.value;
            let participant = &asserted.value;
            let info = lookups.persons.get(&participant.participant_id);
            ParticipantRef {
                human_id: info.map_or_else(|| participant.participant_id.to_string(), |i| i.human_id.clone()),
                id: participant.participant_id.to_string(),
                name: info.and_then(|i| i.name.clone()),
                role: participant.role.clone(),
                confidence: asserted.confidence,
                source_count: asserted.citations.len(),
                assertion_id: attributed.assertion_id.to_string(),
            }
        })
        .collect();
    let addresses = view.addresses().into_iter().cloned().collect();
    let citations = view
        .citations_with_assertions()
        .iter()
        .filter_map(|attributed| {
            lookups.citations.get(&attributed.value).cloned().map(|mut citation| {
                citation.assertion_id = Some(attributed.assertion_id.to_string());
                citation
            })
        })
        .collect();
    let media = view
        .media_with_assertions()
        .iter()
        .filter_map(|attributed| {
            let media = &attributed.value;
            lookups
                .media
                .get(&media.media_id)
                .map(|(human_id, id)| MediaRefSummary {
                    human_id: human_id.clone(),
                    id: id.clone(),
                    caption: media.caption.clone(),
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
    EventSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id: view.event_id().map(|id| id.to_string()).unwrap_or_default(),
        event_type: view.event_type().cloned(),
        event_type_confidence: view.asserted_event_type().map(|a| a.confidence),
        date: view.date().cloned(),
        date_confidence: view.asserted_date().map(|a| a.confidence),
        date_source_count: view.asserted_date().map_or(0, |a| a.citations.len()),
        date_citations: view.asserted_date().map_or_else(Vec::new, |a| {
            a.citations
                .iter()
                .filter_map(|id| lookups.citations.get(id).cloned())
                .collect()
        }),
        place,
        place_confidence: view.asserted_place().map(|a| a.confidence),
        description: view.description().map(ToOwned::to_owned),
        addresses,
        participants,
        citations,
        media,
        notes,
        tags,
        restrictions: view.restrictions().clone(),
    }
}
