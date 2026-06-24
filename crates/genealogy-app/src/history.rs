//! The audit / change-log use-cases (Phase 5 PR 5).
//!
//! The event-sourced differentiator: every aggregate's immutable event stream is read back as
//! frontend-neutral [`ChangeLogEntry`] DTOs recording *who* changed *what*, *when*, and *why*. The
//! decision core stays untouched — this layer only reads the log (`genealogy-db`'s raw-event path)
//! and parses the provenance envelope each event carries (ADR 0004 §1).
//!
//! Corrections are non-destructive: [`undo_assertion`] retracts an assertion by id (the log is
//! append-only — data-model §10), and the History tab marks which entries can be undone.

use genealogy_core::citation::CitationView;
use genealogy_core::citation::command::{CitationCommand, CitationCommandEnvelope};
use genealogy_core::event::EventView;
use genealogy_core::event::command::{EventCommand, EventCommandEnvelope};
use genealogy_core::family::FamilyView;
use genealogy_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use genealogy_core::ids::AssertionId;
use genealogy_core::media::MediaView;
use genealogy_core::media::command::{MediaCommand, MediaCommandEnvelope};
use genealogy_core::note::NoteView;
use genealogy_core::note::command::{NoteCommand, NoteCommandEnvelope};
use genealogy_core::person::PersonView;
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::place::PlaceView;
use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use genealogy_core::provenance::{AgentKind, Confidence, EventContext};
use genealogy_core::repository::RepositoryView;
use genealogy_core::repository::command::{RepositoryCommand, RepositoryCommandEnvelope};
use genealogy_core::source::SourceView;
use genealogy_core::source::command::{SourceCommand, SourceCommandEnvelope};
use genealogy_db::{DbError, Store, StoredEvent};
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{Provenance, map_command_error};
use crate::workspace::Workspace;

/// What kind of actor made a change — the DTO twin of [`AgentKind`], without its payload fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorKind {
    /// A human researcher.
    Human,
    /// An automated process (importer, match engine).
    Software,
    /// An AI model.
    AiModel,
}

/// One entry in an aggregate's change log: a single event rendered for an audit timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeLogEntry {
    /// The aggregate kind (the stored `Aggregate::TYPE`, e.g. `person`).
    pub aggregate_kind: String,
    /// The affected record's user-facing id, when it could be resolved (for linking).
    pub aggregate_human_id: Option<String>,
    /// The assertion this event recorded (its `AssertionId`, a UUID v7 string).
    pub assertion_id: String,
    /// The event's position in its aggregate's stream (1-based).
    pub sequence: i64,
    /// The event variant name (e.g. `NameAsserted`); the frontend maps it to a localized phrase.
    pub event_type: String,
    /// When the assertion was recorded (RFC 3339).
    pub occurred_at: String,
    /// The operator's display name, if any.
    pub operator_display: Option<String>,
    /// The operator's kind (human / software / AI).
    pub operator_kind: OperatorKind,
    /// The operator's surety in this claim.
    pub confidence: Confidence,
    /// Why the change was made, if recorded.
    pub rationale: Option<String>,
    /// Whether this assertion can still be undone (not a creation, retraction, or already retracted).
    pub can_undo: bool,
}

/// Per-aggregate record counts for the workspace — the Dashboard stat cards and the rail badges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkspaceCounts {
    /// Persons.
    pub person: u64,
    /// Families.
    pub family: u64,
    /// Events.
    pub event: u64,
    /// Places.
    pub place: u64,
    /// Sources.
    pub source: u64,
    /// Citations.
    pub citation: u64,
    /// Repositories.
    pub repository: u64,
    /// Media objects.
    pub media: u64,
    /// Notes.
    pub note: u64,
    /// Tags.
    pub tag: u64,
    /// DNA tests.
    pub dna_test: u64,
    /// DNA matches.
    pub dna_match: u64,
}

/// The 12 aggregate kinds, as the `Aggregate::TYPE` strings the store keys on.
const AGGREGATE_KINDS: [&str; 12] = [
    "person",
    "family",
    "event",
    "place",
    "source",
    "citation",
    "repository",
    "media",
    "note",
    "tag",
    "dna_test",
    "dna_match",
];

/// The provenance header every event payload carries, regardless of aggregate (ADR 0004 §1).
#[derive(Debug, Deserialize)]
struct EnvelopeHeader {
    assertion_id: Uuid,
    context: EventContext,
}

/// Reads one person's change log, newest first (the History tab).
///
/// Resolves `human_id` to the Person aggregate, reads its raw event stream, and parses each event's
/// provenance envelope. Entries already retracted/superseded — and the creation event — are marked
/// not-undoable.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or [`AppError`] on a store/parse failure.
pub async fn change_log_for_person(workspace: &Workspace, human_id: &str) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let events = store.read_aggregate_events("person", &person_id.to_string()).await?;

    let retracted = retracted_targets(&events)?;
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        let assertion_id = header.assertion_id.to_string();
        let can_undo = is_undoable(&event.event_type) && !retracted.contains(&assertion_id);
        entries.push(entry(event, &header, Some(human_id.to_owned()), can_undo));
    }
    entries.reverse();
    Ok(entries)
}

/// Reads the most recent changes across the whole workspace, newest first (the Dashboard activity
/// feed). Activity entries are display-only, so `can_undo` is always `false`.
///
/// # Errors
///
/// [`AppError`] on a store or payload-parse failure.
pub async fn recent_activity(workspace: &Workspace, limit: u32) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let events = store.read_recent_events(limit).await?;

    let mut indexes: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        if !indexes.contains_key(&event.aggregate_type) {
            let index = load_human_id_index(store, &event.aggregate_type).await?;
            indexes.insert(event.aggregate_type.clone(), index);
        }
        let human_id = indexes
            .get(&event.aggregate_type)
            .and_then(|index| index.get(&event.aggregate_id).cloned());
        entries.push(entry(event, &header, human_id, false));
    }
    Ok(entries)
}

/// Undoes an assertion by retracting it (non-destructive — the event log is append-only).
///
/// Emits a `RetractAssertion` against the person, stamped with the session operator and an "Undo"
/// rationale. A no-op-equivalent for an unknown assertion is left to the domain core to reject.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if the person is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or the domain rejection if the core refuses the retraction.
pub async fn undo_assertion(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let target = AssertionId::from_uuid(
        Uuid::parse_str(assertion_id).map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}"))))?,
    );
    let provenance = Provenance {
        confidence: Confidence::Normal,
        rationale: Some("Undo".to_owned()),
    };
    let envelope = PersonCommandEnvelope {
        meta: session.new_meta(provenance.confidence, provenance.rationale, Vec::new()),
        command: PersonCommand::RetractAssertion { person_id, target },
    };
    store
        .execute_person(&person_id.to_string(), envelope)
        .await
        .map_err(map_command_error)
}

/// Reads a citation's change log (the History tab), newest first. Mirrors [`change_log_for_person`].
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or [`AppError`] on a store/parse failure.
pub async fn change_log_for_citation(workspace: &Workspace, human_id: &str) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    let events = store
        .read_aggregate_events("citation", &citation_id.to_string())
        .await?;

    let retracted = retracted_targets(&events)?;
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        let assertion_id = header.assertion_id.to_string();
        let can_undo = is_undoable(&event.event_type) && !retracted.contains(&assertion_id);
        entries.push(entry(event, &header, Some(human_id.to_owned()), can_undo));
    }
    entries.reverse();
    Ok(entries)
}

/// Undoes a citation assertion by retracting it (non-destructive — the log is append-only).
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if the citation is unknown, [`AppError::Db`] if `assertion_id` is
/// not a UUID, or the domain rejection if the core refuses the retraction.
pub async fn undo_citation_assertion(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    let target = AssertionId::from_uuid(
        Uuid::parse_str(assertion_id).map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}"))))?,
    );
    let envelope = CitationCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, Some("Undo".to_owned()), Vec::new()),
        command: CitationCommand::RetractAssertion { citation_id, target },
    };
    store
        .execute_citation(&citation_id.to_string(), envelope)
        .await
        .map_err(map_command_error)
}

/// Reads a family's change log (the History tab), newest first. Mirrors [`change_log_for_person`].
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if no such family exists, or [`AppError`] on a store/parse failure.
pub async fn change_log_for_family(workspace: &Workspace, human_id: &str) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let events = store.read_aggregate_events("family", &family_id.to_string()).await?;

    let retracted = retracted_targets(&events)?;
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        let assertion_id = header.assertion_id.to_string();
        let can_undo = is_undoable(&event.event_type) && !retracted.contains(&assertion_id);
        entries.push(entry(event, &header, Some(human_id.to_owned()), can_undo));
    }
    entries.reverse();
    Ok(entries)
}

/// Undoes a family assertion by retracting it (non-destructive — the log is append-only).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if the family is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or the domain rejection if the core refuses the retraction.
pub async fn undo_family_assertion(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let target = AssertionId::from_uuid(
        Uuid::parse_str(assertion_id).map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}"))))?,
    );
    let envelope = FamilyCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, Some("Undo".to_owned()), Vec::new()),
        command: FamilyCommand::RetractAssertion { family_id, target },
    };
    store
        .execute_family(&family_id.to_string(), envelope)
        .await
        .map_err(map_command_error)
}

/// Reads an event's change log (the History tab), newest first. Mirrors [`change_log_for_person`].
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or [`AppError`] on a store/parse failure.
pub async fn change_log_for_event(workspace: &Workspace, human_id: &str) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    let events = store.read_aggregate_events("event", &event_id.to_string()).await?;

    let retracted = retracted_targets(&events)?;
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        let assertion_id = header.assertion_id.to_string();
        let can_undo = is_undoable(&event.event_type) && !retracted.contains(&assertion_id);
        entries.push(entry(event, &header, Some(human_id.to_owned()), can_undo));
    }
    entries.reverse();
    Ok(entries)
}

/// Undoes an event assertion by retracting it (non-destructive — the log is append-only).
///
/// # Errors
///
/// [`AppError::EventNotFound`] if the event is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or the domain rejection if the core refuses the retraction.
pub async fn undo_event_assertion(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let event_id = resolve_event_id(store, human_id).await?;
    let target = AssertionId::from_uuid(
        Uuid::parse_str(assertion_id).map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}"))))?,
    );
    let envelope = EventCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, Some("Undo".to_owned()), Vec::new()),
        command: EventCommand::RetractAssertion { event_id, target },
    };
    store
        .execute_event(&event_id.to_string(), envelope)
        .await
        .map_err(map_command_error)
}

/// Reads a place's change log (the History tab), newest first. Mirrors [`change_log_for_person`].
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if no such place exists, or [`AppError`] on a store/parse failure.
pub async fn change_log_for_place(workspace: &Workspace, human_id: &str) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let events = store.read_aggregate_events("place", &place_id.to_string()).await?;

    let retracted = retracted_targets(&events)?;
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        let assertion_id = header.assertion_id.to_string();
        let can_undo = is_undoable(&event.event_type) && !retracted.contains(&assertion_id);
        entries.push(entry(event, &header, Some(human_id.to_owned()), can_undo));
    }
    entries.reverse();
    Ok(entries)
}

/// Undoes a place assertion by retracting it (non-destructive — the log is append-only).
///
/// # Errors
///
/// [`AppError::PlaceNotFound`] if the place is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or the domain rejection if the core refuses the retraction.
pub async fn undo_place_assertion(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let place_id = resolve_place_id(store, human_id).await?;
    let target = AssertionId::from_uuid(
        Uuid::parse_str(assertion_id).map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}"))))?,
    );
    let envelope = PlaceCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, Some("Undo".to_owned()), Vec::new()),
        command: PlaceCommand::RetractAssertion { place_id, target },
    };
    store
        .execute_place(&place_id.to_string(), envelope)
        .await
        .map_err(map_command_error)
}

/// Reads a source's change log (the History tab), newest first. Mirrors [`change_log_for_person`].
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or [`AppError`] on a store/parse failure.
pub async fn change_log_for_source(workspace: &Workspace, human_id: &str) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    let events = store.read_aggregate_events("source", &source_id.to_string()).await?;

    let retracted = retracted_targets(&events)?;
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        let assertion_id = header.assertion_id.to_string();
        let can_undo = is_undoable(&event.event_type) && !retracted.contains(&assertion_id);
        entries.push(entry(event, &header, Some(human_id.to_owned()), can_undo));
    }
    entries.reverse();
    Ok(entries)
}

/// Undoes a source assertion by retracting it (non-destructive — the log is append-only).
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if the source is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or the domain rejection if the core refuses the retraction.
pub async fn undo_source_assertion(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    let target = AssertionId::from_uuid(
        Uuid::parse_str(assertion_id).map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}"))))?,
    );
    let envelope = SourceCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, Some("Undo".to_owned()), Vec::new()),
        command: SourceCommand::RetractAssertion { source_id, target },
    };
    store
        .execute_source(&source_id.to_string(), envelope)
        .await
        .map_err(map_command_error)
}

/// Reads a repository's change log (the History tab), newest first. Mirrors [`change_log_for_person`].
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or [`AppError`] on a store/parse
/// failure.
pub async fn change_log_for_repository(workspace: &Workspace, human_id: &str) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    let events = store
        .read_aggregate_events("repository", &repository_id.to_string())
        .await?;

    let retracted = retracted_targets(&events)?;
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        let assertion_id = header.assertion_id.to_string();
        let can_undo = is_undoable(&event.event_type) && !retracted.contains(&assertion_id);
        entries.push(entry(event, &header, Some(human_id.to_owned()), can_undo));
    }
    entries.reverse();
    Ok(entries)
}

/// Undoes a repository assertion by retracting it (non-destructive — the log is append-only).
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if the repository is unknown, [`AppError::Db`] if `assertion_id`
/// is not a UUID, or the domain rejection if the core refuses the retraction.
pub async fn undo_repository_assertion(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    let target = AssertionId::from_uuid(
        Uuid::parse_str(assertion_id).map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}"))))?,
    );
    let envelope = RepositoryCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, Some("Undo".to_owned()), Vec::new()),
        command: RepositoryCommand::RetractAssertion { repository_id, target },
    };
    store
        .execute_repository(&repository_id.to_string(), envelope)
        .await
        .map_err(map_command_error)
}

/// Reads a media object's change log (the History tab), newest first. Mirrors [`change_log_for_person`].
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or [`AppError`] on a store/parse failure.
pub async fn change_log_for_media(workspace: &Workspace, human_id: &str) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    let events = store.read_aggregate_events("media", &media_id.to_string()).await?;

    let retracted = retracted_targets(&events)?;
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        let assertion_id = header.assertion_id.to_string();
        let can_undo = is_undoable(&event.event_type) && !retracted.contains(&assertion_id);
        entries.push(entry(event, &header, Some(human_id.to_owned()), can_undo));
    }
    entries.reverse();
    Ok(entries)
}

/// Undoes a media assertion by retracting it (non-destructive — the log is append-only).
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if the media is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or the domain rejection if the core refuses the retraction.
pub async fn undo_media_assertion(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    let target = AssertionId::from_uuid(
        Uuid::parse_str(assertion_id).map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}"))))?,
    );
    let envelope = MediaCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, Some("Undo".to_owned()), Vec::new()),
        command: MediaCommand::RetractAssertion { media_id, target },
    };
    store
        .execute_media(&media_id.to_string(), envelope)
        .await
        .map_err(map_command_error)
}

/// Reads a note's change log (the History tab), newest first. Mirrors [`change_log_for_person`].
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or [`AppError`] on a store/parse failure.
pub async fn change_log_for_note(workspace: &Workspace, human_id: &str) -> Result<Vec<ChangeLogEntry>, AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    let events = store.read_aggregate_events("note", &note_id.to_string()).await?;

    let retracted = retracted_targets(&events)?;
    let mut entries = Vec::with_capacity(events.len());
    for event in &events {
        let header = parse_header(event)?;
        let assertion_id = header.assertion_id.to_string();
        let can_undo = is_undoable(&event.event_type) && !retracted.contains(&assertion_id);
        entries.push(entry(event, &header, Some(human_id.to_owned()), can_undo));
    }
    entries.reverse();
    Ok(entries)
}

/// Undoes a note assertion by retracting it (non-destructive — the log is append-only).
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if the note is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or the domain rejection if the core refuses the retraction.
pub async fn undo_note_assertion(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    let target = AssertionId::from_uuid(
        Uuid::parse_str(assertion_id).map_err(|e| AppError::Db(DbError::Malformed(format!("assertion id: {e}"))))?,
    );
    let envelope = NoteCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, Some("Undo".to_owned()), Vec::new()),
        command: NoteCommand::RetractAssertion { note_id, target },
    };
    store
        .execute_note(&note_id.to_string(), envelope)
        .await
        .map_err(map_command_error)
}

/// Counts every aggregate's projected records for the Dashboard and the rail badges.
///
/// # Errors
///
/// [`AppError`] on a store read failure.
pub async fn workspace_counts(workspace: &Workspace) -> Result<WorkspaceCounts, AppError> {
    let store = workspace.store();
    let mut counts = WorkspaceCounts::default();
    for kind in AGGREGATE_KINDS {
        let count = store.count(kind).await?;
        match kind {
            "person" => counts.person = count,
            "family" => counts.family = count,
            "event" => counts.event = count,
            "place" => counts.place = count,
            "source" => counts.source = count,
            "citation" => counts.citation = count,
            "repository" => counts.repository = count,
            "media" => counts.media = count,
            "note" => counts.note = count,
            "tag" => counts.tag = count,
            "dna_test" => counts.dna_test = count,
            "dna_match" => counts.dna_match = count,
            _ => {}
        }
    }
    Ok(counts)
}

/// Builds a [`ChangeLogEntry`] from a stored event and its parsed provenance header.
fn entry(event: &StoredEvent, header: &EnvelopeHeader, human_id: Option<String>, can_undo: bool) -> ChangeLogEntry {
    let operator = &header.context.operator;
    ChangeLogEntry {
        aggregate_kind: event.aggregate_type.clone(),
        aggregate_human_id: human_id,
        assertion_id: header.assertion_id.to_string(),
        sequence: event.sequence,
        event_type: event.event_type.clone(),
        occurred_at: format_timestamp(&header.context),
        operator_display: operator.display.clone(),
        operator_kind: operator_kind(&operator.kind),
        confidence: header.context.confidence,
        rationale: header.context.rationale.clone(),
        can_undo,
    }
}

/// Parses the provenance header (assertion id + context) from an event payload.
fn parse_header(event: &StoredEvent) -> Result<EnvelopeHeader, AppError> {
    serde_json::from_str(&event.payload).map_err(|e| {
        AppError::Db(DbError::Backend(format!(
            "decoding {} event: {e}",
            event.aggregate_type
        )))
    })
}

/// Collects the `AssertionId`s targeted by any retraction or supersession in the stream.
fn retracted_targets(events: &[StoredEvent]) -> Result<BTreeSet<String>, AppError> {
    let mut targets = BTreeSet::new();
    for event in events {
        if event.event_type != "AssertionRetracted" && event.event_type != "AssertionSuperseded" {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&event.payload)
            .map_err(|e| AppError::Db(DbError::Backend(format!("decoding retraction: {e}"))))?;
        if let Some(target) = value.get("target").and_then(serde_json::Value::as_str) {
            targets.insert(target.to_owned());
        }
    }
    Ok(targets)
}

/// Whether an event of this type can be undone: not a creation, not a retraction/supersession.
fn is_undoable(event_type: &str) -> bool {
    !event_type.ends_with("Created") && event_type != "AssertionRetracted" && event_type != "AssertionSuperseded"
}

/// Reads the timestamp string from the parsed context (already RFC 3339).
fn format_timestamp(context: &EventContext) -> String {
    serde_json::to_value(context.occurred_at)
        .ok()
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

/// Maps a core [`AgentKind`] to the frontend [`OperatorKind`].
fn operator_kind(kind: &AgentKind) -> OperatorKind {
    match kind {
        AgentKind::Human => OperatorKind::Human,
        AgentKind::Software { .. } => OperatorKind::Software,
        AgentKind::AiModel { .. } => OperatorKind::AiModel,
    }
}

/// Loads the aggregate-id → `human_id` map for one aggregate kind, or an empty map for a kind
/// without a human id (e.g. Tag) or an unknown kind.
async fn load_human_id_index(store: &Store, aggregate_type: &str) -> Result<HashMap<String, String>, AppError> {
    match store.human_id_index(aggregate_type).await {
        Ok(pairs) => Ok(pairs.into_iter().collect()),
        Err(DbError::Malformed(_)) => Ok(HashMap::new()),
        Err(other) => Err(AppError::Db(other)),
    }
}

/// Resolves a `human_id` to its aggregate [`PersonId`](genealogy_core::ids::PersonId).
async fn resolve_person_id(store: &Store, human_id: &str) -> Result<genealogy_core::ids::PersonId, AppError> {
    crate::use_case::resolve_id(store.find_person(human_id).await?, PersonView::person_id, || {
        AppError::PersonNotFound(human_id.to_owned())
    })
}

/// Resolves a `human_id` to its aggregate [`CitationId`](genealogy_core::ids::CitationId).
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<genealogy_core::ids::CitationId, AppError> {
    crate::use_case::resolve_id(store.find_citation(human_id).await?, CitationView::citation_id, || {
        AppError::CitationNotFound(human_id.to_owned())
    })
}

/// Resolves a `human_id` to its aggregate [`FamilyId`](genealogy_core::ids::FamilyId).
async fn resolve_family_id(store: &Store, human_id: &str) -> Result<genealogy_core::ids::FamilyId, AppError> {
    crate::use_case::resolve_id(store.find_family(human_id).await?, FamilyView::family_id, || {
        AppError::FamilyNotFound(human_id.to_owned())
    })
}

/// Resolves a `human_id` to its aggregate [`EventId`](genealogy_core::ids::EventId).
async fn resolve_event_id(store: &Store, human_id: &str) -> Result<genealogy_core::ids::EventId, AppError> {
    crate::use_case::resolve_id(store.find_event(human_id).await?, EventView::event_id, || {
        AppError::EventNotFound(human_id.to_owned())
    })
}

/// Resolves a `human_id` to its aggregate [`PlaceId`](genealogy_core::ids::PlaceId).
async fn resolve_place_id(store: &Store, human_id: &str) -> Result<genealogy_core::ids::PlaceId, AppError> {
    crate::use_case::resolve_id(store.find_place(human_id).await?, PlaceView::place_id, || {
        AppError::PlaceNotFound(human_id.to_owned())
    })
}

/// Resolves a `human_id` to its aggregate [`SourceId`](genealogy_core::ids::SourceId).
async fn resolve_source_id(store: &Store, human_id: &str) -> Result<genealogy_core::ids::SourceId, AppError> {
    crate::use_case::resolve_id(store.find_source(human_id).await?, SourceView::source_id, || {
        AppError::SourceNotFound(human_id.to_owned())
    })
}

/// Resolves a `human_id` to its aggregate [`RepositoryId`](genealogy_core::ids::RepositoryId).
async fn resolve_repository_id(store: &Store, human_id: &str) -> Result<genealogy_core::ids::RepositoryId, AppError> {
    crate::use_case::resolve_id(
        store.find_repository(human_id).await?,
        RepositoryView::repository_id,
        || AppError::RepositoryNotFound(human_id.to_owned()),
    )
}

/// Resolves a `human_id` to its aggregate [`MediaId`](genealogy_core::ids::MediaId).
async fn resolve_media_id(store: &Store, human_id: &str) -> Result<genealogy_core::ids::MediaId, AppError> {
    crate::use_case::resolve_id(store.find_media(human_id).await?, MediaView::media_id, || {
        AppError::MediaNotFound(human_id.to_owned())
    })
}

/// Resolves a `human_id` to its aggregate [`NoteId`](genealogy_core::ids::NoteId).
async fn resolve_note_id(store: &Store, human_id: &str) -> Result<genealogy_core::ids::NoteId, AppError> {
    crate::use_case::resolve_id(store.find_note(human_id).await?, NoteView::note_id, || {
        AppError::NoteNotFound(human_id.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::{OperatorKind, change_log_for_person, recent_activity, undo_assertion, workspace_counts};
    use crate::config::{AppDefaults, IdFormats, OperatorConfig, WorkspaceDefaults};
    use crate::person::{NewPerson, assert_sex, create_person, set_restrictions, show_person};
    use crate::session::Session;
    use crate::workspace::Workspace;
    use genealogy_core::enums::{EvidenceLevel, Restriction, Sex};
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind};
    use std::collections::BTreeSet;
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
            },
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

    async fn person_with_sex(workspace: &Workspace, session: &Session) -> String {
        let human_id = create_person(
            workspace,
            session,
            NewPerson {
                human_id: None,
                name: None,
                evidence_level: EvidenceLevel::Conclusion,
            },
        )
        .await
        .expect("create");
        assert_sex(workspace, session, &human_id, Sex::Female)
            .await
            .expect("sex");
        human_id
    }

    #[tokio::test]
    async fn change_log_is_newest_first_and_marks_undoable_entries() {
        let (workspace, session, _dir) = setup().await;
        let human_id = person_with_sex(&workspace, &session).await;

        let log = change_log_for_person(&workspace, &human_id).await.expect("log");
        assert_eq!(log.len(), 2);
        // Newest first: the sex assertion, then the creation.
        assert_eq!(log[0].event_type, "SexAsserted");
        assert!(log[0].can_undo, "an assertion is undoable");
        assert_eq!(log[0].operator_kind, OperatorKind::Human);
        assert_eq!(log[0].operator_display.as_deref(), Some("Ada"));
        assert!(!log[0].occurred_at.is_empty());
        assert_eq!(log[1].event_type, "PersonCreated");
        assert!(!log[1].can_undo, "the creation is not undoable");
    }

    #[tokio::test]
    async fn undo_retracts_the_assertion_and_clears_its_undo_flag() {
        let (workspace, session, _dir) = setup().await;
        let human_id = person_with_sex(&workspace, &session).await;

        let log = change_log_for_person(&workspace, &human_id).await.expect("log");
        let sex_assertion = log[0].assertion_id.clone();
        undo_assertion(&workspace, &session, &human_id, &sex_assertion)
            .await
            .expect("undo");

        let log = change_log_for_person(&workspace, &human_id)
            .await
            .expect("log after undo");
        assert_eq!(log.len(), 3, "the retraction is itself a logged event");
        assert_eq!(log[0].event_type, "AssertionRetracted");
        assert!(!log[0].can_undo, "a retraction is not itself undoable");
        let sex = log.iter().find(|e| e.assertion_id == sex_assertion).expect("sex entry");
        assert!(!sex.can_undo, "the retracted assertion can no longer be undone");
    }

    #[tokio::test]
    async fn undo_of_a_restriction_change_clears_it_in_the_projection() {
        let (workspace, session, _dir) = setup().await;
        let human_id = create_person(
            &workspace,
            &session,
            NewPerson {
                human_id: None,
                name: None,
                evidence_level: EvidenceLevel::Conclusion,
            },
        )
        .await
        .expect("create");
        set_restrictions(&workspace, &session, &human_id, BTreeSet::from([Restriction::Locked]))
            .await
            .expect("set restrictions");

        let log = change_log_for_person(&workspace, &human_id).await.expect("log");
        let change = log
            .iter()
            .find(|entry| entry.event_type == "RestrictionsChanged")
            .expect("restriction change");
        undo_assertion(&workspace, &session, &human_id, &change.assertion_id)
            .await
            .expect("undo");

        let summary = show_person(&workspace, &human_id).await.expect("show").expect("person");
        assert!(
            summary.restrictions.is_empty(),
            "undoing the restriction change clears it: {:?}",
            summary.restrictions
        );
    }

    #[tokio::test]
    async fn recent_activity_resolves_the_record_human_id() {
        let (workspace, session, _dir) = setup().await;
        let human_id = person_with_sex(&workspace, &session).await;

        let activity = recent_activity(&workspace, 10).await.expect("activity");
        assert_eq!(activity.len(), 2);
        assert!(activity.iter().all(|e| e.aggregate_kind == "person"));
        assert!(
            activity
                .iter()
                .all(|e| e.aggregate_human_id.as_deref() == Some(human_id.as_str())),
            "every person event links to the record"
        );
        assert!(activity.iter().all(|e| !e.can_undo), "activity rows are display-only");
    }

    #[tokio::test]
    async fn workspace_counts_reflect_created_records() {
        let (workspace, session, _dir) = setup().await;
        let _ = person_with_sex(&workspace, &session).await;

        let counts = workspace_counts(&workspace).await.expect("counts");
        assert_eq!(counts.person, 1);
        assert_eq!(counts.family, 0);
        assert_eq!(counts.event, 0);
    }
}
