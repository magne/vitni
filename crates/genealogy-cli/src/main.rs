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

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use genealogy_app::config::{self, load, load_or_bootstrap};
use genealogy_app::{AppError, Config, Session, Workspace};
use genealogy_plugin_host::{Capability, ExportTarget, Grants, Invocation, PluginHost, ProgressUpdate, ResourceBudget};

use crate::commands::citation::CitationCmd;
use crate::commands::dna_match::DnaMatchCmd;
use crate::commands::dna_test::DnaTestCmd;
use crate::commands::event::EventCmd;
use crate::commands::family::FamilyCmd;
use crate::commands::media::MediaCmd;
use crate::commands::note::NoteCmd;
use crate::commands::person::PersonCmd;
use crate::commands::place::PlaceCmd;
use crate::commands::repository::RepositoryCmd;
use crate::commands::source::SourceCmd;
use crate::commands::tag::TagCmd;
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

/// The canonical per-aggregate command list: one row per aggregate, the single place the top-level
/// `Command` surface is enumerated (issue #38). Columns: `(Variant, module, doc, CmdType)`.
macro_rules! for_each_cli_command {
    ($callback:ident) => {
        $callback! {
            (Person, person, "Operate on persons.", PersonCmd),
            (Family, family, "Operate on families.", FamilyCmd),
            (Place, place, "Operate on places.", PlaceCmd),
            (Source, source, "Operate on sources.", SourceCmd),
            (Citation, citation, "Operate on citations.", CitationCmd),
            (Event, event, "Operate on events.", EventCmd),
            (DnaTest, dna_test, "Operate on DNA tests.", DnaTestCmd),
            (DnaMatch, dna_match, "Operate on DNA matches.", DnaMatchCmd),
            (Repository, repository, "Operate on repositories.", RepositoryCmd),
            (Note, note, "Operate on notes.", NoteCmd),
            (Media, media, "Operate on media objects.", MediaCmd),
            (Tag, tag, "Operate on tags.", TagCmd),
        }
    };
}

/// Generates the top-level `Command` enum: the hand-written `Init` plus one subcommand-bearing
/// variant per aggregate.
macro_rules! cli_command_enum {
    ($(($Variant:ident, $module:ident, $doc:literal, $Cmd:ty)),+ $(,)?) => {
        /// Top-level commands.
        #[derive(Subcommand)]
        enum Command {
            /// Create and register a named workspace, bootstrapping configuration if needed.
            Init {
                /// The workspace name (e.g. `gen`).
                name: String,
                /// The workspace directory to create.
                path: PathBuf,
                /// The database url to freeze into the workspace (e.g. `postgres://host/db`).
                /// Defaults to the configured engine (SQLite) when omitted.
                #[arg(long, value_name = "URL")]
                database_url: Option<String>,
            },
            /// Rebuild every projection from the event log (a maintenance operation, ADR 0010).
            Rebuild,
            /// Import records into the workspace through a bulk import plugin (ADR 0013).
            Import {
                /// The plugin id to run (e.g. `gedcom-import`).
                plugin: String,
                /// The file to import from.
                file: PathBuf,
            },
            /// Export the workspace through a bulk export plugin (ADR 0013).
            Export {
                /// The plugin id to run (e.g. `gedcom-export`).
                plugin: String,
                /// Where to write the export; defaults to the workspace `exports/` directory.
                #[arg(long, value_name = "FILE")]
                output: Option<PathBuf>,
            },
            $(
                #[doc = $doc]
                $Variant {
                    #[command(subcommand)]
                    command: $Cmd,
                },
            )+
        }
    };
}

for_each_cli_command!(cli_command_enum);

/// Generates the dispatch from a parsed non-`init` command to its aggregate's `run` use-case. A
/// free function (rather than an inline `match`) because a macro cannot expand into match-arm
/// position; `Init` is handled in [`run`] before the workspace opens.
macro_rules! cli_dispatch_fn {
    ($(($Variant:ident, $module:ident, $doc:literal, $Cmd:ty)),+ $(,)?) => {
        async fn dispatch(
            command: Command,
            workspace: &Workspace,
            session: &Session,
            localizer: &Localizer,
        ) -> Result<(), AppError> {
            match command {
                Command::Init { .. } => unreachable!("handled in run() before the workspace opens"),
                Command::Rebuild | Command::Import { .. } | Command::Export { .. } => {
                    unreachable!("handled in run() after the workspace opens")
                }
                $(
                    Command::$Variant { command } => {
                        commands::$module::run(workspace, session, command, localizer).await
                    }
                )+
            }
        }
    };
}

for_each_cli_command!(cli_dispatch_fn);

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
    dir: PathBuf,
    session: Session,
    localizer: Localizer,
}

/// Resolves the workspace and dispatches the parsed command, rendering output and errors through
/// the most context-aware localizer available (workspace-aware once a workspace is open).
async fn run(cli: Cli) -> ExitCode {
    if let Command::Init {
        name,
        path,
        database_url,
    } = cli.command
    {
        let localizer = Localizer::baseline();
        return report(&localizer, init(&localizer, name, path, database_url).await);
    }

    let context = match open_workspace(cli.workspace).await {
        Ok(context) => context,
        Err((localizer, error)) => return report(&localizer, Err(error)),
    };
    let Context {
        workspace,
        dir,
        session,
        localizer,
    } = context;
    let result = match cli.command {
        Command::Rebuild => rebuild(&workspace, &localizer).await,
        // The plugin-host futures are large (Wasmtime store + workspace); box them so the top-level
        // command future stays small.
        Command::Import { plugin, file } => Box::pin(import(workspace, &localizer, &plugin, file)).await,
        Command::Export { plugin, output } => Box::pin(export(workspace, &dir, &localizer, &plugin, output)).await,
        other => dispatch(other, &workspace, &session, &localizer).await,
    };
    report(&localizer, result)
}

/// Rebuilds every projection from the open workspace's event log and reports success (ADR 0010).
async fn rebuild(workspace: &Workspace, localizer: &Localizer) -> Result<(), AppError> {
    workspace.rebuild_projections().await?;
    println!("{}", localizer.rebuild_success());
    Ok(())
}

/// Runs a bulk import plugin against the open workspace, streaming `file` in and reporting progress
/// to stderr (ADR 0013). The plugin is attributed to a Software operator.
async fn import(workspace: Workspace, localizer: &Localizer, plugin: &str, file: PathBuf) -> Result<(), AppError> {
    let host = PluginHost::new().map_err(|error| AppError::Plugin(error.to_string()))?;
    let component = host
        .load_by_id(&plugins_dir(), plugin)
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    let run = Invocation {
        workspace,
        session: Session::software(plugin, env!("CARGO_PKG_VERSION")),
        grants: Grants::none()
            .with(Capability::Commands)
            .with(Capability::Log)
            .with(Capability::Progress)
            .with(Capability::ImportSource),
        budget: ResourceBudget::default(),
    };
    let (count, _workspace) = host
        .run_bulk_import(&component, run, file, render_progress)
        .await
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    println!("{}", localizer.import_success(count, plugin));
    Ok(())
}

/// Runs a bulk export plugin against the open workspace, writing to `output` (or the workspace
/// `exports/` directory) and reporting progress to stderr (ADR 0013).
async fn export(
    workspace: Workspace,
    dir: &Path,
    localizer: &Localizer,
    plugin: &str,
    output: Option<PathBuf>,
) -> Result<(), AppError> {
    let host = PluginHost::new().map_err(|error| AppError::Plugin(error.to_string()))?;
    let component = host
        .load_by_id(&plugins_dir(), plugin)
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    let target = match output {
        Some(path) => ExportTarget::File(path),
        None => ExportTarget::Directory(dir.join("exports")),
    };
    let destination = match &target {
        ExportTarget::File(path) => path.display().to_string(),
        ExportTarget::Directory(directory) => directory.display().to_string(),
    };
    let run = Invocation {
        workspace,
        session: Session::software(plugin, env!("CARGO_PKG_VERSION")),
        grants: Grants::none()
            .with(Capability::Query)
            .with(Capability::Log)
            .with(Capability::Progress)
            .with(Capability::ExportSink),
        budget: ResourceBudget::default(),
    };
    let (count, _workspace) = host
        .run_bulk_export(&component, run, target, render_progress)
        .await
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    println!("{}", localizer.export_success(count, &destination));
    Ok(())
}

/// The directory the host loads plugin components from: `$GENEALOGY_PLUGIN_DIR`, else
/// `target/plugins` relative to the working directory (the dev default; ADR 0014 will add the
/// three-layer override).
fn plugins_dir() -> PathBuf {
    match std::env::var_os("GENEALOGY_PLUGIN_DIR") {
        Some(value) => PathBuf::from(value),
        None => PathBuf::from("target/plugins"),
    }
}

/// Renders a plugin progress update to stderr. The `step` is the plugin's own vocabulary, shown
/// verbatim; only the counts are decorated.
fn render_progress(update: ProgressUpdate) {
    let ProgressUpdate { step, processed, total } = update;
    match total {
        Some(total) => eprintln!("  {step}: {processed}/{total}"),
        None => eprintln!("  {step}: {processed}"),
    }
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
            dir,
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
async fn init(
    localizer: &Localizer,
    name: String,
    path: PathBuf,
    database_url: Option<String>,
) -> Result<(), AppError> {
    let config_path = config::config_path()?;
    let mut config = load_or_bootstrap(&config_path)?;
    if config.workspaces.contains_key(&name) {
        return Err(AppError::Config(format!("workspace {name:?} is already registered")));
    }

    Workspace::init(&path, &config.operator, &config.defaults, database_url.as_deref())?;
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
