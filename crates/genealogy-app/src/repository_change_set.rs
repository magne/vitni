//! The repository change-set use-case (Phase 5): a deferred create that commits a repository's type
//! and name in a single operator action.
//!
//! Mirrors [`crate::source_change_set`]: the form buffers every field and persists nothing until Save
//! (`record-editing.html` §6); this module validates the optional `human_id` override up front, then
//! emits `CreateRepository` plus a setter for each filled field. Provenance follows the shared
//! change-set rule ([`crate::change_set`]). Editing an existing repository is the per-field
//! `dispatch_repository_edit` path (PR27), not this create.

use genealogy_core::enums::RepositoryType;
use genealogy_core::ids::HumanId;
use genealogy_core::provenance::CitationRef;
use genealogy_core::repository::command::{RepositoryCommand, RepositoryCommandEnvelope};
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// The desired end state of a new repository, committed as one operator action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryChangeSet {
    /// A caller-supplied `human_id` override (dup-checked before any write); `None` auto-allocates.
    pub human_id: Option<String>,
    /// The repository type (library, archive, …).
    pub repository_type: Option<RepositoryType>,
    /// The repository's name.
    pub name: Option<String>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// Commits a [`RepositoryChangeSet`]: creates the repository and emits a setter for each filled field.
///
/// Returns the repository's `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied `human_id` is in use (validated before any write),
/// [`AppError::CitationNotFound`] if a backing citation is unknown, [`AppError::RepositoryDomain`] on
/// a domain rejection, or a workspace/store error.
pub async fn commit_repository_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: RepositoryChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match &change_set.human_id {
        Some(id) => {
            if store.find_repository(id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id.clone()));
            }
            id.clone()
        }
        None => {
            store
                .next_repository_human_id(&workspace.repository_id_format()?)
                .await?
        }
    };
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    let repository_id = session.new_repository_id();
    let aggregate_id = repository_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        RepositoryCommand::CreateRepository {
            repository_id,
            human_id: HumanId::new(&human_id),
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;

    if let Some(repository_type) = change_set.repository_type {
        execute(
            store,
            session,
            &aggregate_id,
            RepositoryCommand::SetRepositoryType {
                repository_id,
                repository_type,
            },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(name) = change_set.name {
        execute(
            store,
            session,
            &aggregate_id,
            RepositoryCommand::SetName { repository_id, name },
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
    command: RepositoryCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = RepositoryCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_repository(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

#[cfg(test)]
mod tests {
    use super::{RepositoryChangeSet, commit_repository_change_set};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::history::change_log_for_repository;
    use crate::repository::{NewRepository, create_repository, list_repositories, show_repository};
    use crate::session::Session;
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
    use genealogy_core::enums::RepositoryType;
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind, Confidence};
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

    fn draft() -> RepositoryChangeSet {
        RepositoryChangeSet {
            human_id: None,
            repository_type: None,
            name: None,
            provenance: Provenance::default(),
            citations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn create_commits_the_type_and_name() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_repository_change_set(
            &workspace,
            &session,
            RepositoryChangeSet {
                repository_type: Some(RepositoryType::Archive),
                name: Some("National Archives".to_owned()),
                ..draft()
            },
        )
        .await
        .expect("create");

        let repo = show_repository(&workspace, &human_id)
            .await
            .expect("show")
            .expect("repo");
        assert_eq!(repo.name.as_deref(), Some("National Archives"));
        assert_eq!(repo.repository_type, Some(RepositoryType::Archive));
    }

    #[tokio::test]
    async fn an_empty_draft_creates_a_bare_repository() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_repository_change_set(&workspace, &session, draft())
            .await
            .expect("create");
        let repo = show_repository(&workspace, &human_id)
            .await
            .expect("show")
            .expect("repo");
        assert_eq!(repo.name, None);
    }

    #[tokio::test]
    async fn a_taken_human_id_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let taken = create_repository(
            &workspace,
            &session,
            NewRepository {
                human_id: None,
                name: Some("First".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("first repo");
        let result = commit_repository_change_set(
            &workspace,
            &session,
            RepositoryChangeSet {
                human_id: Some(taken),
                name: Some("Clash".to_owned()),
                ..draft()
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::HumanIdTaken(_))));
        let repos = list_repositories(&workspace).await.expect("repos");
        assert_eq!(repos.len(), 1, "nothing commits when the human_id is taken");
    }

    #[tokio::test]
    async fn create_stamps_block_provenance_on_every_command() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_repository_change_set(
            &workspace,
            &session,
            RepositoryChangeSet {
                name: Some("Archive".to_owned()),
                provenance: Provenance {
                    confidence: Confidence::High,
                    rationale: Some("verified holding".to_owned()),
                    evidence_analysis: None,
                },
                ..draft()
            },
        )
        .await
        .expect("create");
        let log = change_log_for_repository(&workspace, &human_id).await.expect("log");
        assert!(!log.is_empty());
        for entry in &log {
            assert_eq!(entry.confidence, Confidence::High);
            assert_eq!(entry.rationale.as_deref(), Some("verified holding"));
        }
    }
}
