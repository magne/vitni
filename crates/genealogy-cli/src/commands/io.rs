//! The bulk import/export commands (ADR 0013): load a plugin component, stream a file through the
//! host-mediated source/sink, and render progress to stderr. Kept out of `main` so the binary's
//! entry point stays a thin parse-and-dispatch shell, like the per-aggregate command modules.

use std::path::{Path, PathBuf};

use genealogy_app::{AiConfig, AppError, ConfigStore, FileConfigStore, Session, Workspace};
use genealogy_plugin_host::{
    ExportTarget, Grants, Invocation, NetPolicy, PluginHost, ProgressControl, ProgressUpdate, ResourceBudget,
    TrustRoots, resolve_trust_roots,
};

use crate::i18n::Localizer;

/// Runs a bulk import plugin against the open workspace, streaming `file` in and reporting progress
/// to stderr (ADR 0013). The plugin is attributed to a Software operator.
pub async fn import(workspace: Workspace, localizer: &Localizer, plugin: &str, file: PathBuf) -> Result<(), AppError> {
    let host = PluginHost::new().map_err(|error| AppError::Plugin(error.to_string()))?;
    let bundle = resolve_bundle_dir(workspace.dir(), plugin)?;
    let grants = effective_grants(&host, &bundle, workspace.dir())?;
    let component = host
        .load_bundle(&bundle)
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    let run = Invocation {
        workspace,
        session: Session::software(plugin, env!("CARGO_PKG_VERSION")),
        grants,
        budget: ResourceBudget::default(),
        net_policy: NetPolicy::deny_all(),
        ai_config: AiConfig::default(),
        provenance_confidence: None,
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
    let bundle = resolve_bundle_dir(workspace.dir(), plugin)?;
    let grants = effective_grants(&host, &bundle, workspace.dir())?;
    let component = host
        .load_bundle(&bundle)
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
        grants,
        budget: ResourceBudget::default(),
        net_policy: NetPolicy::deny_all(),
        ai_config: AiConfig::default(),
        provenance_confidence: None,
    };
    let (count, _workspace) = host
        .run_bulk_export(&component, run, target, render_progress)
        .await
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    println!("{}", localizer.export_success(count, &destination));
    Ok(())
}

/// The embedded plugin layer: `$GENEALOGY_PLUGIN_DIR`, else `target/plugins` relative to the working
/// directory (the dev default). The lowest-precedence layer of the ADR 0014 §4 override order.
fn embedded_plugins_dir() -> PathBuf {
    match std::env::var_os("GENEALOGY_PLUGIN_DIR") {
        Some(value) => PathBuf::from(value),
        None => PathBuf::from("target/plugins"),
    }
}

/// Resolves the bundle directory for plugin `id` across the ADR 0014 §4 layers — workspace
/// (`<workspace_dir>/plugins`) over the shared app-dir over the embedded fleet — via the app-level
/// resolver. A shared app-dir that cannot be located contributes no layer.
fn resolve_bundle_dir(workspace_dir: &Path, id: &str) -> Result<PathBuf, AppError> {
    let shared = genealogy_app::config::shared_plugins_dir().ok();
    let layers = genealogy_app::plugin_layers(Some(workspace_dir), shared.as_deref(), &embedded_plugins_dir());
    genealogy_app::resolve_bundle(&layers, id)
        .ok_or_else(|| AppError::Plugin(format!("no plugin bundle found for {id:?} in any plugin layer")))
}

/// Resolves the plugin trust roots for classification (ADR 0014 §3): the embedded sanctioned key(s)
/// plus the operator's client-scope pinned publishers from the global config. A config that cannot be
/// located resolves to the embedded roots alone, so the first-party fleet still classifies as
/// sanctioned (the dev key is embedded in debug/CI builds).
fn trust_roots() -> Result<TrustRoots, AppError> {
    let Ok(path) = genealogy_app::config::config_path() else {
        return resolve_trust_roots(&[]).map_err(|error| AppError::Plugin(error.to_string()));
    };
    let trust = FileConfigStore::new(path, None)
        .load_plugin_trust()
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    let pins = genealogy_app::resolve_trust_pins(&trust).map_err(|error| AppError::Plugin(error.to_string()))?;
    resolve_trust_roots(&pins).map_err(|error| AppError::Plugin(error.to_string()))
}

/// The effective capability grant for the bundle at `bundle_dir` (ADR 0014 §5): discovers and
/// classifies it against the trust roots, then intersects its declared capabilities with the open
/// workspace's persisted approval. With no recorded decision a sanctioned/user-trusted plugin grants
/// all its declared capabilities (unchanged for the first-party fleet) and an untrusted plugin grants
/// nothing until explicitly approved.
fn effective_grants(host: &PluginHost, bundle_dir: &Path, workspace_dir: &Path) -> Result<Grants, AppError> {
    let roots = trust_roots()?;
    let info = host
        .discover_bundle(bundle_dir, &roots)
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    let prefs = genealogy_app::read_plugin_preferences(workspace_dir);
    Ok(info.effective_grants(prefs.approved_grants(&info.id)))
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
