//! The typed assisted-import presentation contract (ADR 0017 §5).
//!
//! An assisted-import plugin runs one long invocation and, at each review step, calls the host
//! `present` capability with a JSON [`ImportPayload`]; the host is opaque to it (like GEDCOM bytes and
//! ADR 0022 panels). This is **not** the ADR 0022 UI vocabulary ([`crate::vocabulary`]) — that is a
//! generic widget schema; this is a dedicated, versioned contract for the record-by-record import
//! wizard, so the plugin describes *import data* (records, fields, a scan region, a provenance
//! preview) and the first-party wizard renders it. The plugin, a sandboxed component, cannot link
//! these types; the contract is the documented JSON shape, and [`parse_payload`] validates it.
//!
//! The encoding is the project's **internally-tagged** convention (a `kind` discriminator, matching
//! the event encoding and [`crate::vocabulary`]) and is **additive**: unknown object fields are
//! ignored so an older wizard tolerates a newer plugin's extra data, while an unknown `kind` is a
//! hard error (a stage the wizard cannot render). The stages mirror `docs/mockups/import.html`:
//! Source → Records → Confirm (per record) → Save-scan (once per source page) → Summary.
//!
//! **Label vs. data localization (ADR 0022 §5 split).** A field is either *plugin chrome* — a Fluent
//! message id the wizard resolves against the plugin's own catalogue (ADR 0012 §5) — or *record
//! content* — literal data shown verbatim, never resolved. Each field below documents which it is;
//! the split matters because resolving a person's name as a message id would be wrong.

use serde::{Deserialize, Serialize};

/// The surety stamped on imported claims (ADR 0017 §7), mirroring the five-level
/// `vitni_core::provenance::Confidence`. Serialized kebab-case (`very-low`…`very-high`) to match
/// the WIT `confidence` enum; defined locally so the contract stays self-describing JSON a
/// non-Rust plugin can emit. The assisted flow defaults to [`Low`](Self::Low).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PayloadConfidence {
    /// Very low surety.
    VeryLow,
    /// Low surety — the assisted-import default.
    Low,
    /// Normal surety.
    Normal,
    /// High surety.
    High,
    /// Very high surety.
    VeryHigh,
}

/// A rectangular region of a scan, as left/top/width/height percentages (0–100), mirroring
/// `vitni_core::text::Rect`. In the confirm stage it is the census line the record was read from,
/// which becomes the citation's `MediaRef.crop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CropRegion {
    /// Distance of the left edge from the scan's left, in percent.
    pub left: u8,
    /// Distance of the top edge from the scan's top, in percent.
    pub top: u8,
    /// Width of the region, in percent of the scan width.
    pub width: u8,
    /// Height of the region, in percent of the scan height.
    pub height: u8,
}

/// One payload the plugin sends the wizard through `present` (ADR 0017 §5). The `kind` tag selects the
/// stage (internally-tagged JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ImportPayload {
    /// Stage 2 — the records found on the source page, for the user to pick, skip, or finish.
    Records(RecordsPayload),
    /// Stage 3 — one record to confirm or skip: editable fields, the scan + suggested line region, and
    /// a provenance preview.
    ConfirmRecord(ConfirmRecordPayload),
    /// Stage 4 — where to file the scan in the media library (shown once per source page).
    SaveScan(SaveScanPayload),
    /// Stage 5 — the session summary: what was imported and how many were skipped.
    Summary(SummaryPayload),
}

/// The [`ImportPayload::Records`] body: the source header and the record rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordsPayload {
    /// The fetched source (a census residence, a church-book record). **Record content.**
    pub source: SourceRef,
    /// The records found on the page, in document order.
    pub records: Vec<RecordRow>,
}

/// A reference to the fetched source page. Both fields are **record content** (never resolved).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    /// The source title (e.g. `"Folketelling 1920 for 1017 Greipstad"`).
    pub title: String,
    /// The permanent source page URL.
    pub url: String,
}

/// One row in the records table. `label`/`detail` are **record content**; `id` is a machine handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordRow {
    /// The plugin's opaque handle for this record (a household link / record id) — echoed back in the
    /// response, never shown.
    pub id: String,
    /// The row's primary label (the person's name). **Record content.**
    pub label: String,
    /// An optional secondary line (born / role / occupation). **Record content.**
    #[serde(default)]
    pub detail: Option<String>,
}

/// The [`ImportPayload::ConfirmRecord`] body: the record to review and the actions offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmRecordPayload {
    /// The record under review.
    pub record: ConfirmRecord,
    /// The actions below the form (e.g. import, skip). Each `label` is **plugin chrome** (a Fluent id).
    pub actions: Vec<PayloadAction>,
}

/// A single record to confirm: editable fields, the optional scan + region, and a provenance preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmRecord {
    /// The transcribed fields, top to bottom.
    pub fields: Vec<PayloadField>,
    /// The scan to show (with a suggested line region), if the plugin resolved one.
    #[serde(default)]
    pub scan: Option<ScanRef>,
    /// A preview of the provenance every imported claim will carry.
    pub provenance: ProvenancePreview,
}

/// One editable field. `label` is **plugin chrome** (a Fluent id); `key` is a machine handle echoed in
/// the response; `value` is **record content** (the transcribed value the user edits, never resolved).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadField {
    /// The machine key the response echoes this field by. Never shown.
    pub key: String,
    /// The field's user-facing label — a Fluent message id (**plugin chrome**).
    pub label: String,
    /// The transcribed value, editable by the user. **Record content** (never resolved).
    pub value: String,
}

/// The scan shown on the confirm stage. Both fields are optional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanRef {
    /// Where the wizard loads the scan from — a workspace-relative media path (once stored) or a
    /// remote image URL (before it is filed). **Record content**; the wizard displays it, never
    /// resolves it.
    #[serde(default)]
    pub path: Option<String>,
    /// The suggested line region highlighting the record's row; the user may redraw it.
    #[serde(default)]
    pub region: Option<CropRegion>,
}

/// A preview of what provenance the import will record. `source_title`/`repository`/`citation`/
/// `external_id_url` are **record content**; `confidence` is the default surety the user may change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenancePreview {
    /// The citing source's title. **Record content.**
    pub source_title: String,
    /// The managing repository's name. **Record content.**
    pub repository: String,
    /// The citation locator (e.g. a URN + retrieval date). **Record content.**
    pub citation: String,
    /// The external-id URL back to the source record. **Record content.**
    pub external_id_url: String,
    /// The default confidence for imported claims (the assisted flow proposes
    /// [`Low`](PayloadConfidence::Low)).
    pub confidence: PayloadConfidence,
}

/// One action button. `id` is a machine handle echoed in the response; `label` is **plugin chrome**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadAction {
    /// The machine id the response carries so the plugin knows which button was activated. Never shown.
    pub id: String,
    /// The button's user-facing label — a Fluent message id (**plugin chrome**).
    pub label: String,
}

/// The [`ImportPayload::SaveScan`] body: the proposed target and the category choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveScanPayload {
    /// The plugin's proposed filing target, editable by the user. **Record content.**
    pub suggested: SaveSuggestion,
    /// The category folders offered in the picker (convention folders ∪ existing `media/` folders).
    /// **Record content** (folder names, never resolved).
    pub categories: Vec<String>,
}

/// A proposed media-library filing target: the parts of the workspace-relative path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveSuggestion {
    /// The category folder (e.g. `02_folketelling`).
    pub category: String,
    /// An optional subfolder (e.g. a year); empty when none.
    #[serde(default)]
    pub subfolder: String,
    /// The proposed filename (e.g. `1920_greipstad_folketelling_asbjorn-olsen.jpg`).
    pub filename: String,
}

/// The [`ImportPayload::Summary`] body: what the session imported and skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryPayload {
    /// The imported records (human id + display label). `label` is **record content**.
    pub imported: Vec<ImportedRecord>,
    /// How many records the user skipped.
    pub skipped: u32,
}

/// One imported record in the summary. `label` is **record content**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedRecord {
    /// The created aggregate's human id.
    pub human_id: String,
    /// The record's display label (the person's name). **Record content.**
    pub label: String,
}

/// The wizard's answer to a [`ImportPayload`], sent back to the plugin as JSON (ADR 0017 §5). The
/// `kind` tag selects the variant; the host passes it through opaquely. Mirrors the ADR 0022
/// submit/cancel shape: [`Submit`](Self::Submit) carries the activated action id and the typed values
/// for the stage, and [`Cancel`](Self::Cancel) ends the session from any stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ImportResponse {
    /// The user activated an action. `action` is the button/stage action id (e.g. `"select"`,
    /// `"import"`, `"skip"`, `"skip-all"`, `"save"`, `"done"`, or `"back"` — the cooperative
    /// step-back the plugin honors by re-presenting the previous stage); `values` carries the
    /// stage's data.
    Submit {
        /// The activated action's id.
        action: String,
        /// The stage's response data; every field is optional so one type serves every stage.
        #[serde(default)]
        values: ResponseValues,
    },
    /// The user cancelled the session.
    Cancel,
}

/// The typed values a [`ImportResponse::Submit`] carries. Every field is optional — the plugin reads
/// only the ones its stage's action defines (a records `select` reads `row`; a confirm `import` reads
/// `fields`/`region`/`confidence`; a save reads `save`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseValues {
    /// The picked record's [`RecordRow::id`] (records stage `select`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<String>,
    /// The edited field values, keyed by [`PayloadField::key`] (confirm stage `import`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldValue>,
    /// The confirmed line region (confirm stage `import`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<CropRegion>,
    /// The chosen confidence (confirm stage `import`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<PayloadConfidence>,
    /// The chosen filing target (save-scan stage `save`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save: Option<SaveSuggestion>,
    /// A manual or edited scan URL (confirm stage `import`). Prefilled from [`ScanRef::path`] and
    /// editable, so a record whose scan the plugin could not resolve (e.g. a 1910 census page) can
    /// still be filed by pasting the scanned-page URL. When set, the plugin downloads this instead of
    /// its auto-resolved URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_url: Option<String>,
}

/// One edited field value in a submission: the field's [`PayloadField::key`] and its edited value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldValue {
    /// The field key this value is for (matches a [`PayloadField::key`]).
    pub key: String,
    /// The user's edited value.
    pub value: String,
}

/// A failure to parse an assisted-import payload or response against the contract (ADR 0017 §5).
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ImportPayloadError {
    /// The JSON was not valid or did not match the contract (bad JSON, missing field, unknown `kind`).
    #[error("malformed assisted-import payload: {0}")]
    Malformed(String),
}

/// Parses a plugin-supplied JSON document into an [`ImportPayload`].
///
/// # Errors
///
/// [`ImportPayloadError::Malformed`] if `json` is not valid JSON or does not match the contract (an
/// unknown `kind` is malformed; unknown object fields are tolerated).
pub fn parse_payload(json: &str) -> Result<ImportPayload, ImportPayloadError> {
    serde_json::from_str(json).map_err(|error| ImportPayloadError::Malformed(error.to_string()))
}

/// Parses a wizard response JSON document into an [`ImportResponse`] (for the plugin side and tests).
///
/// # Errors
///
/// [`ImportPayloadError::Malformed`] if `json` is not valid JSON or does not match the contract.
pub fn parse_response(json: &str) -> Result<ImportResponse, ImportPayloadError> {
    serde_json::from_str(json).map_err(|error| ImportPayloadError::Malformed(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        ConfirmRecord, ConfirmRecordPayload, CropRegion, FieldValue, ImportPayload, ImportResponse, ImportedRecord,
        PayloadAction, PayloadConfidence, PayloadField, ProvenancePreview, RecordRow, RecordsPayload, ResponseValues,
        SaveScanPayload, SaveSuggestion, ScanRef, SourceRef, SummaryPayload, parse_payload, parse_response,
    };

    fn records_payload() -> ImportPayload {
        ImportPayload::Records(RecordsPayload {
            source: SourceRef {
                title: "Folketelling 1920 for Greipstad".to_owned(),
                url: "https://www.digitalarkivet.no/census/rural-residence/bf01052209001842".to_owned(),
            },
            records: vec![
                RecordRow {
                    id: "pf01052209001842".to_owned(),
                    label: "Ole Andersen".to_owned(),
                    detail: Some("1874 · hovedperson".to_owned()),
                },
                RecordRow {
                    id: "pf01052209001843".to_owned(),
                    label: "Anna Tobiasdatter".to_owned(),
                    detail: None,
                },
            ],
        })
    }

    fn confirm_payload() -> ImportPayload {
        ImportPayload::ConfirmRecord(ConfirmRecordPayload {
            record: ConfirmRecord {
                fields: vec![PayloadField {
                    key: "name".to_owned(),
                    label: "field-name".to_owned(),
                    value: "Asbjørn Olsen".to_owned(),
                }],
                scan: Some(ScanRef {
                    path: Some("media/02_folketelling/1920/scan.jpg".to_owned()),
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
                    citation: "URN:NBN:no-a1450-fs10771822220997 · retrieved 2026-07-19".to_owned(),
                    external_id_url: "https://www.digitalarkivet.no/census/person/pf01052209001842".to_owned(),
                    confidence: PayloadConfidence::Low,
                },
            },
            actions: vec![
                PayloadAction {
                    id: "import".to_owned(),
                    label: "action-import".to_owned(),
                },
                PayloadAction {
                    id: "skip".to_owned(),
                    label: "action-skip".to_owned(),
                },
            ],
        })
    }

    fn save_scan_payload() -> ImportPayload {
        ImportPayload::SaveScan(SaveScanPayload {
            suggested: SaveSuggestion {
                category: "02_folketelling".to_owned(),
                subfolder: "1920".to_owned(),
                filename: "1920_greipstad_folketelling_asbjorn-olsen.jpg".to_owned(),
            },
            categories: vec!["01_kirkebok".to_owned(), "02_folketelling".to_owned()],
        })
    }

    fn summary_payload() -> ImportPayload {
        ImportPayload::Summary(SummaryPayload {
            imported: vec![ImportedRecord {
                human_id: "I0003".to_owned(),
                label: "Asbjørn Olsen".to_owned(),
            }],
            skipped: 1,
        })
    }

    #[test]
    fn every_payload_kind_round_trips() {
        for payload in [
            records_payload(),
            confirm_payload(),
            save_scan_payload(),
            summary_payload(),
        ] {
            let json = serde_json::to_string(&payload).expect("serialize");
            assert_eq!(parse_payload(&json).expect("parse"), payload);
        }
    }

    #[test]
    fn payload_is_internally_tagged_kebab_case() {
        let json = serde_json::to_value(confirm_payload()).expect("to value");
        assert_eq!(json["kind"], "confirm-record");
        assert_eq!(json["record"]["provenance"]["confidence"], "low");
        let records = serde_json::to_value(records_payload()).expect("to value");
        assert_eq!(records["kind"], "records");
    }

    #[test]
    fn unknown_kind_is_malformed() {
        let result = parse_payload(r#"{"kind":"bogus-stage","x":1}"#);
        assert!(matches!(result, Err(super::ImportPayloadError::Malformed(_))));
    }

    #[test]
    fn unknown_fields_are_tolerated_additively() {
        // A newer plugin adds a field an older wizard does not know; parsing must not fail.
        let json = r#"{"kind":"records","source":{"title":"T","url":"https://x/","extra":1},
            "records":[{"id":"a","label":"Ola","future":true}],"trailing":"ignored"}"#;
        let ImportPayload::Records(records) = parse_payload(json).expect("parse") else {
            panic!("expected records");
        };
        assert_eq!(records.records[0].label, "Ola");
        assert_eq!(records.records[0].detail, None);
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse_payload("{ not json").is_err());
        assert!(parse_payload(r#"{"source":{"title":"T"}}"#).is_err());
    }

    #[test]
    fn missing_required_field_is_malformed() {
        // A confirm-record without its `provenance` block cannot be rendered.
        let json = r#"{"kind":"confirm-record","record":{"fields":[]},"actions":[]}"#;
        assert!(parse_payload(json).is_err());
    }

    #[test]
    fn submit_response_round_trips() {
        let response = ImportResponse::Submit {
            action: "import".to_owned(),
            values: ResponseValues {
                fields: vec![FieldValue {
                    key: "name".to_owned(),
                    value: "Asbjørn Olsen".to_owned(),
                }],
                region: Some(CropRegion {
                    left: 4,
                    top: 47,
                    width: 92,
                    height: 9,
                }),
                confidence: Some(PayloadConfidence::Low),
                ..ResponseValues::default()
            },
        };
        let json = serde_json::to_string(&response).expect("serialize");
        assert_eq!(parse_response(&json).expect("parse"), response);
    }

    #[test]
    fn import_response_carries_an_edited_scan_url() {
        let json =
            r#"{"kind":"submit","action":"import","values":{"scan_url":"https://media.digitalarkivet.no/image/abc"}}"#;
        let ImportResponse::Submit { action, values } = parse_response(json).expect("parse") else {
            panic!("expected submit");
        };
        assert_eq!(action, "import");
        assert_eq!(
            values.scan_url.as_deref(),
            Some("https://media.digitalarkivet.no/image/abc")
        );
        // Absent by default, and skipped on serialize when None.
        let none = ResponseValues::default();
        let out = serde_json::to_string(&none).expect("serialize");
        assert!(!out.contains("scan_url"), "scan_url is skipped when none: {out}");
    }

    #[test]
    fn back_action_round_trips() {
        let json = serde_json::to_string(&submit_back()).expect("serialize");
        assert_eq!(parse_response(&json).expect("parse"), submit_back());
    }

    fn submit_back() -> ImportResponse {
        ImportResponse::Submit {
            action: "back".to_owned(),
            values: ResponseValues::default(),
        }
    }

    #[test]
    fn cancel_response_round_trips() {
        let json = serde_json::to_string(&ImportResponse::Cancel).expect("serialize");
        assert_eq!(parse_response(&json).expect("parse"), ImportResponse::Cancel);
    }

    #[test]
    fn select_response_carries_only_the_row() {
        let json = r#"{"kind":"submit","action":"select","values":{"row":"pf01052209001843"}}"#;
        let ImportResponse::Submit { action, values } = parse_response(json).expect("parse") else {
            panic!("expected submit");
        };
        assert_eq!(action, "select");
        assert_eq!(values.row.as_deref(), Some("pf01052209001843"));
        assert!(values.fields.is_empty());
    }

    #[test]
    fn empty_submit_values_default() {
        let json = r#"{"kind":"submit","action":"done"}"#;
        let ImportResponse::Submit { values, .. } = parse_response(json).expect("parse") else {
            panic!("expected submit");
        };
        assert_eq!(values, ResponseValues::default());
    }
}
