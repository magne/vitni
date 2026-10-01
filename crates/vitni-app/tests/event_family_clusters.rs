//! Event and family clusters (ADR 0039 §1, §4, §5): identity decisions on events and families, judged
//! between clusters, and a merged cluster that reads as one record until its merge is undone.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;

use uuid::Uuid;
use vitni_app::{
    ActivityDetail, AppDefaults, AppError, DateParts, IdentityDecision, MatchBand, MatchableKind, MutationMeta,
    NewEvent, NewParticipation, NewPerson, OperatorConfig, PairDecision, PersonNameParts, Provenance, Session,
    Workspace, WorkspaceDefaults, add_partner, assert_event_date, assert_participation, change_log_for_event,
    change_log_for_family, create_event, create_family, create_person, distinguish_events, distinguish_families,
    event_pair_decision, family_pair_decision, merge_events, merge_families, similar_pairs, undo_event_assertion,
    undo_event_distinction_and_merge, undo_family_assertion, undo_family_distinction_and_merge,
};
use vitni_core::enums::{EventType, EvidenceLevel, ParticipantRole};
use vitni_core::event::EventError;
use vitni_core::family::FamilyError;
use vitni_core::ids::AgentId;
use vitni_core::matching::{CultureId, EngineVersion, MatchEvidence};
use vitni_core::provenance::{Agent, AgentKind};

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

async fn workspace() -> (Workspace, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, dir)
}

async fn person(ws: &Workspace, given: &str) -> String {
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some("Hansen".to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create person")
}

/// A marriage in 1850, returning its `human_id`.
async fn marriage(ws: &Workspace) -> String {
    let new = NewEvent {
        human_id: None,
        event_type: EventType::Marriage,
    };
    let event = create_event(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create event");
    let date = DateParts {
        year: 1850,
        month: Some(6),
        day: Some(12),
    };
    assert_event_date(ws, &session(), &event, date, MutationMeta::default())
        .await
        .expect("date");
    event
}

async fn participate(ws: &Workspace, person: &str, event: &str, role: ParticipantRole) {
    assert_participation(
        ws,
        &session(),
        person,
        event,
        NewParticipation::with_role(role),
        MutationMeta::default(),
    )
    .await
    .expect("participate");
}

async fn family(ws: &Workspace, partners: &[&str]) -> String {
    let family = create_family(ws, &session(), Provenance::default(), &[])
        .await
        .expect("create family");
    for partner in partners {
        add_partner(ws, &session(), &family, partner, MutationMeta::default())
            .await
            .expect("partner");
    }
    family
}

/// The `kind` clusters as `(member, root)` pairs of `human_id`s.
async fn clusters(ws: &Workspace, kind: MatchableKind) -> BTreeSet<(String, String)> {
    let store = ws.store();
    let mut pairs = BTreeSet::new();
    for link in store.identity_links(kind).await.expect("links") {
        let human_id = async |id: &str| {
            store
                .human_id_of(kind.as_str(), id)
                .await
                .expect("human id")
                .expect("indexed")
        };
        pairs.insert((human_id(&link.member).await, human_id(&link.root).await));
    }
    pairs
}

fn pairs(list: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    list.iter().map(|(m, r)| ((*m).to_owned(), (*r).to_owned())).collect()
}

fn evidence() -> MatchEvidence {
    MatchEvidence {
        score_bp: 9300,
        band: MatchBand::Probable,
        engine: EngineVersion(4),
        cultures: vec![CultureId::new("universal")],
        features: Vec::new(),
    }
}

async fn merge_e(ws: &Workspace, surviving: &str, merged: &str) {
    merge_events(ws, &session(), surviving, merged, IdentityDecision::default())
        .await
        .expect("merge events");
}

async fn merge_f(ws: &Workspace, surviving: &str, merged: &str) {
    merge_families(ws, &session(), surviving, merged, IdentityDecision::default())
        .await
        .expect("merge families");
}

fn event_decided<T: std::fmt::Debug>(result: &Result<T, AppError>) -> bool {
    matches!(result, Err(AppError::EventDomain(EventError::IdentityDecided { .. })))
}

fn family_decided<T: std::fmt::Debug>(result: &Result<T, AppError>) -> bool {
    matches!(result, Err(AppError::FamilyDomain(FamilyError::IdentityDecided { .. })))
}

#[tokio::test]
async fn merging_into_a_member_event_targets_its_root() {
    let (ws, _dir) = workspace().await;
    let (a, b, c) = (marriage(&ws).await, marriage(&ws).await, marriage(&ws).await);
    merge_e(&ws, &a, &b).await;
    merge_e(&ws, &b, &c).await;
    assert_eq!(
        clusters(&ws, MatchableKind::Event).await,
        pairs(&[(&b, &a), (&c, &a)]),
        "C joins A's cluster, not B's"
    );
    assert!(clusters(&ws, MatchableKind::Person).await.is_empty());
}

#[tokio::test]
async fn an_event_pair_decided_either_way_cannot_be_decided_again() {
    let (ws, _dir) = workspace().await;
    let (a, b) = (marriage(&ws).await, marriage(&ws).await);
    merge_e(&ws, &a, &b).await;
    let again = merge_events(&ws, &session(), &b, &a, IdentityDecision::default()).await;
    assert!(event_decided(&again), "{again:?}");
    let distinct = distinguish_events(&ws, &session(), &a, &b, IdentityDecision::default()).await;
    assert!(event_decided(&distinct), "{distinct:?}");
    assert_eq!(
        event_pair_decision(&ws, &a, &b).await.expect("decision"),
        Some(PairDecision::SameCluster)
    );
}

#[tokio::test]
async fn a_distinction_against_any_event_of_a_cluster_blocks_the_merge_until_undone() {
    let (ws, _dir) = workspace().await;
    let (a, b, c) = (marriage(&ws).await, marriage(&ws).await, marriage(&ws).await);
    merge_e(&ws, &a, &b).await;
    distinguish_events(&ws, &session(), &c, &b, IdentityDecision::default())
        .await
        .expect("distinguish");
    assert_eq!(
        event_pair_decision(&ws, &a, &c).await.expect("decision"),
        Some(PairDecision::Distinct)
    );
    let blocked = merge_events(&ws, &session(), &a, &c, IdentityDecision::default()).await;
    assert!(event_decided(&blocked), "{blocked:?}");

    let merged = undo_event_distinction_and_merge(&ws, &session(), &a, &c, IdentityDecision::default())
        .await
        .expect("undo and merge");
    assert_eq!(merged.survivor.human_id, a);
    assert_eq!(merged.merged_human_id, c);
    assert_eq!(clusters(&ws, MatchableKind::Event).await, pairs(&[(&b, &a), (&c, &a)]));
}

#[tokio::test]
async fn an_event_cannot_be_merged_with_itself() {
    let (ws, _dir) = workspace().await;
    let a = marriage(&ws).await;
    let result = merge_events(&ws, &session(), &a, &a, IdentityDecision::default()).await;
    assert!(
        matches!(result, Err(AppError::EventDomain(EventError::MergeConflict { .. }))),
        "{result:?}"
    );
    let unknown = merge_events(&ws, &session(), &a, "E9999", IdentityDecision::default()).await;
    assert!(matches!(unknown, Err(AppError::EventNotFound(_))), "{unknown:?}");
}

#[tokio::test]
async fn undoing_an_event_merge_splits_the_cluster() {
    let (ws, _dir) = workspace().await;
    let (a, b) = (marriage(&ws).await, marriage(&ws).await);
    merge_e(&ws, &a, &b).await;
    let entry = change_log_for_event(&ws, &a)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == "EventsMerged")
        .expect("merge logged");
    undo_event_assertion(&ws, &session(), &a, &entry.assertion_id, None)
        .await
        .expect("undo");
    assert!(clusters(&ws, MatchableKind::Event).await.is_empty());
    assert_eq!(event_pair_decision(&ws, &a, &b).await.expect("decision"), None);
}

#[tokio::test]
async fn an_event_decision_records_the_assessment_it_was_made_on() {
    let (ws, _dir) = workspace().await;
    let (a, b) = (marriage(&ws).await, marriage(&ws).await);
    let decision = IdentityDecision {
        provenance: Provenance::default(),
        assessment: Some(evidence()),
    };
    distinguish_events(&ws, &session(), &a, &b, decision)
        .await
        .expect("distinguish");
    let entry = change_log_for_event(&ws, &a)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == "EventsDistinguished")
        .expect("distinction logged");
    assert_eq!(
        entry.detail,
        Some(ActivityDetail::IdentityDecision { assessment: evidence() })
    );
    assert!(entry.can_undo);
}

#[tokio::test]
async fn family_decisions_are_judged_between_clusters() {
    let (ws, _dir) = workspace().await;
    let (ole, kari) = (person(&ws, "Ole").await, person(&ws, "Kari").await);
    let a = family(&ws, &[&ole, &kari]).await;
    let b = family(&ws, &[&ole, &kari]).await;
    let c = family(&ws, &[&ole]).await;
    merge_f(&ws, &a, &b).await;
    assert_eq!(clusters(&ws, MatchableKind::Family).await, pairs(&[(&b, &a)]));
    let again = merge_families(&ws, &session(), &b, &a, IdentityDecision::default()).await;
    assert!(family_decided(&again), "{again:?}");

    distinguish_families(&ws, &session(), &c, &b, IdentityDecision::default())
        .await
        .expect("distinguish");
    assert_eq!(
        family_pair_decision(&ws, &a, &c).await.expect("decision"),
        Some(PairDecision::Distinct)
    );
    let blocked = merge_families(&ws, &session(), &a, &c, IdentityDecision::default()).await;
    assert!(family_decided(&blocked), "{blocked:?}");
    undo_family_distinction_and_merge(&ws, &session(), &a, &c, IdentityDecision::default())
        .await
        .expect("undo and merge");
    assert_eq!(clusters(&ws, MatchableKind::Family).await, pairs(&[(&b, &a), (&c, &a)]));

    let entry = change_log_for_family(&ws, &a)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == "FamiliesMerged")
        .expect("merge logged");
    undo_family_assertion(&ws, &session(), &a, &entry.assertion_id, None)
        .await
        .expect("undo");
    assert_eq!(
        clusters(&ws, MatchableKind::Family).await,
        pairs(&[(&b, &a)]),
        "undoing the newest merge splits C off again"
    );
}

fn sorted(first: &str, second: &str) -> (String, String) {
    let (low, high) = if first < second {
        (first, second)
    } else {
        (second, first)
    };
    (low.to_owned(), high.to_owned())
}

#[tokio::test]
async fn a_decided_event_pair_is_not_proposed_as_a_duplicate() {
    let (ws, _dir) = workspace().await;
    let (ole, kari) = (person(&ws, "Ole").await, person(&ws, "Kari").await);
    let (a, b, c) = (marriage(&ws).await, marriage(&ws).await, marriage(&ws).await);
    for event in [&a, &b, &c] {
        participate(&ws, &ole, event, ParticipantRole::Primary).await;
        participate(&ws, &kari, event, ParticipantRole::Primary).await;
    }
    let proposed = async || -> BTreeSet<(String, String)> {
        let mut found = BTreeSet::new();
        for pair in similar_pairs(&ws, MatchableKind::Event, MatchBand::Possible)
            .await
            .expect("similar pairs")
        {
            found.insert(sorted(&pair.a.human_id, &pair.b.human_id));
        }
        found
    };
    assert!(proposed().await.contains(&sorted(&a, &b)), "{:?}", proposed().await);

    merge_e(&ws, &a, &b).await;
    distinguish_events(&ws, &session(), &c, &b, IdentityDecision::default())
        .await
        .expect("distinguish");
    assert!(
        proposed().await.is_empty(),
        "A and B are one cluster, and C is distinct from it: {:?}",
        proposed().await
    );
}
