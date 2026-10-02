//! The similar-record intents (ADR 0038 §8): the hint a record being created raises, *Find similar* on
//! a stored record, and the draft-against-stored compare view.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    Agent, AgentId, AgentKind, AppDefaults, DateParts, DraftRecord, EvidenceLevel, MatchableKind, MutationMeta,
    NewPerson, OperatorConfig, PersonNameParts, Provenance, Session, Workspace, WorkspaceDefaults, create_person,
    record_birth,
};
use vitni_ui::{Category, Intent, IntentOutcome, Localizer, dispatch};

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

fn born(year: i32) -> DateParts {
    DateParts {
        year,
        month: None,
        day: None,
    }
}

async fn person(ws: &Workspace, given: &str, surname: &str, year: i32) -> String {
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some(surname.to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    let human_id = create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("person");
    record_birth(ws, &session(), &human_id, born(year), MutationMeta::default())
        .await
        .expect("birth");
    human_id
}

/// A workspace holding Guldbrand Olsen (b. 1852) and Kari Hansen (b. 1900).
async fn workspace() -> (Workspace, tempfile::TempDir, Localizer, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ws");
    Workspace::init(&path, &operator(), &AppDefaults::default(), None).expect("init");
    let ws = Workspace::open(&path, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    let guldbrand = person(&ws, "Guldbrand", "Olsen", 1852).await;
    person(&ws, "Kari", "Hansen", 1900).await;
    let loc = Localizer::for_workspace(&path, None);
    (ws, dir, loc, guldbrand)
}

fn gulbrand() -> DraftRecord {
    DraftRecord::Person {
        name: PersonNameParts::simple(Some("Gulbrand".to_owned()), Some("Olsøn".to_owned())),
        birth: Some(born(1852)),
    }
}

#[tokio::test]
async fn typing_a_stored_persons_name_and_birth_year_raises_the_hint() {
    let (ws, _dir, loc, guldbrand) = workspace().await;
    let intent = Intent::SimilarToDraft { draft: gulbrand() };
    let IntentOutcome::Similar(similar) = dispatch(&ws, &loc, &intent).await.expect("dispatch") else {
        panic!("the hint intent loads the similar records");
    };
    let [hit] = similar.hits.as_slice() else {
        panic!("one similar person: {similar:?}");
    };
    assert_eq!(hit.record.human_id, guldbrand);
    assert_eq!(hit.record.category, Category::People);
    assert_eq!(hit.record.label, "Guldbrand Olsen");
    assert!(
        hit.line.contains("Guldbrand Olsen") && hit.line.contains(&guldbrand),
        "{}",
        hit.line
    );
    assert_ne!(hit.reasons, Vec::<String>::new(), "the hint says why");
}

#[tokio::test]
async fn find_similar_lists_the_records_like_a_stored_one() {
    let (ws, _dir, loc, guldbrand) = workspace().await;
    let gulbrand = person(&ws, "Gulbrand", "Olsøn", 1852).await;
    let intent = Intent::FindSimilar {
        kind: MatchableKind::Person,
        human_id: gulbrand,
    };
    let IntentOutcome::Similar(similar) = dispatch(&ws, &loc, &intent).await.expect("dispatch") else {
        panic!("find similar loads the similar records");
    };
    let found: Vec<&str> = similar.hits.iter().map(|hit| hit.record.human_id.as_str()).collect();
    assert_eq!(found, [guldbrand.as_str()]);
}

#[tokio::test]
async fn a_draft_compares_with_a_stored_record_side_by_side() {
    let (ws, _dir, loc, guldbrand) = workspace().await;
    let intent = Intent::DraftCompare {
        draft: gulbrand(),
        right: guldbrand.clone(),
    };
    let IntentOutcome::MatchCompare(compare) = dispatch(&ws, &loc, &intent).await.expect("dispatch") else {
        panic!("the draft compare loads the compare view");
    };
    assert_eq!(compare.left.label, "Gulbrand Olsøn");
    assert_eq!(compare.right.human_id, guldbrand);
    assert_eq!(compare.right.label, "Guldbrand Olsen");
    assert_ne!(compare.rows, Vec::new(), "one row per compared term");
    assert_eq!(compare.earlier_decision, None);
}
