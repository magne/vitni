//! The `backup` command group (ADR 0041): write a `.vitni-backup` of the open workspace, and restore
//! one into a new registered workspace or over the open one (ADR 0044).

use std::path::{Path, PathBuf};

use std::process::ExitCode;

use clap::{ArgGroup, Subcommand};
use vitni_app::{
    AppError, BackupReport, BackupRequest, ReplaceReport, ReplaceRequest, RestoreReport, RestoreRequest, Session,
    Workspace, config, create_backup, read_pre_restore_backup, replace_backup, restore_backup,
};

use crate::i18n::Localizer;

/// `backup` subcommands.
#[derive(Subcommand)]
pub enum BackupCmd {
    /// Write a backup of the workspace's event log to a new `.vitni-backup` file.
    Create {
        /// The archive to write; it must not exist.
        path: PathBuf,
        /// Also carry the media library files, not only their manifest.
        #[arg(long)]
        with_media: bool,
    },
    /// Restore a backup into a new workspace, registered under NAME at PATH, or over the open
    /// workspace with `--replace --yes`.
    #[command(group(ArgGroup::new("target").required(true).args(["new", "replace"])))]
    Restore {
        /// The `.vitni-backup` file to restore.
        archive: PathBuf,
        /// Create and register the new workspace NAME at PATH (absent or empty).
        #[arg(long, num_args = 2, value_names = ["NAME", "PATH"])]
        new: Option<Vec<String>>,
        /// The new workspace's database url (e.g. `postgres://host/db`); defaults to the configured
        /// engine (SQLite).
        #[arg(long, value_name = "URL", requires = "new")]
        database_url: Option<String>,
        /// Replace the open workspace's records with the backup's, discarding its own. Unless the
        /// workspace switched it off, its current state is backed up into its `backups` folder first.
        #[arg(long)]
        replace: bool,
        /// Confirm `--replace`; without it nothing is replaced.
        #[arg(long, requires = "replace")]
        yes: bool,
    },
}

/// Runs a `backup` subcommand against the open `workspace`, registered as `name`: a backup, or a
/// restore with `--replace` (a restore into a new workspace runs before any workspace is open).
pub async fn run_on_open(
    command: BackupCmd,
    workspace: &Workspace,
    session: &Session,
    name: &str,
    localizer: &Localizer,
) -> ExitCode {
    let result = match command {
        BackupCmd::Create { path, with_media } => create(workspace, session, name, &path, with_media, localizer).await,
        BackupCmd::Restore { yes: false, .. } => return refuse_replace(workspace, name, localizer).await,
        BackupCmd::Restore { archive, .. } => replace(workspace, session, name, &archive, localizer).await,
    };
    crate::report(localizer, result)
}

/// Writes a backup of the open `workspace`, registered as `name`, and reports what it holds.
async fn create(
    workspace: &Workspace,
    session: &Session,
    name: &str,
    path: &Path,
    with_media: bool,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let request = BackupRequest {
        workspace_name: name,
        destination: path,
        with_media,
        created_at: session.now(),
    };
    let report = create_backup(workspace, &request).await?;
    print_backup_report(&report, localizer);
    Ok(())
}

fn print_backup_report(report: &BackupReport, localizer: &Localizer) {
    println!(
        "{}",
        localizer.backup_created(report.events, &report.destination.display().to_string())
    );
    if report.media_files > 0 {
        println!("{}", localizer.backup_media_files(report.media_files));
    }
    for path in &report.media_missing {
        println!("{}", localizer.backup_media_missing(path));
    }
}

/// Restores `archive` into the new workspace `new` = `[NAME, PATH]`, before any workspace is open.
pub async fn restore(
    archive: &Path,
    new: &[String],
    database_url: Option<&str>,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let [name, path] = new else {
        unreachable!("clap requires exactly NAME and PATH for --new");
    };
    let config_path = config::config_path()?;
    let dir = PathBuf::from(path);
    let request = RestoreRequest {
        config_path: &config_path,
        archive,
        name,
        dir: Some(&dir),
        database_url,
    };
    let report = restore_backup(&request).await?;
    print_restore_report(&report, localizer);
    Ok(())
}

fn print_restore_report(report: &RestoreReport, localizer: &Localizer) {
    println!(
        "{}",
        localizer.restore_success(
            report.events,
            &report.workspace.name,
            &report.workspace.path.display().to_string(),
            &report.format_version,
        )
    );
    if report.media_restored > 0 {
        println!("{}", localizer.restore_media_restored(report.media_restored));
    }
    for path in &report.media_missing {
        println!("{}", localizer.restore_media_missing(path));
    }
    for path in &report.media_mismatched {
        println!("{}", localizer.restore_media_mismatched(path));
    }
}

/// Refuses a `--replace` without `--yes`: names the open workspace, how many events it would discard,
/// and whether its state would be backed up first, then fails.
async fn refuse_replace(workspace: &Workspace, name: &str, localizer: &Localizer) -> ExitCode {
    let found = async {
        let events = workspace.store().event_count().await?;
        Ok::<_, AppError>((events, read_pre_restore_backup(workspace.dir())?))
    }
    .await;
    match found {
        Ok((events, pre_restore)) => {
            eprintln!("{}", localizer.replace_needs_yes(name, events));
            eprintln!("{}", localizer.replace_needs_yes_backup(pre_restore));
        }
        Err(error) => eprintln!("{}", localizer.error(&error)),
    }
    ExitCode::FAILURE
}

/// Restores `archive` over the open `workspace`, registered as `name` (ADR 0044), and reports it.
async fn replace(
    workspace: &Workspace,
    session: &Session,
    name: &str,
    archive: &Path,
    localizer: &Localizer,
) -> Result<(), AppError> {
    let request = ReplaceRequest {
        archive,
        workspace_name: name,
        now: session.now(),
    };
    let report = replace_backup(workspace, &request).await?;
    print_replace_report(&report, name, localizer);
    Ok(())
}

fn print_replace_report(report: &ReplaceReport, name: &str, localizer: &Localizer) {
    println!(
        "{}",
        localizer.replace_success(name, report.events, &report.format_version)
    );
    match &report.pre_restore_backup {
        Some(path) => println!("{}", localizer.replace_pre_restore(&path.display().to_string())),
        None => println!("{}", localizer.replace_no_pre_restore()),
    }
    if report.media_restored > 0 {
        println!("{}", localizer.restore_media_restored(report.media_restored));
    }
    for path in &report.media_missing {
        println!("{}", localizer.restore_media_missing(path));
    }
    for path in &report.media_mismatched {
        println!("{}", localizer.restore_media_mismatched(path));
    }
}
