//! The assisted-import wizard's session state machine (ADR 0017 §5).
//!
//! [`ImportSession`] is the framework-free heart of the `Tool::Import` wizard: it holds the current
//! [`ImportStage`] and advances it as `present` payloads arrive from the plugin. One plugin invocation
//! drives the whole session (fetch → records → confirm each record → save the scan → summary), so the
//! session simply mirrors whichever stage the plugin last presented. The wizard renderer (PR8) reads
//! the stage, shows it, and sends the user's [`ImportResponse`](crate::import_payload::ImportResponse)
//! back over the channel; each new payload the plugin sends drives [`ImportSession::on_payload`].
//!
//! Malformed payloads land the session in [`ImportStage::Error`] rather than panicking (a plugin
//! could send anything), and [`ImportSession::cancel`] moves it to [`ImportStage::Cancelled`] from any
//! stage. The stages carry the parsed payload structs directly; resolving their Fluent chrome labels
//! against the plugin catalogue is the renderer's job (PR8), not this state machine's.

use crate::import_payload::{
    ConfirmRecordPayload, ImportPayload, ImportPayloadError, RecordsPayload, SaveScanPayload, SummaryPayload,
    parse_payload,
};

/// Where an assisted-import session currently is (ADR 0017 §5). It starts at [`Source`](Self::Source)
/// (awaiting the first fetch) and follows the plugin's payloads through the review stages to
/// [`Summary`](Self::Summary); [`Error`](Self::Error) and [`Cancelled`](Self::Cancelled) are terminal
/// off-ramps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportStage {
    /// The initial stage: the user enters a URL and the plugin has not presented anything yet.
    Source,
    /// The records found on the source page (`present` [`ImportPayload::Records`]).
    Records(RecordsPayload),
    /// One record under review (`present` [`ImportPayload::ConfirmRecord`]).
    Confirm(ConfirmRecordPayload),
    /// The save-scan dialog, shown once per source page (`present` [`ImportPayload::SaveScan`]).
    SaveScan(SaveScanPayload),
    /// The session summary (`present` [`ImportPayload::Summary`]).
    Summary(SummaryPayload),
    /// The plugin sent a payload the wizard could not parse.
    Error(ImportPayloadError),
    /// The user cancelled the session.
    Cancelled,
}

/// The assisted-import wizard's session: the current [`ImportStage`], advanced by incoming payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSession {
    stage: ImportStage,
}

impl Default for ImportSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportSession {
    /// A fresh session at the [`Source`](ImportStage::Source) stage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stage: ImportStage::Source,
        }
    }

    /// The current stage.
    #[must_use]
    pub fn stage(&self) -> &ImportStage {
        &self.stage
    }

    /// Whether the session has reached a terminal stage (summary, error, or cancelled) — the wizard
    /// stops awaiting further payloads.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(
            self.stage,
            ImportStage::Summary(_) | ImportStage::Error(_) | ImportStage::Cancelled
        )
    }

    /// Advances the session with a `present` payload JSON from the plugin, moving to the matching
    /// stage. A payload that does not parse against the contract moves the session to
    /// [`ImportStage::Error`] (the plugin sent something the wizard cannot render).
    pub fn on_payload(&mut self, json: &str) {
        self.stage = match parse_payload(json) {
            Ok(payload) => Self::stage_for(payload),
            Err(error) => ImportStage::Error(error),
        };
    }

    /// Cancels the session from any stage.
    pub fn cancel(&mut self) {
        self.stage = ImportStage::Cancelled;
    }

    /// Maps a parsed payload onto the stage it drives.
    fn stage_for(payload: ImportPayload) -> ImportStage {
        match payload {
            ImportPayload::Records(records) => ImportStage::Records(records),
            ImportPayload::ConfirmRecord(confirm) => ImportStage::Confirm(confirm),
            ImportPayload::SaveScan(save) => ImportStage::SaveScan(save),
            ImportPayload::Summary(summary) => ImportStage::Summary(summary),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ImportSession, ImportStage};

    #[test]
    fn a_new_session_starts_at_source() {
        let session = ImportSession::new();
        assert_eq!(*session.stage(), ImportStage::Source);
        assert!(!session.is_finished());
    }

    #[test]
    fn payloads_drive_the_stages_in_order() {
        let mut session = ImportSession::new();

        session.on_payload(
            r#"{"kind":"records","source":{"title":"1920","url":"https://x/"},
                "records":[{"id":"a","label":"Ola"}]}"#,
        );
        let ImportStage::Records(records) = session.stage() else {
            panic!("expected records, got {:?}", session.stage());
        };
        assert_eq!(records.records.len(), 1);
        assert!(!session.is_finished());

        session.on_payload(
            r#"{"kind":"confirm-record","record":{"fields":[{"key":"name","label":"field-name","value":"Ola"}],
                "provenance":{"source_title":"S","repository":"R","citation":"C","external_id_url":"https://x/1",
                "confidence":"low"}},"actions":[{"id":"import","label":"a-import"}]}"#,
        );
        let ImportStage::Confirm(confirm) = session.stage() else {
            panic!("expected confirm, got {:?}", session.stage());
        };
        assert_eq!(confirm.record.fields[0].value, "Ola");

        session.on_payload(
            r#"{"kind":"save-scan","suggested":{"category":"02_folketelling","filename":"a.jpg"},
                "categories":["02_folketelling"]}"#,
        );
        assert!(matches!(session.stage(), ImportStage::SaveScan(_)));

        session.on_payload(r#"{"kind":"summary","imported":[{"human_id":"I1","label":"Ola"}],"skipped":2}"#);
        let ImportStage::Summary(summary) = session.stage() else {
            panic!("expected summary, got {:?}", session.stage());
        };
        assert_eq!(summary.skipped, 2);
        assert!(session.is_finished());
    }

    #[test]
    fn a_malformed_payload_moves_to_the_error_stage() {
        let mut session = ImportSession::new();
        session.on_payload(r#"{"kind":"bogus"}"#);
        assert!(matches!(session.stage(), ImportStage::Error(_)));
        assert!(session.is_finished());
    }

    #[test]
    fn cancel_from_any_stage_moves_to_cancelled() {
        let mut session = ImportSession::new();
        session.on_payload(r#"{"kind":"records","source":{"title":"1920","url":"https://x/"},"records":[]}"#);
        assert!(matches!(session.stage(), ImportStage::Records(_)));
        session.cancel();
        assert_eq!(*session.stage(), ImportStage::Cancelled);
        assert!(session.is_finished());
    }
}
