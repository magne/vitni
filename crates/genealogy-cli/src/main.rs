//! The `genealogy` binary — a thin terminal frontend over `genealogy-app` (ADR 0006).
//!
//! This crate is I/O only: it parses arguments, resolves the workspace, calls a use-case, and
//! renders the result. All coordination — config, the operator identity, id/clock generation,
//! command execution — lives in `genealogy-app`. stdout/stderr are the interface, so the print
//! lints are relaxed for this crate only.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use genealogy_app::config::{self, default_workspace_dir, load, load_or_bootstrap};
use genealogy_app::{
    AppError, Config, NewPerson, PersonSummary, Session, Workspace, add_name, create_person, list_persons, show_person,
};
use genealogy_core::enums::EvidenceLevel;

/// Event-sourced genealogy at the command line.
#[derive(Parser)]
#[command(name = "genealogy", version, about)]
struct Cli {
    /// Workspace directory (overrides the default and `GENEALOGY_WORKSPACE`).
    #[arg(long, global = true, value_name = "DIR")]
    workspace: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

/// Top-level commands.
#[derive(Subcommand)]
enum Command {
    /// Bootstrap configuration and create/initialize a workspace directory.
    Init {
        /// The workspace directory to create (defaults to `--workspace`/`GENEALOGY_WORKSPACE`, else
        /// the standard data location).
        path: Option<PathBuf>,
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
    let workspace_override = cli.workspace.or_else(workspace_from_env);
    match cli.command {
        Command::Init { path } => init(path.or(workspace_override)).await,
        Command::Person { command } => {
            let (config, workspace) = open(workspace_override).await?;
            let session = Session::new(config.operator_agent());
            run_person(command, &workspace, &session).await
        }
    }
}

/// The workspace directory from `GENEALOGY_WORKSPACE`, if set.
fn workspace_from_env() -> Option<PathBuf> {
    std::env::var_os("GENEALOGY_WORKSPACE").map(PathBuf::from)
}

/// Bootstraps the global config, initializes the workspace directory, and creates its database.
async fn init(dir: Option<PathBuf>) -> Result<(), AppError> {
    let config_path = config::config_path()?;
    let mut config = load_or_bootstrap(&config_path)?;
    let dir = match dir {
        Some(dir) => dir,
        None => default_workspace_dir()?,
    };

    Workspace::init(&dir, &config.operator)?;
    config.register_workspace(dir.clone());
    config::save(&config_path, &config)?;
    // Open once to create the database file and record the operator in the manifest.
    Workspace::open(&dir, &config.operator).await?;

    println!("Initialized workspace at {}", dir.display());
    println!("Config: {}", config_path.display());
    Ok(())
}

/// Loads config and opens the resolved workspace for a non-`init` command.
async fn open(workspace_override: Option<PathBuf>) -> Result<(Config, Workspace), AppError> {
    let config = load(&config::config_path()?)?;
    let dir = workspace_override
        .or_else(|| config.default_workspace.clone())
        .ok_or_else(|| AppError::Config("no workspace given and no default set (run `genealogy init`)".to_owned()))?;
    let workspace = Workspace::open(&dir, &config.operator).await?;
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
