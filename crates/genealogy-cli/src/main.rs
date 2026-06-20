//! The `genealogy` binary — a thin terminal frontend over `genealogy-app` (ADR 0006).
//!
//! This crate is I/O only: it parses arguments, resolves the workspace, calls a use-case, and
//! renders the result. All coordination — config, the operator identity, id/clock generation,
//! command execution — lives in `genealogy-app`. stdout/stderr are the interface, so the print
//! lints are relaxed for this crate only. The per-aggregate command surface lives under
//! [`commands`]; the clap mirrors of the domain enums live in [`args`].

mod args;
mod commands;
mod i18n;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use genealogy_app::config::{self, load, load_or_bootstrap};
use genealogy_app::{AppError, Config, Session, Workspace};

use crate::commands::citation::CitationCmd;
use crate::commands::event::EventCmd;
use crate::commands::family::FamilyCmd;
use crate::commands::person::PersonCmd;
use crate::commands::place::PlaceCmd;
use crate::commands::source::SourceCmd;
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
    /// Operate on families.
    Family {
        #[command(subcommand)]
        command: FamilyCmd,
    },
    /// Operate on places.
    Place {
        #[command(subcommand)]
        command: PlaceCmd,
    },
    /// Operate on sources.
    Source {
        #[command(subcommand)]
        command: SourceCmd,
    },
    /// Operate on citations.
    Citation {
        #[command(subcommand)]
        command: CitationCmd,
    },
    /// Operate on events.
    Event {
        #[command(subcommand)]
        command: EventCmd,
    },
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

/// The open workspace plus the per-command inputs and the workspace-aware localizer.
struct Context {
    workspace: Workspace,
    session: Session,
    localizer: Localizer,
}

/// Resolves the workspace and dispatches the parsed command, rendering output and errors through
/// the most context-aware localizer available (workspace-aware once a workspace is open).
async fn run(cli: Cli) -> ExitCode {
    if let Command::Init { name, path } = cli.command {
        let localizer = Localizer::baseline();
        return report(&localizer, init(&localizer, name, path).await);
    }

    let context = match open_workspace(cli.workspace).await {
        Ok(context) => context,
        Err((localizer, error)) => return report(&localizer, Err(error)),
    };
    let Context {
        workspace,
        session,
        localizer,
    } = context;
    let result = match cli.command {
        Command::Init { .. } => unreachable!("handled above"),
        Command::Person { command } => commands::person::run(&workspace, &session, command, &localizer).await,
        Command::Family { command } => commands::family::run(&workspace, &session, command, &localizer).await,
        Command::Place { command } => commands::place::run(&workspace, &session, command, &localizer).await,
        Command::Source { command } => commands::source::run(&workspace, &session, command, &localizer).await,
        Command::Citation { command } => commands::citation::run(&workspace, &session, command, &localizer).await,
        Command::Event { command } => commands::event::run(&workspace, &session, command, &localizer).await,
    };
    report(&localizer, result)
}

/// Resolves and opens the workspace, returning the [`Context`] every non-`init` command needs.
///
/// On failure returns the localizer to report through (workspace-aware if the directory resolved,
/// else the baseline) alongside the error.
async fn open_workspace(workspace: Option<String>) -> Result<Context, (Localizer, AppError)> {
    let workspace = workspace.or_else(workspace_from_env);
    let (config, dir) = match resolve(workspace.as_deref()) {
        Ok(resolved) => resolved,
        Err(error) => return Err((Localizer::baseline(), error)),
    };
    let localizer = Localizer::for_workspace(&dir);
    match Workspace::open(&dir, &config.operator, &config.workspace_defaults).await {
        Ok(workspace) => Ok(Context {
            workspace,
            session: Session::new(config.operator_agent()),
            localizer,
        }),
        Err(error) => Err((localizer, error)),
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
