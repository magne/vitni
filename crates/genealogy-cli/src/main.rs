//! The `genealogy` binary — a thin terminal frontend over `genealogy-app` (ADR 0006).
//!
//! This crate is I/O only: it parses arguments, resolves the workspace, calls a use-case, and
//! renders the result. All coordination — config, the operator identity, id/clock generation,
//! command execution — lives in `genealogy-app`. stdout/stderr are the interface, so the print
//! lints are relaxed for this crate only.

mod i18n;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use genealogy_app::config::{self, load, load_or_bootstrap};
use genealogy_app::{
    AppError, Config, NewPerson, Session, Workspace, add_name, create_person, list_persons, show_person,
};
use genealogy_core::enums::EvidenceLevel;

use crate::i18n::Localizer;

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
    // Honor RUST_LOG when set; otherwise show errors but silence i18n-embed's benign
    // "unable to parse locale" message that fires for the C/POSIX locale on every run.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error,i18n_embed::requester=off"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    run(Cli::parse()).await
}

/// Resolves the workspace and dispatches the parsed command, rendering output and errors through
/// the localizer that has the most context available (workspace-aware for person commands).
async fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Command::Init { name, path } => {
            let localizer = Localizer::baseline();
            report(&localizer, init(&localizer, name, path).await)
        }
        Command::Person { command } => {
            let baseline = Localizer::baseline();
            let workspace = cli.workspace.or_else(workspace_from_env);
            match resolve(workspace.as_deref()) {
                Ok((config, dir)) => {
                    let localizer = Localizer::for_workspace(&dir);
                    let result = run_person_command(&config, &dir, command, &localizer).await;
                    report(&localizer, result)
                }
                Err(error) => report(&baseline, Err(error)),
            }
        }
    }
}

/// Renders an error to stderr through `localizer` and maps the outcome to an exit code.
fn report(localizer: &Localizer, result: Result<(), AppError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", localizer.error(&error));
            ExitCode::FAILURE
        }
    }
}

/// The workspace name from `GENEALOGY_WORKSPACE`, if set.
fn workspace_from_env() -> Option<String> {
    std::env::var("GENEALOGY_WORKSPACE").ok().filter(|s| !s.is_empty())
}

/// Bootstraps the global config, registers `name` → `path`, and creates the workspace + database.
async fn init(localizer: &Localizer, name: String, path: PathBuf) -> Result<(), AppError> {
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

    println!("{}", localizer.init_success(&name, &path.display().to_string()));
    println!("{}", localizer.config_line(&config_path.display().to_string()));
    Ok(())
}

/// Loads config and resolves the workspace directory (by name) for a non-`init` command.
fn resolve(workspace: Option<&str>) -> Result<(Config, PathBuf), AppError> {
    let config = load(&config::config_path()?)?;
    let dir = config.resolve_workspace(workspace)?;
    Ok((config, dir))
}

/// Opens the resolved workspace and runs a person subcommand against it.
async fn run_person_command(
    config: &Config,
    dir: &Path,
    command: PersonCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let workspace = Workspace::open(dir, &config.operator, &config.workspace_defaults).await?;
    let session = Session::new(config.operator_agent());
    match command {
        PersonCmd::Create {
            id,
            given,
            surname,
            evidence,
        } => {
            let human_id = create_person(
                &workspace,
                &session,
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
        } => {
            add_name(&workspace, &session, &human_id, given, surname).await?;
            println!("{}", localizer.updated(&human_id));
            Ok(())
        }
        PersonCmd::Show { human_id } => show(&workspace, &human_id, localizer).await,
        PersonCmd::List => list(&workspace, localizer).await,
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
