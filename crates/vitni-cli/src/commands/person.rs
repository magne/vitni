//! Person subcommands.

use clap::Subcommand;
use vitni_app::{
    Age, AppError, Attribute, MutationMeta, NewParticipation, NewPerson, PersonNameParts, Provenance, Session,
    Workspace, add_name, assert_participation, create_person, list_persons, show_person,
};

use crate::args::{ConfidenceArg, EvidenceArg, ParticipantRoleArg};
use crate::i18n::Localizer;

/// Parses an `--attribute TYPE=VALUE` argument, splitting on the first `=` (so the value may contain
/// `=`). A missing `=` is an error. Used as a clap `value_parser` — no panic on bad input.
fn parse_attribute(raw: &str) -> Result<Attribute, String> {
    match raw.split_once('=') {
        Some((attribute_type, value)) => Ok(Attribute {
            attribute_type: attribute_type.to_owned(),
            value: value.to_owned(),
        }),
        None => Err(format!("expected TYPE=VALUE, got `{raw}`")),
    }
}

/// Builds a participant [`Age`] from the CLI's optional parts; all-absent yields `None` so no age is
/// asserted (ADR 0019). The CLI has no `<`/`>` bound flag, so `bound` is always `None`.
fn build_age(years: Option<u16>, months: Option<u16>, days: Option<u16>, phrase: Option<String>) -> Option<Age> {
    let age = Age {
        bound: None,
        years,
        months,
        days,
        phrase,
    };
    (!age.is_empty()).then_some(age)
}

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
        /// The operator's surety in this name (data-model §8); omit to record no judgment (ADR 0021 §5).
        #[arg(long, value_enum)]
        confidence: Option<ConfidenceArg>,
        /// Why this name is asserted (free text recorded in the assertion's provenance).
        #[arg(long)]
        rationale: Option<String>,
    },
    /// Assert that a person participated in an event, with a role and optional participant-scoped
    /// detail — the age at the event, typed attributes, and notes (ADR 0019).
    AddParticipation {
        /// The person's human id (e.g. `I0001`).
        human_id: String,
        /// The event's human id (e.g. `E0001`).
        #[arg(long, value_name = "EVENT_ID")]
        event: String,
        /// The participant's role in the event.
        #[arg(long, value_enum, default_value_t = ParticipantRoleArg::Primary)]
        role: ParticipantRoleArg,
        /// The participant's age in whole years at the event.
        #[arg(long, value_name = "YEARS")]
        age_years: Option<u16>,
        /// The participant's age in whole months at the event.
        #[arg(long, value_name = "MONTHS")]
        age_months: Option<u16>,
        /// The participant's age in whole days at the event.
        #[arg(long, value_name = "DAYS")]
        age_days: Option<u16>,
        /// A free-text age that does not decompose into parts (GEDCOM `AGE` phrase).
        #[arg(long, value_name = "TEXT")]
        age_phrase: Option<String>,
        /// A participant-scoped attribute as `TYPE=VALUE` (repeatable).
        #[arg(long = "attribute", value_name = "TYPE=VALUE", value_parser = parse_attribute)]
        attributes: Vec<Attribute>,
        /// A note human id about this participation (repeatable).
        #[arg(long = "note", value_name = "NOTE_ID")]
        notes: Vec<String>,
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
                    name: Some(PersonNameParts::simple(given, surname)),
                    evidence_level: evidence.into(),
                },
                Provenance::default(),
                &[],
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
            confidence,
            rationale,
        } => {
            let provenance = Provenance {
                confidence: confidence.map(Into::into),
                rationale,
                evidence_analysis: None,
            };
            add_name(
                workspace,
                session,
                &human_id,
                PersonNameParts::simple(given, surname),
                MutationMeta {
                    provenance,
                    citations: &citations,
                    dna_matches: &[],
                    supersedes: None,
                },
            )
            .await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PersonCmd::AddParticipation {
            human_id,
            event,
            role,
            age_years,
            age_months,
            age_days,
            age_phrase,
            attributes,
            notes,
        } => {
            assert_participation(
                workspace,
                session,
                &human_id,
                &event,
                NewParticipation {
                    role: role.into(),
                    age: build_age(age_years, age_months, age_days, age_phrase),
                    attributes,
                    notes,
                },
                MutationMeta::default(),
            )
            .await?;
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
