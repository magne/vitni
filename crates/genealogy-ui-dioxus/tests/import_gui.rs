//! SSR assertions for the assisted-import wizard (ADR 0017 §5; `import.html`): each stage rendered in
//! isolation over the framework-free `genealogy-ui` payload types and already-localized label structs.
//! The plugin flow itself is covered by the plugin-host e2e; these prove the wizard renders each
//! `present` payload — the records picker, the confirm split view (fields, scan, provenance,
//! confidence), the save dialog, and the summary — the way the state machine advances them.

use dioxus::prelude::*;
use genealogy_ui::{
    ConfidenceLevel, ConfirmRecord, ConfirmRecordPayload, CropRegion, ImportedRecord, Localizer, PayloadAction,
    PayloadConfidence, PayloadField, ProvenancePreview, RecordRow, RecordsPayload, SaveScanPayload, SaveSuggestion,
    ScanRef, SourceRef,
};
use genealogy_ui_dioxus::components::{MediaSaveLabels, SelectChoice};
use genealogy_ui_dioxus::screens::{
    ConfirmChrome, ConfirmStage, ImportRowStatus, RecordsLabels, RecordsStage, SaveStage, SourceLabels, SourceStage,
    SummaryLabels, SummaryStage, media_viewer_labels,
};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

// ----- Source stage -----

fn source_view() -> Element {
    let plugin_id = use_signal(|| "digitalarkivet-import".to_owned());
    rsx! {
        SourceStage {
            labels: SourceLabels {
                source: "Source".to_owned(),
                url: "Record URL".to_owned(),
                url_placeholder: "https://…".to_owned(),
                fetch: "Fetch".to_owned(),
                no_plugins: "No plugins".to_owned(),
                running: "Importing…".to_owned(),
            },
            options: vec![SelectChoice {
                value: "digitalarkivet-import".to_owned(),
                label: "digitalarkivet-import".to_owned(),
            }],
            plugin_id,
            running: false,
            onfetch: |_: String| {},
        }
    }
}

#[test]
fn source_stage_renders_the_plugin_selector_url_field_and_fetch() {
    let html = render(source_view);
    assert!(html.contains("digitalarkivet-import"), "plugin option: {html}");
    assert!(html.contains("Record URL"), "url label: {html}");
    assert!(html.contains("id=\"import-url\""), "url field: {html}");
    assert!(html.contains("Fetch"), "fetch button: {html}");
}

// ----- Records stage -----

fn records_payload() -> RecordsPayload {
    RecordsPayload {
        source: SourceRef {
            title: "Folketelling 1920 for Greipstad".to_owned(),
            url: "https://www.digitalarkivet.no/census/rural-residence/bf01".to_owned(),
        },
        records: vec![
            RecordRow {
                id: "pf01".to_owned(),
                label: "Ole Andersen".to_owned(),
                detail: Some("1874 · hovedperson".to_owned()),
            },
            RecordRow {
                id: "pf02".to_owned(),
                label: "Anna Tobiasdatter".to_owned(),
                detail: None,
            },
        ],
    }
}

fn records_view() -> Element {
    let mut statuses = std::collections::HashMap::new();
    statuses.insert("pf01".to_owned(), ImportRowStatus::Imported);
    rsx! {
        RecordsStage {
            labels: RecordsLabels {
                heading: "Records found".to_owned(),
                headers: vec!["Name".to_owned(), "Details".to_owned(), "Status".to_owned(), String::new()],
                review: "Review".to_owned(),
                finish: "Finish".to_owned(),
                status: ["Pending".to_owned(), "Imported".to_owned(), "Skipped".to_owned()],
            },
            payload: records_payload(),
            statuses,
            onrespond: |_| {},
        }
    }
}

#[test]
fn records_stage_renders_rows_with_status_chips_and_actions() {
    let html = render(records_view);
    assert!(
        html.contains("Ole Andersen") && html.contains("Anna Tobiasdatter"),
        "record names: {html}"
    );
    assert!(html.contains("1874 · hovedperson"), "record detail: {html}");
    // The imported row shows the Imported chip; the unreviewed row shows Pending.
    assert!(html.contains("Imported"), "imported chip: {html}");
    assert!(html.contains("Pending"), "pending chip: {html}");
    assert!(html.contains("Review"), "per-row review action: {html}");
    assert!(html.contains("Finish"), "finish action: {html}");
}

// ----- Confirm stage -----

fn confirm_payload() -> ConfirmRecordPayload {
    ConfirmRecordPayload {
        record: ConfirmRecord {
            fields: vec![
                PayloadField {
                    key: "name".to_owned(),
                    label: "Name".to_owned(),
                    value: "Asbjørn Olsen".to_owned(),
                },
                PayloadField {
                    key: "occupation".to_owned(),
                    label: "Occupation".to_owned(),
                    value: "gaardsarbeide".to_owned(),
                },
            ],
            scan: Some(ScanRef {
                path: Some("https://urn.digitalarkivet.no/scan.jpg".to_owned()),
                region: Some(CropRegion {
                    left: 4,
                    top: 47,
                    width: 92,
                    height: 9,
                }),
            }),
            provenance: ProvenancePreview {
                source_title: "1920 folketelling for Greipstad".to_owned(),
                repository: "Digitalarkivet (Arkivverket)".to_owned(),
                citation: "URN:NBN:no-a1450-fs10771822220997".to_owned(),
                external_id_url: "https://www.digitalarkivet.no/census/person/pf01".to_owned(),
                confidence: PayloadConfidence::Low,
            },
        },
        actions: vec![
            PayloadAction {
                id: "import".to_owned(),
                label: "Import & next".to_owned(),
            },
            PayloadAction {
                id: "skip".to_owned(),
                label: "Skip".to_owned(),
            },
        ],
    }
}

fn confidence_labels() -> Vec<(ConfidenceLevel, String)> {
    vec![
        (ConfidenceLevel::VeryLow, "Very low".to_owned()),
        (ConfidenceLevel::Low, "Low".to_owned()),
        (ConfidenceLevel::Normal, "Normal".to_owned()),
        (ConfidenceLevel::High, "High".to_owned()),
        (ConfidenceLevel::VeryHigh, "Very high".to_owned()),
    ]
}

fn confirm_view() -> Element {
    rsx! {
        ConfirmStage {
            viewer_labels: media_viewer_labels(&loc()),
            chrome: ConfirmChrome {
                heading: "Confirm record".to_owned(),
                provenance_heading: "What will be recorded".to_owned(),
                prov: [
                    "Operator".to_owned(),
                    "Source".to_owned(),
                    "Repository".to_owned(),
                    "Citation".to_owned(),
                    "External id".to_owned(),
                    "Confidence".to_owned(),
                ],
                software_agent: "software agent".to_owned(),
            },
            confidence_labels: confidence_labels(),
            payload: confirm_payload(),
            onrespond: |_| {},
        }
    }
}

#[test]
fn confirm_stage_renders_fields_scan_provenance_and_confidence() {
    let html = render(confirm_view);
    // Editable transcribed fields (labels resolved by the plugin; values are record content).
    assert!(
        html.contains("Name") && html.contains("Asbjørn Olsen"),
        "name field: {html}"
    );
    assert!(
        html.contains("Occupation") && html.contains("gaardsarbeide"),
        "occupation field: {html}"
    );
    // The scan is shown via the media viewer (a remote URL served verbatim) with the suggested crop.
    assert!(html.contains("urn.digitalarkivet.no/scan.jpg"), "scan image: {html}");
    assert!(
        html.contains("left:4%;top:47%;width:92%;height:9%"),
        "suggested crop region: {html}"
    );
    // The provenance preview and its confidence select.
    assert!(html.contains("What will be recorded"), "provenance heading: {html}");
    assert!(html.contains("Digitalarkivet (Arkivverket)"), "repository: {html}");
    assert!(
        html.contains("URN:NBN:no-a1450-fs10771822220997"),
        "citation locator: {html}"
    );
    assert!(html.contains("name=\"import-confidence\""), "confidence select: {html}");
    // The action labels come from the plugin's (resolved) payload (`&` is HTML-escaped in SSR).
    assert!(
        html.contains("Import") && html.contains("next"),
        "import action: {html}"
    );
    assert!(html.contains("Skip"), "skip action: {html}");
}

// ----- Save-scan stage -----

fn save_view() -> Element {
    rsx! {
        SaveStage {
            labels: MediaSaveLabels {
                title: "Save scan to media library".to_owned(),
                choose_category: "Choose a category".to_owned(),
                category: "Category".to_owned(),
                subfolder: "Subfolder".to_owned(),
                filename: "Filename".to_owned(),
                path_preview: "Path preview".to_owned(),
                save: "Save scan".to_owned(),
                cancel: "Cancel".to_owned(),
            },
            payload: SaveScanPayload {
                suggested: SaveSuggestion {
                    category: "02_folketelling".to_owned(),
                    subfolder: "1920".to_owned(),
                    filename: "1920_greipstad_folketelling_asbjorn-olsen.jpg".to_owned(),
                },
                categories: vec!["01_kirkebok".to_owned(), "02_folketelling".to_owned()],
            },
            onrespond: |_| {},
        }
    }
}

#[test]
fn save_stage_renders_the_dialog_with_the_suggested_target() {
    let html = render(save_view);
    assert!(html.contains("Save scan to media library"), "dialog title: {html}");
    assert!(
        html.contains("media/02_folketelling/1920/1920_greipstad_folketelling_asbjorn-olsen.jpg"),
        "live path preview from the suggested target: {html}"
    );
}

// ----- Summary stage -----

fn summary_view() -> Element {
    rsx! {
        SummaryStage {
            labels: SummaryLabels {
                heading: "Summary".to_owned(),
                imported: "2 imported".to_owned(),
                skipped: "1 skipped".to_owned(),
                another: "Import another".to_owned(),
            },
            imported: vec![
                ImportedRecord {
                    human_id: "I0001".to_owned(),
                    label: "Asbjørn Olsen".to_owned(),
                },
                ImportedRecord {
                    human_id: "I0002".to_owned(),
                    label: "Ole Andersen".to_owned(),
                },
            ],
            onrestart: |()| {},
        }
    }
}

#[test]
fn summary_stage_renders_counts_and_record_links() {
    let html = render(summary_view);
    assert!(html.contains("2 imported"), "imported count: {html}");
    assert!(html.contains("1 skipped"), "skipped count: {html}");
    assert!(
        html.contains("Asbjørn Olsen") && html.contains("Ole Andersen"),
        "imported record links: {html}"
    );
    assert!(html.contains("Import another"), "restart action: {html}");
}
