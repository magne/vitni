//! The source change-set use-case (Phase 5): a deferred create that commits a source's title,
//! author, publication info, and abbreviation in a single operator action.
//!
//! The Dioxus source form buffers every field locally and persists nothing until Save
//! (`record-editing.html` §6). On Save the app is handed the *desired* end state; this module
//! validates the optional `human_id` override up front (before any write) and emits `CreateSource`
//! followed by a setter for each field the operator filled — a bare source when the draft is empty.
//! Editing an existing source is the per-field `dispatch_source_edit` path (PR27), not this create.
//!
//! Provenance follows the shared change-set rule ([`crate::change_set`]): the operator [`Provenance`]
//! is stamped on every command and the backing citations ride on every non-`Create*` command,
//! resolved before any write.

use genealogy_core::ids::{HumanId, SourceId};
use genealogy_core::provenance::EvidenceRef;
use genealogy_core::source::command::{SourceCommand, SourceCommandEnvelope};
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// The desired end state of a new source, committed as one operator action. Every field is optional;
/// an all-empty draft creates a bare source (only `CreateSource` is emitted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChangeSet {
    /// A caller-supplied `human_id` override (dup-checked before any write); `None` auto-allocates.
    pub human_id: Option<String>,
    /// The bibliographic title.
    pub title: Option<String>,
    /// The author.
    pub author: Option<String>,
    /// The publication info.
    pub publication: Option<String>,
    /// The abbreviation.
    pub abbreviation: Option<String>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// Commits a [`SourceChangeSet`]: creates the source and emits a setter for each filled field.
///
/// Returns the source's `human_id` (the assigned one).
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied `human_id` is in use (validated before any write, so
/// nothing commits), [`AppError::CitationNotFound`] if a backing citation is unknown,
/// [`AppError::SourceDomain`] on a domain rejection, or a workspace/store error.
pub async fn commit_source_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: SourceChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match &change_set.human_id {
        Some(id) => {
            if store.find_source(id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id.clone()));
            }
            id.clone()
        }
        None => store.next_source_human_id(&workspace.source_id_format()?).await?,
    };
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    let source_id = session.new_source_id();
    let aggregate_id = source_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        SourceCommand::CreateSource {
            source_id,
            human_id: HumanId::new(&human_id),
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;

    for command in field_commands(source_id, &change_set) {
        execute(
            store,
            session,
            &aggregate_id,
            command,
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    Ok(human_id)
}

/// The setter commands for the fields the operator filled, in a stable order.
fn field_commands(source_id: SourceId, change_set: &SourceChangeSet) -> Vec<SourceCommand> {
    let mut commands = Vec::new();
    if let Some(title) = change_set.title.clone() {
        commands.push(SourceCommand::SetTitle { source_id, title });
    }
    if let Some(author) = change_set.author.clone() {
        commands.push(SourceCommand::SetAuthor { source_id, author });
    }
    if let Some(pub_info) = change_set.publication.clone() {
        commands.push(SourceCommand::SetPubInfo { source_id, pub_info });
    }
    if let Some(abbrev) = change_set.abbreviation.clone() {
        commands.push(SourceCommand::SetAbbrev { source_id, abbrev });
    }
    commands
}

/// Executes one command through the store, stamping the operator `provenance` and backing
/// `citations`, and mapping the command outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: SourceCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = SourceCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_source(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

#[cfg(test)]
mod tests {
    use super::{SourceChangeSet, commit_source_change_set};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::history::change_log_for_source;
    use crate::session::Session;
    use crate::source::{NewSource, create_source, list_sources, show_source};
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
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

    fn draft() -> SourceChangeSet {
        SourceChangeSet {
            human_id: None,
            title: None,
            author: None,
            publication: None,
            abbreviation: None,
            provenance: Provenance::default(),
            citations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn create_commits_a_setter_for_each_filled_field() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_source_change_set(
            &workspace,
            &session,
            SourceChangeSet {
                title: Some("Trinity Church baptisms".to_owned()),
                author: Some("Rev. Smith".to_owned()),
                publication: Some("vol. 3".to_owned()),
                abbreviation: Some("TCB".to_owned()),
                ..draft()
            },
        )
        .await
        .expect("create");

        let source = show_source(&workspace, &human_id).await.expect("show").expect("source");
        assert_eq!(source.title.as_deref(), Some("Trinity Church baptisms"));
        assert_eq!(source.author.as_deref(), Some("Rev. Smith"));
        assert_eq!(source.pub_info.as_deref(), Some("vol. 3"));
        assert_eq!(source.abbrev.as_deref(), Some("TCB"));
    }

    #[tokio::test]
    async fn an_empty_draft_creates_a_bare_source() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_source_change_set(&workspace, &session, draft())
            .await
            .expect("create");
        let source = show_source(&workspace, &human_id).await.expect("show").expect("source");
        assert_eq!(source.title, None);
        assert_eq!(source.author, None);
    }

    #[tokio::test]
    async fn a_taken_human_id_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let taken = create_source(
            &workspace,
            &session,
            NewSource {
                human_id: None,
                title: Some("First".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("first source");

        let result = commit_source_change_set(
            &workspace,
            &session,
            SourceChangeSet {
                human_id: Some(taken),
                title: Some("Clash".to_owned()),
                ..draft()
            },
        )
        .await;

        assert!(matches!(result, Err(crate::error::AppError::HumanIdTaken(_))));
        let sources = list_sources(&workspace).await.expect("sources");
        assert_eq!(sources.len(), 1, "nothing commits when the human_id is taken");
    }

    #[tokio::test]
    async fn create_stamps_block_provenance_on_every_command() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_source_change_set(
            &workspace,
            &session,
            SourceChangeSet {
                title: Some("Register".to_owned()),
                provenance: Provenance {
                    confidence: Some(Confidence::High),
                    rationale: Some("primary source".to_owned()),
                    evidence_analysis: None,
                },
                ..draft()
            },
        )
        .await
        .expect("create");

        let log = change_log_for_source(&workspace, &human_id).await.expect("log");
        assert!(!log.is_empty());
        for entry in &log {
            assert_eq!(entry.confidence, Some(Confidence::High));
            assert_eq!(entry.rationale.as_deref(), Some("primary source"));
        }
    }

    #[tokio::test]
    async fn create_with_an_unknown_block_citation_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let result = commit_source_change_set(
            &workspace,
            &session,
            SourceChangeSet {
                title: Some("Ghost".to_owned()),
                citations: vec!["C9999".to_owned()],
                ..draft()
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::CitationNotFound(_))));
        let sources = list_sources(&workspace).await.expect("sources");
        assert!(sources.is_empty(), "nothing commits when a block citation is unknown");
    }
}
