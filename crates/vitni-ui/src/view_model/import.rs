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

use vitni_app::{MatchQuestion, MatchReply, MatchableKind, PairAnswer};

use crate::i18n::Localizer;
use crate::import_payload::{
    ConfirmRecordPayload, ImportPayload, ImportPayloadError, RecordsPayload, SaveScanPayload, SummaryPayload,
    parse_payload,
};
use crate::navigation::PairJudgment;
use crate::shortcuts::CompareDecision;
use crate::view_model::{CompareSide, MatchCompareVm};

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
    /// The host's own stage: a possible match of the record being imported (ADR 0040 §4).
    Match(Box<MatchStageVm>),
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

    /// Moves the session to the host's match stage for `question`.
    pub fn on_match(&mut self, question: &MatchQuestion, loc: &Localizer) {
        self.stage = ImportStage::Match(Box::new(MatchStageVm::build(question, loc)));
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

/// The host's match stage (ADR 0040 §4): one stored record the engine judged possibly the same as an
/// entity of the record being imported, compared side by side — the stored record on the left, the side
/// a *Same* keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchStageVm {
    /// The kind of both records.
    pub kind: MatchableKind,
    /// The pair.
    pub compare: MatchCompareVm,
    /// Which of the record's entities with possible matches this is, from 1.
    pub position: usize,
    /// How many of the record's entities have possible matches.
    pub total: usize,
}

impl MatchStageVm {
    /// Builds the stage from the host's question. The incoming record has no id yet, so its badge reads
    /// as new.
    #[must_use]
    pub fn build(question: &MatchQuestion, loc: &Localizer) -> Self {
        let incoming_badge = loc.match_incoming_badge();
        let stored = CompareSide {
            human_id: &question.candidate.human_id,
            label: &question.candidate_label,
            origin: question.candidate_origin.as_ref(),
            media: &[],
        };
        let incoming = CompareSide {
            human_id: &incoming_badge,
            label: &question.incoming_label,
            origin: question.incoming_origin.as_ref(),
            media: &[],
        };
        Self {
            kind: question.kind,
            compare: MatchCompareVm::build(stored, incoming, &question.assessment, loc),
            position: question.position,
            total: question.total,
        }
    }

    /// The reply to the host for `decision`, recording `judgment` with the assessment the stage showed.
    #[must_use]
    pub fn reply(&self, decision: CompareDecision, judgment: PairJudgment) -> MatchReply {
        let judgment = PairJudgment {
            assessment: Some(self.compare.assessment.clone()),
            ..judgment
        };
        let answer = match decision {
            CompareDecision::Same => PairAnswer::Same(judgment.decision()),
            CompareDecision::Distinct => PairAnswer::Distinct(judgment.decision()),
            CompareDecision::Later => PairAnswer::Later,
        };
        MatchReply::Pair(Box::new(answer))
    }
}

#[cfg(test)]
mod tests {
    use super::{ImportSession, ImportStage, MatchStageVm};
    use crate::i18n::Localizer;
    use crate::navigation::PairJudgment;
    use crate::presentation::ConfidenceLevel;
    use crate::shortcuts::CompareDecision;
    use vitni_app::{
        AggRef, DatasetId, Feature, FeatureComparison, FeatureValue, ImportRunId, MatchAssessment, MatchBand,
        MatchQuestion, MatchReply, MatchableKind, Outcome, PairAnswer, RecordOrigin,
    };

    fn question() -> MatchQuestion {
        let given = |value: &str| Some(FeatureValue::Name(value.to_owned()));
        MatchQuestion {
            kind: MatchableKind::Person,
            incoming_label: "Ola Eksempelsen Fjellstue".to_owned(),
            incoming_origin: Some(RecordOrigin {
                dataset: DatasetId::global("digitalarkivet"),
                record: "pf01099901000101".to_owned(),
                item: None,
                digest: None,
                run: ImportRunId::from_uuid(uuid::Uuid::from_u128(1)),
            }),
            candidate: AggRef {
                human_id: "I0001".to_owned(),
                id: "00000000-0000-0000-0000-000000000001".to_owned(),
            },
            candidate_label: "Ola Fjellstue".to_owned(),
            candidate_origin: None,
            assessment: MatchAssessment {
                score: 0.8,
                band: MatchBand::Possible,
                features: vec![FeatureComparison {
                    feature: Feature::GivenName,
                    outcome: Outcome::Agree,
                    weight: 3.0,
                    left: given("Ola"),
                    right: given("Ola Eksempelsen"),
                }],
                cultures: Vec::new(),
                parts: Vec::new(),
                engine: vitni_app::ENGINE_VERSION,
            },
            position: 1,
            total: 2,
            group: None,
        }
    }

    #[test]
    fn a_match_question_compares_the_stored_record_left_with_the_incoming_one_right() {
        let loc = Localizer::for_test("en");
        let mut session = ImportSession::new();
        session.on_match(&question(), &loc);
        let ImportStage::Match(stage) = session.stage() else {
            panic!("expected the match stage, got {:?}", session.stage());
        };
        assert_eq!((stage.position, stage.total), (1, 2));
        assert_eq!(
            (stage.compare.left.human_id.as_str(), stage.compare.left.label.as_str()),
            ("I0001", "Ola Fjellstue")
        );
        assert_eq!(stage.compare.right.human_id, "new");
        assert_eq!(stage.compare.right.label, "Ola Eksempelsen Fjellstue");
        assert!(stage.compare.left.origin.is_none());
        assert!(
            stage.compare.right.origin.is_some(),
            "the incoming record names its origin"
        );
        assert_eq!(stage.compare.rows[0].right.as_deref(), Some("Ola Eksempelsen"));
        assert!(!session.is_finished());
    }

    #[test]
    fn a_decision_replies_with_the_judgment_and_the_assessment_shown() {
        let stage = MatchStageVm::build(&question(), &Localizer::for_test("en"));
        let judgment = PairJudgment {
            rationale: Some(" same farm ".to_owned()),
            confidence: Some(ConfidenceLevel::High),
            assessment: None,
        };
        let MatchReply::Pair(answer) = stage.reply(CompareDecision::Same, judgment.clone()) else {
            panic!("expected a pair answer");
        };
        let PairAnswer::Same(decision) = *answer else {
            panic!("expected same, got {answer:?}");
        };
        assert_eq!(decision.provenance.rationale.as_deref(), Some("same farm"));
        assert_eq!(decision.assessment, Some(stage.compare.assessment.clone()));
        assert!(matches!(
            stage.reply(CompareDecision::Distinct, judgment.clone()),
            MatchReply::Pair(answer) if matches!(*answer, PairAnswer::Distinct(_))
        ));
        assert_eq!(
            stage.reply(CompareDecision::Later, judgment),
            MatchReply::Pair(Box::new(PairAnswer::Later))
        );
    }

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
