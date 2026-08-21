//! `Tool::Import` (`import.html`): [`ImportModeSwitch`] picks between two independent wizards —
//! **Bulk file import** (issue #191, the default; body in `screens/bulk_import.rs`) and the
//! **Assisted online import** wizard this file otherwise implements (ADR 0017 §5), which drives one
//! long `run-assisted` plugin invocation and renders the typed `present` payloads it sends (parsed by
//! [`vitni_ui::ImportSession`], **not** the ADR 0022 UI vocabulary).
//!
//! The assisted wizard's stages: Source (pick an installed assisted-import plugin, enter a URL,
//! fetch) → Records (a picker table) → Confirm (a split view: the scan with the PR6 crop tool on the
//! left, the editable transcribed fields + a provenance preview on the right) → Save scan (the PR6
//! media-save dialog, once per source page) → Summary. The plugin drives which stage shows; the
//! wizard answers each payload over the presenter channel.
//!
//! Each stage is a pure component over already-localized label structs and the parsed payload; it
//! emits the user's answer as an [`ImportResponse`] through `onrespond`, so it renders in isolation
//! (the SSR tests do exactly that). The `ImportScreen` owns the session state and the presenter
//! channel and translates each `onrespond` into a channel reply plus the wizard-side status update.
//!
//! Session state is screen-local: navigating away unmounts the screen, dropping the invocation — the
//! plugin's next `present` fails with `backend`, the documented cancel-on-navigate path (ADR 0017 §5).
//! Full mid-session survival across navigation (a root-owned driver) is a noted follow-up.

use std::collections::HashMap;

use serde_json::json;
use tokio::sync::oneshot;
use vitni_plugin_host::PluginRole;
use vitni_ui::{
    ConfirmRecordPayload, CropRegion, FieldValue, ImportResponse, ImportSession, ImportStage, ImportedRecord,
    PayloadConfidence, ProvenancePreview, RecordsPayload, ResponseValues, SaveScanPayload, SaveSuggestion,
    resolve_confirm_record,
};

use super::bulk_import::BulkImportBody;
use super::prelude::*;
use crate::components::{
    MediaCropLabels, MediaCropTools, MediaSaveDialog, MediaSaveLabels, MediaViewer, MediaViewerLabels,
};
use crate::i18n::Chrome;
use crate::screens::shared::{media_crop_labels, media_viewer_labels};
use crate::services::{PluginRow, PresentRequest, discover_plugins, start_assisted_import};

/// A record's review status, tracked wizard-side and shown as a chip in the records table (the
/// `present` contract carries no status field — it is a purely cosmetic, wizard-owned concern).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRowStatus {
    /// Not yet reviewed.
    Pending,
    /// Imported this session.
    Imported,
    /// Skipped this session.
    Skipped,
}

/// The wizard chrome shared across stages: the heading and the five step names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardLabels {
    /// The wizard heading.
    pub heading: String,
    /// The five stage names (Source, Records, Confirm, Save scan, Summary).
    pub stages: [String; 5],
}

/// The Source-stage labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLabels {
    /// The plugin-selector label.
    pub source: String,
    /// The URL field label.
    pub url: String,
    /// The URL field placeholder.
    pub url_placeholder: String,
    /// The Fetch button label.
    pub fetch: String,
    /// The "no plugins installed" message.
    pub no_plugins: String,
    /// The "importing…" progress text.
    pub running: String,
}

/// The Records-stage labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordsLabels {
    /// The table heading.
    pub heading: String,
    /// The column headers (name, details, status, actions).
    pub headers: Vec<String>,
    /// The per-row Review action label.
    pub review: String,
    /// The Finish action label.
    pub finish: String,
    /// The status-chip labels, indexed pending/imported/skipped.
    pub status: [String; 3],
}

/// The Confirm-stage chrome (the field/action labels ride the resolved payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmChrome {
    /// The stage heading.
    pub heading: String,
    /// The provenance-preview card heading.
    pub provenance_heading: String,
    /// The provenance row labels (operator, source, repository, citation, external id, confidence).
    pub prov: [String; 6],
    /// The "software agent" badge label.
    pub software_agent: String,
    /// The scan-URL field label.
    pub scan_url_label: String,
    /// The scan-URL field placeholder.
    pub scan_url_placeholder: String,
}

/// The Summary-stage labels (the counts are pre-formatted by the caller).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryLabels {
    /// The stage heading.
    pub heading: String,
    /// The "{n} imported" badge text.
    pub imported: String,
    /// The "{n} skipped" badge text.
    pub skipped: String,
    /// The "Import another" action label.
    pub another: String,
}

/// `Tool::Import`'s front mode choice (issue #191): a local-file bulk import, or the assisted online
/// wizard below. Held as a plain radio-choice id string (not an enum) so [`ImportModeSwitch`] renders
/// in isolation, matching every other stage component's already-localized-label convention.
const IMPORT_MODE_BULK: &str = "bulk";

/// The mode-switch labels, resolved once from the chrome catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportModeLabels {
    /// The radio group's accessible name.
    pub group_label: String,
    /// The "Bulk file import" choice label.
    pub bulk: String,
    /// The "Assisted online import" choice label.
    pub assisted: String,
}

/// The mode chooser shown above both wizards: bulk file import (the default) versus the assisted
/// online wizard.
#[component]
pub fn ImportModeSwitch(labels: ImportModeLabels, mode: Signal<String>) -> Element {
    let choices = vec![
        RadioChoice {
            id: IMPORT_MODE_BULK.to_owned(),
            label: labels.bulk.clone(),
        },
        RadioChoice {
            id: "assisted".to_owned(),
            label: labels.assisted.clone(),
        },
    ];
    let mut mode = mode;
    rsx! {
        Card {
            RadioGroup {
                group_label: labels.group_label.clone(),
                choices,
                selected: mode(),
                onselect: move |id: String| mode.set(id),
            }
        }
    }
}

/// The `Tool::Import` screen: the mode chooser plus, depending on the pick, either the bulk-import
/// wizard body ([`BulkImportBody`], the default) or the assisted-import wizard below — owning the
/// latter's session and presenter channel, and composing its per-stage components (translating each
/// `onrespond` into a channel reply and a status update).
#[component]
pub fn ImportScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let chrome = use_context::<ChromeCtx>().0;
    let mode = use_signal(|| IMPORT_MODE_BULK.to_owned());
    if mode() == IMPORT_MODE_BULK {
        return rsx! {
            div { style: "display:flex;flex-direction:column;gap:var(--sp-4);padding:var(--sp-4)",
                h1 { style: "border:0;margin:0;font-size:21px", "{chrome.import_tool_heading()}" }
                ImportModeSwitch { labels: import_mode_labels(&chrome), mode }
                BulkImportBody {}
            }
        };
    }
    let session = use_signal(ImportSession::new);
    let responder = use_signal(|| None::<oneshot::Sender<String>>);
    let running = use_signal(|| false);
    let outcome = use_signal(|| None::<Result<String, String>>);
    let plugin_id = use_signal(String::new);
    let statuses = use_signal(HashMap::<String, ImportRowStatus>::new);
    let active_row = use_signal(|| None::<String>);
    // Bumped on every fetch and on start-over; the driver ignores payloads from a superseded run so a
    // cancelled invocation's trailing summary cannot clobber a fresh Source stage.
    let generation = use_signal(|| 0_u64);
    // Whether a records list was presented this run — decides whether confirm-stage Back returns to the
    // records list (residence) or starts over (a single-record page).
    let saw_records = use_signal(|| false);
    let loc = state.data_loc();
    let start_over = move || run_start_over(generation, session, responder, running, outcome, saw_records);

    let body = match session().stage().clone() {
        ImportStage::Records(payload) => rsx! {
            RecordsStage {
                labels: records_labels(&chrome),
                payload,
                statuses: statuses(),
                back_label: chrome.import_start_over(),
                onrespond: move |response| respond_with(&response, responder, statuses, active_row),
                onback: move |()| start_over(),
            }
        },
        ImportStage::Confirm(payload) => {
            let dir = state.services().plugin_catalogue_dir(&plugin_id());
            let resolved =
                resolve_confirm_record(&payload, &dir, &plugin_id(), &state.services().requested_languages());
            // With a records list behind us (residence), Back returns to it; on a single-record page
            // there is nothing to go back to, so Back starts over.
            let to_records = saw_records();
            rsx! {
                ConfirmStage {
                    key: "{resolved.record.provenance.external_id_url}",
                    viewer_labels: media_viewer_labels(loc),
                    crop_labels: media_crop_labels(loc),
                    chrome: confirm_chrome(&chrome),
                    confidence_labels: confidence_levels(loc),
                    payload: resolved,
                    back_label: if to_records { chrome.import_back() } else { chrome.import_start_over() },
                    onrespond: move |response| respond_with(&response, responder, statuses, active_row),
                    onback: move |()| {
                        if to_records {
                            reply(responder, &submit("back", ResponseValues::default()));
                        } else {
                            start_over();
                        }
                    },
                }
            }
        }
        ImportStage::SaveScan(payload) => rsx! {
            SaveStage {
                key: "{payload.suggested.filename}",
                labels: chrome.import_save_labels(),
                payload,
                back_label: chrome.import_back(),
                onrespond: move |response| respond_with(&response, responder, statuses, active_row),
                onback: move |()| reply(responder, &submit("back", ResponseValues::default())),
            }
        },
        ImportStage::Summary(payload) => rsx! {
            SummaryStage {
                labels: summary_labels(&chrome, payload.imported.len(), payload.skipped),
                imported: payload.imported,
                onrestart: move |()| restart(session, running, outcome),
            }
        },
        ImportStage::Error(error) => rsx! { p { class: "empty", "{chrome.plugin_error(&error.to_string())}" } },
        ImportStage::Source | ImportStage::Cancelled => source_body(
            &state,
            &chrome,
            plugin_id,
            session,
            responder,
            running,
            outcome,
            statuses,
            generation,
            saw_records,
        ),
    };

    rsx! {
        div { style: "display:flex;flex-direction:column;gap:var(--sp-4);padding:var(--sp-4)",
            h1 { style: "border:0;margin:0;font-size:21px", "{chrome.import_tool_heading()}" }
            ImportModeSwitch { labels: import_mode_labels(&chrome), mode }
            h2 { style: "border:0;margin:0;font-size:16px", "{chrome.import_heading()}" }
            {step_indicator(&wizard_labels(&chrome), session().stage())}
            {body}
        }
    }
}

/// The mode-switch labels from the chrome catalogue.
fn import_mode_labels(chrome: &Chrome) -> ImportModeLabels {
    ImportModeLabels {
        group_label: chrome.import_mode_label(),
        bulk: chrome.import_mode_bulk(),
        assisted: chrome.import_mode_assisted(),
    }
}

/// Builds the Source stage: discovers installed assisted-import plugins, then renders [`SourceStage`]
/// and, on Fetch, starts the invocation and spawns the driver loop.
#[expect(
    clippy::too_many_arguments,
    reason = "the source stage seeds every session signal plus the run guard"
)]
fn source_body(
    state: &AppState,
    chrome: &Chrome,
    plugin_id: Signal<String>,
    session: Signal<ImportSession>,
    responder: Signal<Option<oneshot::Sender<String>>>,
    running: Signal<bool>,
    outcome: Signal<Option<Result<String, String>>>,
    statuses: Signal<HashMap<String, ImportRowStatus>>,
    generation: Signal<u64>,
    saw_records: Signal<bool>,
) -> Element {
    let discover_services = state.services().clone();
    let plugins = use_resource(move || {
        let services = discover_services.clone();
        async move { discover_plugins(services).await.unwrap_or_default() }
    });
    let assisted: Vec<PluginRow> = plugins
        .read_unchecked()
        .as_ref()
        .map(|rows| {
            rows.iter()
                .filter(|row| row.role == PluginRole::AssistedImport && row.enabled)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let mut plugin_id = plugin_id;
    if plugin_id().is_empty()
        && let Some(first) = assisted.first()
    {
        plugin_id.set(first.id.clone());
    }
    let options: Vec<SelectChoice> = assisted
        .iter()
        .map(|row| SelectChoice {
            value: row.id.clone(),
            label: row.id.clone(),
        })
        .collect();
    let fetch_services = state.services().clone();
    let onfetch = move |url: String| {
        let id = plugin_id();
        if url.trim().is_empty() || id.is_empty() {
            return;
        }
        let (mut session, mut running, mut outcome, mut statuses, mut saw_records, mut generation) =
            (session, running, outcome, statuses, saw_records, generation);
        session.set(ImportSession::new());
        outcome.set(None);
        statuses.write().clear();
        saw_records.set(false);
        let run = generation() + 1;
        generation.set(run);
        running.set(true);
        let request = json!({ "kind": "url", "url": url.trim() }).to_string();
        let (handle, future) = start_assisted_import(fetch_services.clone(), id, request);
        spawn(future);
        spawn(drive(
            handle,
            run,
            generation,
            session,
            responder,
            running,
            outcome,
            saw_records,
        ));
    };
    // A session that ends before reaching a later stage (an unrecognized URL, a fetch/parse failure)
    // lands back here with its error in `outcome`; surface it so Fetch is never a silent no-op.
    let error = match outcome() {
        Some(Err(message)) if !message.is_empty() => Some(message),
        _ => None,
    };
    rsx! {
        SourceStage {
            labels: source_labels(chrome),
            options,
            plugin_id,
            running: running(),
            error,
            onfetch,
        }
    }
}

/// Stage 1 — the source picker: an installed-assisted-import-plugin selector, a URL field, and Fetch.
#[component]
pub fn SourceStage(
    labels: SourceLabels,
    options: Vec<SelectChoice>,
    plugin_id: Signal<String>,
    running: bool,
    error: Option<String>,
    onfetch: EventHandler<String>,
) -> Element {
    let mut plugin_id = plugin_id;
    let mut url = use_signal(String::new);
    let no_plugins = options.is_empty();
    rsx! {
        Card {
            div { class: "grid-2", style: "align-items:end",
                div { class: "field", style: "margin:0",
                    label { r#for: "import-plugin", "{labels.source}" }
                    if no_plugins {
                        p { class: "muted", "{labels.no_plugins}" }
                    } else {
                        Select {
                            label: labels.source.clone(),
                            name: "import-plugin".to_owned(),
                            value: Some(plugin_id()),
                            options,
                            onchange: move |event: FormEvent| plugin_id.set(event.value()),
                        }
                    }
                }
                div { class: "field", style: "margin:0",
                    label { r#for: "import-url", "{labels.url}" }
                    TextInput {
                        id: "import-url",
                        name: "import-url",
                        value: url(),
                        placeholder: Some(labels.url_placeholder.clone()),
                        oninput: move |event: FormEvent| url.set(event.value()),
                    }
                }
            }
            div { style: "margin-top:var(--sp-3)",
                Button {
                    label: labels.fetch.clone(),
                    variant: ButtonVariant::Primary,
                    disabled: no_plugins || running,
                    onclick: move |_| onfetch.call(url()),
                }
                if running {
                    span { class: "muted", style: "margin-left:var(--sp-3)", "{labels.running}" }
                }
            }
            if let Some(message) = error {
                p { class: "empty", style: "margin-top:var(--sp-3)", role: "alert", "{message}" }
            }
        }
    }
}

/// Stage 2 — the records table: name, details, a wizard-tracked status chip, and a per-row Review.
#[component]
pub fn RecordsStage(
    labels: RecordsLabels,
    payload: RecordsPayload,
    statuses: HashMap<String, ImportRowStatus>,
    back_label: String,
    onrespond: EventHandler<ImportResponse>,
    onback: EventHandler<()>,
) -> Element {
    rsx! {
        Card {
            h3 {
                "{labels.heading} "
                span { class: "muted", "— {payload.source.title}" }
            }
            Table {
                caption: labels.heading.clone(),
                headers: labels.headers.clone(),
                for row in payload.records.iter().cloned() {
                    {
                        let status = statuses.get(&row.id).copied().unwrap_or(ImportRowStatus::Pending);
                        let chip = labels.status[status_index(status)].clone();
                        let id = row.id.clone();
                        rsx! {
                            tr {
                                td { "{row.label}" }
                                td { class: "muted", "{row.detail.clone().unwrap_or_default()}" }
                                td { span { class: "chip", "{chip}" } }
                                td {
                                    Button {
                                        label: labels.review.clone(),
                                        small: true,
                                        onclick: move |_| onrespond.call(submit("select", ResponseValues {
                                            row: Some(id.clone()),
                                            ..ResponseValues::default()
                                        })),
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "wrap", style: "margin-top:var(--sp-3);align-items:center",
                Button {
                    label: back_label,
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| onback.call(()),
                }
                span { class: "spacer" }
                Button {
                    label: labels.finish.clone(),
                    onclick: move |_| onrespond.call(submit("done", ResponseValues::default())),
                }
            }
        }
    }
}

/// Stage 3 — the confirm split view (keyed by the parent so its edit signals re-seed per record):
/// the scan + crop tool on the left, the editable fields + a provenance preview on the right.
#[component]
pub fn ConfirmStage(
    viewer_labels: MediaViewerLabels,
    crop_labels: MediaCropLabels,
    chrome: ConfirmChrome,
    confidence_labels: Vec<(ConfidenceLevel, String)>,
    payload: ConfirmRecordPayload,
    back_label: String,
    onrespond: EventHandler<ImportResponse>,
    onback: EventHandler<()>,
) -> Element {
    let record = payload.record.clone();
    let mut edits = use_signal(|| {
        record
            .fields
            .iter()
            .map(|field| (field.key.clone(), field.value.clone()))
            .collect::<HashMap<String, String>>()
    });
    let mut region = use_signal(|| record.scan.as_ref().and_then(|scan| scan.region).map(to_rect));
    let confidence = use_signal(|| record.provenance.confidence);
    // The scan URL is editable and prefilled from the resolved scan (empty when none resolved, e.g. a
    // 1910 census page): the user may paste the scanned-page URL to file a scan the plugin missed.
    let mut scan_url = use_signal(|| {
        record
            .scan
            .as_ref()
            .and_then(|scan| scan.path.clone())
            .unwrap_or_default()
    });
    // The preview follows the live URL, so pasting one shows the scan (and enables the crop tool).
    let current_url = scan_url();
    let scan_item = (!current_url.trim().is_empty()).then(|| MediaRefVm {
        human_id: String::new(),
        assertion_id: String::new(),
        caption: None,
        crop: region(),
        path: Some(current_url.clone()),
        mime: Some("image/jpeg".to_owned()),
    });
    let import_action = payload.actions.first().cloned();
    let skip_action = payload.actions.get(1).cloned();
    let submit_confirm = move |action: &str| {
        let fields = edits()
            .into_iter()
            .map(|(key, value)| FieldValue { key, value })
            .collect();
        let url = scan_url();
        onrespond.call(submit(
            action,
            ResponseValues {
                fields,
                region: region().map(from_rect),
                confidence: Some(confidence()),
                scan_url: (!url.trim().is_empty()).then_some(url),
                ..ResponseValues::default()
            },
        ));
    };

    rsx! {
        Card {
            h3 { "{chrome.heading}" }
            div { class: "confirm-split",
                div {
                    if let Some(item) = scan_item {
                        MediaViewer {
                            item,
                            labels: viewer_labels,
                            crop: Some(MediaCropTools {
                                labels: crop_labels,
                                onset: EventHandler::new(move |rect: Rect| region.set(Some(rect))),
                                onclear: EventHandler::new(move |()| region.set(None)),
                            }),
                            onclose: move |()| {},
                        }
                    }
                    div { class: "field", style: "margin-top:var(--sp-3)",
                        label { r#for: "import-scan-url", "{chrome.scan_url_label}" }
                        TextInput {
                            id: "import-scan-url",
                            name: "import-scan-url",
                            value: scan_url(),
                            placeholder: Some(chrome.scan_url_placeholder.clone()),
                            oninput: move |event: FormEvent| scan_url.set(event.value()),
                        }
                    }
                }
                div {
                    div { class: "stack",
                        for field in record.fields.iter().cloned() {
                            div { class: "field", style: "margin:0",
                                label { "{field.label}" }
                                TextInput {
                                    name: field.key.clone(),
                                    value: edits().get(&field.key).cloned().unwrap_or(field.value.clone()),
                                    oninput: move |event: FormEvent| {
                                        edits.write().insert(field.key.clone(), event.value());
                                    },
                                }
                            }
                        }
                    }
                    {provenance_card(&chrome, &record.provenance, &confidence_labels, confidence)}
                    div { class: "wrap", style: "margin-top:var(--sp-4);align-items:center",
                        Button {
                            label: back_label,
                            variant: ButtonVariant::Ghost,
                            onclick: move |_| onback.call(()),
                        }
                        if let Some(skip) = skip_action {
                            Button {
                                label: skip.label,
                                variant: ButtonVariant::Ghost,
                                onclick: move |_| submit_confirm("skip"),
                            }
                        }
                        span { class: "spacer" }
                        if let Some(import) = import_action {
                            Button {
                                label: import.label,
                                variant: ButtonVariant::Primary,
                                onclick: move |_| submit_confirm("import"),
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The provenance-preview card: the recorded source/repository/citation/external-id, and an editable
/// confidence select (defaulting to the plugin's proposed value — `low` for the assisted flow).
fn provenance_card(
    chrome: &ConfirmChrome,
    provenance: &ProvenancePreview,
    confidence_labels: &[(ConfidenceLevel, String)],
    mut confidence: Signal<PayloadConfidence>,
) -> Element {
    let options: Vec<SelectChoice> = confidence_labels
        .iter()
        .map(|(level, label)| SelectChoice {
            value: level_token(*level).to_owned(),
            label: label.clone(),
        })
        .collect();
    let selected = level_token(payload_to_level(confidence())).to_owned();
    rsx! {
        div { class: "prov", style: "max-width:none;margin-top:var(--sp-4)",
            h4 { "{chrome.provenance_heading}" }
            {prov_row(&chrome.prov[1], &provenance.source_title)}
            {prov_row(&chrome.prov[2], &provenance.repository)}
            {prov_row(&chrome.prov[3], &provenance.citation)}
            {prov_row(&chrome.prov[4], &provenance.external_id_url)}
            div { class: "prov-claim", style: "align-items:center",
                span { class: "muted", style: "width:96px", "{chrome.prov[5]}" }
                Select {
                    label: chrome.prov[5].clone(),
                    name: "import-confidence".to_owned(),
                    value: Some(selected),
                    options,
                    onchange: move |event: FormEvent| confidence.set(token_to_payload(&event.value())),
                }
            }
        }
    }
}

/// One provenance-preview row: a fixed-width muted label and the (record-content) value.
fn prov_row(label: &str, value: &str) -> Element {
    rsx! {
        div { class: "prov-claim",
            span { class: "muted", style: "width:96px", "{label}" }
            span { class: "grow mono", style: "font-size:var(--fs-xs);word-break:break-all", "{value}" }
        }
    }
}

/// Stage 4 — the save-scan dialog (the shared PR6 media-save dialog), keyed by the suggested filename.
#[component]
pub fn SaveStage(
    labels: MediaSaveLabels,
    payload: SaveScanPayload,
    back_label: String,
    onrespond: EventHandler<ImportResponse>,
    onback: EventHandler<()>,
) -> Element {
    let draft = use_signal(|| vitni_ui::MediaSaveDraft {
        category: payload.suggested.category.clone(),
        subfolder: payload.suggested.subfolder.clone(),
        filename: payload.suggested.filename.clone(),
    });
    rsx! {
        MediaSaveDialog {
            open: true,
            labels,
            categories: payload.categories.clone(),
            draft,
            back_label: Some(back_label),
            onback: move |()| onback.call(()),
            onsave: move |_path: String| {
                let draft = draft();
                onrespond.call(submit("save", ResponseValues {
                    save: Some(SaveSuggestion {
                        category: draft.category,
                        subfolder: draft.subfolder,
                        filename: draft.filename,
                    }),
                    ..ResponseValues::default()
                }));
            },
            oncancel: move |()| onrespond.call(ImportResponse::Cancel),
        }
    }
}

/// Stage 5 — the summary: imported/skipped counts, links to created records, and "Import another".
#[component]
pub fn SummaryStage(labels: SummaryLabels, imported: Vec<ImportedRecord>, onrestart: EventHandler<()>) -> Element {
    rsx! {
        Card {
            h3 { "{labels.heading}" }
            div { class: "wrap", style: "gap:var(--sp-4)",
                span { class: "badge", "{labels.imported}" }
                span { class: "badge", "{labels.skipped}" }
            }
            div { class: "stack", style: "margin-top:var(--sp-3)",
                for record in imported.iter().cloned() {
                    div { class: "fact-row",
                        span { class: "grow", "{record.label}" }
                        span { class: "muted mono", "{record.human_id}" }
                    }
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

/// The wizard step indicator: the five stage names, the current one marked `aria-current`.
fn step_indicator(labels: &WizardLabels, stage: &ImportStage) -> Element {
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
fn stage_index(stage: &ImportStage) -> usize {
    match stage {
        ImportStage::Source | ImportStage::Cancelled => 0,
        ImportStage::Records(_) => 1,
        ImportStage::Confirm(_) => 2,
        ImportStage::SaveScan(_) => 3,
        ImportStage::Summary(_) | ImportStage::Error(_) => 4,
    }
}

/// The driver loop: pumps each `present` request into the session and, when the invocation ends,
/// records its outcome. Spawned on the renderer's local executor (the future is `!Send`).
///
/// `run` is this invocation's generation. If `generation` moves on (a new fetch, or start-over) the
/// run is superseded: the loop stops touching the UI and just cancels any trailing `present` (so a
/// cancelled invocation's summary cannot overwrite a fresh Source stage), then exits without
/// recording an outcome.
#[expect(
    clippy::too_many_arguments,
    reason = "the driver threads every session signal plus the run guard"
)]
async fn drive(
    handle: crate::services::AssistedImportHandle,
    run: u64,
    generation: Signal<u64>,
    mut session: Signal<ImportSession>,
    mut responder: Signal<Option<oneshot::Sender<String>>>,
    mut running: Signal<bool>,
    mut outcome: Signal<Option<Result<String, String>>>,
    mut saw_records: Signal<bool>,
) {
    let mut requests = handle.requests;
    while let Some(PresentRequest {
        payload,
        responder: reply,
    }) = requests.recv().await
    {
        if generation() != run {
            drop(reply.send(cancel_json())); // superseded run: unblock the plugin, ignore the payload
            continue;
        }
        session.write().on_payload(&payload);
        if matches!(session().stage(), ImportStage::Records(_)) {
            saw_records.set(true);
        }
        responder.set(Some(reply));
    }
    if generation() == run {
        let result = handle.outcome.await.unwrap_or_else(|_| Err(String::new()));
        outcome.set(Some(result));
        running.set(false);
    }
}

/// Starts the session over: supersede the running invocation (bump `generation` so [`drive`] stops
/// applying its payloads), cancel the present it is blocked on, and reset the wizard to Source.
fn run_start_over(
    mut generation: Signal<u64>,
    session: Signal<ImportSession>,
    responder: Signal<Option<oneshot::Sender<String>>>,
    running: Signal<bool>,
    outcome: Signal<Option<Result<String, String>>>,
    mut saw_records: Signal<bool>,
) {
    generation.set(generation() + 1);
    reply(responder, &ImportResponse::Cancel);
    saw_records.set(false);
    restart(session, running, outcome);
}

/// The `{"kind":"cancel"}` response JSON, used to unblock a superseded invocation's `present`.
fn cancel_json() -> String {
    serde_json::to_string(&ImportResponse::Cancel).unwrap_or_else(|_| r#"{"kind":"cancel"}"#.to_owned())
}

/// Applies a stage response: the wizard-side status side-effect (which record is active / imported /
/// skipped), then the channel reply back to the plugin.
fn respond_with(
    response: &ImportResponse,
    responder: Signal<Option<oneshot::Sender<String>>>,
    mut statuses: Signal<HashMap<String, ImportRowStatus>>,
    mut active_row: Signal<Option<String>>,
) {
    if let ImportResponse::Submit { action, values } = response {
        match action.as_str() {
            "select" => active_row.set(values.row.clone()),
            "import" => {
                if let Some(row) = active_row() {
                    statuses.write().insert(row, ImportRowStatus::Imported);
                }
            }
            "skip" => {
                if let Some(row) = active_row() {
                    statuses.write().insert(row, ImportRowStatus::Skipped);
                }
            }
            _ => {}
        }
    }
    reply(responder, response);
}

/// Sends `response` back to the plugin over the current presenter channel (a no-op if none is live).
fn reply(mut responder: Signal<Option<oneshot::Sender<String>>>, response: &ImportResponse) {
    if let Some(reply) = responder.write().take()
        && let Ok(json) = serde_json::to_string(response)
    {
        drop(reply.send(json));
    }
}

/// Resets the session to a fresh Source stage ("Import another" / a cancelled off-ramp).
fn restart(
    mut session: Signal<ImportSession>,
    mut running: Signal<bool>,
    mut outcome: Signal<Option<Result<String, String>>>,
) {
    session.set(ImportSession::new());
    outcome.set(None);
    running.set(false);
}

/// A `submit` [`ImportResponse`] with an action id and its values.
fn submit(action: &str, values: ResponseValues) -> ImportResponse {
    ImportResponse::Submit {
        action: action.to_owned(),
        values,
    }
}

/// The status chip's index into [`RecordsLabels::status`].
fn status_index(status: ImportRowStatus) -> usize {
    match status {
        ImportRowStatus::Pending => 0,
        ImportRowStatus::Imported => 1,
        ImportRowStatus::Skipped => 2,
    }
}

/// The five confidence levels with their localized labels, for the confirm-stage confidence select.
fn confidence_levels(loc: &Localizer) -> Vec<(ConfidenceLevel, String)> {
    [
        ConfidenceLevel::VeryLow,
        ConfidenceLevel::Low,
        ConfidenceLevel::Normal,
        ConfidenceLevel::High,
        ConfidenceLevel::VeryHigh,
    ]
    .into_iter()
    .map(|level| (level, loc.confidence_label(level)))
    .collect()
}

/// The shared wizard labels from the chrome catalogue.
fn wizard_labels(chrome: &Chrome) -> WizardLabels {
    WizardLabels {
        heading: chrome.import_heading(),
        stages: chrome.import_stages(),
    }
}

/// The Source-stage labels from the chrome catalogue.
fn source_labels(chrome: &Chrome) -> SourceLabels {
    SourceLabels {
        source: chrome.import_source_label(),
        url: chrome.import_url_label(),
        url_placeholder: chrome.import_url_placeholder(),
        fetch: chrome.import_fetch(),
        no_plugins: chrome.import_no_plugins(),
        running: chrome.import_running(),
    }
}

/// The Records-stage labels from the chrome catalogue.
fn records_labels(chrome: &Chrome) -> RecordsLabels {
    RecordsLabels {
        heading: chrome.import_records_heading(),
        headers: chrome.import_records_headers(),
        review: chrome.import_review(),
        finish: chrome.import_finish(),
        status: [
            chrome.import_status(ImportRowStatus::Pending),
            chrome.import_status(ImportRowStatus::Imported),
            chrome.import_status(ImportRowStatus::Skipped),
        ],
    }
}

/// The Confirm-stage chrome from the chrome catalogue.
fn confirm_chrome(chrome: &Chrome) -> ConfirmChrome {
    ConfirmChrome {
        heading: chrome.import_confirm_heading(),
        provenance_heading: chrome.import_provenance_heading(),
        prov: chrome.import_prov_labels(),
        software_agent: chrome.import_software_agent(),
        scan_url_label: chrome.import_scan_url_label(),
        scan_url_placeholder: chrome.import_scan_url_placeholder(),
    }
}

/// The Summary-stage labels from the chrome catalogue, with the counts filled in.
fn summary_labels(chrome: &Chrome, imported: usize, skipped: u32) -> SummaryLabels {
    SummaryLabels {
        heading: chrome.import_summary_heading(),
        imported: chrome.import_summary_imported(imported),
        skipped: chrome.import_summary_skipped(skipped),
        another: chrome.import_another(),
    }
}

/// A [`CropRegion`] as a core [`Rect`] (for the media viewer / crop overlay).
fn to_rect(region: CropRegion) -> Rect {
    Rect {
        left: region.left,
        top: region.top,
        width: region.width,
        height: region.height,
    }
}

/// A core [`Rect`] as a [`CropRegion`] (for the import response the plugin reads).
fn from_rect(rect: Rect) -> CropRegion {
    CropRegion {
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
    }
}

/// The stable token for a confidence level (matching the WIT/contract kebab-case values).
fn level_token(level: ConfidenceLevel) -> &'static str {
    match level {
        ConfidenceLevel::VeryLow => "very-low",
        ConfidenceLevel::Low => "low",
        ConfidenceLevel::Normal => "normal",
        ConfidenceLevel::High => "high",
        ConfidenceLevel::VeryHigh => "very-high",
    }
}

/// Maps a confidence token onto the payload confidence.
fn token_to_payload(token: &str) -> PayloadConfidence {
    match token {
        "very-low" => PayloadConfidence::VeryLow,
        "normal" => PayloadConfidence::Normal,
        "high" => PayloadConfidence::High,
        "very-high" => PayloadConfidence::VeryHigh,
        _ => PayloadConfidence::Low,
    }
}

/// Maps a payload confidence onto the presentation level (for the select's current value).
fn payload_to_level(confidence: PayloadConfidence) -> ConfidenceLevel {
    match confidence {
        PayloadConfidence::VeryLow => ConfidenceLevel::VeryLow,
        PayloadConfidence::Low => ConfidenceLevel::Low,
        PayloadConfidence::Normal => ConfidenceLevel::Normal,
        PayloadConfidence::High => ConfidenceLevel::High,
        PayloadConfidence::VeryHigh => ConfidenceLevel::VeryHigh,
    }
}
