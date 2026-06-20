//! Person subcommands.

use clap::Subcommand;
use genealogy_app::{
    AppError, NewPerson, Session, Workspace, add_name, assert_participation, create_person, list_persons, show_person,
};

use crate::args::{EvidenceArg, ParticipantRoleArg};
use crate::i18n::Localizer;

/// Person subcommands.
#[derive(Subcommand)]
pub enum PersonCmd {
    /// Create a new person (auto-assigns a human id unless `--id` is given).
    Create {
        /// A specific human id (e.g. `I0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// The given name(s).
        #[arg(long)]
        given: Option<String>,
        /// The surname.
        #[arg(long)]
        surname: Option<String>,
        /// Whether this is a persona or a conclusion.
        #[arg(long, value_enum, default_value_t = EvidenceArg::Conclusion)]
        evidence: EvidenceArg,
    },
    /// Assert an additional name on an existing person.
    AddName {
        /// The person's human id (e.g. `I0001`).
        human_id: String,
        /// The given name(s).
        #[arg(long)]
        given: Option<String>,
        /// The surname.
        #[arg(long)]
        surname: Option<String>,
        /// A citation human id backing this name (repeatable); links the assertion's provenance to
        /// a real Citation aggregate (data-model §8).
        #[arg(long = "citation", value_name = "CITATION_ID")]
        citations: Vec<String>,
    },
    /// Assert that a person participated in an event, with a role.
    AddParticipation {
        /// The person's human id (e.g. `I0001`).
        human_id: String,
        /// The event's human id (e.g. `E0001`).
        #[arg(long, value_name = "EVENT_ID")]
        event: String,
        /// The participant's role in the event.
        #[arg(long, value_enum, default_value_t = ParticipantRoleArg::Primary)]
        role: ParticipantRoleArg,
    },
    /// Show one person.
    Show {
        /// The person's human id (e.g. `I0001`).
        human_id: String,
    },
    /// List all persons.
    List,
}

/// Runs a person subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: PersonCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        PersonCmd::Create {
            id,
            given,
            surname,
            evidence,
        } => {
            let human_id = create_person(
                workspace,
                session,
                NewPerson {
                    human_id: id,
                    given,
                    surname,
                    evidence_level: evidence.into(),
                },
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        PersonCmd::AddName {
            human_id,
            given,
            surname,
            citations,
        } => {
            add_name(workspace, session, &human_id, given, surname, &citations).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PersonCmd::AddParticipation { human_id, event, role } => {
            assert_participation(workspace, session, &human_id, &event, role.into()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PersonCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        PersonCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one person, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_person(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::PersonNotFound(human_id.to_owned())),
    }
}

/// Renders every person, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let people = list_persons(workspace).await?;
    if people.is_empty() {
        println!("{}", localizer.list_empty());
        return Ok(());
    }
    for summary in &people {
        println!("{}", localizer.summary_line(summary));
    }
    Ok(())
}
