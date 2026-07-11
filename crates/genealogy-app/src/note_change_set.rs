//! The note change-set use-case (Phase 5): a deferred create that commits a note's type and
//! language-tagged content in a single operator action.
//!
//! Mirrors [`crate::source_change_set`]: the form buffers every field and persists nothing until Save
//! (`record-editing.html` §6). This module validates the optional `human_id` override up front, then
//! emits `CreateNote`, an optional `SetNoteType`, and — when the operator entered content —
//! `SetRichText` carrying the Markdown body and its optional BCP-47 language. Provenance follows the
//! shared change-set rule ([`crate::change_set`]). Editing an existing note is the per-field
//! `dispatch_note_edit` path (PR27), not this create.

use genealogy_core::enums::NoteType;
use genealogy_core::ids::HumanId;
use genealogy_core::name::LanguageTag;
use genealogy_core::note::command::{NoteCommand, NoteCommandEnvelope};
use genealogy_core::provenance::CitationRef;
use genealogy_core::text::{MediaType, RichText};
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// The desired end state of a new note, committed as one operator action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteChangeSet {
    /// A caller-supplied `human_id` override (dup-checked before any write); `None` auto-allocates.
    pub human_id: Option<String>,
    /// The note type (general, research, transcript, …).
    pub note_type: Option<NoteType>,
    /// The note's Markdown content; blank writes no `SetRichText`.
    pub text: Option<String>,
    /// The content's BCP-47 language, applied to the `RichText` when `text` is present.
    pub language: Option<String>,
    /// The operator intent stamped on every emitted command (`record-editing.html` §5b).
    pub provenance: Provenance,
    /// Citation `human_id`s backing every non-`Create*` command; resolved before any write.
    pub citations: Vec<String>,
}

/// Commits a [`NoteChangeSet`]: creates the note and emits a setter for each filled field.
///
/// Returns the note's `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied `human_id` is in use (validated before any write),
/// [`AppError::CitationNotFound`] if a backing citation is unknown, [`AppError::NoteDomain`] on a
/// domain rejection, or a workspace/store error.
pub async fn commit_note_change_set(
    workspace: &Workspace,
    session: &Session,
    change_set: NoteChangeSet,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match &change_set.human_id {
        Some(id) => {
            if store.find_note(id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id.clone()));
            }
            id.clone()
        }
        None => store.next_note_human_id(&workspace.note_id_format()?).await?,
    };
    let block = use_case::resolve_citation_refs(store, &change_set.citations).await?;

    let note_id = session.new_note_id();
    let aggregate_id = note_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        NoteCommand::CreateNote {
            note_id,
            human_id: HumanId::new(&human_id),
        },
        change_set.provenance.clone(),
        Vec::new(),
    )
    .await?;

    if let Some(note_type) = change_set.note_type {
        execute(
            store,
            session,
            &aggregate_id,
            NoteCommand::SetNoteType { note_id, note_type },
            change_set.provenance.clone(),
            block.clone(),
        )
        .await?;
    }
    if let Some(text) = change_set.text {
        let rich = RichText {
            text,
            media_type: MediaType::Markdown,
            language: change_set.language.map(LanguageTag::new),
            translator: None,
            translations: Vec::new(),
        };
        execute(
            store,
            session,
            &aggregate_id,
            NoteCommand::SetRichText { note_id, text: rich },
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
    command: NoteCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = NoteCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_note(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

#[cfg(test)]
mod tests {
    use super::{NoteChangeSet, commit_note_change_set};
    use crate::config::{AppDefaults, OperatorConfig, WorkspaceDefaults};
    use crate::history::change_log_for_note;
    use crate::note::{NewNote, create_note, list_notes, show_note};
    use crate::session::Session;
    use crate::use_case::Provenance;
    use crate::workspace::Workspace;
    use genealogy_core::enums::NoteType;
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

    fn draft() -> NoteChangeSet {
        NoteChangeSet {
            human_id: None,
            note_type: None,
            text: None,
            language: None,
            provenance: Provenance::default(),
            citations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn create_commits_the_type_and_content() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_note_change_set(
            &workspace,
            &session,
            NoteChangeSet {
                note_type: Some(NoteType::Research),
                text: Some("An estate inventory".to_owned()),
                language: Some("en".to_owned()),
                ..draft()
            },
        )
        .await
        .expect("create");

        let note = show_note(&workspace, &human_id).await.expect("show").expect("note");
        assert_eq!(note.note_type, Some(NoteType::Research));
        assert_eq!(note.text.as_deref(), Some("An estate inventory"));
    }

    #[tokio::test]
    async fn an_empty_draft_creates_a_bare_note() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_note_change_set(&workspace, &session, draft())
            .await
            .expect("create");
        let note = show_note(&workspace, &human_id).await.expect("show").expect("note");
        assert_eq!(note.text, None);
    }

    #[tokio::test]
    async fn a_taken_human_id_is_rejected_and_nothing_commits() {
        let (workspace, session, _dir) = setup().await;
        let taken = create_note(
            &workspace,
            &session,
            NewNote {
                human_id: None,
                text: Some("First".to_owned()),
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("first note");
        let result = commit_note_change_set(
            &workspace,
            &session,
            NoteChangeSet {
                human_id: Some(taken),
                text: Some("Clash".to_owned()),
                ..draft()
            },
        )
        .await;
        assert!(matches!(result, Err(crate::error::AppError::HumanIdTaken(_))));
        let notes = list_notes(&workspace).await.expect("notes");
        assert_eq!(notes.len(), 1, "nothing commits when the human_id is taken");
    }

    #[tokio::test]
    async fn create_stamps_block_provenance_on_every_command() {
        let (workspace, session, _dir) = setup().await;
        let human_id = commit_note_change_set(
            &workspace,
            &session,
            NoteChangeSet {
                text: Some("Body".to_owned()),
                provenance: Provenance {
                    confidence: Some(Confidence::High),
                    rationale: Some("transcribed".to_owned()),
                    evidence_analysis: None,
                },
                ..draft()
            },
        )
        .await
        .expect("create");
        let log = change_log_for_note(&workspace, &human_id).await.expect("log");
        assert!(!log.is_empty());
        for entry in &log {
            assert_eq!(entry.confidence, Some(Confidence::High));
            assert_eq!(entry.rationale.as_deref(), Some("transcribed"));
        }
    }
}
