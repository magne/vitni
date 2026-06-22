//! The bulk import/export commands (ADR 0013): load a plugin component, stream a file through the
//! host-mediated source/sink, and render progress to stderr. Kept out of `main` so the binary's
//! entry point stays a thin parse-and-dispatch shell, like the per-aggregate command modules.

use std::path::{Path, PathBuf};

use genealogy_app::{AppError, Session, Workspace};
use genealogy_plugin_host::{
    Capability, ExportTarget, Grants, Invocation, PluginHost, ProgressControl, ProgressUpdate, ResourceBudget,
};

use crate::i18n::Localizer;

/// Runs a bulk import plugin against the open workspace, streaming `file` in and reporting progress
/// to stderr (ADR 0013). The plugin is attributed to a Software operator.
pub async fn import(workspace: Workspace, localizer: &Localizer, plugin: &str, file: PathBuf) -> Result<(), AppError> {
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
pub async fn export(
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

/// Renders a plugin progress update to stderr and tells the plugin to proceed. The `step` is the
/// plugin's own vocabulary, shown verbatim; only the counts are decorated. The CLI does not yet
/// trigger cancellation (a future interrupt handler will return [`ProgressControl::Cancel`]).
fn render_progress(update: ProgressUpdate) -> ProgressControl {
    let ProgressUpdate { step, processed, total } = update;
    match total {
        Some(total) => eprintln!("  {step}: {processed}/{total}"),
        None => eprintln!("  {step}: {processed}"),
    }
    ProgressControl::Proceed
}
