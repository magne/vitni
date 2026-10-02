//! The bulk import/export commands (ADR 0013): load a plugin component, stream a file through the
//! host-mediated source/sink, and render progress to stderr. Kept out of `main` so the binary's
//! entry point stays a thin parse-and-dispatch shell, like the per-aggregate command modules.

use std::path::{Path, PathBuf};

use tokio::sync::oneshot;
use vitni_app::{
    AiConfig, AppError, ConfigStore, DatasetChoice, DatasetProposal, FileConfigStore, PlanSummary, Session, Workspace,
};
use vitni_plugin_host::{
    DeferMatches, ExportTarget, Grants, ImportRunSpec, Invocation, NetPolicy, PluginError, PluginHost, PluginInfo,
    ProgressControl, ProgressStep, ProgressUpdate, ResourceBudget, RunDataset, TrustRoots, channel_reviewer,
    resolve_trust_roots,
};

use crate::commands::review::{self, PlanFormat, ReviewMode, Reviewed, Terminal};
use crate::i18n::Localizer;

/// A dataset proposal the host asks the operator about, with where to send the answer.
type DatasetQuestion = (DatasetProposal, oneshot::Sender<Option<DatasetChoice>>);

/// A bulk import resolved up to the point of running: the plugin, its grants, and the import run it
/// will write (ADR 0037 §3, §5). Resolving it first lets a dataset choice be refused before anything
/// is asked or written.
pub struct PreparedImport {
    host: PluginHost,
    bundle: PathBuf,
    grants: Grants,
    run: ImportRunSpec,
    /// Where the host asks which dataset the file belongs to, when the operator named none while
    /// datasets of its scheme exist.
    question: Option<oneshot::Receiver<DatasetQuestion>>,
}

impl PreparedImport {
    /// Resolves the plugin and the dataset `choice` for importing `file` into `workspace`, with
    /// `operator` as the run's operator.
    ///
    /// # Errors
    /// [`AppError::Plugin`] if the plugin cannot be found, verified, or declares no dataset;
    /// [`AppError::Dataset`] if the choice cannot be resolved.
    pub async fn prepare(
        workspace: &Workspace,
        plugin: &str,
        file: &Path,
        choice: DatasetChoice,
        operator: Session,
    ) -> Result<Self, AppError> {
        let host = PluginHost::new().map_err(|error| AppError::Plugin(error.to_string()))?;
        let bundle = resolve_bundle_dir(workspace.dir(), plugin)?;
        let info = discover(&host, &bundle)?;
        let grants = effective_grants(&info, workspace.dir());
        let Some(spec) = info.dataset.clone() else {
            return Err(AppError::Plugin(format!(
                "plugin {plugin:?} declares no dataset, so it cannot import"
            )));
        };
        let source_label = file.file_name().map_or_else(
            || file.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let chosen = vitni_app::choose_dataset(workspace, &operator, &spec, choice, &source_label).await?;
        let (dataset, question) = if let Some(chosen) = chosen {
            (RunDataset::Chosen(chosen), None)
        } else {
            let (asked, question) = oneshot::channel::<DatasetQuestion>();
            let confirm = RunDataset::Propose {
                spec,
                confirm: Box::new(move |proposal| {
                    let (reply, answer) = oneshot::channel();
                    // A dropped receiver means the import is over; the answer then never comes.
                    drop(asked.send((proposal, reply)));
                    Box::pin(async move { answer.await.ok().flatten() })
                }),
            };
            (confirm, Some(question))
        };
        let run = ImportRunSpec {
            operator,
            dataset,
            source_label,
            plugin: info.id,
            plugin_version: info.version,
            reviewer: Box::new(DeferMatches),
        };
        Ok(Self {
            host,
            bundle,
            grants,
            run,
            question,
        })
    }

    /// Runs the import, streaming `file` in and reporting progress to stderr (ADR 0013). The plugin's
    /// claims are attributed to a Software operator; the run to the invoking human. When the host
    /// proposes a dataset, the operator confirms it, or `options.yes` accepts it (ADR 0037 §3). The
    /// plan is printed, reviewed or committed as `options.mode` says (ADR 0040 §4).
    ///
    /// # Errors
    /// [`AppError::Plugin`] if the component cannot be loaded or the import fails; [`AppError::Dataset`]
    /// if the file's dataset is left unresolved.
    pub async fn run(
        self,
        workspace: Workspace,
        localizer: &Localizer,
        file: PathBuf,
        options: ImportOptions,
    ) -> Result<(), AppError> {
        let Self {
            host,
            bundle,
            grants,
            mut run,
            question,
        } = self;
        let ImportOptions { yes, mode } = options;
        let (reviewer, requests) = channel_reviewer();
        run.reviewer = reviewer;
        let source = run.source_label.clone();
        let component = host
            .load_bundle(&bundle)
            .map_err(|error| AppError::Plugin(error.to_string()))?;
        let plugin = run.plugin.clone();
        let invocation = Invocation {
            workspace,
            session: Session::software(&run.plugin, &run.plugin_version),
            grants,
            budget: ResourceBudget::default(),
            net_policy: NetPolicy::deny_all(),
            ai_config: AiConfig::default(),
            provenance_confidence: None,
            import: Some(run),
        };
        let import = Box::pin(host.run_bulk_import(&component, invocation, file, progress_renderer(localizer)));
        let answer = async {
            let Some(question) = question else {
                return;
            };
            if let Ok((proposal, reply)) = question.await {
                drop(reply.send(Some(answer_proposal(localizer, &proposal, yes))));
            }
        };
        let mut terminal = Terminal {
            // Unlocked: the dataset proposal's `confirm` reads stdin too, before the review does.
            input: std::io::BufReader::new(std::io::stdin()),
            prompts: std::io::stderr(),
            output: std::io::stdout(),
        };
        let review = review::answer(requests, localizer, (mode, &source), &mut terminal);
        let (imported, (), reviewed) = tokio::join!(import, answer, review);
        let (count, _workspace) = imported.map_err(|error| match error {
            PluginError::Dataset(error) => AppError::Dataset(error),
            other => AppError::Plugin(other.to_string()),
        })?;
        match (reviewed, mode) {
            (Reviewed::Printed, _) => {}
            (Reviewed::NotPlanned, ReviewMode::Print(format)) => {
                let empty = PlanSummary::default();
                let lines = match format {
                    PlanFormat::Text => review::plan_lines(localizer, &source, &empty, true),
                    PlanFormat::Json => vec![review::plan_json(&source, &empty)],
                };
                for line in lines {
                    println!("{line}");
                }
            }
            (Reviewed::Cancelled, _) => println!("{}", localizer.import_cancelled()),
            (Reviewed::NotPlanned | Reviewed::Committed, _) => {
                println!("{}", localizer.import_success(count, &plugin));
            }
        }
        Ok(())
    }
}

/// How `vitni import` runs once prepared.
#[derive(Debug, Clone, Copy)]
pub struct ImportOptions {
    /// Accept the host's dataset proposal without asking.
    pub yes: bool,
    /// How the plan is shown and its possible matches decided.
    pub mode: ReviewMode,
}

/// The operator's answer to the host's dataset proposal: the proposed dataset once confirmed, or
/// accepted by `yes`. A declined proposal, or none, is no choice, which the host refuses with the
/// datasets to choose from.
fn answer_proposal(localizer: &Localizer, proposal: &DatasetProposal, yes: bool) -> DatasetChoice {
    let Some(candidate) = proposal.proposed_candidate() else {
        return DatasetChoice::Unspecified;
    };
    let accepted = if yes {
        eprintln!(
            "{}",
            localizer.import_dataset_proposed_accepted(&candidate.label, candidate.shared, proposal.keys)
        );
        true
    } else {
        crate::confirm(&localizer.import_dataset_proposed(&candidate.label, candidate.shared, proposal.keys))
    };
    if accepted {
        DatasetChoice::Existing(candidate.id.to_string())
    } else {
        DatasetChoice::Unspecified
    }
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
    let info = discover(&host, &bundle)?;
    let grants = effective_grants(&info, workspace.dir());
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
        session: Session::software(plugin, &info.version),
        grants,
        budget: ResourceBudget::default(),
        net_policy: NetPolicy::deny_all(),
        ai_config: AiConfig::default(),
        provenance_confidence: None,
        import: None,
    };
    let (count, _workspace) = host
        .run_bulk_export(&component, run, target, progress_renderer(localizer))
        .await
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    println!("{}", localizer.export_success(count, &destination));
    Ok(())
}

/// The ordered plugin-bundle layers for `workspace_dir` (ADR 0014 §4): workspace over the shared
/// app-dir over the embedded fleet. A shared app-dir that cannot be located contributes no layer.
pub(crate) fn plugin_layers(workspace_dir: &Path) -> Vec<PathBuf> {
    let shared = vitni_app::config::shared_plugins_dir().ok();
    vitni_app::plugin_layers(
        Some(workspace_dir),
        shared.as_deref(),
        &vitni_app::embedded_plugins_dir(),
    )
}

/// Resolves the bundle directory for plugin `id` across the ADR 0014 §4 layers via the app-level
/// resolver.
fn resolve_bundle_dir(workspace_dir: &Path, id: &str) -> Result<PathBuf, AppError> {
    vitni_app::resolve_bundle(&plugin_layers(workspace_dir), id)
        .ok_or_else(|| AppError::Plugin(format!("no plugin bundle found for {id:?} in any plugin layer")))
}

/// Resolves the plugin trust roots for classification (ADR 0014 §3): the embedded sanctioned key(s)
/// plus the operator's client-scope pinned publishers from the global config. A config that cannot be
/// located resolves to the embedded roots alone, so the first-party fleet still classifies as
/// sanctioned (the dev key is embedded in debug/CI builds).
pub(crate) fn trust_roots() -> Result<TrustRoots, AppError> {
    let Ok(path) = vitni_app::config::config_path() else {
        return resolve_trust_roots(&[]).map_err(|error| AppError::Plugin(error.to_string()));
    };
    let trust = FileConfigStore::new(path, None)
        .load_plugin_trust()
        .map_err(|error| AppError::Plugin(error.to_string()))?;
    let pins = vitni_app::resolve_trust_pins(&trust).map_err(|error| AppError::Plugin(error.to_string()))?;
    resolve_trust_roots(&pins).map_err(|error| AppError::Plugin(error.to_string()))
}

/// Discovers and classifies the bundle at `bundle_dir` against the trust roots (ADR 0014 §3).
fn discover(host: &PluginHost, bundle_dir: &Path) -> Result<PluginInfo, AppError> {
    let roots = trust_roots()?;
    host.discover_bundle(bundle_dir, &roots)
        .map_err(|error| AppError::Plugin(error.to_string()))
}

/// The effective capability grant for a discovered plugin (ADR 0014 §5): its declared capabilities
/// intersected with the open workspace's persisted approval. With no recorded decision a
/// sanctioned/user-trusted plugin grants all its declared capabilities (unchanged for the first-party
/// fleet) and an untrusted plugin grants nothing until explicitly approved.
fn effective_grants(info: &PluginInfo, workspace_dir: &Path) -> Grants {
    let prefs = vitni_app::read_plugin_preferences(workspace_dir);
    info.effective_grants(prefs.approved_grants(&info.id))
}

/// A progress sink that renders each update to stderr and tells the plugin to proceed. A plugin's step
/// is its own vocabulary, shown verbatim; the host's writing step is `writing`, localized. Only the
/// counts are decorated. The CLI does not yet trigger cancellation (a future interrupt handler will
/// return [`ProgressControl::Cancel`]).
fn progress_renderer(localizer: &Localizer) -> impl FnMut(ProgressUpdate) -> ProgressControl + Send + 'static {
    let writing = localizer.import_progress_writing();
    move |update| render_progress(update, &writing)
}

fn render_progress(update: ProgressUpdate, writing: &str) -> ProgressControl {
    let ProgressUpdate { step, processed, total } = update;
    let step = match step {
        ProgressStep::Plugin(step) => step,
        ProgressStep::Writing => writing.to_owned(),
    };
    match total {
        Some(total) => eprintln!("  {step}: {processed}/{total}"),
        None => eprintln!("  {step}: {processed}"),
    }
    ProgressControl::Proceed
}
