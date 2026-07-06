//! Dispatch-layer provenance round trip: a `ProvenanceDraft` filled on an edit form reaches the
//! change log through `dispatch_*_edit`, proving the UI intent layer threads the operator's intent
//! (rationale · confidence · citations · evidence analysis) into every mutation, not just the two
//! it carried before (PR25).
//!
//! These run against a real on-disk workspace over `genealogy-app`'s public surface only — the
//! presentation layer never names a `genealogy-core` type, even in tests (ADR 0008).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use genealogy_app::{
    Agent, AgentId, AgentKind, AppDefaults, ChangeLogEntry, EvidenceLevel, NewCitation, NewNote, NewPerson, NewSource,
    OperatorConfig, PersonNameParts, Provenance, Session, Workspace, WorkspaceDefaults, change_log_for_person,
    create_citation, create_note, create_person, create_source,
};
use genealogy_ui::{
    ConfidenceLevel, EvidenceKind, InformationKind, PersonEdit, ProvenanceDraft, SourceQuality, dispatch_edit,
};
use uuid::Uuid;

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn session() -> Session {
    Session::new(Agent {
        kind: AgentKind::Human,
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
    })
}

async fn setup() -> (Workspace, Session, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, session(), dir)
}

/// A person with no initial name, so the only `NameAsserted` in the log is the one under test.
async fn person(ws: &Workspace, session: &Session) -> String {
    create_person(
        ws,
        session,
        NewPerson {
            human_id: None,
            name: None,
            evidence_level: EvidenceLevel::Conclusion,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("person")
}

/// Creates a source + citation and returns the citation `human_id` a draft can attach.
async fn citation(ws: &Workspace, session: &Session) -> String {
    let source = create_source(
        ws,
        session,
        NewSource {
            human_id: None,
            title: Some("Parish register".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("source");
    create_citation(
        ws,
        session,
        NewCitation {
            human_id: None,
            source,
            page: Some("f. 3".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("citation")
}

/// A draft carrying every axis of operator intent, so a passing round trip proves all four fields
/// are threaded (not just confidence, as before PR25).
fn filled_draft(citation_id: String) -> ProvenanceDraft {
    ProvenanceDraft {
        rationale: "  Baptism register gives the date  ".to_owned(),
        confidence: ConfidenceLevel::High,
        citations: vec![citation_id],
        source: Some(SourceQuality::Original),
        information: Some(InformationKind::Primary),
        evidence: Some(EvidenceKind::Direct),
    }
}

/// The entry that carries the draft's (unique) rationale.
fn entry_with_rationale<'a>(log: &'a [ChangeLogEntry], rationale: &str) -> &'a ChangeLogEntry {
    log.iter()
        .find(|entry| entry.rationale.as_deref() == Some(rationale))
        .expect("the dispatched mutation is logged with its rationale")
}

#[tokio::test]
async fn an_edit_carries_the_drafts_provenance_into_the_change_log() {
    let (ws, session, _dir) = setup().await;
    let person = person(&ws, &session).await;
    let citation = citation(&ws, &session).await;
    let draft = filled_draft(citation);

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AssertName {
            human_id: person.clone(),
            name: PersonNameParts::simple(Some("Ada".to_owned()), Some("Byron".to_owned())),
        },
        &draft,
    )
    .await
    .expect("dispatch AssertName");

    let log = change_log_for_person(&ws, &person).await.expect("log");
    let entry = entry_with_rationale(&log, "Baptism register gives the date");
    assert_eq!(entry.event_type, "NameAsserted", "the name assertion carries the draft");
    assert_eq!(
        entry.confidence,
        genealogy_app::Confidence::High,
        "confidence threads through"
    );
    assert_eq!(entry.citations.len(), 1, "the backing citation threads through");
    assert_eq!(
        entry.evidence_analysis,
        Some(genealogy_app::EvidenceAnalysis {
            source: SourceQuality::Original,
            information: InformationKind::Primary,
            evidence: EvidenceKind::Direct,
        }),
        "the evidence analysis threads through"
    );
}

#[tokio::test]
async fn an_attach_flow_carries_provenance() {
    let (ws, session, _dir) = setup().await;
    let person = person(&ws, &session).await;
    let citation = citation(&ws, &session).await;
    let note = create_note(
        &ws,
        &session,
        NewNote {
            human_id: None,
            text: Some("An estate inventory".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("note");
    let draft = filled_draft(citation);

    dispatch_edit(
        &ws,
        &session,
        &PersonEdit::AttachNote {
            human_id: person.clone(),
            note_id: note,
        },
        &draft,
    )
    .await
    .expect("dispatch AttachNote");

    let log = change_log_for_person(&ws, &person).await.expect("log");
    let entry = entry_with_rationale(&log, "Baptism register gives the date");
    assert_eq!(entry.event_type, "NoteAttached", "the note attach carries the draft");
    assert_eq!(
        entry.confidence,
        genealogy_app::Confidence::High,
        "confidence threads through"
    );
    assert_eq!(entry.citations.len(), 1, "the backing citation threads through");
}
