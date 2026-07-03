//! The tag change-set use-case (Phase 5): a deferred create/edit that commits a tag's name, sort
//! priority, and colour in a single operator action.
//!
//! # Why a change-set
//!
//! The Dioxus tag record is directly editable: the operator fills Name, Priority, and Colour and
//! nothing persists until Save (see `docs/phase5/edit-patterns.html`). On Save the app is handed the
//! *desired* end state; this module validates it up front (all three fields present) and turns it
//! into the minimal set of commands — only the fields that differ from the current projection are
//! emitted — mirroring [`commit_person_change_set`](crate::person_change_set::commit_person_change_set).
//!
//! Unlike the person change-set there is only one aggregate (the Tag), so the sequenced-commit
//! caveat does not apply: a create emits `CreateTag` then the priority/colour setters against the
//! same aggregate, and an edit emits only the changed `Rename`/`SetTagPriority`/`SetTagColor`.

use genealogy_core::ids::TagId;
use genealogy_core::provenance::Confidence;
use genealogy_core::tag::TagError;
use genealogy_core::tag::command::{TagCommand, TagCommandEnvelope};
use genealogy_db::Store;
use uuid::Uuid;

use crate::error::AppError;
use crate::session::Session;
use crate::tag::show_tag;
use crate::use_case;
use crate::workspace::Workspace;

/// Whether the change-set creates a new tag or edits an existing one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagTarget {
    /// Create a new tag; the app mints its id.
    New,
    /// Edit the existing tag with this aggregate id (a UUID string); only the diff is committed.
    Existing {
        /// The aggregate id of the tag to edit.
        id: String,
    },
}

/// The desired end state of a tag — its name, sort priority, and colour — committed as one operator
/// action. All three are required (the editable record disables Save until they are set); the create
/// form seeds a priority and colour default, so an empty field here is rejected before any write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagChangeSet {
    /// Create a new tag or edit an existing one.
    pub target: TagTarget,
    /// The tag's name (must be non-empty).
    pub name: String,
    /// The tag's sort priority (lower sorts first).
    pub priority: i32,
    /// The tag's colour (a CSS hex string, e.g. `#e5534b`).
    pub color: String,
}

/// Commits a [`TagChangeSet`]: creates or edits the tag's name, priority, and colour in one operator
/// action, emitting only the fields that changed.
///
/// Returns the tag's aggregate id (the minted one on create).
///
/// # Errors
///
/// [`AppError::TagDomain`] if the name or colour is empty (validated before any write, so nothing
/// commits), [`AppError::TagNotFound`] if an edited tag is gone or its id is malformed, or a
/// workspace/store error.
pub async fn commit_tag_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: TagChangeSet,
) -> Result<String, AppError> {
    if change_set.name.trim().is_empty() || change_set.color.trim().is_empty() {
        return Err(AppError::TagDomain(TagError::EmptyName));
    }
    let store = workspace.store();
    match &change_set.target {
        TagTarget::New => create_tag_graph(session, store, &change_set).await,
        TagTarget::Existing { id } => edit_tag_graph(workspace, session, store, id, &change_set).await,
    }
}

/// Emits the create-tag command graph: `CreateTag`, then the priority and colour setters — the tag's
/// full initial state.
async fn create_tag_graph(session: &Session, store: &Store, change_set: &TagChangeSet) -> Result<String, AppError> {
    let tag_id = session.new_tag_id();
    let aggregate_id = tag_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        TagCommand::CreateTag {
            tag_id,
            name: change_set.name.clone(),
        },
    )
    .await?;
    execute(
        store,
        session,
        &aggregate_id,
        TagCommand::SetTagPriority {
            tag_id,
            priority: change_set.priority,
        },
    )
    .await?;
    execute(
        store,
        session,
        &aggregate_id,
        TagCommand::SetTagColor {
            tag_id,
            color: change_set.color.clone(),
        },
    )
    .await?;
    Ok(aggregate_id)
}

/// Emits only the setters that differ from the tag's current projection: a changed name, priority,
/// and/or colour.
async fn edit_tag_graph(
    workspace: &Workspace,
    session: &Session,
    store: &Store,
    id: &str,
    change_set: &TagChangeSet,
) -> Result<String, AppError> {
    let current = show_tag(workspace, id)
        .await?
        .ok_or_else(|| AppError::TagNotFound(id.to_owned()))?;
    let tag_id = parse_tag_id(id)?;

    if current.name.as_deref() != Some(change_set.name.as_str()) {
        execute(
            store,
            session,
            id,
            TagCommand::RenameTag {
                tag_id,
                name: change_set.name.clone(),
            },
        )
        .await?;
    }
    if current.priority != Some(change_set.priority) {
        execute(
            store,
            session,
            id,
            TagCommand::SetTagPriority {
                tag_id,
                priority: change_set.priority,
            },
        )
        .await?;
    }
    if current.color.as_deref() != Some(change_set.color.as_str()) {
        execute(
            store,
            session,
            id,
            TagCommand::SetTagColor {
                tag_id,
                color: change_set.color.clone(),
            },
        )
        .await?;
    }
    Ok(id.to_owned())
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: TagCommand) -> Result<(), AppError> {
    let envelope = TagCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_tag(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Parses a tag aggregate id (a UUID string), or [`AppError::TagNotFound`] if malformed.
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{TagChangeSet, TagTarget, commit_tag_change_set};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::history::change_log_for_tag;
    use crate::session::Session;
    use crate::tag::show_tag;
    use crate::workspace::Workspace;
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

    #[tokio::test]
    async fn create_commits_name_priority_and_colour() {
        let (workspace, session, _dir) = setup().await;
        let id = commit_tag_change_set(
            &workspace,
            &session,
            TagChangeSet {
                target: TagTarget::New,
                name: "Direct ancestor".to_owned(),
                priority: 1,
                color: "#e5534b".to_owned(),
            },
        )
        .await
        .expect("create");

        let tag = show_tag(&workspace, &id).await.expect("show").expect("tag");
        assert_eq!(tag.name.as_deref(), Some("Direct ancestor"));
        assert_eq!(tag.priority, Some(1));
        assert_eq!(tag.color.as_deref(), Some("#e5534b"));
    }

    #[tokio::test]
    async fn create_with_empty_name_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_tag_change_set(
            &workspace,
            &session,
            TagChangeSet {
                target: TagTarget::New,
                name: "   ".to_owned(),
                priority: 1,
                color: "#1A2129".to_owned(),
            },
        )
        .await;

        assert!(matches!(result, Err(crate::error::AppError::TagDomain(_))));
        let tags = crate::tag::list_tags(&workspace).await.expect("tags");
        assert!(tags.is_empty(), "nothing commits when the name is empty");
    }

    #[tokio::test]
    async fn create_with_empty_colour_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_tag_change_set(
            &workspace,
            &session,
            TagChangeSet {
                target: TagTarget::New,
                name: "Immigrant".to_owned(),
                priority: 1,
                color: String::new(),
            },
        )
        .await;

        assert!(matches!(result, Err(crate::error::AppError::TagDomain(_))));
        let tags = crate::tag::list_tags(&workspace).await.expect("tags");
        assert!(tags.is_empty(), "nothing commits when the colour is empty");
    }

    #[tokio::test]
    async fn edit_commits_only_changed_fields() {
        let (workspace, session, _dir) = setup().await;
        let id = commit_tag_change_set(
            &workspace,
            &session,
            TagChangeSet {
                target: TagTarget::New,
                name: "Needs sources".to_owned(),
                priority: 2,
                color: "#e0884a".to_owned(),
            },
        )
        .await
        .expect("create");
        let baseline = change_log_for_tag(&workspace, &id).await.expect("log").len();

        // Edit: change only the colour; keep name and priority.
        commit_tag_change_set(
            &workspace,
            &session,
            TagChangeSet {
                target: TagTarget::Existing { id: id.clone() },
                name: "Needs sources".to_owned(),
                priority: 2,
                color: "#2faa6a".to_owned(),
            },
        )
        .await
        .expect("edit");

        let tag = show_tag(&workspace, &id).await.expect("show").expect("tag");
        assert_eq!(tag.color.as_deref(), Some("#2faa6a"));
        assert_eq!(tag.name.as_deref(), Some("Needs sources"));
        assert_eq!(tag.priority, Some(2));
        let after = change_log_for_tag(&workspace, &id).await.expect("log").len();
        assert_eq!(after, baseline + 1, "only the colour change is emitted");
    }

    #[tokio::test]
    async fn edit_with_no_changes_emits_nothing() {
        let (workspace, session, _dir) = setup().await;
        let id = commit_tag_change_set(
            &workspace,
            &session,
            TagChangeSet {
                target: TagTarget::New,
                name: "DNA confirmed".to_owned(),
                priority: 3,
                color: "#2faa6a".to_owned(),
            },
        )
        .await
        .expect("create");
        let baseline = change_log_for_tag(&workspace, &id).await.expect("log").len();

        commit_tag_change_set(
            &workspace,
            &session,
            TagChangeSet {
                target: TagTarget::Existing { id: id.clone() },
                name: "DNA confirmed".to_owned(),
                priority: 3,
                color: "#2faa6a".to_owned(),
            },
        )
        .await
        .expect("no-op edit");

        let after = change_log_for_tag(&workspace, &id).await.expect("log").len();
        assert_eq!(after, baseline, "an unchanged tag emits no events");
    }

    #[tokio::test]
    async fn edit_of_a_missing_tag_is_not_found() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_tag_change_set(
            &workspace,
            &session,
            TagChangeSet {
                target: TagTarget::Existing {
                    id: Uuid::from_u128(999).to_string(),
                },
                name: "Ghost".to_owned(),
                priority: 1,
                color: "#1A2129".to_owned(),
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::TagNotFound(_))));
    }
}
