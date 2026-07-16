//! The media change-set use-case (Phase 5): a deferred create that commits a media object's location
//! (a file path or a web reference) and MIME type in a single operator action.
//!
//! Mirrors [`crate::source_change_set`]: the form buffers every field and persists nothing until Save
//! (`record-editing.html` §6). This module validates the optional `human_id` override up front, then
//! emits `CreateMedia`, a `SetPath` for a file path and/or a web path the operator entered, and an
//! optional `SetMime`. A media object holds one location, so when both a file path and a web path are
//! given the web path is asserted last. No date rides here — it is asserted afterwards via
//! [`crate::media::assert_media_date_value`]. Provenance follows the shared change-set rule
//! ([`crate::change_set`]). Editing an existing media
//! object is the per-field `dispatch_media_edit` path (PR27), not this create.

use genealogy_core::ids::HumanId;
use genealogy_core::media::command::{MediaCommand, MediaCommandEnvelope};
use genealogy_core::media_path::MediaPath;
use genealogy_core::provenance::EvidenceRef;
use genealogy_core::text::Url;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// The desired end state of a new media object, committed as one operator action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaChangeSet {
    /// A caller-supplied `human_id` override (dup-checked before any write); `None` auto-allocates.
    pub human_id: Option<String>,
    /// A local file path for the media (asserted as a `MediaPath::File`).
    pub file_path: Option<String>,
    /// A web reference for the media (asserted as a `MediaPath::Web`).
    pub web_path: Option<String>,
    /// The MIME type (e.g. `image/jpeg`).
    pub mime: Option<String>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// Commits a [`MediaChangeSet`]: creates the media object and emits a setter for each filled field.
///
/// Returns the media object's `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied `human_id` is in use (validated before any write),
/// [`AppError::CitationNotFound`] if a backing citation is unknown, [`AppError::MediaDomain`] on a
/// domain rejection, or a workspace/store error.
pub async fn commit_media_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: MediaChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match &change_set.human_id {
        Some(id) => {
            if store.find_media(id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id.clone()));
            }
            id.clone()
        }
        None => store.next_media_human_id(&workspace.media_id_format()?).await?,
    };
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    let media_id = session.new_media_id();
    let aggregate_id = media_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        MediaCommand::CreateMedia {
            media_id,
            human_id: HumanId::new(&human_id),
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;

    if let Some(file_path) = change_set.file_path {
        execute(
            store,
            session,
            &aggregate_id,
            MediaCommand::SetPath {
                media_id,
                path: MediaPath::File(file_path),
            },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(href) = change_set.web_path {
        execute(
            store,
            session,
            &aggregate_id,
            MediaCommand::SetPath {
                media_id,
                path: MediaPath::Web(Url {
                    url_type: None,
                    href,
                    description: None,
                }),
            },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(mime) = change_set.mime {
        execute(
            store,
            session,
            &aggregate_id,
            MediaCommand::SetMime { media_id, mime },
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
    command: MediaCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = MediaCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_media(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

#[cfg(test)]
mod tests {
    use super::{MediaChangeSet, commit_media_change_set};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::history::change_log_for_media;
    use crate::media::{NewMedia, create_media, list_media, show_media};
    use crate::session::Session;
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

    fn draft() -> MediaChangeSet {
        MediaChangeSet {
            human_id: None,
            file_path: None,
            web_path: None,
            mime: None,
            provenance: Provenance::default(),
            citations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn create_commits_the_file_path_and_mime() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_media_change_set(
            &workspace,
            &session,
            MediaChangeSet {
                file_path: Some("photos/ada.jpg".to_owned()),
                mime: Some("image/jpeg".to_owned()),
                ..draft()
            },
        )
        .await
        .expect("create");

        let media = show_media(&workspace, &human_id).await.expect("show").expect("media");
        assert_eq!(media.mime.as_deref(), Some("image/jpeg"));
        assert!(media.path.is_some(), "the file path is asserted");
    }

    #[tokio::test]
    async fn an_empty_draft_creates_a_bare_media() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_media_change_set(&workspace, &session, draft())
            .await
            .expect("create");
        let media = show_media(&workspace, &human_id).await.expect("show").expect("media");
        assert_eq!(media.path, None);
        assert_eq!(media.mime, None);
    }

    #[tokio::test]
    async fn a_taken_human_id_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let taken = create_media(
            &workspace,
            &session,
            NewMedia {
                human_id: None,
                path: Some("first.jpg".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("first media");
        let result = commit_media_change_set(
            &workspace,
            &session,
            MediaChangeSet {
                human_id: Some(taken),
                file_path: Some("clash.jpg".to_owned()),
                ..draft()
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::HumanIdTaken(_))));
        let media = list_media(&workspace).await.expect("media");
        assert_eq!(media.len(), 1, "nothing commits when the human_id is taken");
    }

    #[tokio::test]
    async fn create_stamps_block_provenance_on_every_command() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_media_change_set(
            &workspace,
            &session,
            MediaChangeSet {
                file_path: Some("scan.jpg".to_owned()),
                provenance: Provenance {
                    confidence: Some(Confidence::High),
                    rationale: Some("original scan".to_owned()),
                    evidence_analysis: None,
                },
                ..draft()
            },
        )
        .await
        .expect("create");
        let log = change_log_for_media(&workspace, &human_id).await.expect("log");
        assert!(!log.is_empty());
        for entry in &log {
            assert_eq!(entry.confidence, Some(Confidence::High));
            assert_eq!(entry.rationale.as_deref(), Some("original scan"));
        }
    }
}
