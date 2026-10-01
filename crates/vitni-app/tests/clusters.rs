//! Persona clusters (ADR 0039 §4, §5): identity decisions judged between clusters, and a cluster
//! that reads as one person everywhere until its merge is undone.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;

use uuid::Uuid;
use vitni_app::{
    AppDefaults, AppError, IdentityDecision, MutationMeta, NewFact, NewPerson, OperatorConfig, PairDecision,
    PersonChangeSet, PersonNameParts, PersonTarget, Provenance, Session, Workspace, WorkspaceDefaults, ancestors,
    assert_fact, change_log_for_person, claim_owner, commit_person_change_set, create_person, create_tag,
    distinguish_persons, list_person_rows, list_persons, list_tags, merge_persons, pair_decision, set_restrictions,
    show_person, tag_person, undo_assertion, undo_distinction_and_merge, workspace_counts,
};
use vitni_core::enums::{EvidenceLevel, FactType, Restriction};
use vitni_core::ids::AgentId;
use vitni_core::matching::MatchableKind;
use vitni_core::person::PersonError;
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

/// Creates a persona named `given surname`, returning its `human_id`.
async fn person(ws: &Workspace, given: &str, surname: &str) -> String {
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some(surname.to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create person")
}

async fn merge(ws: &Workspace, surviving: &str, merged: &str) {
    merge_persons(ws, &session(), surviving, merged, IdentityDecision::default())
        .await
        .expect("merge");
}

async fn distinguish(ws: &Workspace, person: &str, other: &str) {
    distinguish_persons(ws, &session(), person, other, IdentityDecision::default())
        .await
        .expect("distinguish");
}

/// The person clusters as `(member, root)` pairs of `human_id`s.
async fn clusters(ws: &Workspace) -> BTreeSet<(String, String)> {
    let store = ws.store();
    let mut pairs = BTreeSet::new();
    for link in store.identity_links(MatchableKind::Person).await.expect("links") {
        let member = store
            .human_id_of("person", &link.member)
            .await
            .expect("member")
            .expect("member id");
        let root = store
            .human_id_of("person", &link.root)
            .await
            .expect("root")
            .expect("root id");
        pairs.insert((member, root));
    }
    pairs
}

fn pairs(list: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    list.iter().map(|(m, r)| ((*m).to_owned(), (*r).to_owned())).collect()
}

fn is_decided<T: std::fmt::Debug>(result: &Result<T, AppError>) -> bool {
    matches!(result, Err(AppError::Domain(PersonError::IdentityDecided { .. })))
}

/// Undoes the newest `event_type` assertion on `human_id`'s stream.
async fn undo_latest(ws: &Workspace, human_id: &str, event_type: &str) {
    let entry = change_log_for_person(ws, human_id)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == event_type)
        .expect("decision logged");
    undo_assertion(ws, &session(), human_id, &entry.assertion_id, None)
        .await
        .expect("undo");
}

#[tokio::test]
async fn merging_into_a_member_targets_its_root() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let c = person(&ws, "Ola", "Hansen").await;
    merge(&ws, &a, &b).await;

    // B already belongs to A's cluster, so C's merge lands on A, and C stays the survivor.
    merge(&ws, &c, &b).await;
    assert_eq!(clusters(&ws).await, pairs(&[(&a, &c), (&b, &c)]));

    // Merging a member as the survivor writes on its root.
    let d = person(&ws, "Ole", "Hansen").await;
    merge(&ws, &b, &d).await;
    assert_eq!(clusters(&ws).await, pairs(&[(&a, &c), (&b, &c), (&d, &c)]));
}

#[tokio::test]
async fn a_pair_already_in_one_cluster_cannot_be_decided_again() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let c = person(&ws, "Ola", "Hansen").await;
    merge(&ws, &a, &b).await;
    merge(&ws, &b, &c).await;

    for (first, second) in [(&a, &b), (&b, &a), (&b, &c), (&c, &a)] {
        let again = merge_persons(&ws, &session(), first, second, IdentityDecision::default()).await;
        assert!(is_decided(&again), "{first} and {second} are one cluster: {again:?}");
        let apart = distinguish_persons(&ws, &session(), first, second, IdentityDecision::default()).await;
        assert!(is_decided(&apart), "{first} and {second} are one cluster: {apart:?}");
    }
}

#[tokio::test]
async fn a_distinction_against_any_member_blocks_merging_the_clusters() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let c = person(&ws, "Ola", "Hansen").await;
    distinguish(&ws, &c, &b).await;
    merge(&ws, &a, &b).await;

    let blocked = merge_persons(&ws, &session(), &a, &c, IdentityDecision::default()).await;
    assert!(
        is_decided(&blocked),
        "C was distinguished from B, a member of A: {blocked:?}"
    );
    let reversed = merge_persons(&ws, &session(), &c, &a, IdentityDecision::default()).await;
    assert!(is_decided(&reversed), "{reversed:?}");
    let again = distinguish_persons(&ws, &session(), &a, &c, IdentityDecision::default()).await;
    assert!(is_decided(&again), "the clusters are already distinguished: {again:?}");
}

#[tokio::test]
async fn undoing_the_distinction_and_merging_joins_the_clusters() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let c = person(&ws, "Ola", "Hansen").await;
    distinguish(&ws, &c, &b).await;
    merge(&ws, &a, &b).await;

    let result = undo_distinction_and_merge(&ws, &session(), &a, &c, IdentityDecision::default())
        .await
        .expect("undo and merge");
    assert_eq!(result.survivor.human_id, a);
    assert_eq!(clusters(&ws).await, pairs(&[(&b, &a), (&c, &a)]));
    let log = change_log_for_person(&ws, &c).await.expect("log");
    assert!(
        log.iter().any(|entry| entry.event_type == "AssertionRetracted"),
        "the distinction is retracted on the stream that holds it: {log:?}"
    );
}

#[tokio::test]
async fn undoing_and_merging_an_undistinguished_pair_just_merges() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    undo_distinction_and_merge(&ws, &session(), &a, &b, IdentityDecision::default())
        .await
        .expect("merge");
    assert_eq!(clusters(&ws).await, pairs(&[(&b, &a)]));
}

#[tokio::test]
async fn undoing_the_merge_splits_the_cluster() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    merge(&ws, &a, &b).await;
    undo_latest(&ws, &a, "PersonsMerged").await;
    assert!(clusters(&ws).await.is_empty());
}

async fn listed(ws: &Workspace) -> BTreeSet<String> {
    let rows: BTreeSet<String> = list_person_rows(ws)
        .await
        .expect("rows")
        .into_iter()
        .map(|row| row.human_id)
        .collect();
    let summaries: BTreeSet<String> = list_persons(ws)
        .await
        .expect("summaries")
        .into_iter()
        .map(|summary| summary.human_id)
        .collect();
    assert_eq!(rows, summaries, "rows and summaries list the same people");
    rows
}

#[tokio::test]
async fn a_member_is_hidden_from_lists_and_counts_until_the_merge_is_undone() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    merge(&ws, &a, &b).await;
    assert_eq!(listed(&ws).await, BTreeSet::from([a.clone()]));
    assert_eq!(workspace_counts(&ws).await.expect("counts").person, 1);

    undo_latest(&ws, &a, "PersonsMerged").await;
    assert_eq!(listed(&ws).await, BTreeSet::from([a, b]));
    assert_eq!(workspace_counts(&ws).await.expect("counts").person, 2);
}

async fn occupation(ws: &Workspace, human_id: &str, title: &str) {
    let fact = NewFact {
        fact_type: FactType::Occupation,
        value: Some(title.to_owned()),
        date: None,
    };
    assert_fact(ws, &session(), human_id, fact, MutationMeta::default())
        .await
        .expect("assert fact");
}

#[tokio::test]
async fn a_root_reads_every_claim_of_its_cluster_each_attributed_to_its_owner() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let c = person(&ws, "Ola", "Hansen").await;
    occupation(&ws, &a, "Farmer").await;
    occupation(&ws, &c, "Fisherman").await;
    merge(&ws, &b, &c).await;
    merge(&ws, &a, &b).await;

    let root = show_person(&ws, &a).await.expect("show").expect("root");
    let given: Vec<_> = root
        .names
        .iter()
        .map(|name| name.name.given.clone().unwrap_or_default())
        .collect();
    assert_eq!(
        given,
        ["Ole", "Ole", "Ola"],
        "the root's name first, then each member's"
    );
    assert_eq!(
        root.display_name.as_deref(),
        Some("Ole Hansen"),
        "the root's primary name leads"
    );
    let jobs: Vec<_> = root
        .facts
        .iter()
        .map(|fact| fact.fact.value.clone().unwrap_or_default())
        .collect();
    assert_eq!(jobs, ["Farmer", "Fisherman"]);

    let owners: Vec<_> = root
        .names
        .iter()
        .map(|name| root.owner_of(&name.assertion_id))
        .collect();
    assert_eq!(owners, [a.as_str(), b.as_str(), c.as_str()]);
    let fact_owners: Vec<_> = root
        .facts
        .iter()
        .map(|fact| root.owner_of(&fact.assertion_id))
        .collect();
    assert_eq!(fact_owners, [a.as_str(), c.as_str()]);
    let merged: BTreeSet<_> = root.merged.iter().map(|persona| persona.human_id.clone()).collect();
    assert_eq!(
        merged,
        BTreeSet::from([b.clone(), c.clone()]),
        "every member, not only the direct one"
    );

    // A member resolves to its root's composed record.
    let via_member = show_person(&ws, &c).await.expect("show").expect("member");
    assert_eq!(via_member, root);
    let listed = list_persons(&ws).await.expect("list");
    assert_eq!(
        listed,
        std::slice::from_ref(&root),
        "the list carries the composed record"
    );
}

#[tokio::test]
async fn a_root_with_no_name_reads_its_members_name() {
    let (ws, _dir) = workspace().await;
    let unnamed = NewPerson {
        human_id: None,
        name: None,
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    let a = create_person(&ws, &session(), unnamed, Provenance::default(), &[])
        .await
        .expect("create");
    let b = person(&ws, "Kari", "Olsen").await;
    merge(&ws, &a, &b).await;

    let root = show_person(&ws, &a).await.expect("show").expect("root");
    assert_eq!(root.human_id, a);
    assert_eq!(root.display_name.as_deref(), Some("Kari Olsen"));
    let primary = root.primary_name_assertion.clone().expect("a primary name");
    assert_eq!(root.owner_of(&primary), b);
}

#[tokio::test]
async fn undoing_the_merge_separates_the_records_again() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    occupation(&ws, &b, "Fisherman").await;
    merge(&ws, &a, &b).await;
    undo_latest(&ws, &a, "PersonsMerged").await;

    let root = show_person(&ws, &a).await.expect("show").expect("a");
    assert!(root.facts.is_empty(), "B's fact is B's again: {:?}", root.facts);
    assert!(root.merged.is_empty());
    let member = show_person(&ws, &b).await.expect("show").expect("b");
    assert_eq!(member.human_id, b);
    assert_eq!(member.owner_of(&member.facts[0].assertion_id), b);
}

/// An edit of the whole record with only `name` and `tags` set, as the person dialog commits it.
fn record_edit(human_id: &str, name: Option<PersonNameParts>, tags: Vec<String>) -> PersonChangeSet {
    PersonChangeSet {
        target: PersonTarget::Existing {
            human_id: human_id.to_owned(),
        },
        name,
        name_citation: None,
        sex: None,
        tags,
        new_sources: Vec::new(),
        new_citations: Vec::new(),
        provenance: Provenance::default(),
        citations: Vec::new(),
    }
}

#[tokio::test]
async fn renaming_a_root_named_only_by_its_member_names_the_root() {
    let (ws, _dir) = workspace().await;
    let unnamed = NewPerson {
        human_id: None,
        name: None,
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    let a = create_person(&ws, &session(), unnamed, Provenance::default(), &[])
        .await
        .expect("create");
    let b = person(&ws, "Kari", "Olsen").await;
    merge(&ws, &a, &b).await;

    let parts = PersonNameParts::simple(Some("Kari".to_owned()), Some("Olsdatter".to_owned()));
    commit_person_change_set(&ws, &session(), record_edit(&a, Some(parts), Vec::new()))
        .await
        .expect("rename");
    let root = show_person(&ws, &a).await.expect("show").expect("root");
    let names: Vec<_> = root
        .names
        .iter()
        .map(|name| {
            (
                name.name.surnames[0].surname.clone(),
                root.owner_of(&name.assertion_id).to_owned(),
            )
        })
        .collect();
    assert_eq!(
        names,
        [("Olsdatter".to_owned(), a.clone()), ("Olsen".to_owned(), b.clone())],
        "the new name is the root's own; the member's stays the member's"
    );
}

#[tokio::test]
async fn removing_a_members_tag_in_the_record_editor_untags_the_member() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let tag = create_tag(&ws, &session(), "Emigrant".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    tag_person(&ws, &session(), &b, &tag, false, MutationMeta::default())
        .await
        .expect("tag member");
    merge(&ws, &a, &b).await;
    assert_eq!(
        show_person(&ws, &a).await.expect("show").expect("root").tags,
        std::slice::from_ref(&tag)
    );

    commit_person_change_set(&ws, &session(), record_edit(&a, None, Vec::new()))
        .await
        .expect("untag");
    assert!(show_person(&ws, &a).await.expect("show").expect("root").tags.is_empty());
}

#[tokio::test]
async fn a_claim_is_owned_by_the_cluster_record_whose_stream_holds_it() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    occupation(&ws, &b, "Fisherman").await;
    merge(&ws, &a, &b).await;
    let root = show_person(&ws, &a).await.expect("show").expect("root");

    let fact = &root.facts[0].assertion_id;
    assert_eq!(claim_owner(&ws, &a, fact).await.expect("owner"), b);
    let own_name = &root.names[0].assertion_id;
    assert_eq!(claim_owner(&ws, &a, own_name).await.expect("owner"), a);
    assert_eq!(
        claim_owner(&ws, &b, own_name).await.expect("owner"),
        a,
        "from a member too"
    );
    let unknown = "01890000-0000-7000-8000-000000000000";
    assert_eq!(
        claim_owner(&ws, &a, unknown).await.expect("owner"),
        a,
        "nobody holds it"
    );
}

#[tokio::test]
async fn untagging_a_root_untags_every_cluster_record_holding_the_tag() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let tag = create_tag(&ws, &session(), "Emigrant".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    tag_person(&ws, &session(), &b, &tag, false, MutationMeta::default())
        .await
        .expect("tag member");
    merge(&ws, &a, &b).await;

    tag_person(&ws, &session(), &a, &tag, true, MutationMeta::default())
        .await
        .expect("untag through the root");
    assert!(show_person(&ws, &a).await.expect("show").expect("root").tags.is_empty());
}

#[tokio::test]
async fn the_decision_between_two_clusters_is_read_whichever_record_names_them() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let c = person(&ws, "Ola", "Hansen").await;
    assert_eq!(pair_decision(&ws, &a, &c).await.expect("decision"), None);
    merge(&ws, &a, &b).await;
    assert_eq!(
        pair_decision(&ws, &b, &a).await.expect("decision"),
        Some(PairDecision::SameCluster)
    );
    distinguish(&ws, &c, &a).await;
    assert_eq!(
        pair_decision(&ws, &b, &c).await.expect("decision"),
        Some(PairDecision::Distinct)
    );
    assert_eq!(pair_decision(&ws, &a, &a).await.expect("decision"), None);
}

#[tokio::test]
async fn a_tag_both_cluster_records_hold_counts_the_cluster_once() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let tag = create_tag(&ws, &session(), "Emigrant".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    for record in [&a, &b] {
        tag_person(&ws, &session(), record, &tag, false, MutationMeta::default())
            .await
            .expect("tag record");
    }
    merge(&ws, &a, &b).await;

    let tags = list_tags(&ws).await.expect("tags");
    assert_eq!(tags.iter().map(|tag| tag.usage_count).collect::<Vec<_>>(), vec![1]);
}

#[tokio::test]
async fn clearing_a_restriction_clears_it_on_every_cluster_record() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    let private = BTreeSet::from([Restriction::Privacy]);
    set_restrictions(&ws, &session(), &b, private.clone(), MutationMeta::default())
        .await
        .expect("restrict member");
    merge(&ws, &a, &b).await;
    assert_eq!(
        show_person(&ws, &a).await.expect("show").expect("root").restrictions,
        private
    );

    set_restrictions(&ws, &session(), &a, BTreeSet::new(), MutationMeta::default())
        .await
        .expect("clear through the root");
    assert!(
        show_person(&ws, &a)
            .await
            .expect("show")
            .expect("root")
            .restrictions
            .is_empty()
    );
}

#[tokio::test]
async fn a_members_pedigree_is_its_roots() {
    let (ws, _dir) = workspace().await;
    let a = person(&ws, "Ole", "Hansen").await;
    let b = person(&ws, "Ole", "Hanssen").await;
    merge(&ws, &a, &b).await;

    let chart = ancestors(&ws, &b, 2).await.expect("a member's pedigree");
    assert_eq!(chart.focus.human_id, a);
}
