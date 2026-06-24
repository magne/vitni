//! Place subcommands.

use clap::Subcommand;
use genealogy_app::{
    AppError, NewPlace, Session, Workspace, add_place_citation, add_place_name, assert_place_coordinates,
    assert_place_enclosed_by, attach_place_media, attach_place_note, create_place, list_places, set_place_code,
    set_place_type, show_place, tag_place,
};
use genealogy_core::geo::{GeoCoordinates, Microdegrees};
use genealogy_core::ids::{MediaId, NoteId};
use uuid::Uuid;

use crate::args::PlaceTypeArg;
use crate::i18n::Localizer;

/// Place subcommands.
#[derive(Subcommand)]
pub enum PlaceCmd {
    /// Create a new place (auto-assigns a human id unless `--id` is given).
    Create {
        /// A specific human id (e.g. `P0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// The place's type.
        #[arg(long, value_enum, default_value_t = PlaceTypeArg::Parish)]
        r#type: PlaceTypeArg,
        /// An initial name for the place.
        #[arg(long)]
        name: Option<String>,
    },
    /// Set (or change) an existing place's type.
    SetType {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The new place type.
        #[arg(long, value_enum)]
        r#type: PlaceTypeArg,
    },
    /// Assert an additional name on an existing place.
    AddName {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The name to assert.
        name: String,
    },
    /// Assert that a place is enclosed by another place.
    EncloseBy {
        /// The (enclosed) place's human id (e.g. `P0001`).
        human_id: String,
        /// The enclosing place's human id.
        #[arg(long, value_name = "HUMAN_ID")]
        enclosing: String,
    },
    /// Assert a place's geographic coordinates (decimal degrees).
    SetCoordinates {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// Latitude in decimal degrees (north positive).
        #[arg(long)]
        lat: Microdegrees,
        /// Longitude in decimal degrees (east positive).
        #[arg(long)]
        long: Microdegrees,
    },
    /// Set (or change) a place's code.
    SetCode {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The code.
        code: String,
    },
    /// Add a citation backing a place's claims.
    AddCitation {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The citation's human id (e.g. `C0001`).
        #[arg(long = "citation", value_name = "CITATION_ID")]
        citation: String,
    },
    /// Attach a media reference to a place.
    AttachMedia {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The media aggregate id (UUID).
        #[arg(long)]
        media: Uuid,
        /// A caption specific to this use.
        #[arg(long)]
        caption: Option<String>,
    },
    /// Attach a note to a place.
    AttachNote {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The note aggregate id (UUID).
        #[arg(long)]
        note: Uuid,
    },
    /// Apply a tag to a place.
    Tag {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Remove a tag from a place.
    Untag {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Show one place.
    Show {
        /// The place's human id (e.g. `P0001`).
        human_id: String,
    },
    /// List all places.
    List,
}

/// Runs a place subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: PlaceCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        PlaceCmd::Create { id, r#type, name } => {
            let human_id = create_place(
                workspace,
                session,
                NewPlace {
                    human_id: id,
                    place_type: r#type.into(),
                    name,
                },
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        PlaceCmd::SetType { human_id, r#type } => {
            set_place_type(workspace, session, &human_id, r#type.into()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::AddName { human_id, name } => {
            add_place_name(workspace, session, &human_id, name).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::EncloseBy { human_id, enclosing } => {
            assert_place_enclosed_by(workspace, session, &human_id, &enclosing).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::SetCoordinates { human_id, lat, long } => {
            let coordinates = GeoCoordinates {
                latitude: lat,
                longitude: long,
            };
            assert_place_coordinates(workspace, session, &human_id, coordinates).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::SetCode { human_id, code } => {
            set_place_code(workspace, session, &human_id, code).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::AddCitation { human_id, citation } => {
            add_place_citation(workspace, session, &human_id, &citation).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::AttachMedia {
            human_id,
            media,
            caption,
        } => {
            attach_place_media(workspace, session, &human_id, MediaId::from_uuid(media), caption).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::AttachNote { human_id, note } => {
            attach_place_note(workspace, session, &human_id, NoteId::from_uuid(note)).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::Tag { human_id, tag } => {
            tag_place(workspace, session, &human_id, &tag.to_string(), false).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::Untag { human_id, tag } => {
            tag_place(workspace, session, &human_id, &tag.to_string(), true).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PlaceCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        PlaceCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one place, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_place(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.place_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::PlaceNotFound(human_id.to_owned())),
    }
}

/// Renders every place, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let places = list_places(workspace).await?;
    if places.is_empty() {
        println!("{}", localizer.place_list_empty());
        return Ok(());
    }
    for summary in &places {
        println!("{}", localizer.place_summary_line(summary));
    }
    Ok(())
}
