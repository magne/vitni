//! Edits on a merged cluster's detail are routed to the record that owns the row (ADR 0039 §5): an
//! undo or a supersede of a member's row is written to that member's stream, while the pane keeps
//! showing the root.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::DecidableKind;
use vitni_app::{
    Agent, AgentId, AgentKind, AppDefaults, EvidenceLevel, FactType, IdentityDecision, MutationMeta, NewFact,
    NewPerson, OperatorConfig, PairDecision, PersonNameParts, Provenance, Session, Workspace, WorkspaceDefaults,
    assert_fact, create_person, distinguish_persons, merge_persons, show_person,
};
use vitni_ui::{
    DecideMatch, Intent, IntentOutcome, Localizer, MatchDecision, PairJudgment, PersonEdit, ProvenanceDraft, dispatch,
    dispatch_decide_match, dispatch_person_edit,
};

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

/// A workspace where `member` (holding one occupation) is merged into `root`.
async fn merged() -> (Workspace, tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ws");
    Workspace::init(&path, &operator(), &AppDefaults::default(), None).expect("init");
    let ws = Workspace::open(&path, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    let mut ids = Vec::new();
    for given in ["Ole", "Ola"] {
        let new = NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(
                Some(given.to_owned()),
                Some("Hansen".to_owned()),
            )),
            evidence_level: EvidenceLevel::Persona,
            external_ids: Vec::new(),
        };
        ids.push(
            create_person(&ws, &session(), new, Provenance::default(), &[])
                .await
                .expect("person"),
        );
    }
    let (root, member) = (ids[0].clone(), ids[1].clone());
    let fact = NewFact {
        fact_type: FactType::Occupation,
        value: Some("Fisherman".to_owned()),
        date: None,
    };
    assert_fact(&ws, &session(), &member, fact, MutationMeta::default())
        .await
        .expect("fact");
    merge_persons(&ws, &session(), &root, &member, IdentityDecision::default())
        .await
        .expect("merge");
    (ws, dir, root, member)
}

#[tokio::test]
async fn undoing_a_members_row_from_the_root_writes_to_the_member() {
    let (ws, _dir, root, member) = merged().await;
    let fact = show_person(&ws, &root).await.expect("show").expect("root").facts[0]
        .assertion_id
        .clone();

    let target = dispatch_person_edit(
        &ws,
        &session(),
        &PersonEdit::UndoAssertion {
            human_id: root.clone(),
            assertion_id: fact,
        },
        &ProvenanceDraft::default(),
    )
    .await
    .expect("undo routed to the member");
    assert_eq!(target, root, "the pane reloads the root");
    let checked = show_person(&ws, &root).await.expect("show").expect("root").facts;
    assert!(checked.is_empty(), "{checked:?}");
    let _ = member;
}

#[tokio::test]
async fn superseding_a_members_row_from_the_root_replaces_it_on_the_member() {
    let (ws, _dir, root, member) = merged().await;
    let summary = show_person(&ws, &root).await.expect("show").expect("root");
    let prov = ProvenanceDraft {
        supersedes: Some(summary.facts[0].assertion_id.clone()),
        ..ProvenanceDraft::default()
    };
    dispatch_person_edit(
        &ws,
        &session(),
        &PersonEdit::AssertFact {
            human_id: root.clone(),
            fact_type: FactType::Occupation,
            value: Some("Sailor".to_owned()),
        },
        &prov,
    )
    .await
    .expect("supersede routed to the member");
    let after = show_person(&ws, &root).await.expect("show").expect("root");
    let jobs: Vec<_> = after
        .facts
        .iter()
        .map(|fact| {
            (
                fact.fact.value.clone().unwrap_or_default(),
                after.owner_of(&fact.assertion_id).to_owned(),
            )
        })
        .collect();
    assert_eq!(jobs, [("Sailor".to_owned(), member)]);
}

#[tokio::test]
async fn the_compare_view_shows_an_earlier_distinction_and_undoing_it_merges() {
    let (ws, dir, root, member) = merged().await;
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some("Ole".to_owned()),
            Some("Hansen".to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    let other = create_person(&ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("person");
    distinguish_persons(&ws, &session(), &other, &member, IdentityDecision::default())
        .await
        .expect("distinguish");
    let loc = Localizer::for_workspace(&dir.path().join("ws"), None);

    let compare = Intent::MatchCompare {
        kind: DecidableKind::Person,
        left: root.clone(),
        right: other.clone(),
    };
    let IntentOutcome::MatchCompare(vm) = dispatch(&ws, &loc, &compare).await.expect("compare") else {
        panic!("the compare intent loads the compare view");
    };
    assert_eq!(vm.earlier_decision, Some(PairDecision::Distinct));

    let request = DecideMatch {
        kind: DecidableKind::Person,
        left: root.clone(),
        right: other.clone(),
        decision: MatchDecision::UndoDistinctionAndSame,
        judgment: PairJudgment::default(),
    };
    dispatch_decide_match(&ws, &session(), &loc, &request)
        .await
        .expect("undo and merge");
    let merged_into: Vec<_> = show_person(&ws, &root)
        .await
        .expect("show")
        .expect("root")
        .merged
        .into_iter()
        .map(|persona| persona.human_id)
        .collect();
    assert!(merged_into.contains(&other), "{merged_into:?}");
}
