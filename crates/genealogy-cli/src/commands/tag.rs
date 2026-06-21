//! Tag subcommands.
//!
//! Tags have no `human_id`; they are referenced by their aggregate id (a UUID), which `tag create`
//! prints on creation.

use clap::Subcommand;
use genealogy_app::{
    AppError, Session, Workspace, create_tag, list_tags, rename_tag, set_tag_color, set_tag_priority, show_tag,
};

use crate::i18n::Localizer;

/// Tag subcommands.
#[derive(Subcommand)]
pub enum TagCmd {
    /// Create a new tag with a name (prints the new tag's id).
    Create {
        /// The tag's name.
        #[arg(long)]
        name: String,
    },
    /// Rename an existing tag.
    Rename {
        /// The tag's id (a UUID).
        id: String,
        /// The new name.
        #[arg(long)]
        name: String,
    },
    /// Set (or change) a tag's colour.
    SetColor {
        /// The tag's id (a UUID).
        id: String,
        /// The colour (e.g. a hex string like `#1f77b4`).
        #[arg(long)]
        color: String,
    },
    /// Set (or change) a tag's sort priority.
    SetPriority {
        /// The tag's id (a UUID).
        id: String,
        /// The priority (lower sorts first).
        #[arg(long)]
        priority: i32,
    },
    /// Show one tag.
    Show {
        /// The tag's id (a UUID).
        id: String,
    },
    /// List all tags.
    List,
}

/// Runs a tag subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: TagCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        TagCmd::Create { name } => {
            let id = create_tag(workspace, session, name).await?;
            println!("{}", localizer.created(&id));
            Ok(())
        }
        TagCmd::Rename { id, name } => {
            rename_tag(workspace, session, &id, name).await?;
            println!("{}", localizer.updated(&id));
            Ok(())
        }
        TagCmd::SetColor { id, color } => {
            set_tag_color(workspace, session, &id, color).await?;
            println!("{}", localizer.updated(&id));
            Ok(())
        }
        TagCmd::SetPriority { id, priority } => {
            set_tag_priority(workspace, session, &id, priority).await?;
            println!("{}", localizer.updated(&id));
            Ok(())
        }
        TagCmd::Show { id } => show(workspace, &id, localizer).await,
        TagCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one tag, or errors if absent.
async fn show(workspace: &Workspace, id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_tag(workspace, id).await? {
        Some(summary) => {
            println!("{}", localizer.tag_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::TagNotFound(id.to_owned())),
    }
}

/// Renders every tag.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let tags = list_tags(workspace).await?;
    if tags.is_empty() {
        println!("{}", localizer.tag_list_empty());
        return Ok(());
    }
    for summary in &tags {
        println!("{}", localizer.tag_summary_line(summary));
    }
    Ok(())
}
