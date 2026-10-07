//! The bulk-import wizard body (issue #191): the GUI counterpart of `vitni import <plugin> <file>
//! (--new NAME PATH | --into NAME) [--yes]`, reached as a mode on `Tool::Import` (`screens/import.rs`)
//! rather than a separate `Tool` — mirrors the shipped bulk-export wizard (`screens/export.rs`).
//!
//! Stages: Source (pick an installed bulk-import plugin, a source file, and a target workspace) →
//! Running (progress, cancellable) → Plan (what the import would write, by kind; ADR 0040 §4) →
//! Review (each possible match, only while one is asked about) → Summary. The Plan and Review stages
//! are `screens/bulk_review.rs`. A failed run and a cancelled run share the export wizard's
//! [`NoticeStage`]/[`WizardNoticeTone`].
//!
//! Unlike the export wizard the target may not be the workspace currently open: importing into an
//! *existing* non-empty workspace is confirmed first in a [`Modal`], mirroring the CLI's own confirm
//! (`main.rs:350-359`); a freshly registered workspace is always empty, so a `New` target never
//! prompts. When the target holds earlier imports of the plugin's scheme, the import starts at once
//! and the confirm comes once the file is read: the host proposes which earlier tree the file is a
//! later export of (ADR 0037 §3), the dialog starts on that tree, and the operator confirms or
//! overrules it. After a successful import into the workspace already open this session, the shell's
//! data version is bumped ([`NavState::mark_changed`]) so the views shown elsewhere refetch, while the
//! wizard keeps its summary on screen.
//!
//! Each stage is a pure component over already-localized label structs, so it renders in isolation
//! (the SSR tests do exactly that). [`BulkImportBody`] owns the session, probes/confirms, starts the
//! invocation, and pumps the host's progress into [`BulkImportSession`].

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use vitni_plugin_host::{PluginRole, ProgressStep, ReviewRequest};
use vitni_ui::{
    BulkImportProgress, BulkImportSession, BulkImportStage, BulkImportStep, BulkImportSummary, EarlierImportVm,
    ImportSourcePath, ImportTargetChoice, ImportTargetError, Localizer,
};

use super::export::{NoticeStage, WizardNoticeTone};
use super::prelude::*;
use crate::components::Modal;
use crate::i18n::Chrome;
use crate::screens::shared::confidence_choices;
use crate::screens::{
    BulkConfirmStage, BulkPlanStage, BulkReviewStage, bulk_confirm_labels, bulk_plan_labels, bulk_review_labels,
};
use crate::services::{
    BulkImportHandle, DatasetQuestion, PluginRow, Services, discover_plugins, import_runs, probe_import_target,
    start_bulk_import,
};
use tokio::sync::oneshot;
use vitni_app::{DatasetChoice, DatasetProposal, PlanStep, ReviewReply, RunResume};

/// The wizard chrome shared across stages: the heading and the three step names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkImportWizardLabels {
    /// The wizard heading.
    pub heading: String,
    /// The stage names (Source, Running, Plan, Review, Summary).
    pub stages: [String; 5],
}

/// The Source-stage labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkSourceLabels {
    /// The stage heading.
    pub heading: String,
    /// The plugin-selector label.
    pub plugin: String,
    /// The "no plugins installed" message.
    pub no_plugins: String,
    /// The source-file field label.
    pub source: String,
    /// The source-file field placeholder.
    pub source_placeholder: String,
    /// The live path-preview label.
    pub preview: String,
    /// The hint shown when the typed path names a directory rather than a file.
    pub directory_hint: String,
    /// The target radio group's accessible name.
    pub target_label: String,
    /// The "Import into an existing workspace" choice label.
    pub target_existing: String,
    /// The "Create a new workspace" choice label.
    pub target_new: String,
    /// The existing-workspace selector label.
    pub workspace_label: String,
    /// The Run action label.
    pub run: String,
}

/// The Running-stage labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkRunningLabels {
    /// The stage heading.
    pub heading: String,
    /// The step name shown before the plugin reports its first step.
    pub starting: String,
    /// The step name shown while the host writes the records the plugin read.
    pub writing: String,
    /// The already-formatted progress count (e.g. "40 of 120").
    pub count: String,
    /// The cancel action label.
    pub cancel: String,
}

/// The Summary-stage labels (the record count is pre-formatted by the caller).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkSummaryLabels {
    /// The stage heading.
    pub heading: String,
    /// The "{n} records imported" text.
    pub records: String,
    /// The source-row label.
    pub source: String,
    /// The "Import another" action label.
    pub another: String,
}

/// The non-empty-target confirm modal's already-localized text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkConfirmLabels {
    /// The dialog heading (names the target workspace).
    pub title: String,
    /// The dialog body (names the person count).
    pub body: String,
    /// The label of the "which tree is this file" select (ADR 0037 §3).
    pub dataset: String,
    /// The select's first, unchosen entry.
    pub dataset_placeholder: String,
    /// The note shown once an earlier tree is chosen: its records are updated, not duplicated.
    pub later_export: String,
    /// The Cancel action label.
    pub cancel: String,
    /// The "Import anyway" action label.
    pub run: String,
    /// The accessible name for the dialog's click-away scrim.
    pub dismiss: String,
}

/// The run the operator is about to start once they resolve the non-empty-workspace confirm — enough
/// to launch it unchanged from the [`Modal`]'s "Import anyway".
#[derive(Clone)]
struct PendingRun {
    services: Services,
    plugin_id: String,
    source: PathBuf,
    workspace: String,
    count: usize,
    unknown_failure: String,
}

/// A bulk import ready to launch: what to run, where, and into which dataset.
struct BulkRun {
    services: Services,
    plugin_id: String,
    source: PathBuf,
    target: ImportTargetChoice,
    dataset: DatasetChoice,
    /// How many persons the target already holds, for the confirm a proposal opens.
    persons: usize,
    unknown_failure: String,
}

/// The dataset the host proposed for a file (ADR 0037 §3): its select value, and the note naming the
/// evidence, shown while it stays chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedDataset {
    /// The proposed dataset's id, as the select's value.
    pub id: String,
    /// The evidence: how many of the file's records the dataset already holds.
    pub note: String,
}

/// The confirm dialog's dataset question for a host proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetQuestionView {
    /// Every earlier tree of the scheme, then "a different tree".
    pub options: Vec<SelectChoice>,
    /// The value the select starts on: the proposed dataset, or nothing.
    pub value: String,
    /// The proposed dataset, if the evidence pointed to one.
    pub proposed: Option<ProposedDataset>,
}

/// Builds the dataset question for `proposal`: one option per earlier tree, "a different tree", and
/// the proposed tree preselected with its evidence. With no proposal nothing is preselected, so the
/// import waits for the operator's choice.
#[must_use]
pub fn dataset_question(chrome: &Chrome, proposal: &DatasetProposal) -> DatasetQuestionView {
    let mut options = Vec::with_capacity(proposal.candidates.len() + 1);
    for candidate in &proposal.candidates {
        options.push(SelectChoice {
            value: candidate.id.to_string(),
            label: chrome.bulk_import_dataset_existing(&candidate.label),
        });
    }
    options.push(SelectChoice {
        value: NEW_DATASET.to_owned(),
        label: chrome.bulk_import_dataset_new(),
    });
    let proposed = proposal.proposed_candidate().map(|candidate| ProposedDataset {
        id: candidate.id.to_string(),
        note: chrome.bulk_import_dataset_proposed(&candidate.label, candidate.shared, proposal.keys),
    });
    let value = proposed
        .as_ref()
        .map(|proposed| proposed.id.clone())
        .unwrap_or_default();
    DatasetQuestionView {
        options,
        value,
        proposed,
    }
}

/// The host's question which tree a file belongs to, while it waits for the operator's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AskedDataset {
    workspace: String,
    persons: usize,
    question: DatasetQuestionView,
}

/// The signals the host's questions after the read run on: the dataset confirm's question on screen,
/// where its answer goes, and the select's value (shared with the pre-launch confirm); and where the
/// answer to the plan or the possible match on screen goes.
#[derive(Clone, Copy)]
struct Asking {
    asked: Signal<Option<AskedDataset>>,
    reply: Signal<Option<oneshot::Sender<Option<DatasetChoice>>>>,
    dataset: Signal<String>,
    review: Signal<Option<ReviewResponder>>,
}

/// Where the answer to the host's Plan or Review question goes.
enum ReviewResponder {
    /// What to do with the plan.
    Plan(oneshot::Sender<PlanStep>),
    /// The answer to one possible match.
    Match(oneshot::Sender<ReviewReply>),
    /// Whether to commit the plan as the review's answers leave it.
    Confirm(oneshot::Sender<bool>),
}

impl Asking {
    /// Sends `answer` to the waiting import and closes the question.
    fn answer(mut self, answer: Option<DatasetChoice>) {
        if let Some(reply) = self.reply.write().take() {
            // A dropped receiver means the import already ended; there is nothing left to answer.
            drop(reply.send(answer));
        }
        self.asked.set(None);
        self.dataset.set(String::new());
    }
}

/// The value the dataset select gives "a different tree".
const NEW_DATASET: &str = "new";

/// The dataset choice a confirm-dialog value stands for: nothing to choose, a new tree, or an
/// existing dataset's id.
fn dataset_choice(value: &str) -> DatasetChoice {
    match value {
        "" => DatasetChoice::Unspecified,
        NEW_DATASET => DatasetChoice::New,
        id => DatasetChoice::Existing(id.to_owned()),
    }
}

/// The bulk-import wizard body: owns the session, probes a non-empty existing target, starts the
/// invocation, and drives its progress. Mounted by `screens/import.rs` when the operator's mode choice
/// is "Bulk file import".
#[component]
pub fn BulkImportBody() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let chrome = use_context::<ChromeCtx>().0;
    let session = use_signal(BulkImportSession::new);
    let cancel = use_signal(|| None::<Arc<AtomicBool>>);
    let plugin_id = use_signal(String::new);
    let source_typed = use_signal(String::new);
    let target_mode = use_signal(|| "existing".to_owned());
    let target_workspace = use_signal(String::new);
    let register = RegisterFields {
        open: use_signal(|| true),
        name: use_signal(String::new),
        directory: use_signal(String::new),
        database_url: use_signal(String::new),
    };
    let pending = use_signal(|| None::<PendingRun>);
    let dataset = use_signal(String::new);
    let asking = Asking {
        asked: use_signal(|| None),
        reply: use_signal(|| None),
        dataset,
        review: use_signal(|| None),
    };
    let default_dir = state.services().dir.clone();
    let mut resume = resumer(&state, &chrome, session, (cancel, asking));
    // A run row's *Resume* hands its run over through `import_resume`; take it and re-run it at once.
    use_effect(move || {
        let Some(mut nav) = try_consume_context::<NavState>() else {
            return;
        };
        let handed = nav.import_resume.read().clone();
        if let Some(run) = handed {
            nav.import_resume.set(None);
            resume(run);
        }
    });

    let body = match session().stage().clone() {
        BulkImportStage::Source => bulk_source_body(
            &state,
            &chrome,
            plugin_id,
            source_typed,
            target_mode,
            target_workspace,
            register,
            session,
            cancel,
            pending,
            asking,
            &default_dir,
        ),
        BulkImportStage::Running(progress) => rsx! {
            BulkRunningStage {
                labels: bulk_running_labels(&chrome, &progress),
                progress,
                oncancel: move |()| bulk_request_cancel(session, cancel),
            }
        },
        BulkImportStage::Plan(summary) => rsx! {
            BulkPlanStage {
                labels: bulk_plan_labels(&chrome, &summary),
                onstep: move |step| answer_plan(step, session, asking.review),
            }
        },
        BulkImportStage::Review(stage) => rsx! {
            BulkReviewStage {
                key: "{stage.position}-{stage.compare.left.human_id}",
                labels: bulk_review_labels(&chrome, &stage),
                stage: *stage,
                confidence_options: confidence_choices(state.data_loc()),
                onanswer: move |reply| answer_review(reply, session, asking.review),
            }
        },
        BulkImportStage::Confirm(summary) => rsx! {
            BulkConfirmStage {
                labels: bulk_confirm_labels(&chrome, &summary),
                onconfirm: move |commit| answer_confirm(commit, session, asking.review),
            }
        },
        BulkImportStage::Summary(summary) => rsx! {
            BulkSummaryStage {
                labels: bulk_summary_labels(&chrome, summary.records),
                source: summary.source,
                onrestart: move |()| bulk_restart(session),
            }
        },
        BulkImportStage::Error(message) => rsx! {
            NoticeStage {
                tone: WizardNoticeTone::Failure,
                heading: chrome.bulk_import_error_heading(),
                message,
                restart_label: chrome.bulk_import_another(),
                onrestart: move |()| bulk_restart(session),
            }
        },
        BulkImportStage::Cancelled => rsx! {
            NoticeStage {
                tone: WizardNoticeTone::Cancelled,
                heading: chrome.bulk_import_cancelled_heading(),
                message: chrome.bulk_import_cancelled_message(),
                restart_label: chrome.bulk_import_another(),
                onrestart: move |()| bulk_restart(session),
            }
        },
    };

    rsx! {
        div { style: "display:flex;flex-direction:column;gap:var(--sp-4)",
            {bulk_step_indicator(&bulk_wizard_labels(&chrome), session().stage())}
            {body}
            {asked_modal(&chrome, asking, session, cancel)}
        }
    }
}

/// Builds the Source stage: discovers installed bulk-import plugins and registered workspaces, then
/// renders [`BulkSourceStage`] plus the non-empty-target confirm [`Modal`] when armed.
#[expect(
    clippy::too_many_arguments,
    reason = "the source stage seeds every session signal plus the run/confirm guards"
)]
fn bulk_source_body(
    state: &AppState,
    chrome: &Chrome,
    plugin_id: Signal<String>,
    source_typed: Signal<String>,
    target_mode: Signal<String>,
    target_workspace: Signal<String>,
    register: RegisterFields,
    session: Signal<BulkImportSession>,
    cancel: Signal<Option<Arc<AtomicBool>>>,
    pending: Signal<Option<PendingRun>>,
    asking: Asking,
    default_dir: &Path,
) -> Element {
    let plugin_options = discover_import_plugins(state, plugin_id);
    let (workspace_options, registered_names) = target_workspace_options(state, target_workspace);
    let target = build_target_choice(&target_mode(), &target_workspace(), &register);
    let target_error = target_error_message(chrome, &target, &registered_names);
    let onrun = build_bulk_onrun(
        state,
        chrome,
        plugin_id,
        source_typed,
        target_mode,
        target_workspace,
        register,
        session,
        cancel,
        pending,
        asking,
        default_dir.to_path_buf(),
        registered_names,
    );
    let new_workspace_fields = rsx! { {register_fields_form(chrome, register, "register")} };
    let importers: Vec<String> = plugin_options.iter().map(|choice| choice.value.clone()).collect();
    let onresume = resumer(state, chrome, session, (cancel, asking));
    rsx! {
        BulkSourceStage {
            labels: bulk_source_labels(chrome),
            plugin_options,
            plugin_id,
            source_typed,
            default_dir: default_dir.to_path_buf(),
            target_mode,
            target_workspace,
            workspace_options,
            new_workspace_fields,
            target_error,
            onrun,
        }
        SourceEarlierImports { importers, onresume }
        {confirm_modal(chrome, pending, asking, session, cancel)}
    }
}

/// The Source stage's [`EarlierImports`], loading the open workspace's runs of `importers` itself.
#[component]
fn SourceEarlierImports(importers: Vec<String>, onresume: EventHandler<RunResume>) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let chrome = use_context::<ChromeCtx>().0;
    let services = state.services().clone();
    let runs = use_resource(move || {
        let services = services.clone();
        async move {
            import_runs(services)
                .await
                .inspect_err(|error| tracing::warn!(%error, "could not list the earlier imports"))
                .unwrap_or_default()
        }
    });
    let rows = runs
        .read_unchecked()
        .as_ref()
        .map(|runs| EarlierImportVm::list(runs, &importers, state.data_loc()))
        .unwrap_or_default();
    rsx! {
        EarlierImports {
            labels: EarlierImportsLabels {
                heading: chrome.bulk_import_earlier_heading(),
                resume: state.data_loc().import_run_resume(),
            },
            rows,
            onresume,
        }
    }
}

/// Re-runs an abandoned bulk run at once (ADR 0040 §5): its plugin over its file, into its dataset, in
/// the open workspace, where what it already wrote resolves as unchanged.
fn resumer(
    state: &AppState,
    chrome: &Chrome,
    session: Signal<BulkImportSession>,
    guards: (Signal<Option<Arc<AtomicBool>>>, Asking),
) -> impl FnMut(RunResume) + Clone + 'static {
    let services = state.services().clone();
    let unknown_failure = chrome.bulk_import_failed_unknown();
    move |resume: RunResume| {
        let RunResume {
            plugin,
            dataset,
            source,
        } = resume;
        let run = BulkRun {
            target: ImportTargetChoice::Existing {
                workspace: services.open_workspace.clone(),
            },
            services: services.clone(),
            plugin_id: plugin,
            source,
            dataset: DatasetChoice::Existing(dataset.to_string()),
            persons: 0,
            unknown_failure: unknown_failure.clone(),
        };
        launch_bulk_import(run, session, guards);
    }
}

/// Discovers installed bulk-import plugins, auto-selecting the first when nothing is picked yet, and
/// returns the `Select`'s options.
fn discover_import_plugins(state: &AppState, plugin_id: Signal<String>) -> Vec<SelectChoice> {
    let mut plugin_id = plugin_id;
    let discover_services = state.services().clone();
    let plugins = use_resource(move || {
        let services = discover_services.clone();
        async move { discover_plugins(services).await.unwrap_or_default() }
    });
    let importers: Vec<PluginRow> = plugins
        .read_unchecked()
        .as_ref()
        .map(|rows| {
            rows.iter()
                .filter(|row| row.role == PluginRole::BulkImport && row.enabled)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if plugin_id().is_empty()
        && let Some(first) = importers.first()
    {
        plugin_id.set(first.id.clone());
    }
    importers
        .iter()
        .map(|row| SelectChoice {
            value: row.id.clone(),
            label: row.id.clone(),
        })
        .collect()
}

/// The registered workspaces as `Select` options (auto-selecting the first when nothing is picked
/// yet) plus the plain name list [`ImportTargetChoice::validate`] checks a new name against.
fn target_workspace_options(state: &AppState, target_workspace: Signal<String>) -> (Vec<SelectChoice>, Vec<String>) {
    let mut target_workspace = target_workspace;
    let workspaces = vitni_app::list_workspaces(&state.services().config);
    if target_workspace().is_empty()
        && let Some(first) = workspaces.first()
    {
        target_workspace.set(first.name.clone());
    }
    let options: Vec<SelectChoice> = workspaces
        .iter()
        .map(|summary| SelectChoice {
            value: summary.name.clone(),
            label: summary.name.clone(),
        })
        .collect();
    let names: Vec<String> = workspaces.into_iter().map(|summary| summary.name).collect();
    (options, names)
}

/// Builds the Run handler: validates the current plugin/source/target, then probes an existing target.
/// One holding earlier imports of the plugin's scheme launches at once, its confirm asked once the
/// file is read; one holding only persons arms the confirm [`Modal`] first; an empty one, or a fresh
/// `New` target, launches directly.
#[expect(
    clippy::too_many_arguments,
    reason = "the Run handler threads every session signal plus the validated run inputs"
)]
fn build_bulk_onrun(
    state: &AppState,
    chrome: &Chrome,
    plugin_id: Signal<String>,
    source_typed: Signal<String>,
    target_mode: Signal<String>,
    target_workspace: Signal<String>,
    register: RegisterFields,
    session: Signal<BulkImportSession>,
    cancel: Signal<Option<Arc<AtomicBool>>>,
    pending: Signal<Option<PendingRun>>,
    asking: Asking,
    default_dir: PathBuf,
    registered_names: Vec<String>,
) -> impl FnMut(()) + 'static {
    let run_services = state.services().clone();
    let unknown_failure = chrome.bulk_import_failed_unknown();
    move |()| {
        let id = plugin_id();
        let source = ImportSourcePath::parse(&source_typed(), &default_dir);
        if id.is_empty() || !source.is_usable() {
            return;
        }
        let Some(source_path) = source.path().map(Path::to_path_buf) else {
            return;
        };
        let target = build_target_choice(&target_mode(), &target_workspace(), &register);
        if target.validate(&registered_names).is_err() {
            return;
        }
        let (mut session, cancel, mut pending) = (session, cancel, pending);
        let services = run_services.clone();
        let unknown_failure = unknown_failure.clone();
        match target {
            ImportTargetChoice::Existing { workspace } => {
                let probe_services = services.clone();
                spawn(async move {
                    match probe_import_target(&probe_services, &workspace, &id).await {
                        Ok(probe) if probe.persons == 0 || !probe.datasets.is_empty() => launch_bulk_import(
                            BulkRun {
                                services,
                                plugin_id: id,
                                source: source_path,
                                target: ImportTargetChoice::Existing { workspace },
                                dataset: DatasetChoice::Unspecified,
                                persons: probe.persons,
                                unknown_failure,
                            },
                            session,
                            (cancel, asking),
                        ),
                        Ok(probe) => pending.set(Some(PendingRun {
                            services: probe_services,
                            plugin_id: id,
                            source: source_path,
                            workspace,
                            count: probe.persons,
                            unknown_failure,
                        })),
                        Err(message) => session.write().on_failure(message),
                    }
                });
            }
            ImportTargetChoice::New { .. } => {
                let run = BulkRun {
                    services,
                    plugin_id: id,
                    source: source_path,
                    target,
                    dataset: DatasetChoice::Unspecified,
                    persons: 0,
                    unknown_failure,
                };
                launch_bulk_import(run, session, (cancel, asking));
            }
        }
    }
}

/// The confirm dialog's labels for a target named `workspace` holding `persons` persons.
fn confirm_labels(chrome: &Chrome, workspace: &str, persons: usize) -> BulkConfirmLabels {
    let body = if persons == 0 {
        chrome.bulk_import_confirm_datasets_body(workspace)
    } else {
        chrome.bulk_import_confirm_body(workspace, persons)
    };
    BulkConfirmLabels {
        title: chrome.bulk_import_confirm_title(workspace),
        body,
        dataset: chrome.bulk_import_dataset_label(),
        dataset_placeholder: chrome.bulk_import_dataset_placeholder(),
        later_export: chrome.bulk_import_dataset_later_export(),
        cancel: chrome.bulk_import_confirm_cancel(),
        run: chrome.bulk_import_confirm_run(),
        dismiss: chrome.dismiss(),
    }
}

/// The confirm dialog for an existing target holding persons but no earlier imports of the plugin's
/// scheme: shown while [`PendingRun`] is armed. Cancel drops the pending run; "Import anyway"
/// launches it.
fn confirm_modal(
    chrome: &Chrome,
    mut pending: Signal<Option<PendingRun>>,
    asking: Asking,
    session: Signal<BulkImportSession>,
    cancel: Signal<Option<Arc<AtomicBool>>>,
) -> Element {
    let Some(run) = pending() else {
        return rsx! {};
    };
    rsx! {
        BulkConfirmDialog {
            labels: confirm_labels(chrome, &run.workspace, run.count),
            datasets: Vec::new(),
            dataset: asking.dataset,
            oncancel: move |()| pending.set(None),
            onrun: move |_: String| {
                pending.set(None);
                let bulk = BulkRun {
                    services: run.services.clone(),
                    plugin_id: run.plugin_id.clone(),
                    source: run.source.clone(),
                    target: ImportTargetChoice::Existing { workspace: run.workspace.clone() },
                    dataset: DatasetChoice::Unspecified,
                    persons: run.count,
                    unknown_failure: run.unknown_failure.clone(),
                };
                launch_bulk_import(bulk, session, (cancel, asking));
            },
        }
    }
}

/// The confirm dialog the host's dataset question opens once the file is read (ADR 0037 §3): which
/// earlier tree the file is a later export of, starting on the proposed one. "Import anyway" sends the
/// choice to the waiting import; Cancel stops it before anything is written.
fn asked_modal(
    chrome: &Chrome,
    asking: Asking,
    session: Signal<BulkImportSession>,
    cancel: Signal<Option<Arc<AtomicBool>>>,
) -> Element {
    let Some(asked) = (asking.asked)() else {
        return rsx! {};
    };
    let AskedDataset {
        workspace,
        persons,
        question,
    } = asked;
    rsx! {
        BulkConfirmDialog {
            labels: confirm_labels(chrome, &workspace, persons),
            datasets: question.options,
            dataset: asking.dataset,
            proposed: question.proposed,
            oncancel: move |()| {
                asking.answer(None);
                bulk_request_cancel(session, cancel);
            },
            onrun: move |chosen: String| asking.answer(Some(dataset_choice(&chosen))),
        }
    }
}

/// The existing-target confirm dialog: names the target and what it already holds and — when the
/// target has datasets of the plugin's scheme — asks which tree the file belongs to (ADR 0037 §3).
/// The choice starts on the `proposed` tree, with the evidence for it, when the file's own fingerprint
/// and records point to one; otherwise the import waits for a choice, since guessing would silently
/// merge one file's records into another's. `onrun` receives the chosen value: empty when there was
/// nothing to choose, `new`, or a dataset id.
#[component]
pub fn BulkConfirmDialog(
    labels: BulkConfirmLabels,
    datasets: Vec<SelectChoice>,
    mut dataset: Signal<String>,
    proposed: Option<ProposedDataset>,
    oncancel: EventHandler<()>,
    onrun: EventHandler<String>,
) -> Element {
    let needs_choice = !datasets.is_empty();
    let chosen = dataset();
    let blocked = needs_choice && chosen.is_empty();
    let later_export = needs_choice && !chosen.is_empty() && chosen != NEW_DATASET;
    let evidence = proposed
        .filter(|proposed| proposed.id == chosen)
        .map(|proposed| proposed.note);
    let mut options = Vec::with_capacity(datasets.len() + 1);
    if needs_choice {
        options.push(SelectChoice {
            value: String::new(),
            label: labels.dataset_placeholder.clone(),
        });
        options.extend(datasets);
    }
    rsx! {
        Modal {
            title: labels.title,
            open: true,
            close_label: labels.dismiss,
            onclose: move |()| oncancel.call(()),
            footer: rsx! {
                Button {
                    label: labels.cancel,
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| oncancel.call(()),
                }
                Button {
                    label: labels.run,
                    variant: ButtonVariant::Primary,
                    disabled: blocked,
                    onclick: move |_| onrun.call(dataset()),
                }
            },
            p { "{labels.body}" }
            if needs_choice {
                Select {
                    label: labels.dataset,
                    name: "import-dataset".to_owned(),
                    value: Some(chosen),
                    options,
                    onchange: move |event: FormEvent| dataset.set(event.value()),
                }
            }
            if let Some(evidence) = evidence {
                p { class: "muted", "{evidence}" }
            }
            if later_export {
                p { class: "muted", "{labels.later_export}" }
            }
        }
    }
}

/// The [`EarlierImports`] card's labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlierImportsLabels {
    /// The card's heading.
    pub heading: String,
    /// The *Resume* button on an interrupted run.
    pub resume: String,
}

/// The Source stage's *Earlier imports*: the newest bulk runs into this workspace, each with its state,
/// and *Resume* on an interrupted one (ADR 0040 §5), which re-runs it through `onresume`. Renders
/// nothing when there are none.
#[component]
pub fn EarlierImports(
    labels: EarlierImportsLabels,
    rows: Vec<EarlierImportVm>,
    onresume: EventHandler<RunResume>,
) -> Element {
    if rows.is_empty() {
        return rsx! {};
    }
    rsx! {
        Card {
            h3 { "{labels.heading}" }
            div { class: "timeline", style: "margin-top:8px",
                for row in rows {
                    div { class: "tl-item",
                        div { class: "tl-when", "{row.when}" }
                        div { class: "tl-what",
                            "{row.source}"
                            span { class: "muted tl-count", "{row.status}" }
                            if let Some(resume) = row.resume {
                                button {
                                    class: "btn sm ghost",
                                    style: "margin-left:var(--sp-2)",
                                    r#type: "button",
                                    aria_label: "{resume.label}",
                                    onclick: move |_| onresume.call(resume.run.clone()),
                                    "{labels.resume}"
                                }
                            }
                        }
                        div { class: "tl-who", "{row.plugin}" }
                    }
                }
            }
        }
    }
}

/// Stage 1 — the source picker: an installed-bulk-import-plugin selector, a source-file field with a
/// live path preview, and a target radio (an existing-workspace selector, or the shared new-workspace
/// register fields). Run is disabled until a plugin is chosen, the path names a usable file, and the
/// target passes [`ImportTargetChoice::validate`].
#[component]
pub fn BulkSourceStage(
    labels: BulkSourceLabels,
    plugin_options: Vec<SelectChoice>,
    plugin_id: Signal<String>,
    source_typed: Signal<String>,
    default_dir: PathBuf,
    target_mode: Signal<String>,
    target_workspace: Signal<String>,
    workspace_options: Vec<SelectChoice>,
    new_workspace_fields: Element,
    target_error: Option<String>,
    onrun: EventHandler<()>,
) -> Element {
    let mut plugin_id = plugin_id;
    let mut source_typed = source_typed;
    let mut target_mode = target_mode;
    let mut target_workspace = target_workspace;
    let no_plugins = plugin_options.is_empty();
    let source = ImportSourcePath::parse(&source_typed(), &default_dir);
    let run_disabled = no_plugins || !source.is_usable() || target_error.is_some();
    let mode_choices = vec![
        RadioChoice {
            id: "existing".to_owned(),
            label: labels.target_existing.clone(),
        },
        RadioChoice {
            id: "new".to_owned(),
            label: labels.target_new.clone(),
        },
    ];
    rsx! {
        Card {
            h3 { "{labels.heading}" }
            div { class: "grid-2", style: "align-items:end",
                if no_plugins {
                    div { class: "field", style: "margin:0",
                        span { class: "field-label", "{labels.plugin}" }
                        p { class: "muted", "{labels.no_plugins}" }
                    }
                } else {
                    Select {
                        label: labels.plugin.clone(),
                        name: "bulk-import-plugin".to_owned(),
                        value: Some(plugin_id()),
                        options: plugin_options,
                        onchange: move |event: FormEvent| plugin_id.set(event.value()),
                    }
                }
                div { class: "field", style: "margin:0",
                    label { r#for: "bulk-import-source", "{labels.source}" }
                    TextInput {
                        id: "bulk-import-source",
                        name: "bulk-import-source",
                        value: source_typed(),
                        placeholder: Some(labels.source_placeholder.clone()),
                        oninput: move |event: FormEvent| source_typed.set(event.value()),
                    }
                }
            }
            div { class: "path-preview",
                span { class: "field-label", "{labels.preview}" }
                div { class: "picker-value",
                    span { class: "mono grow", style: "word-break:break-all",
                        {source.path().map(|path| path.display().to_string()).unwrap_or_default()}
                    }
                    if source.names_a_directory() {
                        span { class: "muted", "{labels.directory_hint}" }
                    }
                }
            }
            div { style: "margin-top:var(--sp-3)",
                RadioGroup {
                    group_label: labels.target_label.clone(),
                    choices: mode_choices,
                    selected: target_mode(),
                    onselect: move |id: String| target_mode.set(id),
                }
            }
            div { style: "margin-top:var(--sp-3)",
                if target_mode() == "new" {
                    {new_workspace_fields}
                } else if workspace_options.is_empty() {
                    p { class: "muted", "{labels.workspace_label}" }
                } else {
                    Select {
                        label: labels.workspace_label.clone(),
                        name: "bulk-import-target-workspace".to_owned(),
                        value: Some(target_workspace()),
                        options: workspace_options,
                        onchange: move |event: FormEvent| target_workspace.set(event.value()),
                    }
                }
            }
            if let Some(message) = target_error {
                p { class: "empty", style: "margin-top:var(--sp-3)", role: "alert", "{message}" }
            }
            div { style: "margin-top:var(--sp-3)",
                Button {
                    label: labels.run.clone(),
                    variant: ButtonVariant::Primary,
                    disabled: run_disabled,
                    onclick: move |_| onrun.call(()),
                }
            }
        }
    }
}

/// Stage 2 — the running import: the plugin's current step, a progress bar, and Cancel. Identical
/// layout to the export wizard's `RunningStage`, over [`BulkImportProgress`] instead of `ExportProgress`.
#[component]
pub fn BulkRunningStage(
    labels: BulkRunningLabels,
    progress: BulkImportProgress,
    oncancel: EventHandler<()>,
) -> Element {
    let step = match &progress.step {
        BulkImportStep::Plugin(step) if step.trim().is_empty() => labels.starting.clone(),
        BulkImportStep::Plugin(step) => step.clone(),
        BulkImportStep::Writing => labels.writing.clone(),
    };
    let percent = progress
        .total
        .filter(|total| *total > 0)
        .map(|total| f64::from(progress.processed.min(total)) * 100.0 / f64::from(total));
    rsx! {
        Card {
            h3 { "{labels.heading}" }
            div { class: "wrap", style: "align-items:baseline",
                span { class: "grow", "{step}" }
                span { class: "muted mono", "{labels.count}" }
            }
            div {
                class: if percent.is_some() { "run-progress" } else { "run-progress indeterminate" },
                role: "progressbar",
                aria_label: labels.heading.clone(),
                aria_valuemin: "0",
                aria_valuenow: "{progress.processed}",
                aria_valuemax: progress.total.map(|total| total.to_string()),
                div {
                    class: "run-progress-fill",
                    style: match percent {
                        Some(percent) => format!("width:{percent:.1}%"),
                        None => "width:100%".to_owned(),
                    },
                }
            }
            div { style: "margin-top:var(--sp-3)",
                Button {
                    label: labels.cancel.clone(),
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| oncancel.call(()),
                }
            }
        }
    }
}

/// Stage 3 — the summary: how many records were imported, from where, and "Import another".
#[component]
pub fn BulkSummaryStage(labels: BulkSummaryLabels, source: String, onrestart: EventHandler<()>) -> Element {
    rsx! {
        Card {
            h3 { "{labels.heading}" }
            div { class: "wrap", style: "gap:var(--sp-4)",
                span { class: "badge", "{labels.records}" }
            }
            div { class: "path-preview", style: "margin-top:var(--sp-3)",
                span { class: "field-label", "{labels.source}" }
                div { class: "picker-value",
                    span { class: "mono grow", style: "word-break:break-all", "{source}" }
                }
            }
            div { style: "margin-top:var(--sp-3)",
                Button {
                    label: labels.another.clone(),
                    variant: ButtonVariant::Primary,
                    onclick: move |_| onrestart.call(()),
                }
            }
        }
    }
}

/// The step index of the Review stage, shown only while it is up.
const REVIEW_STEP: usize = 3;

/// The wizard step indicator, identical in shape to the export wizard's own (`.wiz-steps`). The Review
/// step shows only while its stage is up — a plan with no possible match never meets it.
pub fn bulk_step_indicator(labels: &BulkImportWizardLabels, stage: &BulkImportStage) -> Element {
    let current = bulk_stage_index(stage);
    let mut steps = Vec::with_capacity(labels.stages.len());
    for (index, name) in labels.stages.iter().enumerate() {
        if index != REVIEW_STEP || current == REVIEW_STEP {
            steps.push((index, name.clone()));
        }
    }
    rsx! {
        div { class: "wiz-steps", role: "list", aria_label: "{labels.heading}",
            for (number , (index , name)) in steps.into_iter().enumerate() {
                span {
                    class: if index == current { "wiz-step active" } else if index < current { "wiz-step done" } else { "wiz-step" },
                    role: "listitem",
                    aria_current: if index == current { Some("step") } else { None },
                    span { class: "num", "{number + 1}" }
                    " {name}"
                }
            }
        }
    }
}

/// The 0-based step index a stage maps to.
fn bulk_stage_index(stage: &BulkImportStage) -> usize {
    match stage {
        BulkImportStage::Source | BulkImportStage::Cancelled => 0,
        BulkImportStage::Running(_) => 1,
        BulkImportStage::Plan(_) => 2,
        BulkImportStage::Review(_) | BulkImportStage::Confirm(_) => REVIEW_STEP,
        BulkImportStage::Summary(_) | BulkImportStage::Error(_) => 4,
    }
}

/// Sends the operator's `step` for the plan on screen. Discarding it cancels the wizard, as the import
/// writes nothing; otherwise the wizard moves on to writing until the host asks or reports.
fn answer_plan(step: PlanStep, mut session: Signal<BulkImportSession>, mut review: Signal<Option<ReviewResponder>>) {
    let Some(ReviewResponder::Plan(reply)) = review.write().take() else {
        return;
    };
    // A dropped receiver means the import already ended; there is nothing left to answer.
    let _ = reply.send(step);
    match step {
        PlanStep::Discard => session.write().cancel(),
        PlanStep::Review | PlanStep::DeferMatches => session.write().resume(),
    }
}

/// Sends the operator's `reply` to the possible match on screen. Cancelling cancels the wizard, as the
/// import writes nothing; any other answer leaves the pair up until the host asks the next or writes.
fn answer_review(
    reply: ReviewReply,
    mut session: Signal<BulkImportSession>,
    mut review: Signal<Option<ReviewResponder>>,
) {
    let Some(ReviewResponder::Match(responder)) = review.write().take() else {
        return;
    };
    let cancelled = reply == ReviewReply::Cancel;
    // A dropped receiver means the import already ended; there is nothing left to answer.
    let _ = responder.send(reply);
    if cancelled {
        session.write().cancel();
    }
}

/// Sends whether to commit the reviewed plan on screen. Cancelling cancels the wizard, as the import
/// writes nothing; importing moves on to writing.
fn answer_confirm(commit: bool, mut session: Signal<BulkImportSession>, mut review: Signal<Option<ReviewResponder>>) {
    let Some(ReviewResponder::Confirm(reply)) = review.write().take() else {
        return;
    };
    // A dropped receiver means the import already ended; there is nothing left to answer.
    let _ = reply.send(commit);
    if commit {
        session.write().resume();
    } else {
        session.write().cancel();
    }
}

/// The target radio + fields as an [`ImportTargetChoice`], read live from the signals (so validity is
/// recomputed every render, before Run is ever clicked).
fn build_target_choice(mode: &str, workspace: &str, register: &RegisterFields) -> ImportTargetChoice {
    let RegisterFields {
        name,
        directory,
        database_url,
        ..
    } = *register;
    if mode == "new" {
        ImportTargetChoice::New {
            name: name(),
            directory: non_empty(directory()).map(PathBuf::from),
            database_url: non_empty(database_url()),
        }
    } else {
        ImportTargetChoice::Existing {
            workspace: workspace.to_owned(),
        }
    }
}

/// The localized message for a target validation failure, or `None` when the current target (and,
/// for the existing-workspace mode, a made selection) is valid.
fn target_error_message(chrome: &Chrome, target: &ImportTargetChoice, names: &[String]) -> Option<String> {
    if let ImportTargetChoice::Existing { workspace } = target
        && workspace.is_empty()
    {
        return Some(chrome.bulk_import_target_name_required());
    }
    match target.validate(names) {
        Ok(()) => None,
        Err(ImportTargetError::EmptyName) => Some(chrome.prefs_register_name_required()),
        Err(ImportTargetError::NameTaken) => Some(chrome.bulk_import_target_name_taken()),
    }
}

/// Cancels the running import: raises the host-side flag (the run stops at the plugin's next progress
/// report) and moves the wizard to its Cancelled stage straight away.
fn bulk_request_cancel(mut session: Signal<BulkImportSession>, cancel: Signal<Option<Arc<AtomicBool>>>) {
    if let Some(flag) = cancel() {
        flag.store(true, Ordering::Relaxed);
    }
    session.write().cancel();
}

/// Resets the wizard to a fresh Source stage ("Import another").
fn bulk_restart(mut session: Signal<BulkImportSession>) {
    session.set(BulkImportSession::new());
}

/// Starts the invocation and spawns the driver loop, mirroring the export wizard's `onrun`. `target`
/// decides whether a success should [`NavState::mark_changed`] (only when it names the workspace
/// already open this session — a different or freshly created target is not what is displayed).
fn launch_bulk_import(
    run: BulkRun,
    mut session: Signal<BulkImportSession>,
    (mut cancel, asking): (Signal<Option<Arc<AtomicBool>>>, Asking),
) {
    let BulkRun {
        services,
        plugin_id,
        source,
        target,
        dataset,
        persons,
        unknown_failure,
    } = run;
    let refresh =
        matches!(&target, ImportTargetChoice::Existing { workspace } if *workspace == services.open_workspace)
            .then(try_consume_context::<NavState>)
            .flatten();
    let workspace = match &target {
        ImportTargetChoice::Existing { workspace } => workspace.clone(),
        ImportTargetChoice::New { name, .. } => name.clone(),
    };
    let drive = BulkDrive {
        chrome: services.chrome(),
        loc: services.localizer(),
        source_display: source.display().to_string(),
        workspace,
        persons,
        refresh,
        unknown_failure,
        asking,
    };
    session.write().start();
    let (handle, future) = start_bulk_import(services, plugin_id, source, target, dataset);
    cancel.set(Some(Arc::clone(&handle.cancel)));
    spawn(future);
    spawn(bulk_drive(handle, session, drive));
}

/// What the driver loop needs beyond the run's handle.
struct BulkDrive {
    chrome: Chrome,
    /// The data localizer a possible match's comparison is labelled with.
    loc: Localizer,
    source_display: String,
    /// The target workspace's name and person count, for the dataset question's dialog.
    workspace: String,
    persons: usize,
    /// The shell to tell about the new data, when the import went into the workspace on screen.
    refresh: Option<NavState>,
    unknown_failure: String,
    asking: Asking,
}

impl BulkDrive {
    /// Opens the dialog for the host's dataset question, starting on the proposed tree.
    fn ask(&self, (proposal, reply): DatasetQuestion) {
        let Asking {
            mut asked,
            reply: mut reply_slot,
            mut dataset,
            ..
        } = self.asking;
        let question = dataset_question(&self.chrome, &proposal);
        dataset.set(question.value.clone());
        reply_slot.set(Some(reply));
        asked.set(Some(AskedDataset {
            workspace: self.workspace.clone(),
            persons: self.persons,
            question,
        }));
    }

    /// Shows the host's Plan or Review question, keeping where its answer goes.
    fn review(&self, request: ReviewRequest, mut session: Signal<BulkImportSession>) {
        let mut review = self.asking.review;
        match request {
            ReviewRequest::Plan { summary, reply } => {
                session.write().on_plan(summary);
                review.set(Some(ReviewResponder::Plan(reply)));
            }
            ReviewRequest::Match { question, reply } => {
                session.write().on_match(&question, &self.loc);
                review.set(Some(ReviewResponder::Match(reply)));
            }
            ReviewRequest::Confirm { summary, reply } => {
                session.write().on_confirm(summary);
                review.set(Some(ReviewResponder::Confirm(reply)));
            }
        }
    }
}

/// The driver loop: pumps the host's progress reports into the session, opens the dataset question's
/// dialog if the host asks one (the run waits for its answer, so both are awaited together), then
/// records the outcome and — on a success into the workspace already open this session — requests
/// the app-state restart so its projections are not stale.
///
/// Nothing here checks for cancellation: [`BulkImportSession`] itself ignores everything after a
/// terminal stage, so a cancelled run's trailing reports and its eventual failure cannot overwrite the
/// operator's decision.
async fn bulk_drive(handle: BulkImportHandle, mut session: Signal<BulkImportSession>, drive: BulkDrive) {
    let BulkImportHandle {
        mut progress,
        mut question,
        mut reviews,
        outcome,
        cancel: _,
    } = handle;
    let mut asked = false;
    let mut reviewing = true;
    loop {
        tokio::select! {
            update = progress.recv() => {
                let Some(update) = update else {
                    break;
                };
                let step = match update.step {
                    ProgressStep::Plugin(step) => BulkImportStep::Plugin(step),
                    ProgressStep::Writing => BulkImportStep::Writing,
                };
                session.write().on_progress(BulkImportProgress {
                    step,
                    processed: update.processed,
                    total: update.total,
                });
            }
            received = &mut question, if !asked => {
                asked = true;
                if let Ok(received) = received {
                    drive.ask(received);
                }
            }
            request = reviews.recv(), if reviewing => {
                match request {
                    Some(request) => drive.review(request, session),
                    None => reviewing = false,
                }
            }
        }
    }
    // The run is over: a question still on screen can no longer be answered.
    drive.asking.answer(None);
    let mut review = drive.asking.review;
    review.set(None);
    let BulkDrive {
        source_display,
        refresh,
        unknown_failure,
        ..
    } = drive;
    match outcome.await {
        Ok(Ok(records)) => {
            session.write().on_success(BulkImportSummary {
                records,
                source: source_display,
            });
            if let Some(mut nav) = refresh {
                nav.mark_changed();
            }
        }
        Ok(Err(message)) => session.write().on_failure(message),
        Err(_) => session.write().on_failure(unknown_failure),
    }
}

/// The shared wizard labels from the chrome catalogue.
fn bulk_wizard_labels(chrome: &Chrome) -> BulkImportWizardLabels {
    BulkImportWizardLabels {
        heading: chrome.bulk_import_heading(),
        stages: chrome.bulk_import_stages(),
    }
}

/// The Source-stage labels from the chrome catalogue.
fn bulk_source_labels(chrome: &Chrome) -> BulkSourceLabels {
    BulkSourceLabels {
        heading: chrome.bulk_import_source_heading(),
        plugin: chrome.bulk_import_plugin_label(),
        no_plugins: chrome.bulk_import_no_plugins(),
        source: chrome.bulk_import_source_label(),
        source_placeholder: chrome.bulk_import_source_placeholder(),
        preview: chrome.bulk_import_source_preview(),
        directory_hint: chrome.bulk_import_source_directory_hint(),
        target_label: chrome.bulk_import_target_label(),
        target_existing: chrome.bulk_import_target_existing(),
        target_new: chrome.bulk_import_target_new(),
        workspace_label: chrome.bulk_import_target_workspace_label(),
        run: chrome.bulk_import_run(),
    }
}

/// The Running-stage labels from the chrome catalogue, with the counts filled in.
fn bulk_running_labels(chrome: &Chrome, progress: &BulkImportProgress) -> BulkRunningLabels {
    BulkRunningLabels {
        heading: chrome.bulk_import_running_heading(),
        starting: chrome.bulk_import_progress_starting(),
        writing: chrome.bulk_import_progress_writing(),
        count: chrome.bulk_import_progress_count(progress.processed, progress.total),
        cancel: chrome.bulk_import_cancel(),
    }
}

/// The Summary-stage labels from the chrome catalogue, with the count filled in.
fn bulk_summary_labels(chrome: &Chrome, records: u32) -> BulkSummaryLabels {
    BulkSummaryLabels {
        heading: chrome.bulk_import_summary_heading(),
        records: chrome.bulk_import_summary_records(records),
        source: chrome.bulk_import_summary_source(),
        another: chrome.bulk_import_another(),
    }
}
