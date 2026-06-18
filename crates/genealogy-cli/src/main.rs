//! The `genealogy` binary — a thin terminal frontend over `genealogy-app` (ADR 0006).
//!
//! This crate is I/O only: it parses arguments, resolves the workspace, calls a use-case, and
//! renders the result. All coordination — config, the operator identity, id/clock generation,
//! command execution — lives in `genealogy-app`. stdout/stderr are the interface, so the print
//! lints are relaxed for this crate only.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use genealogy_app::config::{self, load, load_or_bootstrap};
use genealogy_app::{
    AppError, Config, NewPerson, PersonSummary, Session, Workspace, add_name, create_person, list_persons, show_person,
};
use genealogy_core::enums::EvidenceLevel;

/// Event-sourced genealogy at the command line.
#[derive(Parser)]
#[command(name = "genealogy", version, about)]
struct Cli {
    /// Workspace name (overrides the default and `GENEALOGY_WORKSPACE`).
    #[arg(long, global = true, value_name = "NAME")]
    workspace: Option<String>,
    #[command(subcommand)]
    command: Command,
}

/// Top-level commands.
#[derive(Subcommand)]
enum Command {
    /// Create and register a named workspace, bootstrapping configuration if needed.
    Init {
        /// The workspace name (e.g. `gen`).
        name: String,
        /// The workspace directory to create.
        path: PathBuf,
    },
    /// Operate on persons.
    Person {
        #[command(subcommand)]
        command: PersonCmd,
    },
}

/// Person subcommands.
#[derive(Subcommand)]
enum PersonCmd {
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
    },
    /// Show one person.
    Show {
        /// The person's human id (e.g. `I0001`).
        human_id: String,
    },
    /// List all persons.
    List,
}

/// CLI mirror of [`EvidenceLevel`] (keeps clap's `ValueEnum` off the domain type).
#[derive(Clone, Copy, ValueEnum)]
enum EvidenceArg {
    /// A single-source persona.
    Persona,
    /// A researcher's conclusion.
    Conclusion,
}

impl From<EvidenceArg> for EvidenceLevel {
    fn from(value: EvidenceArg) -> Self {
        match value {
            EvidenceArg::Persona => Self::Persona,
            EvidenceArg::Conclusion => Self::Conclusion,
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Resolves the workspace and dispatches the parsed command.
async fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command {
        Command::Init { name, path } => init(name, path).await,
        Command::Person { command } => {
            let (config, workspace) = open(cli.workspace.or_else(workspace_from_env)).await?;
            let session = Session::new(config.operator_agent());
            run_person(command, &workspace, &session).await
        }
    }
}

/// The workspace name from `GENEALOGY_WORKSPACE`, if set.
fn workspace_from_env() -> Option<String> {
    std::env::var("GENEALOGY_WORKSPACE").ok().filter(|s| !s.is_empty())
}

/// Bootstraps the global config, registers `name` → `path`, and creates the workspace + database.
async fn init(name: String, path: PathBuf) -> Result<(), AppError> {
    let config_path = config::config_path()?;
    let mut config = load_or_bootstrap(&config_path)?;
    if config.workspaces.contains_key(&name) {
        return Err(AppError::Config(format!("workspace {name:?} is already registered")));
    }

    Workspace::init(&path, &config.operator, &config.defaults)?;
    config.register_workspace(name.clone(), path.clone());
    config::save(&config_path, &config)?;
    // Open once to create the database file and record the operator in the manifest.
    Workspace::open(&path, &config.operator, &config.workspace_defaults).await?;

    println!("Initialized workspace {name:?} at {}", path.display());
    println!("Config: {}", config_path.display());
    Ok(())
}

/// Loads config and opens the resolved (by name) workspace for a non-`init` command.
async fn open(workspace: Option<String>) -> Result<(Config, Workspace), AppError> {
    let config = load(&config::config_path()?)?;
    let dir = config.resolve_workspace(workspace.as_deref())?;
    let workspace = Workspace::open(&dir, &config.operator, &config.workspace_defaults).await?;
    Ok((config, workspace))
}

/// Runs a person subcommand against the open workspace.
async fn run_person(command: PersonCmd, workspace: &Workspace, session: &Session) -> Result<(), AppError> {
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
            println!("Created {human_id}");
            Ok(())
        }
        PersonCmd::AddName {
            human_id,
            given,
            surname,
        } => {
            add_name(workspace, session, &human_id, given, surname).await?;
            println!("Updated {human_id}");
            Ok(())
        }
        PersonCmd::Show { human_id } => show(workspace, &human_id).await,
        PersonCmd::List => list(workspace).await,
    }
}

/// Renders one person, or errors if absent.
async fn show(workspace: &Workspace, human_id: &str) -> Result<(), AppError> {
    match show_person(workspace, human_id).await? {
        Some(summary) => {
            print_summary(&summary);
            Ok(())
        }
        None => Err(AppError::PersonNotFound(human_id.to_owned())),
    }
}

/// Renders every person, ordered by human id.
async fn list(workspace: &Workspace) -> Result<(), AppError> {
    let people = list_persons(workspace).await?;
    if people.is_empty() {
        println!("No persons yet.");
        return Ok(());
    }
    for summary in &people {
        print_summary(summary);
    }
    Ok(())
}

/// Prints one summary line: `I0001  Ada Lovelace  sex: female`.
fn print_summary(summary: &PersonSummary) {
    let name = summary.display_name.as_deref().unwrap_or("(no name)");
    let sex = summary.sex.as_deref().unwrap_or("-");
    let privacy = if summary.private { " [private]" } else { "" };
    println!("{}  {name}  sex: {sex}{privacy}", summary.human_id);
}
