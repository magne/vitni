//! SSR assertions for the bulk-import wizard (issue #191; `import.html`): each stage rendered in
//! isolation over the framework-free `vitni-ui` types and already-localized label structs, mirroring
//! `tests/export_gui.rs`. The import itself is covered by the plugin-host round-trip tests; these prove
//! the mode switch, the target sub-form, the non-empty confirm, the live progress, and each terminal
//! outcome render correctly.

use std::path::PathBuf;

use dioxus::prelude::*;
use vitni_ui::BulkImportProgress;
use vitni_ui_dioxus::components::{Button, ButtonVariant, Modal, SelectChoice};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::screens::{
    BulkRunningLabels, BulkRunningStage, BulkSourceLabels, BulkSourceStage, BulkSummaryLabels, BulkSummaryStage,
    ImportModeLabels, ImportModeSwitch, NoticeStage, RegisterFields, WizardNoticeTone, register_fields_form,
};

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

// ----- Mode switch -----

fn mode_switch_view() -> Element {
    let mode = use_signal(|| "bulk".to_owned());
    rsx! {
        ImportModeSwitch {
            labels: ImportModeLabels {
                group_label: "How do you want to import?".to_owned(),
                bulk: "Bulk file import".to_owned(),
                assisted: "Assisted online import".to_owned(),
            },
            mode,
        }
    }
}

#[test]
fn the_mode_switch_offers_both_bulk_and_assisted_import() {
    let html = render(mode_switch_view);
    assert!(html.contains("role=\"radiogroup\""), "a true radio group: {html}");
    assert!(html.contains("Bulk file import"), "the bulk choice: {html}");
    assert!(html.contains("Assisted online import"), "the assisted choice: {html}");
}

// ----- Source stage -----

fn register_fields() -> RegisterFields {
    RegisterFields {
        open: use_signal(|| true),
        name: use_signal(String::new),
        directory: use_signal(String::new),
        database_url: use_signal(String::new),
    }
}

fn source_labels() -> BulkSourceLabels {
    BulkSourceLabels {
        heading: "Choose what to read, and where to import it".to_owned(),
        plugin: "Import format".to_owned(),
        no_plugins: "No bulk-import plugins are installed.".to_owned(),
        source: "Source file".to_owned(),
        source_placeholder: "Path to the file to import".to_owned(),
        preview: "Reads from".to_owned(),
        directory_hint: "That path names a directory".to_owned(),
        target_label: "Import into".to_owned(),
        target_existing: "An existing workspace".to_owned(),
        target_new: "A new workspace".to_owned(),
        workspace_label: "Workspace".to_owned(),
        run: "Import".to_owned(),
    }
}

fn import_options() -> Vec<SelectChoice> {
    vec![SelectChoice {
        value: "gedcom-import".to_owned(),
        label: "gedcom-import".to_owned(),
    }]
}

fn workspace_options() -> Vec<SelectChoice> {
    vec![SelectChoice {
        value: "family".to_owned(),
        label: "family".to_owned(),
    }]
}

fn source_existing_view() -> Element {
    let plugin_id = use_signal(|| "gedcom-import".to_owned());
    let source_typed = use_signal(|| "family.ged".to_owned());
    let target_mode = use_signal(|| "existing".to_owned());
    let target_workspace = use_signal(|| "family".to_owned());
    rsx! {
        BulkSourceStage {
            labels: source_labels(),
            plugin_options: import_options(),
            plugin_id,
            source_typed,
            default_dir: PathBuf::from("/ws"),
            target_mode,
            target_workspace,
            workspace_options: workspace_options(),
            new_workspace_fields: rsx! {},
            target_error: None,
            onrun: |()| {},
        }
    }
}

fn source_new_view() -> Element {
    let chrome = Chrome::with_languages(None, &["en".parse().unwrap_or_default()]);
    let plugin_id = use_signal(|| "gedcom-import".to_owned());
    let source_typed = use_signal(|| "family.ged".to_owned());
    let target_mode = use_signal(|| "new".to_owned());
    let target_workspace = use_signal(String::new);
    let new_workspace_fields = rsx! { {register_fields_form(&chrome, register_fields())} };
    rsx! {
        BulkSourceStage {
            labels: source_labels(),
            plugin_options: import_options(),
            plugin_id,
            source_typed,
            default_dir: PathBuf::from("/ws"),
            target_mode,
            target_workspace,
            workspace_options: workspace_options(),
            new_workspace_fields,
            target_error: None,
            onrun: |()| {},
        }
    }
}

fn source_no_plugins_view() -> Element {
    let plugin_id = use_signal(String::new);
    let source_typed = use_signal(String::new);
    let target_mode = use_signal(|| "existing".to_owned());
    let target_workspace = use_signal(String::new);
    rsx! {
        BulkSourceStage {
            labels: source_labels(),
            plugin_options: Vec::new(),
            plugin_id,
            source_typed,
            default_dir: PathBuf::from("/ws"),
            target_mode,
            target_workspace,
            workspace_options: Vec::new(),
            new_workspace_fields: rsx! {},
            target_error: None,
            onrun: |()| {},
        }
    }
}

fn source_directory_path_view() -> Element {
    let plugin_id = use_signal(|| "gedcom-import".to_owned());
    let source_typed = use_signal(|| "imports/".to_owned());
    let target_mode = use_signal(|| "existing".to_owned());
    let target_workspace = use_signal(|| "family".to_owned());
    rsx! {
        BulkSourceStage {
            labels: source_labels(),
            plugin_options: import_options(),
            plugin_id,
            source_typed,
            default_dir: PathBuf::from("/ws"),
            target_mode,
            target_workspace,
            workspace_options: workspace_options(),
            new_workspace_fields: rsx! {},
            target_error: None,
            onrun: |()| {},
        }
    }
}

#[test]
fn source_stage_lists_the_import_plugins_and_previews_the_typed_source() {
    let html = render(source_existing_view);
    assert!(html.contains("gedcom-import"), "plugin option: {html}");
    assert!(html.contains("id=\"bulk-import-source\""), "source field: {html}");
    assert!(html.contains("/ws/family.ged"), "resolved source preview: {html}");
    assert!(!html.contains("disabled"), "a usable file enables Run: {html}");
}

#[test]
fn source_stage_shows_the_empty_state_when_no_import_plugins_are_installed() {
    let html = render(source_no_plugins_view);
    assert!(
        html.contains("No bulk-import plugins are installed."),
        "empty state: {html}"
    );
    assert!(!html.contains("<select"), "no plugin selector to offer: {html}");
    assert!(html.contains("disabled"), "Run is blocked with no plugins: {html}");
}

#[test]
fn source_stage_shows_the_directory_hint_and_disables_run_for_a_directory_path() {
    let html = render(source_directory_path_view);
    assert!(html.contains("/ws/imports"), "resolved directory preview: {html}");
    assert!(html.contains("That path names a directory"), "directory hint: {html}");
    assert!(html.contains("disabled"), "Run is blocked on a directory: {html}");
}

#[test]
fn the_target_radio_switches_between_the_workspace_select_and_the_new_workspace_fields() {
    let existing_html = render(source_existing_view);
    assert!(
        existing_html.contains("family"),
        "the existing-workspace select: {existing_html}"
    );
    assert!(
        !existing_html.contains("id=\"register-name\""),
        "no new-workspace fields in existing mode: {existing_html}"
    );

    let new_html = render(source_new_view);
    assert!(
        new_html.contains("id=\"register-name\""),
        "the shared register-name field renders when New is picked: {new_html}"
    );
    assert!(
        new_html.contains("id=\"register-directory\""),
        "the shared register-directory field renders when New is picked: {new_html}"
    );
}

// ----- Running stage -----

fn running_labels() -> BulkRunningLabels {
    BulkRunningLabels {
        heading: "Importing…".to_owned(),
        starting: "Starting…".to_owned(),
        count: "10 of 40".to_owned(),
        cancel: "Cancel".to_owned(),
    }
}

fn running_view() -> Element {
    rsx! {
        BulkRunningStage {
            labels: running_labels(),
            progress: BulkImportProgress {
                step: "persons".to_owned(),
                processed: 10,
                total: Some(40),
            },
            oncancel: |()| {},
        }
    }
}

fn running_without_total_view() -> Element {
    rsx! {
        BulkRunningStage {
            labels: BulkRunningLabels {
                count: "10 imported".to_owned(),
                ..running_labels()
            },
            progress: BulkImportProgress {
                step: String::new(),
                processed: 10,
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
    assert!(html.contains("10 of 40"), "progress count: {html}");
    assert!(html.contains("role=\"progressbar\""), "progressbar role: {html}");
    assert!(html.contains("aria-valuenow=\"10\""), "current value: {html}");
    assert!(html.contains("aria-valuemax=\"40\""), "expected total: {html}");
    assert!(html.contains("Cancel"), "cancel action: {html}");
}

#[test]
fn running_stage_is_indeterminate_until_the_plugin_knows_the_total() {
    let html = render(running_without_total_view);
    assert!(html.contains("indeterminate"), "indeterminate bar: {html}");
    assert!(!html.contains("aria-valuemax"), "no total to announce yet: {html}");
    assert!(html.contains("Starting…"), "placeholder step: {html}");
}

// ----- Summary stage -----

fn summary_view() -> Element {
    rsx! {
        BulkSummaryStage {
            labels: BulkSummaryLabels {
                heading: "Import complete".to_owned(),
                records: "40 records imported".to_owned(),
                source: "Imported from".to_owned(),
                another: "Import another".to_owned(),
            },
            source: "/ws/imports/family.ged".to_owned(),
            onrestart: |()| {},
        }
    }
}

#[test]
fn summary_stage_shows_the_count_and_the_source() {
    let html = render(summary_view);
    assert!(html.contains("40 records imported"), "record count: {html}");
    assert!(html.contains("/ws/imports/family.ged"), "the source path: {html}");
    assert!(html.contains("Import another"), "restart action: {html}");
}

// ----- Terminal notices (shared with the export wizard) -----

fn failure_view() -> Element {
    rsx! {
        NoticeStage {
            tone: WizardNoticeTone::Failure,
            heading: "Import failed".to_owned(),
            message: "plugin trapped: out of fuel".to_owned(),
            restart_label: "Import another".to_owned(),
            onrestart: |()| {},
        }
    }
}

fn cancelled_view() -> Element {
    rsx! {
        NoticeStage {
            tone: WizardNoticeTone::Cancelled,
            heading: "Import cancelled".to_owned(),
            message: "The import stopped before it finished.".to_owned(),
            restart_label: "Import another".to_owned(),
            onrestart: |()| {},
        }
    }
}

#[test]
fn a_failed_bulk_import_is_announced_as_an_alert() {
    let html = render(failure_view);
    assert!(html.contains("plugin trapped: out of fuel"), "failure message: {html}");
    assert!(html.contains("role=\"alert\""), "failure is announced: {html}");
}

#[test]
fn a_cancelled_bulk_import_is_a_status_not_an_alert() {
    let html = render(cancelled_view);
    assert!(
        html.contains("The import stopped before it finished."),
        "notice: {html}"
    );
    assert!(html.contains("role=\"status\""), "cancellation is a status: {html}");
    assert!(
        !html.contains("role=\"alert\""),
        "a cancellation is not an alert: {html}"
    );
}

// ----- Non-empty-target confirm -----

fn confirm_modal_view() -> Element {
    let chrome = Chrome::with_languages(None, &["en".parse().unwrap_or_default()]);
    rsx! {
        Modal {
            title: chrome.bulk_import_confirm_title("family"),
            open: true,
            close_label: chrome.dismiss(),
            onclose: |()| {},
            footer: rsx! {
                Button { label: chrome.bulk_import_confirm_cancel(), variant: ButtonVariant::Ghost, onclick: |_| {} }
                Button { label: chrome.bulk_import_confirm_run(), variant: ButtonVariant::Primary, onclick: |_| {} }
            },
            p { "{chrome.bulk_import_confirm_body(\"family\", 3)}" }
        }
    }
}

#[test]
fn the_non_empty_target_confirm_names_the_workspace_and_the_person_count() {
    let html = render(confirm_modal_view);
    assert!(html.contains("family"), "names the target workspace: {html}");
    assert!(html.contains('3'), "names the person count: {html}");
    assert!(html.contains("Import anyway"), "the confirm action: {html}");
    assert!(html.contains("role=\"dialog\""), "a real dialog: {html}");
}

// ----- Localization -----

fn norwegian_source_view() -> Element {
    let chrome = Chrome::with_languages(None, &["no".parse().unwrap_or_default()]);
    let plugin_id = use_signal(|| "gedcom-import".to_owned());
    let source_typed = use_signal(String::new);
    let target_mode = use_signal(|| "existing".to_owned());
    let target_workspace = use_signal(String::new);
    rsx! {
        BulkSourceStage {
            labels: BulkSourceLabels {
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
            },
            plugin_options: import_options(),
            plugin_id,
            source_typed,
            default_dir: PathBuf::from("/ws"),
            target_mode,
            target_workspace,
            workspace_options: Vec::new(),
            new_workspace_fields: rsx! {},
            target_error: None,
            onrun: |()| {},
        }
    }
}

#[test]
fn the_bulk_wizard_localizes_into_norwegian() {
    let html = render(norwegian_source_view);
    assert!(html.contains("Importformat"), "plugin label in Norwegian: {html}");
    assert!(html.contains("Importer"), "run action in Norwegian: {html}");
}
