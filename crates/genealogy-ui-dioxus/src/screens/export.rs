//! The bulk-export wizard (`Tool::Export`, ADR 0013; `export.html`): the GUI counterpart of
//! `genealogy export <plugin> [--output FILE]`.
//!
//! Three stages: Destination (pick an installed bulk-export plugin and a destination path) → Running
//! (the plugin's live progress, cancellable) → Summary. A failed run and a cancelled run are the two
//! other terminal stages.
//!
//! There is no native file dialog: the destination is a text field pre-filled with nothing and
//! previewed live against the workspace's `exports/` directory. What the operator typed is classified
//! by the framework-free [`ExportDestination`] — a bare directory (or an empty field) lets the plugin
//! name the file, a path with a file name pins it.
//!
//! Each stage is a pure component over already-localized label structs, so it renders in isolation
//! (the SSR tests do exactly that). [`ExportScreen`] owns the session, starts the invocation, and
//! pumps the host's progress into [`ExportSession`].
//!
//! Session state is screen-local: navigating away unmounts the screen and drops the handle. The run
//! itself is spawned on the renderer's executor and finishes on its own; only the reporting stops.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use genealogy_plugin_host::{ExportTarget, PluginRole};
use genealogy_ui::{ExportDestination, ExportProgress, ExportSession, ExportStage, ExportSummary};

use super::prelude::*;
use crate::i18n::Chrome;
use crate::services::{BulkExportHandle, PluginRow, discover_plugins, start_bulk_export};

/// The wizard chrome shared across stages: the heading and the three step names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportWizardLabels {
    /// The wizard heading.
    pub heading: String,
    /// The three stage names (Destination, Running, Summary).
    pub stages: [String; 3],
}

/// The Destination-stage labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDestinationLabels {
    /// The stage heading.
    pub heading: String,
    /// The plugin-selector label.
    pub plugin: String,
    /// The "no plugins installed" message.
    pub no_plugins: String,
    /// The destination field label.
    pub destination: String,
    /// The destination field placeholder.
    pub destination_placeholder: String,
    /// The live path-preview label.
    pub preview: String,
    /// The hint appended to a directory preview (the plugin names the file).
    pub dir_hint: String,
    /// The run action label.
    pub run: String,
}

/// The Running-stage labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRunningLabels {
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
pub struct ExportSummaryLabels {
    /// The stage heading.
    pub heading: String,
    /// The "{n} records written" text.
    pub records: String,
    /// The destination-row label.
    pub destination: String,
    /// The "Export again" action label.
    pub another: String,
}

/// Whether a terminal notice reports a failure or the operator's own cancellation. The two differ in
/// how assistive tech announces them: a failure interrupts, a cancellation the operator just asked
/// for does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardNoticeTone {
    /// The run failed.
    Failure,
    /// The operator cancelled the run.
    Cancelled,
}

/// The bulk-export wizard screen: owns the session, starts the invocation, and drives its progress.
#[component]
pub fn ExportScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let chrome = use_context::<ChromeCtx>().0;
    let session = use_signal(ExportSession::new);
    let cancel = use_signal(|| None::<Arc<AtomicBool>>);
    let plugin_id = use_signal(String::new);
    let typed = use_signal(String::new);
    let default_dir = state.services().dir.join("exports");

    let body = match session().stage().clone() {
        ExportStage::Destination => destination_body(&state, &chrome, plugin_id, typed, session, cancel, &default_dir),
        ExportStage::Running(progress) => rsx! {
            RunningStage {
                labels: running_labels(&chrome, &progress),
                progress,
                oncancel: move |()| request_cancel(session, cancel),
            }
        },
        ExportStage::Summary(summary) => rsx! {
            ExportSummaryStage {
                labels: summary_labels(&chrome, summary.records),
                destination: summary.destination,
                onrestart: move |()| restart(session),
            }
        },
        ExportStage::Error(message) => rsx! {
            NoticeStage {
                tone: WizardNoticeTone::Failure,
                heading: chrome.export_error_heading(),
                message,
                restart_label: chrome.export_another(),
                onrestart: move |()| restart(session),
            }
        },
        ExportStage::Cancelled => rsx! {
            NoticeStage {
                tone: WizardNoticeTone::Cancelled,
                heading: chrome.export_cancelled_heading(),
                message: chrome.export_cancelled_message(),
                restart_label: chrome.export_another(),
                onrestart: move |()| restart(session),
            }
        },
    };

    rsx! {
        div { style: "display:flex;flex-direction:column;gap:var(--sp-4);padding:var(--sp-4)",
            h1 { style: "border:0;margin:0;font-size:21px", "{chrome.export_heading()}" }
            {step_indicator(&wizard_labels(&chrome), session().stage())}
            {body}
        }
    }
}

/// Builds the Destination stage: discovers installed bulk-export plugins, then renders
/// [`DestinationStage`] and, on Export, starts the invocation and spawns the driver loop.
fn destination_body(
    state: &AppState,
    chrome: &Chrome,
    plugin_id: Signal<String>,
    typed: Signal<String>,
    session: Signal<ExportSession>,
    cancel: Signal<Option<Arc<AtomicBool>>>,
    default_dir: &Path,
) -> Element {
    let discover_services = state.services().clone();
    let plugins = use_resource(move || {
        let services = discover_services.clone();
        async move { discover_plugins(services).await.unwrap_or_default() }
    });
    let exporters: Vec<PluginRow> = plugins
        .read_unchecked()
        .as_ref()
        .map(|rows| {
            rows.iter()
                .filter(|row| row.role == PluginRole::BulkExport && row.enabled)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let mut plugin_id = plugin_id;
    if plugin_id().is_empty()
        && let Some(first) = exporters.first()
    {
        plugin_id.set(first.id.clone());
    }
    let options: Vec<SelectChoice> = exporters
        .iter()
        .map(|row| SelectChoice {
            value: row.id.clone(),
            label: row.id.clone(),
        })
        .collect();
    let run_services = state.services().clone();
    let unknown_failure = chrome.export_failed_unknown();
    let default_dir = default_dir.to_path_buf();
    let run_dir = default_dir.clone();
    let onrun = move |()| {
        let id = plugin_id();
        if id.is_empty() {
            return;
        }
        let (mut session, mut cancel) = (session, cancel);
        let destination = ExportDestination::parse(&typed(), &run_dir);
        let (handle, future) = start_bulk_export(run_services.clone(), id, export_target(&destination));
        cancel.set(Some(Arc::clone(&handle.cancel)));
        session.write().start();
        spawn(future);
        spawn(drive(handle, session, unknown_failure.clone()));
    };
    rsx! {
        DestinationStage {
            labels: destination_labels(chrome),
            options,
            plugin_id,
            typed,
            default_dir,
            onrun,
        }
    }
}

/// Stage 1 — the destination picker: an installed-bulk-export-plugin selector, a destination field
/// with a live path preview, and the Export action.
#[component]
pub fn DestinationStage(
    labels: ExportDestinationLabels,
    options: Vec<SelectChoice>,
    plugin_id: Signal<String>,
    typed: Signal<String>,
    default_dir: PathBuf,
    onrun: EventHandler<()>,
) -> Element {
    let mut plugin_id = plugin_id;
    let mut typed = typed;
    let no_plugins = options.is_empty();
    let destination = ExportDestination::parse(&typed(), &default_dir);
    rsx! {
        Card {
            h3 { "{labels.heading}" }
            div { class: "grid-2", style: "align-items:end",
                // `Select` renders its own `.field` and `<label for=…>`; wrapping it in a second one
                // would leave the field with two labels.
                if no_plugins {
                    div { class: "field", style: "margin:0",
                        span { class: "field-label", "{labels.plugin}" }
                        p { class: "muted", "{labels.no_plugins}" }
                    }
                } else {
                    Select {
                        label: labels.plugin.clone(),
                        name: "export-plugin".to_owned(),
                        value: Some(plugin_id()),
                        options,
                        onchange: move |event: FormEvent| plugin_id.set(event.value()),
                    }
                }
                div { class: "field", style: "margin:0",
                    label { r#for: "export-destination", "{labels.destination}" }
                    TextInput {
                        id: "export-destination",
                        name: "export-destination",
                        value: typed(),
                        placeholder: Some(labels.destination_placeholder.clone()),
                        oninput: move |event: FormEvent| typed.set(event.value()),
                    }
                }
            }
            div { class: "path-preview",
                span { class: "field-label", "{labels.preview}" }
                div { class: "picker-value",
                    span { class: "mono grow", style: "word-break:break-all", "{destination.path().display()}" }
                    if destination.is_directory() {
                        span { class: "muted", "{labels.dir_hint}" }
                    }
                }
            }
            div { style: "margin-top:var(--sp-3)",
                Button {
                    label: labels.run.clone(),
                    variant: ButtonVariant::Primary,
                    disabled: no_plugins,
                    onclick: move |_| onrun.call(()),
                }
            }
        }
    }
}

/// Stage 2 — the running export: the plugin's current step, a progress bar, and Cancel.
#[component]
pub fn RunningStage(labels: ExportRunningLabels, progress: ExportProgress, oncancel: EventHandler<()>) -> Element {
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

/// Stage 3 — the summary: how many records were written, where, and "Export again". Named for the
/// wizard because the assisted-import wizard already exports a `SummaryStage`.
#[component]
pub fn ExportSummaryStage(labels: ExportSummaryLabels, destination: String, onrestart: EventHandler<()>) -> Element {
    rsx! {
        Card {
            h3 { "{labels.heading}" }
            div { class: "wrap", style: "gap:var(--sp-4)",
                span { class: "badge", "{labels.records}" }
            }
            div { class: "path-preview", style: "margin-top:var(--sp-3)",
                span { class: "field-label", "{labels.destination}" }
                div { class: "picker-value",
                    span { class: "mono grow", style: "word-break:break-all", "{destination}" }
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

/// The failure / cancellation off-ramp: the message and a way back to the Destination stage.
#[component]
pub fn NoticeStage(
    tone: WizardNoticeTone,
    heading: String,
    message: String,
    restart_label: String,
    onrestart: EventHandler<()>,
) -> Element {
    let role = match tone {
        WizardNoticeTone::Failure => "alert",
        WizardNoticeTone::Cancelled => "status",
    };
    rsx! {
        Card {
            h3 { "{heading}" }
            p { class: "empty", role, "{message}" }
            div { style: "margin-top:var(--sp-3)",
                Button {
                    label: restart_label,
                    variant: ButtonVariant::Primary,
                    onclick: move |_| onrestart.call(()),
                }
            }
        }
    }
}

/// The wizard step indicator: the three stage names, the current one marked `aria-current`.
fn step_indicator(labels: &ExportWizardLabels, stage: &ExportStage) -> Element {
    let current = stage_index(stage);
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
fn stage_index(stage: &ExportStage) -> usize {
    match stage {
        ExportStage::Destination | ExportStage::Cancelled => 0,
        ExportStage::Running(_) => 1,
        ExportStage::Summary(_) | ExportStage::Error(_) => 2,
    }
}

/// The plugin-host target a parsed destination asks for.
fn export_target(destination: &ExportDestination) -> ExportTarget {
    match destination {
        ExportDestination::Directory(path) => ExportTarget::Directory(path.clone()),
        ExportDestination::File(path) => ExportTarget::File(path.clone()),
    }
}

/// The driver loop: pumps the host's progress reports into the session, then records the outcome.
/// Spawned on the renderer's local executor (the future is `!Send`).
///
/// Nothing here checks for cancellation: [`ExportSession`] itself ignores everything after a terminal
/// stage, so a cancelled run's trailing reports and its eventual failure cannot overwrite the
/// operator's decision.
async fn drive(handle: BulkExportHandle, mut session: Signal<ExportSession>, unknown_failure: String) {
    let mut progress = handle.progress;
    while let Some(update) = progress.recv().await {
        session.write().on_progress(ExportProgress {
            step: update.step,
            processed: update.processed,
            total: update.total,
        });
    }
    match handle.outcome.await {
        Ok(Ok((records, destination))) => session.write().on_success(ExportSummary {
            records,
            destination: destination.display().to_string(),
        }),
        Ok(Err(message)) => session.write().on_failure(message),
        Err(_) => session.write().on_failure(unknown_failure),
    }
}

/// Cancels the running export: raises the host-side flag (the run stops at the plugin's next progress
/// report) and moves the wizard to its Cancelled stage straight away.
fn request_cancel(mut session: Signal<ExportSession>, cancel: Signal<Option<Arc<AtomicBool>>>) {
    if let Some(flag) = cancel() {
        flag.store(true, Ordering::Relaxed);
    }
    session.write().cancel();
}

/// Resets the wizard to a fresh Destination stage ("Export again").
fn restart(mut session: Signal<ExportSession>) {
    session.set(ExportSession::new());
}

/// The shared wizard labels from the chrome catalogue.
fn wizard_labels(chrome: &Chrome) -> ExportWizardLabels {
    ExportWizardLabels {
        heading: chrome.export_heading(),
        stages: chrome.export_stages(),
    }
}

/// The Destination-stage labels from the chrome catalogue.
fn destination_labels(chrome: &Chrome) -> ExportDestinationLabels {
    ExportDestinationLabels {
        heading: chrome.export_destination_heading(),
        plugin: chrome.export_plugin_label(),
        no_plugins: chrome.export_no_plugins(),
        destination: chrome.export_destination_label(),
        destination_placeholder: chrome.export_destination_placeholder(),
        preview: chrome.export_destination_preview(),
        dir_hint: chrome.export_destination_dir_hint(),
        run: chrome.export_run(),
    }
}

/// The Running-stage labels from the chrome catalogue, with the counts filled in.
fn running_labels(chrome: &Chrome, progress: &ExportProgress) -> ExportRunningLabels {
    ExportRunningLabels {
        heading: chrome.export_running_heading(),
        starting: chrome.export_progress_starting(),
        count: chrome.export_progress_count(progress.processed, progress.total),
        cancel: chrome.export_cancel(),
    }
}

/// The Summary-stage labels from the chrome catalogue, with the count filled in.
fn summary_labels(chrome: &Chrome, records: u32) -> ExportSummaryLabels {
    ExportSummaryLabels {
        heading: chrome.export_summary_heading(),
        records: chrome.export_summary_records(records),
        destination: chrome.export_summary_destination(),
        another: chrome.export_another(),
    }
}
