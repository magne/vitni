//! The bulk-import wizard body (issue #191): the GUI counterpart of `genealogy import <plugin> <file>
//! (--new NAME PATH | --into NAME) [--yes]`, reached as a mode on `Tool::Import` (`screens/import.rs`)
//! rather than a separate `Tool` — mirrors the shipped bulk-export wizard (`screens/export.rs`).
//!
//! Three stages: Source (pick an installed bulk-import plugin, a source file, and a target workspace)
//! → Running (progress, cancellable) → Summary. A failed run and a cancelled run share the export
//! wizard's [`NoticeStage`]/[`WizardNoticeTone`].
//!
//! Unlike the export wizard the target may not be the workspace currently open: importing into an
//! *existing* non-empty workspace is confirmed first in a [`Modal`], mirroring the CLI's own confirm
//! (`main.rs:350-359`); a freshly registered workspace is always empty, so a `New` target never
//! prompts. After a successful import into the workspace already open this session, the app state is
//! restarted ([`request_restart`]) so the projections shown elsewhere are not stale.
//!
//! Each stage is a pure component over already-localized label structs, so it renders in isolation
//! (the SSR tests do exactly that). [`BulkImportBody`] owns the session, probes/confirms, starts the
//! invocation, and pumps the host's progress into [`BulkImportSession`].

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use genealogy_plugin_host::PluginRole;
use genealogy_ui::{
    BulkImportProgress, BulkImportSession, BulkImportStage, BulkImportSummary, ImportSourcePath, ImportTargetChoice,
    ImportTargetError,
};

use super::export::{NoticeStage, WizardNoticeTone};
use super::prelude::*;
use crate::app::request_restart;
use crate::components::Modal;
use crate::i18n::Chrome;
use crate::services::{
    BulkImportHandle, PluginRow, Services, count_workspace_persons, discover_plugins, start_bulk_import,
};

/// The wizard chrome shared across stages: the heading and the three step names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkImportWizardLabels {
    /// The wizard heading.
    pub heading: String,
    /// The three stage names (Source, Running, Summary).
    pub stages: [String; 3],
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
    let default_dir = state.services().dir.clone();

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
            &default_dir,
        ),
        BulkImportStage::Running(progress) => rsx! {
            BulkRunningStage {
                labels: bulk_running_labels(&chrome, &progress),
                progress,
                oncancel: move |()| bulk_request_cancel(session, cancel),
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
        default_dir.to_path_buf(),
        registered_names,
    );
    let new_workspace_fields = rsx! { {register_fields_form(chrome, register)} };
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
        {confirm_modal(chrome, pending, session, cancel)}
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
    let workspaces = genealogy_app::list_workspaces(&state.services().config);
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

/// Builds the Run handler: validates the current plugin/source/target, then either probes an existing
/// target's person count (arming the confirm [`Modal`] when non-empty) or launches the run directly
/// for a fresh `New` target, which is always empty.
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
                    match count_workspace_persons(&probe_services, &workspace).await {
                        Ok(0) => launch_bulk_import(
                            services,
                            id,
                            source_path,
                            ImportTargetChoice::Existing { workspace },
                            session,
                            cancel,
                            unknown_failure,
                        ),
                        Ok(count) => pending.set(Some(PendingRun {
                            services: probe_services,
                            plugin_id: id,
                            source: source_path,
                            workspace,
                            count,
                            unknown_failure,
                        })),
                        Err(message) => session.write().on_failure(message),
                    }
                });
            }
            ImportTargetChoice::New { .. } => {
                launch_bulk_import(services, id, source_path, target, session, cancel, unknown_failure);
            }
        }
    }
}

/// The non-empty-target confirm dialog: shown while [`PendingRun`] is armed, naming the target
/// workspace and its person count; Cancel drops the pending run, "Import anyway" launches it.
fn confirm_modal(
    chrome: &Chrome,
    mut pending: Signal<Option<PendingRun>>,
    session: Signal<BulkImportSession>,
    cancel: Signal<Option<Arc<AtomicBool>>>,
) -> Element {
    let Some(run) = pending() else {
        return rsx! {};
    };
    let labels = BulkConfirmLabels {
        title: chrome.bulk_import_confirm_title(&run.workspace),
        body: chrome.bulk_import_confirm_body(&run.workspace, run.count),
        cancel: chrome.bulk_import_confirm_cancel(),
        run: chrome.bulk_import_confirm_run(),
        dismiss: chrome.dismiss(),
    };
    rsx! {
        Modal {
            title: labels.title,
            open: true,
            close_label: labels.dismiss,
            onclose: move |()| pending.set(None),
            footer: rsx! {
                Button {
                    label: labels.cancel,
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| pending.set(None),
                }
                Button {
                    label: labels.run,
                    variant: ButtonVariant::Primary,
                    onclick: move |_| {
                        pending.set(None);
                        launch_bulk_import(
                            run.services.clone(),
                            run.plugin_id.clone(),
                            run.source.clone(),
                            ImportTargetChoice::Existing { workspace: run.workspace.clone() },
                            session,
                            cancel,
                            run.unknown_failure.clone(),
                        );
                    },
                }
            },
            p { "{labels.body}" }
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
    let step = if progress.step.trim().is_empty() {
        labels.starting.clone()
    } else {
        progress.step.clone()
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

/// The wizard step indicator, identical in shape to the export wizard's own (`.wiz-steps`).
fn bulk_step_indicator(labels: &BulkImportWizardLabels, stage: &BulkImportStage) -> Element {
    let current = bulk_stage_index(stage);
    rsx! {
        div { class: "wiz-steps", role: "list", aria_label: "{labels.heading}",
            for (index , name) in labels.stages.iter().enumerate() {
                span {
                    class: if index == current { "wiz-step active" } else if index < current { "wiz-step done" } else { "wiz-step" },
                    role: "listitem",
                    aria_current: if index == current { Some("step") } else { None },
                    span { class: "num", "{index + 1}" }
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
        BulkImportStage::Summary(_) | BulkImportStage::Error(_) => 2,
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
/// decides whether a success should [`request_restart`] (only when it names the workspace already
/// open this session — a different or freshly created target is not what is currently displayed).
fn launch_bulk_import(
    services: Services,
    plugin_id: String,
    source: PathBuf,
    target: ImportTargetChoice,
    mut session: Signal<BulkImportSession>,
    mut cancel: Signal<Option<Arc<AtomicBool>>>,
    unknown_failure: String,
) {
    let refresh =
        matches!(&target, ImportTargetChoice::Existing { workspace } if *workspace == services.open_workspace);
    let source_display = source.display().to_string();
    session.write().start();
    let (handle, future) = start_bulk_import(services, plugin_id, source, target);
    cancel.set(Some(Arc::clone(&handle.cancel)));
    spawn(future);
    spawn(bulk_drive(handle, session, source_display, refresh, unknown_failure));
}

/// The driver loop: pumps the host's progress reports into the session, then records the outcome and
/// — on a success into the workspace already open this session — requests the app-state restart so
/// its projections are not stale.
///
/// Nothing here checks for cancellation: [`BulkImportSession`] itself ignores everything after a
/// terminal stage, so a cancelled run's trailing reports and its eventual failure cannot overwrite the
/// operator's decision.
async fn bulk_drive(
    handle: BulkImportHandle,
    mut session: Signal<BulkImportSession>,
    source_display: String,
    refresh: bool,
    unknown_failure: String,
) {
    let mut progress = handle.progress;
    while let Some(update) = progress.recv().await {
        session.write().on_progress(BulkImportProgress {
            step: update.step,
            processed: update.processed,
            total: update.total,
        });
    }
    match handle.outcome.await {
        Ok(Ok(records)) => {
            session.write().on_success(BulkImportSummary {
                records,
                source: source_display,
            });
            if refresh {
                request_restart();
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
