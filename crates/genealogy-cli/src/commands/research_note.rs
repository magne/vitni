//! `ResearchNote` subcommands (ADR 0028).

use clap::Subcommand;
use genealogy_app::{
    AppError, MutationMeta, NewResearchNote, NewResearchNoteSubject, Provenance, Session, Workspace,
    add_subject_to_research_note, create_research_note, list_research_notes, remove_subject_from_research_note,
    set_research_note_body, show_research_note,
};

use crate::args::SubjectKindArg;
use crate::i18n::Localizer;

/// `ResearchNote` subcommands.
#[derive(Subcommand)]
pub enum ResearchNoteCmd {
    /// Create a new research note arguing about one or more subjects (auto-assigns a human id
    /// unless `--id` is given).
    Create {
        /// A specific human id (e.g. `A0500`); omitted to auto-allocate the next free one.
        #[arg(long, value_name = "HUMAN_ID")]
        id: Option<String>,
        /// A subject, as `KIND:HUMAN_ID` (e.g. `person:I0001`); repeatable, at least one required
        /// (ADR 0028 §2).
        #[arg(long = "subject", value_name = "KIND:HUMAN_ID", value_parser = parse_subject_arg, required = true)]
        subjects: Vec<NewResearchNoteSubject>,
        /// An optional short title.
        #[arg(long)]
        title: Option<String>,
    },
    /// Add a subject to an existing research note (idempotent if already named).
    AddSubject {
        /// The research note's human id (e.g. `A0001`).
        human_id: String,
        /// Which kind of aggregate `--subject` names.
        #[arg(long, value_enum)]
        subject_kind: SubjectKindArg,
        /// The subject's own human id (e.g. `I0002`).
        #[arg(long)]
        subject: String,
    },
    /// Remove a subject from an existing research note (rejected if it is the only one named).
    RemoveSubject {
        /// The research note's human id (e.g. `A0001`).
        human_id: String,
        /// Which kind of aggregate `--subject` names.
        #[arg(long, value_enum)]
        subject_kind: SubjectKindArg,
        /// The subject's own human id (e.g. `I0002`).
        #[arg(long)]
        subject: String,
    },
    /// Set (or change) an existing research note's written argument.
    SetBody {
        /// The research note's human id (e.g. `A0001`).
        human_id: String,
        /// The Markdown argument text.
        body: String,
    },
    /// Show one research note.
    Show {
        /// The research note's human id (e.g. `A0001`).
        human_id: String,
    },
    /// List all research notes.
    List,
}

/// Parses a repeatable `--subject KIND:HUMAN_ID` argument (e.g. `person:I0001`) into a
/// [`NewResearchNoteSubject`] — `Create`'s subject list (ADR 0028 §2, at least one required).
fn parse_subject_arg(raw: &str) -> Result<NewResearchNoteSubject, String> {
    let (kind, human_id) = raw
        .split_once(':')
        .ok_or_else(|| format!("expected KIND:HUMAN_ID (e.g. person:I0001), got {raw:?}"))?;
    let human_id = human_id.to_owned();
    match kind {
        "person" => Ok(NewResearchNoteSubject::Person(human_id)),
        "family" => Ok(NewResearchNoteSubject::Family(human_id)),
        "event" => Ok(NewResearchNoteSubject::Event(human_id)),
        "place" => Ok(NewResearchNoteSubject::Place(human_id)),
        other => Err(format!(
            "unknown subject kind {other:?} (expected person/family/event/place)"
        )),
    }
}

/// Converts an `--subject-kind`/`--subject` pair (`AddSubject`/`RemoveSubject`) to a
/// [`NewResearchNoteSubject`].
fn to_new_subject(kind: SubjectKindArg, human_id: String) -> NewResearchNoteSubject {
    match kind {
        SubjectKindArg::Person => NewResearchNoteSubject::Person(human_id),
        SubjectKindArg::Family => NewResearchNoteSubject::Family(human_id),
        SubjectKindArg::Event => NewResearchNoteSubject::Event(human_id),
        SubjectKindArg::Place => NewResearchNoteSubject::Place(human_id),
    }
}

/// Runs a research-note subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    session: &Session,
    command: ResearchNoteCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        ResearchNoteCmd::Create { id, subjects, title } => {
            let human_id = create_research_note(
                workspace,
                session,
                NewResearchNote {
                    human_id: id,
                    subjects,
                    title,
                },
                Provenance::default(),
                &[],
            )
            .await?;
            println!("{}", localizer.created(&human_id));
            Ok(())
        }
        ResearchNoteCmd::AddSubject {
            human_id,
            subject_kind,
            subject,
        } => {
            let subject = to_new_subject(subject_kind, subject);
            add_subject_to_research_note(workspace, session, &human_id, subject, MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        ResearchNoteCmd::RemoveSubject {
            human_id,
            subject_kind,
            subject,
        } => {
            let subject = to_new_subject(subject_kind, subject);
            remove_subject_from_research_note(workspace, session, &human_id, subject, MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        ResearchNoteCmd::SetBody { human_id, body } => {
            set_research_note_body(workspace, session, &human_id, body, None, MutationMeta::default()).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        ResearchNoteCmd::Show { human_id } => show(workspace, &human_id, localizer).await,
        ResearchNoteCmd::List => list(workspace, localizer).await,
    }
}

/// Renders one research note, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str, localizer: &Localizer) -> Result<(), AppError> {
    match show_research_note(workspace, human_id).await? {
        Some(summary) => {
            println!("{}", localizer.research_note_summary_line(&summary));
            Ok(())
        }
        None => Err(AppError::ResearchNoteNotFound(human_id.to_owned())),
    }
}

/// Renders every research note, ordered by human id.
async fn list(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    let notes = list_research_notes(workspace).await?;
    if notes.is_empty() {
        println!("{}", localizer.research_note_list_empty());
        return Ok(());
    }
    for summary in &notes {
        println!("{}", localizer.research_note_summary_line(summary));
    }
    Ok(())
}
