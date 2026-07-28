//! SSR assertions for the bulk-export wizard (ADR 0013; `export.html`): each stage rendered in
//! isolation over the framework-free `genealogy-ui` types and already-localized label structs.
//! The export itself is covered by the plugin-host round-trip tests; these prove the wizard renders
//! the destination preview, the live progress, and each terminal outcome.

use std::path::PathBuf;

use dioxus::prelude::*;
use genealogy_ui::ExportProgress;
use genealogy_ui_dioxus::components::SelectChoice;
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::screens::{
    DestinationStage, ExportDestinationLabels, ExportRunningLabels, ExportSummaryLabels, ExportSummaryStage,
    NoticeStage, RunningStage, WizardNoticeTone,
};

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

// ----- Destination stage -----

fn destination_labels() -> ExportDestinationLabels {
    ExportDestinationLabels {
        heading: "Choose what to write, and where".to_owned(),
        plugin: "Export format".to_owned(),
        no_plugins: "No bulk-export plugins are installed.".to_owned(),
        destination: "Destination".to_owned(),
        destination_placeholder: "Leave empty for the exports folder".to_owned(),
        preview: "Writes to".to_owned(),
        dir_hint: "under the plugin's file name".to_owned(),
        run: "Export".to_owned(),
    }
}

fn export_options() -> Vec<SelectChoice> {
    vec![SelectChoice {
        value: "gedcom-export".to_owned(),
        label: "gedcom-export".to_owned(),
    }]
}

fn destination_view() -> Element {
    let plugin_id = use_signal(|| "gedcom-export".to_owned());
    let typed = use_signal(String::new);
    rsx! {
        DestinationStage {
            labels: destination_labels(),
            options: export_options(),
            plugin_id,
            typed,
            default_dir: PathBuf::from("/ws/exports"),
            onrun: |()| {},
        }
    }
}

fn destination_with_file_view() -> Element {
    let plugin_id = use_signal(|| "gedcom-export".to_owned());
    let typed = use_signal(|| "2026/family.ged".to_owned());
    rsx! {
        DestinationStage {
            labels: destination_labels(),
            options: export_options(),
            plugin_id,
            typed,
            default_dir: PathBuf::from("/ws/exports"),
            onrun: |()| {},
        }
    }
}

fn destination_without_plugins_view() -> Element {
    let plugin_id = use_signal(String::new);
    let typed = use_signal(String::new);
    rsx! {
        DestinationStage {
            labels: destination_labels(),
            options: Vec::new(),
            plugin_id,
            typed,
            default_dir: PathBuf::from("/ws/exports"),
            onrun: |()| {},
        }
    }
}

#[test]
fn destination_stage_lists_the_export_plugins_and_previews_the_default_directory() {
    let html = render(destination_view);
    assert!(html.contains("gedcom-export"), "plugin option: {html}");
    assert!(html.contains("id=\"export-destination\""), "destination field: {html}");
    // An empty field means the workspace exports directory, with the plugin naming the file.
    assert!(html.contains("/ws/exports"), "default destination preview: {html}");
    assert!(html.contains("under the plugin"), "directory hint: {html}");
    assert!(html.contains("Export"), "run action: {html}");
}

#[test]
fn destination_stage_previews_a_typed_file_path_without_the_directory_hint() {
    let html = render(destination_with_file_view);
    assert!(
        html.contains("/ws/exports/2026/family.ged"),
        "typed path resolved against the default directory: {html}"
    );
    assert!(
        !html.contains("under the plugin"),
        "a pinned file name leaves the plugin no say: {html}"
    );
}

#[test]
fn destination_stage_shows_the_empty_state_when_no_export_plugins_are_installed() {
    let html = render(destination_without_plugins_view);
    assert!(
        html.contains("No bulk-export plugins are installed."),
        "empty state: {html}"
    );
    assert!(!html.contains("<select"), "no plugin selector to offer: {html}");
}

// ----- Running stage -----

fn running_labels() -> ExportRunningLabels {
    ExportRunningLabels {
        heading: "Exporting…".to_owned(),
        starting: "Starting…".to_owned(),
        count: "40 of 120".to_owned(),
        cancel: "Cancel".to_owned(),
    }
}

fn running_view() -> Element {
    rsx! {
        RunningStage {
            labels: running_labels(),
            progress: ExportProgress {
                step: "persons".to_owned(),
                processed: 40,
                total: Some(120),
            },
            oncancel: |()| {},
        }
    }
}

fn running_without_total_view() -> Element {
    rsx! {
        RunningStage {
            labels: ExportRunningLabels {
                count: "40 written".to_owned(),
                ..running_labels()
            },
            progress: ExportProgress {
                step: String::new(),
                processed: 40,
                total: None,
            },
            oncancel: |()| {},
        }
    }
}

#[test]
fn running_stage_exposes_the_progress_bar_and_cancel() {
    let html = render(running_view);
    assert!(html.contains("persons"), "the plugin's current step: {html}");
    assert!(html.contains("40 of 120"), "progress count: {html}");
    assert!(html.contains("role=\"progressbar\""), "progressbar role: {html}");
    assert!(html.contains("aria-valuenow=\"40\""), "current value: {html}");
    assert!(html.contains("aria-valuemax=\"120\""), "expected total: {html}");
    assert!(html.contains("Cancel"), "cancel action: {html}");
}

#[test]
fn running_stage_is_indeterminate_until_the_plugin_knows_the_total() {
    let html = render(running_without_total_view);
    assert!(html.contains("indeterminate"), "indeterminate bar: {html}");
    assert!(!html.contains("aria-valuemax"), "no total to announce yet: {html}");
    // With no step reported yet the wizard shows its own "Starting…" placeholder.
    assert!(html.contains("Starting…"), "placeholder step: {html}");
}

// ----- Summary stage -----

fn summary_view() -> Element {
    rsx! {
        ExportSummaryStage {
            labels: ExportSummaryLabels {
                heading: "Export complete".to_owned(),
                records: "120 records written".to_owned(),
                destination: "Written to".to_owned(),
                another: "Export again".to_owned(),
            },
            destination: "/ws/exports/family.ged".to_owned(),
            onrestart: |()| {},
        }
    }
}

#[test]
fn summary_stage_shows_the_count_and_the_destination() {
    let html = render(summary_view);
    assert!(html.contains("120 records written"), "record count: {html}");
    assert!(html.contains("/ws/exports/family.ged"), "resolved destination: {html}");
    assert!(html.contains("Export again"), "restart action: {html}");
}

// ----- Terminal notices -----

fn failure_view() -> Element {
    rsx! {
        NoticeStage {
            tone: WizardNoticeTone::Failure,
            heading: "Export failed".to_owned(),
            message: "plugin trapped: out of fuel".to_owned(),
            restart_label: "Export again".to_owned(),
            onrestart: |()| {},
        }
    }
}

fn cancelled_view() -> Element {
    rsx! {
        NoticeStage {
            tone: WizardNoticeTone::Cancelled,
            heading: "Export cancelled".to_owned(),
            message: "The export stopped before it finished.".to_owned(),
            restart_label: "Export again".to_owned(),
            onrestart: |()| {},
        }
    }
}

#[test]
fn a_failed_export_is_announced_as_an_alert() {
    let html = render(failure_view);
    assert!(html.contains("plugin trapped: out of fuel"), "failure message: {html}");
    assert!(html.contains("role=\"alert\""), "failure is announced: {html}");
}

#[test]
fn a_cancelled_export_is_a_status_not_an_alert() {
    // The operator asked for it, so it must not interrupt assistive tech the way a failure does.
    let html = render(cancelled_view);
    assert!(
        html.contains("The export stopped before it finished."),
        "notice: {html}"
    );
    assert!(html.contains("role=\"status\""), "cancellation is a status: {html}");
    assert!(
        !html.contains("role=\"alert\""),
        "a cancellation is not an alert: {html}"
    );
}

// ----- Localization -----

fn norwegian_destination_view() -> Element {
    let chrome = Chrome::with_languages(None, &["no".parse().unwrap_or_default()]);
    let plugin_id = use_signal(|| "gedcom-export".to_owned());
    let typed = use_signal(String::new);
    rsx! {
        DestinationStage {
            labels: ExportDestinationLabels {
                heading: chrome.export_destination_heading(),
                plugin: chrome.export_plugin_label(),
                no_plugins: chrome.export_no_plugins(),
                destination: chrome.export_destination_label(),
                destination_placeholder: chrome.export_destination_placeholder(),
                preview: chrome.export_destination_preview(),
                dir_hint: chrome.export_destination_dir_hint(),
                run: chrome.export_run(),
            },
            options: export_options(),
            plugin_id,
            typed,
            default_dir: PathBuf::from("/ws/exports"),
            onrun: |()| {},
        }
    }
}

#[test]
fn the_wizard_localizes_into_norwegian() {
    let html = render(norwegian_destination_view);
    assert!(html.contains("Eksportformat"), "plugin label in Norwegian: {html}");
    assert!(html.contains("Eksporter"), "run action in Norwegian: {html}");
}
