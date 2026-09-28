//! The `backup` command group (ADR 0041): write a `.vitni-backup` of the open workspace, and restore
//! one into a new registered workspace.

use std::path::{Path, PathBuf};

use clap::Subcommand;
use vitni_app::{
    AppError, BackupReport, BackupRequest, RestoreReport, RestoreRequest, Session, Workspace, config, create_backup,
    restore_backup,
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
    /// Restore a backup into a new workspace, registered under NAME at PATH.
    Restore {
        /// The `.vitni-backup` file to restore.
        archive: PathBuf,
        /// Create and register the new workspace NAME at PATH (absent or empty).
        #[arg(long, num_args = 2, value_names = ["NAME", "PATH"], required = true)]
        new: Vec<String>,
        /// The new workspace's database url (e.g. `postgres://host/db`); defaults to the configured
        /// engine (SQLite).
        #[arg(long, value_name = "URL")]
        database_url: Option<String>,
    },
}

/// Writes a backup of the open `workspace`, registered as `name`, and reports what it holds.
pub async fn create(
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
