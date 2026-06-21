//! Media subcommands.

use clap::Subcommand;
use genealogy_app::{
    AppError, DateParts, NewMedia, Session, Workspace, add_media_attribute, add_media_citation, assert_media_date,
    attach_media_note, create_media, list_media, set_media_checksum, set_media_file_path, set_media_web_path,
    show_media, tag_media,
};
use genealogy_core::ids::{NoteId, TagId};
use uuid::Uuid;

use crate::i18n::Localizer;

/// Media subcommands.
#[derive(Subcommand)]
pub enum MediaCmd {
    /// Create a new media object (auto-assigns a human id unless `--id` is given).
    Create {
        /// A specific human id (e.g. `O0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// An initial file path for the media.
        #[arg(long)]
        path: Option<String>,
    },
    /// Set (or change) a media object's file path.
    SetPath {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
        /// The file path.
        path: String,
    },
    /// Set (or change) a media object's web reference.
    SetWeb {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
        /// The web URL.
        href: String,
    },
    /// Set (or change) a media object's checksum.
    SetChecksum {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
        /// The checksum.
        checksum: String,
    },
    /// Assert a media object's date (Gregorian; year required, month/day optional).
    AssertDate {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
        /// The year (negative for BCE).
        #[arg(long)]
        year: i32,
        /// The month, 1–12.
        #[arg(long)]
        month: Option<u8>,
        /// The day, 1–31.
        #[arg(long)]
        day: Option<u8>,
    },
    /// Add a typed attribute to a media object.
    AddAttribute {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
        /// The attribute name / type.
        attribute_type: String,
        /// The attribute value.
        value: String,
    },
    /// Add a citation backing a media object's claims.
    AddCitation {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
        /// The citation's human id (e.g. `C0001`).
        #[arg(long = "citation", value_name = "CITATION_ID")]
        citation: String,
    },
    /// Attach a note to a media object.
    AttachNote {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
        /// The note aggregate id (UUID).
        #[arg(long)]
        note: Uuid,
    },
    /// Apply a tag to a media object.
    Tag {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Remove a tag from a media object.
    Untag {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Show one media object.
    Show {
        /// The media's human id (e.g. `O0001`).
        human_id: String,
    },
    /// List all media objects.
    List,
}

/// Runs a media subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: MediaCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        MediaCmd::Create { id, path } => {
            let human_id = create_media(workspace, session, NewMedia { human_id: id, path }).await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        MediaCmd::SetPath { human_id, path } => {
            set_media_file_path(workspace, session, &human_id, path).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        MediaCmd::SetWeb { human_id, href } => {
            set_media_web_path(workspace, session, &human_id, href).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        MediaCmd::SetChecksum { human_id, checksum } => {
            set_media_checksum(workspace, session, &human_id, checksum).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        MediaCmd::AssertDate {
            human_id,
            year,
            month,
            day,
        } => {
            assert_media_date(workspace, session, &human_id, DateParts { year, month, day }).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        MediaCmd::AddAttribute {
            human_id,
            attribute_type,
            value,
        } => {
            add_media_attribute(workspace, session, &human_id, attribute_type, value).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        MediaCmd::AddCitation { human_id, citation } => {
            add_media_citation(workspace, session, &human_id, &citation).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        MediaCmd::AttachNote { human_id, note } => {
            attach_media_note(workspace, session, &human_id, NoteId::from_uuid(note)).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        MediaCmd::Tag { human_id, tag } => {
            tag_media(workspace, session, &human_id, TagId::from_uuid(tag), false).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        MediaCmd::Untag { human_id, tag } => {
            tag_media(workspace, session, &human_id, TagId::from_uuid(tag), true).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        MediaCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        MediaCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one media object, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_media(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.media_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::MediaNotFound(human_id.to_owned())),
    }
}

/// Renders every media object, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let media = list_media(workspace).await?;
    if media.is_empty() {
        println!("{}", localizer.media_list_empty());
        return Ok(());
    }
    for summary in &media {
        println!("{}", localizer.media_summary_line(summary));
    }
    Ok(())
}
