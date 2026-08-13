//! The event change-set use-case (Phase 5): a deferred create that commits an event's type
//! (required), description, and place link — including a place created inline (a cascading create,
//! `record-editing.html` §6b) — in a single operator action.
//!
//! The place reference is either an existing place (validated before any write, so an unknown place
//! rejects the whole change-set) or a place created in this same set (a [`PlaceholderRef`] resolved to
//! the minted [`PlaceId`], mirroring the source/citation cascade in [`crate::change_set`]). No date
//! rides here — the structured date is asserted afterwards via [`crate::event::assert_event_date_value`].
//! Provenance follows the shared change-set rule.

use vitni_core::enums::{EventType, PlaceType};
use vitni_core::event::command::{EventCommand, EventCommandEnvelope};
use vitni_core::ids::{HumanId, PlaceId};
use vitni_core::provenance::EvidenceRef;
use vitni_db::Store;

use crate::change_set::PlaceholderRef;
use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// Which place an event links: one that already exists, or one created in this same change-set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceRefInput {
    /// An existing place, by its `human_id` (e.g. `P0001`).
    Existing(String),
    /// A place created in this same change-set, by its placeholder.
    Pending(PlaceholderRef),
}

/// A new Place to create as part of the event change-set (referenced by the event's place link).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPlaceEntry {
    /// The placeholder the event's place reference points at.
    pub placeholder: PlaceholderRef,
    /// The new place's type.
    pub place_type: PlaceType,
    /// The new place's name, if given.
    pub name: Option<String>,
}

/// The desired end state of a new event, committed as one operator action. The type is required; the
/// place is optional and may be an existing place or one created inline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventChangeSet {
    /// A caller-supplied `human_id` override (dup-checked before any write); `None` auto-allocates.
    pub human_id: Option<String>,
    /// The event type (required).
    pub event_type: EventType,
    /// The event's free-text description.
    pub description: Option<String>,
    /// The place the event occurred (existing or pending in this set), if any.
    pub place: Option<PlaceRefInput>,
    /// New places to create in this set (referenced by `place`).
    pub new_places: Vec<NewPlaceEntry>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// Commits an [`EventChangeSet`]: creates the event with its type, sets the description, and links the
/// place (creating a pending place first if the event references one).
///
/// Returns the event's `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied `human_id` is in use, [`AppError::PlaceNotFound`] if an
/// existing referenced place is unknown (both validated before any write), [`AppError::CitationNotFound`]
/// if a backing citation is unknown, [`AppError::EventDomain`]/[`AppError::PlaceDomain`] on a domain
/// rejection, or a workspace/store error.
pub async fn commit_event_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: EventChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match &change_set.human_id {
        Some(id) => {
            if store.find_event(id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id.clone()));
            }
            id.clone()
        }
        None => store.next_event_human_id(&workspace.event_id_format()?).await?,
    };
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;
    // Resolve an existing referenced place before any write, so an unknown place rejects the set.
    let existing_place = match &change_set.place {
        Some(PlaceRefInput::Existing(place_human_id)) => {
            Some(crate::place::resolve_place_id_public(store, place_human_id).await?)
        }
        _ => None,
    };

    // Create the pending places (writes), keeping their minted ids for the link.
    let mut pending: Vec<(PlaceholderRef, PlaceId)> = Vec::with_capacity(change_set.new_places.len());
    for entry in &change_set.new_places {
        let place_human_id = store.next_place_human_id(&workspace.place_id_format()?).await?;
        let place_id = crate::place::create_place_returning_id(
            session,
            store,
            &place_human_id,
            entry.place_type.clone(),
            entry.name.clone(),
            change_set.provenance.clone(),
        )
        .await?;
        pending.push((entry.placeholder.clone(), place_id));
    }
    let place_id = match &change_set.place {
        Some(PlaceRefInput::Existing(_)) => existing_place,
        Some(PlaceRefInput::Pending(placeholder)) => Some(
            pending
                .iter()
                .find(|(p, _)| p == placeholder)
                .map(|(_, id)| *id)
                .ok_or_else(|| AppError::PlaceNotFound(placeholder.0.clone()))?,
        ),
        None => None,
    };

    let event_id = session.new_event_id();
    let aggregate_id = event_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        EventCommand::CreateEvent {
            event_id,
            human_id: HumanId::new(&human_id),
            event_type: change_set.event_type.clone(),
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;
    if let Some(description) = change_set.description {
        execute(
            store,
            session,
            &aggregate_id,
            EventCommand::SetDescription { event_id, description },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(place_id) = place_id {
        execute(
            store,
            session,
            &aggregate_id,
            EventCommand::LinkPlace { event_id, place_id },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
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
    citations: Vec<EvidenceRef>,
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

#[cfg(test)]
mod tests {
    use super::{EventChangeSet, NewPlaceEntry, PlaceRefInput, commit_event_change_set};
    use crate::change_set::PlaceholderRef;
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::event::{list_events, show_event};
    use crate::place::list_places;
    use crate::session::Session;
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
    use tempfile::TempDir;
    use uuid::Uuid;
    use vitni_core::enums::{EventType, PlaceType};
    use vitni_core::ids::AgentId;
    use vitni_core::provenance::{Agent, AgentKind, Confidence};

    fn operator() -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
            email: None,
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
        let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
            .await
            .expect("open");
        (workspace, session(), dir)
    }

    fn draft(event_type: EventType) -> EventChangeSet {
        EventChangeSet {
            human_id: None,
            event_type,
            description: None,
            place: None,
            new_places: Vec::new(),
            provenance: Provenance::default(),
            citations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn create_uses_the_draft_type_and_description() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_event_change_set(
            &workspace,
            &session,
            EventChangeSet {
                description: Some("baptised at the parish church".to_owned()),
                ..draft(EventType::Baptism)
            },
        )
        .await
        .expect("create");
        let event = show_event(&workspace, &human_id).await.expect("show").expect("event");
        assert_eq!(
            event.event_type,
            Some(EventType::Baptism),
            "the draft type is used, not a placeholder"
        );
        assert_eq!(event.description.as_deref(), Some("baptised at the parish church"));
    }

    #[tokio::test]
    async fn a_pending_place_is_created_once_and_linked() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_event_change_set(
            &workspace,
            &session,
            EventChangeSet {
                place: Some(PlaceRefInput::Pending(PlaceholderRef("p1".to_owned()))),
                new_places: vec![NewPlaceEntry {
                    placeholder: PlaceholderRef("p1".to_owned()),
                    place_type: PlaceType::Building,
                    name: Some("Trinity Church".to_owned()),
                }],
                ..draft(EventType::Baptism)
            },
        )
        .await
        .expect("create");

        let places = list_places(&workspace).await.expect("places");
        assert_eq!(places.len(), 1, "exactly one place is created");
        let event = show_event(&workspace, &human_id).await.expect("show").expect("event");
        assert!(event.place.is_some(), "the event links the new place");
    }

    #[tokio::test]
    async fn an_unknown_existing_place_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_event_change_set(
            &workspace,
            &session,
            EventChangeSet {
                place: Some(PlaceRefInput::Existing("P9999".to_owned())),
                ..draft(EventType::Baptism)
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::PlaceNotFound(_))));
        let events = list_events(&workspace).await.expect("events");
        assert!(events.is_empty(), "nothing commits when the linked place is unknown");
    }

    #[tokio::test]
    async fn create_stamps_block_provenance_on_every_command() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_event_change_set(
            &workspace,
            &session,
            EventChangeSet {
                description: Some("noted".to_owned()),
                provenance: Provenance {
                    confidence: Some(Confidence::High),
                    rationale: Some("parish register".to_owned()),
                    evidence_analysis: None,
                },
                ..draft(EventType::Baptism)
            },
        )
        .await
        .expect("create");
        let log = crate::history::change_log_for_event(&workspace, &human_id)
            .await
            .expect("log");
        assert!(!log.is_empty());
        for entry in &log {
            assert_eq!(entry.confidence, Some(Confidence::High));
            assert_eq!(entry.rationale.as_deref(), Some("parish register"));
        }
    }
}
