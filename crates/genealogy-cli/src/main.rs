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

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use genealogy_app::config::{self, load, load_or_bootstrap};
use genealogy_app::{AppError, Config, Session, Workspace, read_resolved_locale};

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
            /// Import records through a bulk import plugin (ADR 0013).
            ///
            /// Imports into a fresh workspace by default (`--new`); use `--into` to import into an
            /// existing one (which prompts for confirmation when it already holds data).
            #[command(group(clap::ArgGroup::new("import_target").required(true).args(["new", "into"])))]
            Import {
                /// The plugin id to run (e.g. `gedcom-import`).
                plugin: String,
                /// The file to import from.
                file: PathBuf,
                /// Create and register a new workspace NAME at PATH, then import into it.
                #[arg(long, num_args = 2, value_names = ["NAME", "PATH"])]
                new: Option<Vec<String>>,
                /// Import into the existing registered workspace NAME.
                #[arg(long, value_name = "NAME")]
                into: Option<String>,
                /// Skip the confirmation prompt when importing into a non-empty workspace.
                #[arg(long)]
                yes: bool,
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

    // Import resolves its own target workspace (a fresh `--new` or an existing `--into`), so it is
    // handled before the generic workspace open below.
    if let Command::Import {
        plugin,
        file,
        new,
        into,
        yes,
    } = cli.command
    {
        // The import future is large (Wasmtime store + workspace); box it.
        return Box::pin(import(plugin, file, new, into, yes)).await;
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
        // The plugin-host future is large (Wasmtime store + workspace); box it so the top-level
        // command future stays small.
        Command::Export { plugin, output } => {
            Box::pin(commands::io::export(workspace, &dir, &localizer, &plugin, output)).await
        }
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
    let config_ui_language = read_resolved_locale(&dir, &config.workspace_defaults).ui_language;
    let localizer = Localizer::for_workspace(&dir, config_ui_language.as_ref());
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
    let summary = genealogy_app::register_workspace(&config_path, &name, Some(&path), database_url.as_deref()).await?;

    println!(
        "{}",
        localizer.init_success(&summary.name, &summary.path.display().to_string())
    );
    println!("{}", localizer.config_line(&config_path.display().to_string()));
    Ok(())
}

/// Loads config and resolves the workspace directory (by name) for a non-`init` command.
fn resolve(workspace: Option<&str>) -> Result<(Config, PathBuf), AppError> {
    let config = load(&config::config_path()?)?;
    let dir = config.resolve_workspace(workspace)?;
    Ok((config, dir))
}

/// One resolved import target: the open workspace, its localizer and registered name, and whether it
/// was just created (a fresh `--new`, which never prompts) rather than an existing `--into`.
struct ImportTarget {
    workspace: Workspace,
    localizer: Localizer,
    name: String,
    created: bool,
}

/// Imports through a bulk plugin (ADR 0013) into a fresh `--new` workspace or an existing `--into`
/// one. A non-empty existing workspace is confirmed first unless `--yes`. Clap guarantees exactly one
/// of `new`/`into` is set (the `import_target` group).
async fn import(plugin: String, file: PathBuf, new: Option<Vec<String>>, into: Option<String>, yes: bool) -> ExitCode {
    let baseline = Localizer::baseline();
    let target = match prepare_import_target(new, into).await {
        Ok(target) => target,
        Err(error) => return report(&baseline, Err(error)),
    };
    let ImportTarget {
        workspace,
        localizer,
        name,
        created,
    } = target;

    // Importing into a workspace that already holds data is confirmed first (unless --yes); a fresh
    // --new workspace is always empty, so it never prompts.
    if !created && !yes {
        let count = match genealogy_app::list_persons(&workspace).await {
            Ok(persons) => persons.len(),
            Err(error) => return report(&localizer, Err(error)),
        };
        if count > 0 && !confirm(&localizer.import_confirm(&name, count)) {
            println!("{}", localizer.import_cancelled());
            return ExitCode::SUCCESS;
        }
    }

    // The plugin-host future is large (Wasmtime store + workspace); box it.
    report(
        &localizer,
        Box::pin(commands::io::import(workspace, &localizer, &plugin, file)).await,
    )
}

/// Resolves the import target: `--new NAME PATH` creates and registers a fresh workspace; `--into
/// NAME` opens an existing registered one.
async fn prepare_import_target(new: Option<Vec<String>>, into: Option<String>) -> Result<ImportTarget, AppError> {
    if let Some(mut spec) = new {
        // clap fixes `num_args = 2`, so both values are present (path last, name first).
        let path = PathBuf::from(spec.pop().unwrap_or_default());
        let name = spec.pop().unwrap_or_default();
        let config_path = config::config_path()?;
        let mut config = load_or_bootstrap(&config_path)?;
        if config.workspaces.contains_key(&name) {
            return Err(AppError::Config(format!("workspace {name:?} is already registered")));
        }
        Workspace::init(&path, &config.operator, &config.defaults, None)?;
        config.register_workspace(name.clone(), path.clone());
        config::save(&config_path, &config)?;
        let workspace = Workspace::open(&path, &config.operator, &config.workspace_defaults).await?;
        let config_ui_language = read_resolved_locale(&path, &config.workspace_defaults).ui_language;
        let localizer = Localizer::for_workspace(&path, config_ui_language.as_ref());
        println!("{}", localizer.init_success(&name, &path.display().to_string()));
        return Ok(ImportTarget {
            workspace,
            localizer,
            name,
            created: true,
        });
    }

    let name = into.unwrap_or_default();
    let config = load(&config::config_path()?)?;
    let dir = config.resolve_workspace(Some(&name))?;
    let config_ui_language = read_resolved_locale(&dir, &config.workspace_defaults).ui_language;
    let localizer = Localizer::for_workspace(&dir, config_ui_language.as_ref());
    let workspace = Workspace::open(&dir, &config.operator, &config.workspace_defaults).await?;
    Ok(ImportTarget {
        workspace,
        localizer,
        name,
        created: false,
    })
}

/// Prompts on stderr and reads a yes/no answer from stdin. Returns `true` only on an affirmative
/// (`y`/`yes`, or `j`/`ja` for Norwegian); EOF or anything else is a no.
fn confirm(prompt: &str) -> bool {
    eprint!("{prompt} ");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    let answer = answer.trim().to_lowercase();
    answer == "y" || answer == "yes" || answer == "j" || answer == "ja"
}
