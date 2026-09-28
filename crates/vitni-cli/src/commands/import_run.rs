//! Import-run subcommands (ADR 0037 §5): read-only views of the runs imports wrote and the datasets
//! they grouped into. Runs are written by `vitni import`, never by hand.

use clap::Subcommand;
use vitni_app::{AppError, Session, Workspace, list_datasets, list_import_runs};

use crate::i18n::Localizer;

/// Import-run subcommands.
#[derive(Subcommand)]
pub enum ImportRunCmd {
    /// List every import run, oldest first.
    List,
    /// List the datasets import runs wrote into (the names `import --dataset` accepts).
    Datasets,
}

/// Runs an import-run subcommand against the open workspace.
pub async fn run(
    workspace: &Workspace,
    _session: &Session,
    command: ImportRunCmd,
    localizer: &Localizer,
) -> Result<(), AppError> {
    match command {
        ImportRunCmd::List => {
            let runs = list_import_runs(workspace).await?;
            if runs.is_empty() {
                println!("{}", localizer.import_run_list_empty());
            }
            for run in &runs {
                println!("{}", localizer.import_run_line(run));
            }
            Ok(())
        }
        ImportRunCmd::Datasets => {
            let datasets = list_datasets(workspace).await?;
            if datasets.is_empty() {
                println!("{}", localizer.dataset_list_empty());
            }
            for dataset in &datasets {
                println!("{}", localizer.dataset_line(dataset));
            }
            Ok(())
        }
    }
}
