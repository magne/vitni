//! Source subcommands.

use clap::Subcommand;
use genealogy_app::{
    AppError, MediaRefInput, MutationMeta, NewSource, Provenance, Session, Workspace, add_source_attribute,
    attach_source_media, attach_source_note, create_source, link_source_repository, list_sources, set_source_abbrev,
    set_source_author, set_source_pub_info, set_title, show_source, tag_source,
};
use genealogy_core::ids::{MediaId, NoteId};
use uuid::Uuid;

use crate::args::SourceMediaTypeArg;
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
    /// Set (or change) an existing source's author.
    SetAuthor {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The author.
        author: String,
    },
    /// Set (or change) an existing source's publication info.
    SetPubInfo {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The publication info.
        pub_info: String,
    },
    /// Set (or change) an existing source's abbreviation.
    SetAbbrev {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The abbreviation.
        abbrev: String,
    },
    /// Link a source to a repository that holds it.
    LinkRepository {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The repository's human id (e.g. `R0001`).
        #[arg(long = "repository", value_name = "REPOSITORY_ID")]
        repository: String,
        /// The call number / shelf mark within the repository.
        #[arg(long)]
        call_number: Option<String>,
        /// The medium the source is held as.
        #[arg(long = "media-type", value_enum, default_value_t = SourceMediaTypeArg::Book)]
        media_type: SourceMediaTypeArg,
    },
    /// Add a typed attribute to a source.
    AddAttribute {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The attribute name/type.
        #[arg(long = "type", value_name = "TYPE")]
        attribute_type: String,
        /// The attribute value.
        #[arg(long)]
        value: String,
    },
    /// Attach a media reference to a source.
    AttachMedia {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The media aggregate id (UUID).
        #[arg(long)]
        media: Uuid,
        /// A caption specific to this use.
        #[arg(long)]
        caption: Option<String>,
    },
    /// Attach a note to a source.
    AttachNote {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The note aggregate id (UUID).
        #[arg(long)]
        note: Uuid,
    },
    /// Apply a tag to a source.
    Tag {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Remove a tag from a source.
    Untag {
        /// The source's human id (e.g. `S0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
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
    let meta = MutationMeta::default();
    match command {
        SourceCmd::Create { id, title } => {
            let human_id = create_source(
                workspace,
                session,
                NewSource { human_id: id, title },
                Provenance::default(),
                &[],
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        SourceCmd::SetTitle { human_id, title } => {
            set_title(workspace, session, &human_id, title, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::SetAuthor { human_id, author } => {
            set_source_author(workspace, session, &human_id, author, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::SetPubInfo { human_id, pub_info } => {
            set_source_pub_info(workspace, session, &human_id, pub_info, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::SetAbbrev { human_id, abbrev } => {
            set_source_abbrev(workspace, session, &human_id, abbrev, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::LinkRepository {
            human_id,
            repository,
            call_number,
            media_type,
        } => {
            link_source_repository(
                workspace,
                session,
                &human_id,
                &repository,
                call_number,
                media_type.into(),
                meta,
            )
            .await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::AddAttribute {
            human_id,
            attribute_type,
            value,
        } => {
            add_source_attribute(workspace, session, &human_id, attribute_type, value, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::AttachMedia {
            human_id,
            media,
            caption,
        } => {
            let input = MediaRefInput { crop: None, caption };
            attach_source_media(workspace, session, &human_id, MediaId::from_uuid(media), input, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::AttachNote { human_id, note } => {
            attach_source_note(workspace, session, &human_id, NoteId::from_uuid(note), meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::Tag { human_id, tag } => {
            tag_source(workspace, session, &human_id, &tag.to_string(), false, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        SourceCmd::Untag { human_id, tag } => {
            tag_source(workspace, session, &human_id, &tag.to_string(), true, meta).await?;
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
