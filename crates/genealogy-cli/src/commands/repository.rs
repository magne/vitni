//! Repository subcommands.

use clap::Subcommand;
use genealogy_app::{
    Address, AppError, NewRepository, Session, Url, Workspace, add_repository_address, add_repository_url,
    attach_repository_note, create_repository, list_repositories, set_repository_name, set_repository_type,
    show_repository, tag_repository,
};
use genealogy_core::ids::{NoteId, TagId};
use uuid::Uuid;

use crate::args::RepositoryTypeArg;
use crate::i18n::Localizer;

/// Repository subcommands.
#[derive(Subcommand)]
pub enum RepositoryCmd {
    /// Create a new repository (auto-assigns a human id unless `--id` is given).
    Create {
        /// A specific human id (e.g. `R0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// An initial name for the repository.
        #[arg(long)]
        name: Option<String>,
    },
    /// Set (or change) an existing repository's type.
    SetType {
        /// The repository's human id (e.g. `R0001`).
        human_id: String,
        /// The new repository type.
        #[arg(long, value_enum)]
        r#type: RepositoryTypeArg,
    },
    /// Set (or change) an existing repository's name.
    SetName {
        /// The repository's human id (e.g. `R0001`).
        human_id: String,
        /// The name.
        name: String,
    },
    /// Add a postal address to a repository.
    AddAddress {
        /// The repository's human id (e.g. `R0001`).
        human_id: String,
        /// The street address line.
        #[arg(long)]
        street: Option<String>,
        /// The locality (city / town).
        #[arg(long)]
        locality: Option<String>,
        /// The region (county / state).
        #[arg(long)]
        region: Option<String>,
        /// The postal code.
        #[arg(long)]
        postal_code: Option<String>,
        /// The country.
        #[arg(long)]
        country: Option<String>,
    },
    /// Add a URL to a repository.
    AddUrl {
        /// The repository's human id (e.g. `R0001`).
        human_id: String,
        /// The URL.
        href: String,
        /// The kind of URL (e.g. `home page`).
        #[arg(long = "type")]
        url_type: Option<String>,
        /// A human-readable description.
        #[arg(long)]
        description: Option<String>,
    },
    /// Attach a note to a repository.
    AttachNote {
        /// The repository's human id (e.g. `R0001`).
        human_id: String,
        /// The note aggregate id (UUID).
        #[arg(long)]
        note: Uuid,
    },
    /// Apply a tag to a repository.
    Tag {
        /// The repository's human id (e.g. `R0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Remove a tag from a repository.
    Untag {
        /// The repository's human id (e.g. `R0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Show one repository.
    Show {
        /// The repository's human id (e.g. `R0001`).
        human_id: String,
    },
    /// List all repositories.
    List,
}

/// Runs a repository subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: RepositoryCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        RepositoryCmd::Create { id, name } => {
            let human_id = create_repository(workspace, session, NewRepository { human_id: id, name }).await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        RepositoryCmd::SetType { human_id, r#type } => {
            set_repository_type(workspace, session, &human_id, r#type.into()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        RepositoryCmd::SetName { human_id, name } => {
            set_repository_name(workspace, session, &human_id, name).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        RepositoryCmd::AddAddress {
            human_id,
            street,
            locality,
            region,
            postal_code,
            country,
        } => {
            let address = Address {
                lines: street.into_iter().collect(),
                locality,
                region,
                postal_code,
                country,
                ..Address::default()
            };
            add_repository_address(workspace, session, &human_id, address).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        RepositoryCmd::AddUrl {
            human_id,
            href,
            url_type,
            description,
        } => {
            let url = Url {
                url_type,
                href,
                description,
            };
            add_repository_url(workspace, session, &human_id, url).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        RepositoryCmd::AttachNote { human_id, note } => {
            attach_repository_note(workspace, session, &human_id, NoteId::from_uuid(note)).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        RepositoryCmd::Tag { human_id, tag } => {
            tag_repository(workspace, session, &human_id, TagId::from_uuid(tag), false).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        RepositoryCmd::Untag { human_id, tag } => {
            tag_repository(workspace, session, &human_id, TagId::from_uuid(tag), true).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        RepositoryCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        RepositoryCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one repository, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_repository(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.repository_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::RepositoryNotFound(human_id.to_owned())),
    }
}

/// Renders every repository, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let repositories = list_repositories(workspace).await?;
    if repositories.is_empty() {
        println!("{}", localizer.repository_list_empty());
        return Ok(());
    }
    for summary in &repositories {
        println!("{}", localizer.repository_summary_line(summary));
    }
    Ok(())
}
