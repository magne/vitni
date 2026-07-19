//! The typed assisted-import presentation contract, plugin side (ADR 0017 §5).
//!
//! These mirror `genealogy_ui::import_payload` as the documented JSON shape the sandboxed plugin
//! cannot link: the plugin **emits** [`Payload`] through `present` and **parses** the wizard's
//! [`Response`]. Field names are snake_case and the `kind`/`confidence` discriminators kebab-case, to
//! match the wizard's serde exactly; the wizard's `parse_payload` is the contract check that a payload
//! is well-formed. Field/action `label`s are Fluent message ids the wizard resolves against this
//! plugin's catalogue (ADR 0012 §5); record content (names, dates, places) is sent verbatim.

use genealogy_digitalarkivet::PersonRecord;
use serde::{Deserialize, Serialize};

/// The `run-assisted` request: `{"kind":"url","url":…}` (additive kinds later).
#[derive(Debug, Deserialize)]
pub struct Request {
    /// The request discriminator (only `"url"` today).
    pub kind: String,
    /// The record or residence URL to import.
    pub url: String,
    /// An optional explicit page-kind override (`"census-person"`, `"census-residence"`,
    /// `"churchbook-record"`). The GUI omits it — the plugin classifies by the Digitalarkivet host and
    /// path. It exists for host-mediated tests, which serve rewritten fixtures from a mock host that
    /// [`classify_url`](genealogy_digitalarkivet::classify_url) (host-restricted by design) would not
    /// recognize; supplying it routes the flow without weakening classification.
    #[serde(default)]
    pub page: Option<String>,
}

/// The wizard's answer to a presented [`Payload`] (mirrors `genealogy_ui::ImportResponse`).
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Response {
    /// The user activated an action; `values` carries the stage's data.
    Submit {
        /// The activated action id (`"select"`, `"import"`, `"skip"`, `"save"`, `"done"`).
        action: String,
        /// The stage's response data (every field optional; a stage reads only what it needs).
        #[serde(default)]
        values: Values,
    },
    /// The user cancelled the session.
    Cancel,
}

/// The optional values a [`Response::Submit`] carries.
#[derive(Debug, Default, Deserialize)]
pub struct Values {
    /// The picked record id (records stage `select`).
    #[serde(default)]
    pub row: Option<String>,
    /// The edited field values (confirm stage `import`).
    #[serde(default)]
    pub fields: Vec<FieldValue>,
    /// The confirmed line region (confirm stage `import`).
    #[serde(default)]
    pub region: Option<Region>,
    /// The chosen confidence token (confirm stage `import`).
    #[serde(default)]
    pub confidence: Option<String>,
    /// The chosen filing target (save-scan stage `save`).
    #[serde(default)]
    pub save: Option<Save>,
}

/// One edited field value: the field key and its edited value.
#[derive(Debug, Deserialize)]
pub struct FieldValue {
    /// The field key this value is for.
    pub key: String,
    /// The user's edited value.
    pub value: String,
}

/// A confirmed crop region, as left/top/width/height percentages.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Region {
    /// Distance of the left edge from the scan's left, in percent.
    pub left: u8,
    /// Distance of the top edge from the scan's top, in percent.
    pub top: u8,
    /// Width of the region, in percent.
    pub width: u8,
    /// Height of the region, in percent.
    pub height: u8,
}

/// A chosen media-library filing target (save-scan `save`).
#[derive(Debug, Deserialize)]
pub struct Save {
    /// The category folder.
    pub category: String,
    /// An optional subfolder.
    #[serde(default)]
    pub subfolder: String,
    /// The filename.
    pub filename: String,
}

impl Save {
    /// The workspace-media-relative target path (the non-empty parts joined with `/`), which the host
    /// `media-store` writes under `<workspace>/media/` and enforces safety on.
    pub fn rel_path(&self) -> String {
        let mut parts = Vec::new();
        for part in [self.category.trim(), self.subfolder.trim(), self.filename.trim()] {
            if !part.is_empty() {
                parts.push(part);
            }
        }
        parts.join("/")
    }
}

/// One payload the plugin sends the wizard through `present` (mirrors `genealogy_ui::ImportPayload`).
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Payload {
    /// Stage 2 — the records found on the source page.
    Records {
        /// The fetched source header.
        source: SourceRef,
        /// The records found, in document order.
        records: Vec<RecordRow>,
    },
    /// Stage 3 — one record to confirm or skip.
    ConfirmRecord {
        /// The record under review.
        record: ConfirmRecord,
        /// The actions below the form.
        actions: Vec<Action>,
    },
    /// Stage 4 — where to file the scan.
    SaveScan {
        /// The proposed filing target.
        suggested: Suggestion,
        /// The category folders offered.
        categories: Vec<String>,
    },
    /// Stage 5 — the session summary.
    Summary {
        /// The imported records.
        imported: Vec<Imported>,
        /// How many records were skipped.
        skipped: u32,
    },
}

/// The fetched source page reference (record content).
#[derive(Debug, Serialize)]
pub struct SourceRef {
    /// The source title.
    pub title: String,
    /// The permanent source page URL.
    pub url: String,
}

/// One row in the records table (record content).
#[derive(Debug, Serialize)]
pub struct RecordRow {
    /// The record's opaque id (echoed back in `select`).
    pub id: String,
    /// The row's primary label (the person's name).
    pub label: String,
    /// An optional secondary line (born · role).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A record to confirm: editable fields, the optional scan, and a provenance preview.
#[derive(Debug, Serialize)]
pub struct ConfirmRecord {
    /// The transcribed fields, top to bottom.
    pub fields: Vec<PayloadField>,
    /// The scan to show (a remote image URL), if one resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan: Option<ScanRef>,
    /// A preview of the provenance every imported claim will carry.
    pub provenance: ProvenancePreview,
}

/// One editable field: `label` is a Fluent id (plugin chrome), `value` is record content.
#[derive(Debug, Serialize)]
pub struct PayloadField {
    /// The machine key the response echoes this field by.
    pub key: String,
    /// The field's label — a Fluent message id.
    pub label: String,
    /// The transcribed value, editable by the user.
    pub value: String,
}

/// The scan shown on the confirm stage.
#[derive(Debug, Serialize)]
pub struct ScanRef {
    /// The remote permanent image URL the wizard displays directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// A suggested line region (none today — the user draws it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
}

/// A preview of what provenance the import will record (all record content except `confidence`).
#[derive(Debug, Serialize)]
pub struct ProvenancePreview {
    /// The citing source's title.
    pub source_title: String,
    /// The managing repository's name.
    pub repository: String,
    /// The citation locator (a URN or record URL).
    pub citation: String,
    /// The external-id URL back to the source record.
    pub external_id_url: String,
    /// The default confidence for imported claims (kebab-case; the assisted flow proposes `low`).
    pub confidence: &'static str,
}

/// One action button: `id` is echoed in the response, `label` is a Fluent id (plugin chrome).
#[derive(Debug, Serialize)]
pub struct Action {
    /// The machine id the response carries.
    pub id: String,
    /// The button's label — a Fluent message id.
    pub label: String,
}

/// A proposed media-library filing target (record content, user-editable).
#[derive(Debug, Serialize)]
pub struct Suggestion {
    /// The category folder.
    pub category: String,
    /// An optional subfolder (empty when none).
    pub subfolder: String,
    /// The proposed filename.
    pub filename: String,
}

/// One imported record in the summary (human id + display name).
#[derive(Debug, Serialize)]
pub struct Imported {
    /// The created aggregate's human id.
    pub human_id: String,
    /// The record's display label.
    pub label: String,
}

/// A confirm field's key and its Fluent label id.
struct FieldSpec {
    key: &'static str,
    label: &'static str,
}

/// The confirm fields, in display order: the name (always) then the optional census/church-book
/// fields the parser found. Each `label` resolves against this plugin's catalogue.
const FIELD_SPECS: &[FieldSpec] = &[
    FieldSpec { key: "name", label: "field-name" },
    FieldSpec { key: "birth", label: "field-birth" },
    FieldSpec { key: "birthplace", label: "field-birthplace" },
    FieldSpec { key: "residence", label: "field-residence" },
    FieldSpec { key: "role", label: "field-role" },
    FieldSpec { key: "marital-status", label: "field-marital-status" },
    FieldSpec { key: "occupation", label: "field-occupation" },
];

impl Payload {
    /// The records-list payload for a fetched household.
    pub fn records(url: &str, records: &[PersonRecord]) -> Self {
        let title = records
            .first()
            .and_then(|record| record.source.title.clone())
            .unwrap_or_else(|| url.to_owned());
        Payload::Records {
            source: SourceRef {
                title,
                url: url.to_owned(),
            },
            records: records.iter().map(record_row).collect(),
        }
    }

    /// The confirm payload for one record, with the remote scan URL (when one resolved).
    pub fn confirm(record: &PersonRecord, scan_url: Option<&str>) -> Self {
        Payload::ConfirmRecord {
            record: ConfirmRecord {
                fields: confirm_fields(record),
                scan: scan_url.map(|url| ScanRef {
                    path: Some(url.to_owned()),
                    region: None,
                }),
                provenance: provenance(record, scan_url),
            },
            actions: vec![
                Action { id: "import".to_owned(), label: "action-import".to_owned() },
                Action { id: "skip".to_owned(), label: "action-skip".to_owned() },
            ],
        }
    }

    /// The save-scan payload: the proposed target and the category choices.
    pub fn save_scan(suggested: Suggestion, categories: &[&str]) -> Self {
        Payload::SaveScan {
            suggested,
            categories: categories.iter().map(|category| (*category).to_owned()).collect(),
        }
    }

    /// The summary payload.
    pub fn summary(imported: &[(String, String)], skipped: u32) -> Self {
        Payload::Summary {
            imported: imported
                .iter()
                .map(|(human_id, label)| Imported {
                    human_id: human_id.clone(),
                    label: label.clone(),
                })
                .collect(),
            skipped,
        }
    }
}

/// The record content behind a confirm field key, or `None` when the parser found nothing.
fn field_content(record: &PersonRecord, key: &str) -> Option<String> {
    match key {
        "name" => Some(record.name.clone()),
        "birth" => record.birth.clone(),
        "birthplace" => record.birthplace.clone(),
        "residence" => record.residence.clone(),
        "role" => record.role.clone(),
        "marital-status" => record.marital_status.clone(),
        "occupation" => record.occupation.clone(),
        _ => None,
    }
}

/// The confirm fields for a record: the name always, then each optional field the parser found.
fn confirm_fields(record: &PersonRecord) -> Vec<PayloadField> {
    let mut fields = Vec::new();
    for spec in FIELD_SPECS {
        let value = field_content(record, spec.key).unwrap_or_default();
        if spec.key == "name" || !value.trim().is_empty() {
            fields.push(PayloadField {
                key: spec.key.to_owned(),
                label: spec.label.to_owned(),
                value,
            });
        }
    }
    fields
}

/// The records-table row for a record: name + a "born · role" detail line.
fn record_row(record: &PersonRecord) -> RecordRow {
    let mut parts = Vec::new();
    if let Some(birth) = &record.birth {
        parts.push(birth.clone());
    }
    if let Some(role) = &record.role {
        parts.push(role.clone());
    }
    RecordRow {
        id: record.external_id.value.clone(),
        label: record.name.clone(),
        detail: (!parts.is_empty()).then(|| parts.join(" · ")),
    }
}

/// The provenance preview for a record: source title, repository, the citation locator, the
/// external-id URL, and the default `low` confidence.
fn provenance(record: &PersonRecord, scan_url: Option<&str>) -> ProvenancePreview {
    let citation = scan_url
        .and_then(genealogy_digitalarkivet::extract_urn)
        .unwrap_or_else(|| record.record_url.clone());
    ProvenancePreview {
        source_title: record.source.title.clone().unwrap_or_else(|| record.record_url.clone()),
        repository: record.source.repository.to_owned(),
        citation,
        external_id_url: record.record_url.clone(),
        confidence: "low",
    }
}
