//! The possible-matches review queue (ADR 0039 §3): computed across every decidable kind, filtered by
//! kind, band and import run, and emptied by deciding its pairs either way.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;
use std::sync::Arc;

use uuid::Uuid;
use vitni_app::{
    AppDefaults, DatasetId, DateParts, DecidableKind, EntityFields, EntityRef, EventType, IdentityDecision,
    ImportRunId, LinkKind, MatchBand, MatchQueueFilter, MatchVerdict, MutationMeta, NewEvent, NewImportRun,
    NewParticipation, NewPerson, NewPlace, OperatorConfig, PairDecision, PendingRun, PersonNameParts, PlaceType,
    Provenance, RecordGraph, RunToEnd, Session, StagedEntity, StagedEvent, StagedLink, StagedPerson, Workspace,
    WorkspaceDefaults, commit_import, decide_match, gregorian_date, list_import_runs, match_pair_decision, match_queue,
    plan_import,
};
use vitni_core::enums::{EvidenceLevel, ParticipantRole, Sex};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn human() -> Session {
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

/// An importer's session writing a fresh run of its own dataset, labelled `label`.
fn importer(dataset: u128, label: &str) -> Session {
    let run = Arc::new(PendingRun::new(
        human(),
        NewImportRun {
            plugin: "gedcom-import".to_owned(),
            plugin_version: "0.1.0".to_owned(),
            dataset: DatasetId::lineage("gedcom", Uuid::from_u128(dataset)),
            dataset_label: label.to_owned(),
            source_label: label.to_owned(),
            source_path: None,
            file_asserted_at: None,
            dataset_hint: None,
        },
    ));
    Session::software("gedcom-import", "0.1.0").with_import_run(run)
}

fn year(year: i32) -> DateParts {
    DateParts {
        year,
        month: None,
        day: None,
    }
}

/// A record of one person `given Hansen`, born in 1850.
fn individual(record: &str, given: &str) -> RecordGraph {
    RecordGraph {
        record: record.to_owned(),
        entities: vec![
            StagedEntity {
                local_id: 0,
                item: None,
                fields: EntityFields::Person(StagedPerson {
                    names: vec![PersonNameParts::simple(
                        Some(given.to_owned()),
                        Some("Hansen".to_owned()),
                    )],
                    sex: Some(Sex::Male),
                    ..StagedPerson::default()
                }),
            },
            StagedEntity {
                local_id: 1,
                item: Some("event:BIRT:0".to_owned()),
                fields: EntityFields::Event(StagedEvent {
                    event_type: EventType::Birth,
                    date: Some(gregorian_date(year(1850))),
                    addresses: Vec::new(),
                    restrictions: BTreeSet::default(),
                }),
            },
        ],
        links: vec![StagedLink {
            item: Some("event:BIRT:0".to_owned()),
            link: LinkKind::Participation {
                person: EntityRef::Local(0),
                event: EntityRef::Local(1),
                role: ParticipantRole::Primary,
                age: None,
                attributes: Vec::new(),
                notes: Vec::new(),
                citations: Vec::new(),
            },
        }],
    }
}

/// Imports `graphs` as one run labelled `label`, every possible match left for later; returns the run.
async fn import_deferring(workspace: &Workspace, dataset: u128, label: &str, graphs: Vec<RecordGraph>) -> ImportRunId {
    let session = importer(dataset, label);
    let plan = plan_import(workspace, &session, graphs, None).await.expect("plan");
    commit_import(workspace, &session, &plan, &Provenance::default(), &mut RunToEnd)
        .await
        .expect("commit");
    list_import_runs(workspace)
        .await
        .expect("runs")
        .into_iter()
        .find(|run| run.source_label == label)
        .map(|run| run.id)
        .expect("the run")
}

/// A person `given Hansen` born in 1850, entered at the keyboard.
async fn stored_person(workspace: &Workspace, given: &str) -> String {
    let session = human();
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some("Hansen".to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    let person = vitni_app::create_person(workspace, &session, new, Provenance::default(), &[])
        .await
        .expect("person");
    vitni_app::person::assert_sex(workspace, &session, &person, Sex::Male, MutationMeta::default())
        .await
        .expect("sex");
    let new = NewEvent {
        human_id: None,
        event_type: EventType::Birth,
    };
    let event = vitni_app::create_event(workspace, &session, new, Provenance::default(), &[])
        .await
        .expect("event");
    vitni_app::assert_event_date(workspace, &session, &event, year(1850), MutationMeta::default())
        .await
        .expect("date");
    vitni_app::assert_participation(
        workspace,
        &session,
        &person,
        &event,
        NewParticipation::with_role(ParticipantRole::Primary),
        MutationMeta::default(),
    )
    .await
    .expect("participation");
    person
}

async fn farm(workspace: &Workspace, name: &str) -> String {
    let new = NewPlace {
        human_id: None,
        place_type: PlaceType::Farm,
        name: Some(name.to_owned()),
    };
    vitni_app::create_place(workspace, &human(), new, Provenance::default(), &[])
        .await
        .expect("place")
}

fn all() -> MatchQueueFilter {
    MatchQueueFilter {
        run: None,
        kind: None,
        min_band: MatchBand::Possible,
    }
}

/// The queue under `filter`, as `(kind, sorted human-id pair)`s.
async fn queued(workspace: &Workspace, filter: &MatchQueueFilter) -> BTreeSet<(DecidableKind, String, String)> {
    let mut pairs = BTreeSet::new();
    for queued in match_queue(workspace, filter, None).await.expect("queue").pairs {
        let (a, b) = if queued.a.human_id <= queued.b.human_id {
            (queued.a.human_id, queued.b.human_id)
        } else {
            (queued.b.human_id, queued.a.human_id)
        };
        pairs.insert((queued.kind, a, b));
    }
    pairs
}

fn pair(kind: DecidableKind, a: &str, b: &str) -> (DecidableKind, String, String) {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    (kind, a.to_owned(), b.to_owned())
}

fn decision() -> IdentityDecision {
    IdentityDecision {
        provenance: Provenance {
            rationale: Some("same farm, same year".to_owned()),
            ..Provenance::default()
        },
        assessment: None,
    }
}

#[tokio::test]
async fn the_queue_lists_pairs_of_every_kind_the_most_similar_first() {
    let (workspace, _dir) = workspace().await;
    let ole = stored_person(&workspace, "Ole").await;
    let ole_again = stored_person(&workspace, "Ole").await;
    let nordaas = farm(&workspace, "Nordaas").await;
    let nordas = farm(&workspace, "Nordås").await;
    farm(&workspace, "Bergen").await;

    let queue = match_queue(&workspace, &all(), None).await.expect("queue").pairs;
    for window in queue.windows(2) {
        let (first, second) = (&window[0].assessment, &window[1].assessment);
        assert!((first.band, first.score) >= (second.band, second.score), "{queue:?}");
    }
    let pairs = queued(&workspace, &all()).await;
    assert!(
        pairs.contains(&pair(DecidableKind::Person, &ole, &ole_again)),
        "{pairs:?}"
    );
    assert!(
        pairs.contains(&pair(DecidableKind::Place, &nordaas, &nordas)),
        "{pairs:?}"
    );
    assert!(pairs.iter().all(|(_, a, b)| a != b), "{pairs:?}");
}

#[tokio::test]
async fn the_queue_is_filtered_by_kind_and_band() {
    let (workspace, _dir) = workspace().await;
    let ole = stored_person(&workspace, "Ole").await;
    let ole_again = stored_person(&workspace, "Ole").await;
    farm(&workspace, "Nordaas").await;
    farm(&workspace, "Nordås").await;

    let persons = MatchQueueFilter {
        kind: Some(DecidableKind::Person),
        ..all()
    };
    assert_eq!(
        queued(&workspace, &persons).await,
        BTreeSet::from([pair(DecidableKind::Person, &ole, &ole_again)])
    );
    let queue = match_queue(&workspace, &all(), None).await.expect("queue").pairs;
    let strongest = queue.first().expect("a pair").assessment.band;
    let banded = MatchQueueFilter {
        min_band: strongest,
        ..all()
    };
    for queued in match_queue(&workspace, &banded, None).await.expect("queue").pairs {
        assert!(queued.assessment.band >= strongest);
    }
    let deterministic = MatchQueueFilter {
        min_band: MatchBand::Deterministic,
        ..all()
    };
    assert!(queued(&workspace, &deterministic).await.is_empty());
}

#[tokio::test]
async fn a_limited_queue_lists_the_strongest_pairs_and_counts_them_all() {
    let (workspace, _dir) = workspace().await;
    for _ in 0..3 {
        stored_person(&workspace, "Ole").await;
    }
    farm(&workspace, "Nordaas").await;
    farm(&workspace, "Nordås").await;
    let every = match_queue(&workspace, &all(), None).await.expect("queue");
    assert!(
        every.total > 2,
        "three persons, their births and two farms pair up: {every:?}"
    );
    assert_eq!(every.pairs.len(), every.total);
    let limited = match_queue(&workspace, &all(), Some(2)).await.expect("queue");
    assert_eq!(limited.total, every.total);
    assert_eq!(limited.pairs[..], every.pairs[..2], "the strongest, in the same order");
}

#[tokio::test]
async fn deferred_pairs_are_listed_under_their_run_and_emptied_by_deciding_them() {
    let (workspace, _dir) = workspace().await;
    let ole = stored_person(&workspace, "Ole").await;
    let per = stored_person(&workspace, "Per").await;
    let first = import_deferring(&workspace, 1, "first.ged", vec![individual("I1", "Ole")]).await;
    let second = import_deferring(&workspace, 2, "second.ged", vec![individual("I1", "Per")]).await;

    let under = |run| MatchQueueFilter {
        run: Some(run),
        kind: Some(DecidableKind::Person),
        ..all()
    };
    let first_pairs = queued(&workspace, &under(first)).await;
    assert_eq!(first_pairs.len(), 1, "{first_pairs:?}");
    assert!(
        first_pairs.iter().all(|(_, a, b)| *a == ole || *b == ole),
        "{first_pairs:?}"
    );
    let second_pairs = queued(&workspace, &under(second)).await;
    assert_eq!(second_pairs.len(), 1, "{second_pairs:?}");
    assert!(
        second_pairs.iter().all(|(_, a, b)| *a == per || *b == per),
        "{second_pairs:?}"
    );

    let (_, a, b) = first_pairs.into_iter().next().expect("pair");
    decide_match(
        &workspace,
        &human(),
        DecidableKind::Person,
        &a,
        &b,
        MatchVerdict::Same,
        decision(),
    )
    .await
    .expect("same");
    assert!(queued(&workspace, &under(first)).await.is_empty());
    assert_eq!(
        match_pair_decision(&workspace, DecidableKind::Person, &a, &b)
            .await
            .expect("decision"),
        Some(PairDecision::SameCluster)
    );

    let (_, a, b) = second_pairs.into_iter().next().expect("pair");
    decide_match(
        &workspace,
        &human(),
        DecidableKind::Person,
        &a,
        &b,
        MatchVerdict::Distinct,
        decision(),
    )
    .await
    .expect("distinct");
    assert!(queued(&workspace, &under(second)).await.is_empty());
    assert_eq!(
        match_pair_decision(&workspace, DecidableKind::Person, &a, &b)
            .await
            .expect("decision"),
        Some(PairDecision::Distinct)
    );
}

#[tokio::test]
async fn a_place_pair_is_decided_through_the_queue() {
    let (workspace, _dir) = workspace().await;
    let nordaas = farm(&workspace, "Nordaas").await;
    let nordas = farm(&workspace, "Nordås").await;
    decide_match(
        &workspace,
        &human(),
        DecidableKind::Place,
        &nordaas,
        &nordas,
        MatchVerdict::Distinct,
        decision(),
    )
    .await
    .expect("distinct");
    assert!(queued(&workspace, &all()).await.is_empty());
}

#[tokio::test]
async fn deciding_a_decided_pair_again_is_an_identity_refusal() {
    let (workspace, _dir) = workspace().await;
    let ole = stored_person(&workspace, "Ole").await;
    let ole_again = stored_person(&workspace, "Ole").await;
    decide_match(
        &workspace,
        &human(),
        DecidableKind::Person,
        &ole,
        &ole_again,
        MatchVerdict::Distinct,
        decision(),
    )
    .await
    .expect("distinct");
    let error = decide_match(
        &workspace,
        &human(),
        DecidableKind::Person,
        &ole,
        &ole_again,
        MatchVerdict::Same,
        decision(),
    )
    .await
    .expect_err("already decided");
    assert_eq!(error.identity_refusal(), Some(vitni_app::IdentityRefusal::Decided));
}

#[test]
fn every_matchable_kind_but_tag_is_decidable() {
    let mut decidable = Vec::new();
    for kind in vitni_app::MatchableKind::ALL {
        if let Some(kind) = DecidableKind::from_matchable(kind) {
            assert_eq!(DecidableKind::from_matchable(kind.matchable()), Some(kind));
            decidable.push(kind.matchable());
        }
    }
    assert_eq!(decidable.len(), DecidableKind::ALL.len());
    assert!(!decidable.contains(&vitni_app::MatchableKind::Tag));
}
