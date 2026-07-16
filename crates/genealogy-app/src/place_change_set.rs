//! The place change-set use-case (Phase 5): a deferred create that commits a place's type, name,
//! coordinates, and code in a single operator action.
//!
//! Mirrors [`crate::source_change_set`], with a required place type carried on `CreatePlace`. The
//! coordinates arrive already parsed (`GeoCoordinates`); numeric parsing/rejection happens at the UI
//! boundary (`record-editing.html` §7), never here. Provenance follows the shared change-set rule
//! ([`crate::change_set`]). Editing an existing place is the per-field `dispatch_place_edit` path
//! (PR27), not this create.

use genealogy_core::enums::PlaceType;
use genealogy_core::geo::GeoCoordinates;
use genealogy_core::ids::HumanId;
use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use genealogy_core::place_name::PlaceName;
use genealogy_core::provenance::EvidenceRef;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// The desired end state of a new place, committed as one operator action. The type is required (it
/// rides on `CreatePlace`); the rest is optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceChangeSet {
    /// A caller-supplied `human_id` override (dup-checked before any write); `None` auto-allocates.
    pub human_id: Option<String>,
    /// The place type (required).
    pub place_type: PlaceType,
    /// The place's primary name.
    pub name: Option<String>,
    /// The place's coordinates (already parsed from the UI's decimal-degree fields).
    pub coordinates: Option<GeoCoordinates>,
    /// The place's code (e.g. a postal or GOV code).
    pub code: Option<String>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// Commits a [`PlaceChangeSet`]: creates the place with its type and emits a setter for each filled
/// field.
///
/// Returns the place's `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied `human_id` is in use (validated before any write),
/// [`AppError::CitationNotFound`] if a backing citation is unknown, [`AppError::PlaceDomain`] on a
/// domain rejection, or a workspace/store error.
pub async fn commit_place_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: PlaceChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match &change_set.human_id {
        Some(id) => {
            if store.find_place(id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id.clone()));
            }
            id.clone()
        }
        None => store.next_place_human_id(&workspace.place_id_format()?).await?,
    };
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    let place_id = session.new_place_id();
    let aggregate_id = place_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        PlaceCommand::CreatePlace {
            place_id,
            human_id: HumanId::new(&human_id),
            place_type: change_set.place_type.clone(),
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;

    if let Some(text) = change_set.name {
        execute(
            store,
            session,
            &aggregate_id,
            PlaceCommand::AssertName {
                place_id,
                name: PlaceName {
                    text,
                    language: None,
                    date: None,
                },
            },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(coordinates) = change_set.coordinates {
        execute(
            store,
            session,
            &aggregate_id,
            PlaceCommand::AssertCoordinates { place_id, coordinates },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(code) = change_set.code {
        execute(
            store,
            session,
            &aggregate_id,
            PlaceCommand::SetCode { place_id, code },
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
    command: PlaceCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = PlaceCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_place(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

#[cfg(test)]
mod tests {
    use super::{PlaceChangeSet, commit_place_change_set};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::place::{list_places, show_place};
    use crate::session::Session;
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
    use genealogy_core::enums::PlaceType;
    use genealogy_core::geo::{GeoCoordinates, Microdegrees};
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind, Confidence};
    use std::str::FromStr;
    use tempfile::TempDir;
    use uuid::Uuid;

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

    fn draft(place_type: PlaceType) -> PlaceChangeSet {
        PlaceChangeSet {
            human_id: None,
            place_type,
            name: None,
            coordinates: None,
            code: None,
            provenance: Provenance::default(),
            citations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn create_commits_the_type_name_coordinates_and_code() {
        let (workspace, session, _dir) = setup().await;
        let coordinates = GeoCoordinates {
            latitude: Microdegrees::from_str("40.7128").expect("lat"),
            longitude: Microdegrees::from_str("-74.006").expect("long"),
        };
        let human_id = commit_place_change_set(
            &workspace,
            &session,
            PlaceChangeSet {
                name: Some("New York".to_owned()),
                coordinates: Some(coordinates),
                code: Some("NYC".to_owned()),
                ..draft(PlaceType::City)
            },
        )
        .await
        .expect("create");

        let place = show_place(&workspace, &human_id).await.expect("show").expect("place");
        assert_eq!(place.place_type, Some(PlaceType::City));
        assert_eq!(place.names.first().map(|n| n.text.as_str()), Some("New York"));
        assert_eq!(place.code.as_deref(), Some("NYC"));
        assert!(place.coordinates.is_some(), "coordinates asserted");
    }

    #[tokio::test]
    async fn a_type_only_draft_creates_a_bare_place() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_place_change_set(&workspace, &session, draft(PlaceType::Country))
            .await
            .expect("create");
        let place = show_place(&workspace, &human_id).await.expect("show").expect("place");
        assert_eq!(place.place_type, Some(PlaceType::Country));
        assert!(place.names.is_empty());
        assert_eq!(place.coordinates, None);
    }

    #[tokio::test]
    async fn a_taken_human_id_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let taken = commit_place_change_set(&workspace, &session, draft(PlaceType::City))
            .await
            .expect("first place");
        let result = commit_place_change_set(
            &workspace,
            &session,
            PlaceChangeSet {
                human_id: Some(taken),
                ..draft(PlaceType::City)
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::HumanIdTaken(_))));
        let places = list_places(&workspace).await.expect("places");
        assert_eq!(places.len(), 1, "nothing commits when the human_id is taken");
    }

    #[tokio::test]
    async fn create_stamps_block_provenance_on_every_command() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_place_change_set(
            &workspace,
            &session,
            PlaceChangeSet {
                name: Some("Oslo".to_owned()),
                provenance: Provenance {
                    confidence: Some(Confidence::High),
                    rationale: Some("gazetteer".to_owned()),
                    evidence_analysis: None,
                },
                ..draft(PlaceType::City)
            },
        )
        .await
        .expect("create");
        let log = crate::history::change_log_for_place(&workspace, &human_id)
            .await
            .expect("log");
        assert!(!log.is_empty());
        for entry in &log {
            assert_eq!(entry.confidence, Some(Confidence::High));
            assert_eq!(entry.rationale.as_deref(), Some("gazetteer"));
        }
    }
}
