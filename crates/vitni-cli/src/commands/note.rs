//! Note subcommands.

use clap::Subcommand;
use uuid::Uuid;
use vitni_app::{
    AppError, ListPage, MatchableKind, MutationMeta, NewNote, Provenance, Session, Workspace, create_note,
    list_notes_page, set_note_restrictions, set_note_text, set_note_type, show_note, tag_note,
};

use crate::args::{ListArgs, NoteTypeArg, RestrictionsArgs};
use crate::commands::similar::hint_similar;
use crate::i18n::Localizer;

/// Note subcommands.
#[derive(Subcommand)]
pub enum NoteCmd {
    /// Create a new note (auto-assigns a human id unless `--id` is given).
    Create {
        /// A specific human id (e.g. `N0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// Initial Markdown text for the note.
        #[arg(long)]
        text: Option<String>,
    },
    /// Set (or change) an existing note's type.
    SetType {
        /// The note's human id (e.g. `N0001`).
        human_id: String,
        /// The new note type.
        #[arg(long, value_enum)]
        r#type: NoteTypeArg,
    },
    /// Set (or change) an existing note's Markdown text.
    SetText {
        /// The note's human id (e.g. `N0001`).
        human_id: String,
        /// The Markdown text.
        text: String,
    },
    /// Apply a tag to a note.
    Tag {
        /// The note's human id (e.g. `N0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Remove a tag from a note.
    Untag {
        /// The note's human id (e.g. `N0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Set (or clear) a note's privacy restrictions, replacing its current set.
    SetRestrictions {
        /// The note's human id (e.g. `N0001`).
        human_id: String,
        #[command(flatten)]
        restrictions: RestrictionsArgs,
    },
    /// Show one note.
    Show {
        /// The note's human id (e.g. `N0001`).
        human_id: String,
    },
    /// List the notes by human id, all of them or one window.
    List(ListArgs),
}

/// Runs a note subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: NoteCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        NoteCmd::Create { id, text } => {
            let human_id = create_note(
                workspace,
                session,
                NewNote { human_id: id, text },
                Provenance::default(),
                &[],
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            hint_similar(workspace, MatchableKind::Note, &human_id, localizer).await;
            Ok(())
        }
        NoteCmd::SetType { human_id, r#type } => {
            set_note_type(workspace, session, &human_id, r#type.into(), MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        NoteCmd::SetText { human_id, text } => {
            set_note_text(workspace, session, &human_id, text, None, MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        NoteCmd::Tag { human_id, tag } => {
            tag_note(
                workspace,
                session,
                &human_id,
                &tag.to_string(),
                false,
                MutationMeta::default(),
            )
            .await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        NoteCmd::Untag { human_id, tag } => {
            tag_note(
                workspace,
                session,
                &human_id,
                &tag.to_string(),
                true,
                MutationMeta::default(),
            )
            .await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        NoteCmd::SetRestrictions { human_id, restrictions } => {
            restrict(workspace, session, &human_id, restrictions, localizer).await
        }
        NoteCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        NoteCmd::List(window) => list(workspace, window.page(), localizer).await,
    }
}

/// Replaces a note's privacy restrictions and reports the update.
async fn restrict(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: RestrictionsArgs,
    localizer: &Localizer,
) -> Result<(), AppError> {
    set_note_restrictions(
        workspace,
        session,
        human_id,
        restrictions.into_set(),
        MutationMeta::default(),
    )
    .await?;
    println!("{}", localizer.updated(human_id));
    Ok(())
}

/// Renders one note, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_note(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.note_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::NoteNotFound(human_id.to_owned())),
    }
}

/// Renders the `page` of note records, ordered by human id.
async fn list(workspace: &Workspace, page: ListPage, localizer: &Localizer) -> Result<(), AppError> {
    let notes = list_notes_page(workspace, page).await?;
    if notes.is_empty() {
        println!("{}", localizer.empty_page(page, localizer.note_list_empty()));
        return Ok(());
    }
    for summary in &notes {
        println!("{}", localizer.note_summary_line(summary));
    }
    Ok(())
}
