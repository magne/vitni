//! Place subcommands.

use clap::Subcommand;
use genealogy_app::{
    AppError, NewPlace, Session, Workspace, add_place_name, create_place, list_places, set_place_type, show_place,
};

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
