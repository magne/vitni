//! Event subcommands.

use clap::Subcommand;
use genealogy_app::{
    AppError, DateParts, NewEvent, Session, Workspace, assert_event_date, create_event, link_place, list_events,
    set_event_type, show_event,
};

use crate::args::EventTypeArg;
use crate::i18n::Localizer;

/// Event subcommands.
#[derive(Subcommand)]
pub enum EventCmd {
    /// Create a new event (auto-assigns a human id unless `--id` is given).
    Create {
        /// The kind of event.
        #[arg(long, value_enum)]
        r#type: EventTypeArg,
        /// A specific human id (e.g. `E0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
    },
    /// Set (or change) an existing event's type.
    SetType {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The new event type.
        #[arg(long, value_enum)]
        r#type: EventTypeArg,
    },
    /// Assert when an event occurred (Gregorian; year required, month/day optional).
    AssertDate {
        /// The event's human id (e.g. `E0001`).
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
    /// Link an event to the place it occurred.
    LinkPlace {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The place's human id (e.g. `P0001`).
        place_id: String,
    },
    /// Show one event.
    Show {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
    },
    /// List all events.
    List,
}

/// Runs an event subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: EventCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        EventCmd::Create { r#type, id } => {
            let human_id = create_event(
                workspace,
                session,
                NewEvent {
                    human_id: id,
                    event_type: r#type.into(),
                    private: false,
                },
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        EventCmd::SetType { human_id, r#type } => {
            set_event_type(workspace, session, &human_id, r#type.into()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::AssertDate {
            human_id,
            year,
            month,
            day,
        } => {
            assert_event_date(workspace, session, &human_id, DateParts { year, month, day }).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::LinkPlace { human_id, place_id } => {
            link_place(workspace, session, &human_id, &place_id).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        EventCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one event, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_event(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.event_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::EventNotFound(human_id.to_owned())),
    }
}

/// Renders every event, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let events = list_events(workspace).await?;
    if events.is_empty() {
        println!("{}", localizer.event_list_empty());
        return Ok(());
    }
    for summary in &events {
        println!("{}", localizer.event_summary_line(summary));
    }
    Ok(())
}
