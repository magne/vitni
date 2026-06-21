//! Citation subcommands.

use clap::Subcommand;
use genealogy_app::{
    AppError, NewCitation, Session, Workspace, create_citation, list_citations, set_page, show_citation,
};

use crate::i18n::Localizer;

/// Citation subcommands.
#[derive(Subcommand)]
pub enum CitationCmd {
    /// Create a new citation against a source (auto-assigns a human id unless `--id` is given).
    Create {
        /// The cited source's human id (e.g. `S0001`).
        #[arg(long, value_name = "SOURCE_ID")]
        source: String,
        /// A specific human id (e.g. `C0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// An initial page / locator within the source.
        #[arg(long)]
        page: Option<String>,
    },
    /// Set (or change) an existing citation's page / locator.
    SetPage {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
        /// The page / locator text.
        page: String,
    },
    /// Show one citation.
    Show {
        /// The citation's human id (e.g. `C0001`).
        human_id: String,
    },
    /// List all citations.
    List,
}

/// Runs a citation subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: CitationCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        CitationCmd::Create { source, id, page } => {
            let human_id = create_citation(
                workspace,
                session,
                NewCitation {
                    human_id: id,
                    source,
                    page,
                },
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        CitationCmd::SetPage { human_id, page } => {
            set_page(workspace, session, &human_id, page).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        CitationCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        CitationCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one citation, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_citation(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.citation_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::CitationNotFound(human_id.to_owned())),
    }
}

/// Renders every citation, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let citations = list_citations(workspace).await?;
    if citations.is_empty() {
        println!("{}", localizer.citation_list_empty());
        return Ok(());
    }
    for summary in &citations {
        println!("{}", localizer.citation_summary_line(summary));
    }
    Ok(())
}
