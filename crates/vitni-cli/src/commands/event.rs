//! Event subcommands.

use clap::Subcommand;
use uuid::Uuid;
use vitni_app::{
    AppError, DateParts, ListedKind, MatchableKind, MediaRefInput, MutationMeta, NewEvent, Provenance, Session,
    Workspace, add_event_citation, assert_event_date, attach_event_media, attach_event_note, create_event, link_place,
    list_events_page, set_event_description, set_event_restrictions, set_event_type, show_event, tag_event,
};

use crate::args::{EventTypeArg, ListArgs, RestrictionsArgs};
use crate::commands::similar::hint_similar;
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
    /// Set (or change) an event's free-text description.
    SetDescription {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The description.
        description: String,
    },
    /// Add a citation backing an event's claims.
    AddCitation {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The citation's human id (e.g. `C0001`).
        #[arg(long = "citation", value_name = "CITATION_ID")]
        citation: String,
    },
    /// Attach a media reference to an event.
    AttachMedia {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The media object's human id (e.g. `O0001`).
        #[arg(long)]
        media: String,
        /// A caption specific to this use.
        #[arg(long)]
        caption: Option<String>,
    },
    /// Attach a note to an event.
    AttachNote {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The note's human id (e.g. `N0001`).
        #[arg(long)]
        note: String,
    },
    /// Apply a tag to an event.
    Tag {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Remove a tag from an event.
    Untag {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        /// The tag aggregate id (UUID).
        #[arg(long)]
        tag: Uuid,
    },
    /// Set (or clear) an event's privacy restrictions, replacing its current set.
    SetRestrictions {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
        #[command(flatten)]
        restrictions: RestrictionsArgs,
    },
    /// Show one event.
    Show {
        /// The event's human id (e.g. `E0001`).
        human_id: String,
    },
    /// List the events by human id, all of them or one window.
    List(ListArgs),
}

/// Runs an event subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: EventCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let meta = MutationMeta::default();
    match command {
        EventCmd::Create { r#type, id } => {
            let human_id = create_event(
                workspace,
                session,
                NewEvent {
                    human_id: id,
                    event_type: r#type.into(),
                },
                Provenance::default(),
                &[],
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            hint_similar(workspace, MatchableKind::Event, &human_id, localizer).await;
            Ok(())
        }
        EventCmd::SetType { human_id, r#type } => {
            set_event_type(workspace, session, &human_id, r#type.into(), meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::AssertDate {
            human_id,
            year,
            month,
            day,
        } => {
            assert_event_date(workspace, session, &human_id, DateParts { year, month, day }, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::LinkPlace { human_id, place_id } => {
            link_place(workspace, session, &human_id, &place_id, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::SetDescription { human_id, description } => {
            set_event_description(workspace, session, &human_id, description, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::AddCitation { human_id, citation } => {
            add_event_citation(workspace, session, &human_id, &citation, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::AttachMedia {
            human_id,
            media,
            caption,
        } => {
            let input = MediaRefInput { crop: None, caption };
            attach_event_media(workspace, session, &human_id, &media, input, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::AttachNote { human_id, note } => {
            attach_event_note(workspace, session, &human_id, &note, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::Tag { human_id, tag } => {
            tag_event(workspace, session, &human_id, &tag.to_string(), false, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::Untag { human_id, tag } => {
            tag_event(workspace, session, &human_id, &tag.to_string(), true, meta).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        EventCmd::SetRestrictions { human_id, restrictions } => {
            restrict(workspace, session, &human_id, restrictions, localizer).await
        }
        EventCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        EventCmd::List(window) => list(workspace, &window, localizer).await,
    }
}

/// Replaces an event's privacy restrictions and reports the update.
async fn restrict(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: RestrictionsArgs,
    localizer: &Localizer,
) -> Result<(), AppError> {
    set_event_restrictions(
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

/// Renders the `page` of event records, ordered by human id.
async fn list(workspace: &Workspace, window: &ListArgs, localizer: &Localizer) -> Result<(), AppError> {
    let query = window.query(workspace, ListedKind::Event).await?;
    let page = window.page();
    let events = list_events_page(workspace, &query, page).await?.items;
    if events.is_empty() {
        println!(
            "{}",
            localizer.empty_page(page, window.narrows(), localizer.event_list_empty())
        );
        return Ok(());
    }
    for summary in &events {
        println!("{}", localizer.event_summary_line(summary));
    }
    Ok(())
}
