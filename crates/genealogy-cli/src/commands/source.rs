//! Source subcommands.

use clap::Subcommand;
use genealogy_app::{AppError, NewSource, Session, Workspace, create_source, list_sources, set_title, show_source};

use crate::i18n::Localizer;

/// Source subcommands.
#[derive(Subcommand)]
pub enum SourceCmd {
    /// Create a new source (auto-assigns a human id unless `--id` is given).
    Create {
        /// A specific human id (e.g. `S0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// An initial bibliographic title.
        #[arg(long)]
        title: Option<String>,
    },
    /// Set (or change) an existing source's title.
    SetTitle {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The bibliographic title.
        title: String,
    },
    /// Show one source.
    Show {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
    },
    /// List all sources.
    List,
}

/// Runs a source subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: SourceCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        SourceCmd::Create { id, title } => {
            let human_id = create_source(workspace, session, NewSource { human_id: id, title }).await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        SourceCmd::SetTitle { human_id, title } => {
            set_title(workspace, session, &human_id, title).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        SourceCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one source, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_source(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.source_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::SourceNotFound(human_id.to_owned())),
    }
}

/// Renders every source, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let sources = list_sources(workspace).await?;
    if sources.is_empty() {
        println!("{}", localizer.source_list_empty());
        return Ok(());
    }
    for summary in &sources {
        println!("{}", localizer.source_summary_line(summary));
    }
    Ok(())
}
