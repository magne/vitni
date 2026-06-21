//! Note use-cases (ADR 0006): create, set type, set rich text, tag, show, and list.

use genealogy_core::enums::NoteType;
use genealogy_core::ids::{HumanId, NoteId, TagId};
use genealogy_core::note::NoteView;
use genealogy_core::note::command::{NoteCommand, NoteCommandEnvelope};
use genealogy_core::provenance::Confidence;
use genealogy_core::text::{MediaType, RichText};
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a note (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSummary {
    /// The user-facing identifier (e.g. `N0001`).
    pub human_id: String,
    /// The note's type. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub note_type: Option<NoteType>,
    /// The note's text content, if set.
    pub text: Option<String>,
}

/// What to create a note with (the auto/override `human_id` and optional initial text).
#[derive(Debug, Clone)]
pub struct NewNote {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// Optional initial Markdown text for an initial `SetRichText`.
    pub text: Option<String>,
}

/// Creates a note, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::NoteDomain`] if a domain rule
/// rejects the command, or a workspace/store error.
pub async fn create_note(workspace: &Workspace, session: &Session, new: NewNote) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_note(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_note_human_id(&workspace.note_id_format()?).await?,
    };

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
    )
    .await?;

    if let Some(text) = new.text {
        execute(
            store,
            session,
            &aggregate_id,
            NoteCommand::SetRichText {
                note_id,
                text: markdown(text),
            },
        )
        .await?;
    }

    Ok(human_id)
}

/// Sets (or changes) a note's type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or a workspace/store error.
pub async fn set_note_type(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_type: NoteType,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    execute(
        store,
        session,
        &note_id.to_string(),
        NoteCommand::SetNoteType { note_id, note_type },
    )
    .await
}

/// Sets (or changes) a note's Markdown text, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or a workspace/store error.
pub async fn set_note_text(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    text: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    execute(
        store,
        session,
        &note_id.to_string(),
        NoteCommand::SetRichText {
            note_id,
            text: markdown(text),
        },
    )
    .await
}

/// Applies (or removes) a tag on a note, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::NoteNotFound`] if no such note exists, or a workspace/store error.
pub async fn tag_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = resolve_note_id(store, human_id).await?;
    let command = if remove {
        NoteCommand::Untag { note_id, tag_id }
    } else {
        NoteCommand::Tag { note_id, tag_id }
    };
    execute(store, session, &note_id.to_string(), command).await
}

/// Loads a single note's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_note(workspace: &Workspace, human_id: &str) -> Result<Option<NoteSummary>, AppError> {
    let found = workspace.store().find_note(human_id).await?;
    Ok(found.as_ref().map(summarize))
}

/// Lists every note's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_notes(workspace: &Workspace) -> Result<Vec<NoteSummary>, AppError> {
    let views = workspace.store().list_notes().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view));
    }
    Ok(summaries)
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: NoteCommand) -> Result<(), AppError> {
    let envelope = NoteCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_note(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Resolves a `human_id` to its aggregate [`NoteId`], or [`AppError::NoteNotFound`].
async fn resolve_note_id(store: &Store, human_id: &str) -> Result<NoteId, AppError> {
    use_case::resolve_id(store.find_note(human_id).await?, NoteView::note_id, || {
        AppError::NoteNotFound(human_id.to_owned())
    })
}

/// Builds a Markdown [`RichText`] from plain text (language is not collected by the CLI yet).
fn markdown(text: String) -> RichText {
    RichText {
        text,
        media_type: MediaType::Markdown,
        language: None,
    }
}

/// Renders a [`NoteView`] into the frontend DTO.
fn summarize(view: &NoteView) -> NoteSummary {
    NoteSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        note_type: view.note_type().cloned(),
        text: view.text().map(|t| t.text.clone()),
    }
}
