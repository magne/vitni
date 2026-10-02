//! Staged import (ADR 0040 §2, §5): planning record graphs into dispositions, committing a plan, and
//! finishing an interrupted commit by running the same import again.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;
use std::sync::Arc;

use uuid::Uuid;
use vitni_app::{
    AppDefaults, CommitControl, CommitOutcome, DatasetId, DateParts, Disposition, EntityFields, EntityRef, ExternalId,
    IdentityDecision, ImportCounts, ImportPlan, ImportReview, LinkBasis, LinkKind, MatchGroup, MutationMeta, NewEvent,
    NewFact, NewImportRun, NewParticipation, NewPerson, NewPlace, NewSource, OperatorConfig, PairAnswer, PairDecision,
    PendingRun, PersonNameParts, PlaceType, PlanCounts, Provenance, RecordGraph, ResolutionDecision, RunToEnd, Session,
    StagedEntity, StagedEvent, StagedFamily, StagedLink, StagedPerson, StagedPlace, StagedSource, StagedTag, Workspace,
    WorkspaceDefaults, WriteScope, commit_import, gregorian_date, plan_import, record_origin,
};
use vitni_core::enums::{EventType, EvidenceLevel, FactType, ParticipantRole, Sex};
use vitni_core::ids::AgentId;
use vitni_core::matching::{MatchBand, MatchableKind};
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

fn dataset(n: u128) -> DatasetId {
    DatasetId::lineage("gedcom", Uuid::from_u128(n))
}

/// An importer's session writing a fresh run of `dataset`.
fn importer(dataset: DatasetId) -> Session {
    let run = Arc::new(PendingRun::new(
        human(),
        NewImportRun {
            plugin: "gedcom-import".to_owned(),
            plugin_version: "0.1.0".to_owned(),
            dataset,
            dataset_label: "tree.ged".to_owned(),
            source_label: "tree.ged".to_owned(),
            file_asserted_at: None,
            dataset_hint: None,
        },
    ));
    Session::software("gedcom-import", "0.1.0").with_import_run(run)
}

fn uid(value: &str) -> ExternalId {
    ExternalId {
        authority: "gedcom-uid".to_owned(),
        value: value.to_owned(),
        kind: None,
        url: None,
    }
}

fn name(given: &str, surname: &str) -> PersonNameParts {
    PersonNameParts::simple(Some(given.to_owned()), Some(surname.to_owned()))
}

fn person(local_id: u32, item: Option<&str>, given: &str, surname: &str) -> StagedEntity {
    StagedEntity {
        local_id,
        item: item.map(str::to_owned),
        fields: EntityFields::Person(StagedPerson {
            names: vec![name(given, surname)],
            sex: Some(Sex::Male),
            ..StagedPerson::default()
        }),
    }
}

fn with_uid(mut entity: StagedEntity, value: &str) -> StagedEntity {
    if let EntityFields::Person(person) = &mut entity.fields {
        person.external_ids.push(uid(value));
    }
    entity
}

fn with_occupation(mut entity: StagedEntity, occupation: &str) -> StagedEntity {
    if let EntityFields::Person(person) = &mut entity.fields {
        person.facts.push(NewFact {
            fact_type: FactType::Occupation,
            value: Some(occupation.to_owned()),
            date: None,
        });
    }
    entity
}

fn birth(local_id: u32, item: &str, year: i32) -> StagedEntity {
    StagedEntity {
        local_id,
        item: Some(item.to_owned()),
        fields: EntityFields::Event(StagedEvent {
            event_type: EventType::Birth,
            date: Some(gregorian_date(DateParts {
                year,
                month: None,
                day: None,
            })),
            addresses: Vec::new(),
            restrictions: BTreeSet::default(),
        }),
    }
}

fn primary(person: EntityRef, event: EntityRef, item: &str) -> StagedLink {
    StagedLink {
        item: Some(item.to_owned()),
        link: LinkKind::Participation {
            person,
            event,
            role: ParticipantRole::Primary,
            age: None,
            attributes: Vec::new(),
            notes: Vec::new(),
            citations: Vec::new(),
        },
    }
}

fn place_ref(record: &str) -> EntityRef {
    EntityRef::Origin {
        kind: MatchableKind::Place,
        record: record.to_owned(),
        item: None,
    }
}

fn person_ref(record: &str) -> EntityRef {
    EntityRef::Origin {
        kind: MatchableKind::Person,
        record: record.to_owned(),
        item: None,
    }
}

/// `I1`: a person born in 1850 at Mandal, the place its own record.
fn individual(record: &str, given: &str) -> RecordGraph {
    RecordGraph {
        record: record.to_owned(),
        entities: vec![person(0, None, given, "Hansen"), birth(1, "event:BIRT:0", 1850)],
        links: vec![
            primary(EntityRef::Local(0), EntityRef::Local(1), "event:BIRT:0"),
            StagedLink {
                item: Some("event:BIRT:0".to_owned()),
                link: LinkKind::EventPlace {
                    event: EntityRef::Local(1),
                    place: place_ref("plac:Mandal"),
                },
            },
        ],
    }
}

fn place(record: &str, name: &str) -> RecordGraph {
    RecordGraph {
        record: record.to_owned(),
        entities: vec![StagedEntity {
            local_id: 0,
            item: None,
            fields: EntityFields::Place(StagedPlace {
                name: name.to_owned(),
                place_type: None,
                restrictions: BTreeSet::default(),
            }),
        }],
        links: Vec::new(),
    }
}

/// A family of `partner` and `child` (records of other graphs).
fn family(record: &str, partner: &str, child: &str) -> RecordGraph {
    RecordGraph {
        record: record.to_owned(),
        entities: vec![StagedEntity {
            local_id: 0,
            item: None,
            fields: EntityFields::Family(StagedFamily::default()),
        }],
        links: vec![
            StagedLink {
                item: None,
                link: LinkKind::Partner {
                    family: EntityRef::Local(0),
                    person: person_ref(partner),
                },
            },
            StagedLink {
                item: None,
                link: LinkKind::Child {
                    family: EntityRef::Local(0),
                    child: person_ref(child),
                    relationships: Vec::new(),
                },
            },
        ],
    }
}

fn tree() -> Vec<RecordGraph> {
    vec![
        individual("I1", "Ole"),
        individual("I2", "Hans"),
        place("plac:Mandal", "Mandal"),
        family("F1", "I1", "I2"),
    ]
}

async fn plan(workspace: &Workspace, session: &Session, graphs: Vec<RecordGraph>) -> ImportPlan {
    plan_import(workspace, session, graphs, None).await.expect("plan")
}

async fn import(workspace: &Workspace, session: &Session, graphs: Vec<RecordGraph>) -> ImportPlan {
    let plan = plan(workspace, session, graphs).await;
    commit_import(workspace, session, &plan, &Provenance::default(), &mut RunToEnd)
        .await
        .expect("commit");
    plan
}

async fn events(workspace: &Workspace) -> u64 {
    workspace.store().event_count().await.expect("count")
}

/// How many persons, events, places and families the workspace holds.
async fn records(workspace: &Workspace) -> [usize; 4] {
    [
        vitni_app::list_persons(workspace).await.expect("persons").len(),
        vitni_app::list_events(workspace).await.expect("events").len(),
        vitni_app::list_places(workspace).await.expect("places").len(),
        vitni_app::list_families(workspace).await.expect("families").len(),
    ]
}

fn disposition(plan: &ImportPlan, graph: usize, local_id: u32) -> &Disposition {
    &plan.entity(graph, local_id).expect("entity").disposition
}

/// A stored person named `given Hansen`, born in `year`, carrying `uid` if given — entered at the
/// keyboard.
async fn stored_person(workspace: &Workspace, given: &str, year: i32, external_id: Option<&str>) -> String {
    let session = human();
    let new = NewPerson {
        human_id: None,
        name: Some(name(given, "Hansen")),
        evidence_level: EvidenceLevel::Persona,
        external_ids: external_id.map(uid).into_iter().collect(),
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
    let date = gregorian_date(DateParts {
        year,
        month: None,
        day: None,
    });
    vitni_app::assert_event_date_value(workspace, &session, &event, date, MutationMeta::default())
        .await
        .expect("date");
    let participation = NewParticipation {
        role: ParticipantRole::Primary,
        age: None,
        attributes: Vec::new(),
        notes: Vec::new(),
    };
    vitni_app::assert_participation(
        workspace,
        &session,
        &person,
        &event,
        participation,
        MutationMeta::default(),
    )
    .await
    .expect("participation");
    person
}

#[tokio::test]
async fn a_first_import_plans_every_entity_new_and_creates_them() {
    let (workspace, _dir) = workspace().await;
    let session = importer(dataset(1));
    let plan = import(&workspace, &session, tree()).await;

    let counts = plan.counts();
    assert_eq!(
        counts.new, 6,
        "two persons, two births, a place and a family: {counts:?}"
    );
    assert_eq!(records(&workspace).await, [2, 2, 1, 1]);
}

#[tokio::test]
async fn a_reimport_plans_every_entity_unchanged_and_writes_nothing() {
    let (workspace, _dir) = workspace().await;
    import(&workspace, &importer(dataset(1)), tree()).await;
    let before = events(&workspace).await;

    let session = importer(dataset(1));
    let plan = import(&workspace, &session, tree()).await;
    let counts = plan.counts();
    assert_eq!(counts.unchanged, 6, "{counts:?}");
    assert!(plan.links.iter().all(|link| !link.writes), "every link is on record");
    assert_eq!(events(&workspace).await, before);
    assert!(
        !session.import_run().expect("run").started(),
        "a run that writes nothing is not started"
    );
}

#[tokio::test]
async fn a_new_fact_plans_an_update_naming_its_field() {
    let (workspace, _dir) = workspace().await;
    import(&workspace, &importer(dataset(1)), tree()).await;

    let mut graphs = tree();
    graphs[0].entities[0] = with_occupation(graphs[0].entities[0].clone(), "Farmer");
    let plan = import(&workspace, &importer(dataset(1)), graphs).await;
    let Disposition::Update { fields, .. } = disposition(&plan, 0, 0) else {
        panic!("expected an update: {:?}", disposition(&plan, 0, 0));
    };
    assert_eq!(fields, &["person.FactAsserted.Occupation".to_owned()]);
    assert_eq!(plan.counts().unchanged, 5);
}

#[tokio::test]
async fn a_known_external_id_links_and_records_the_resolution() {
    let (workspace, _dir) = workspace().await;
    let stored = stored_person(&workspace, "Ole", 1850, Some("UID-1")).await;

    let graph = RecordGraph {
        record: "I9".to_owned(),
        entities: vec![with_uid(person(0, None, "Ole", "Hansen"), "UID-1")],
        links: Vec::new(),
    };
    let session = importer(dataset(2));
    let plan = plan(&workspace, &session, vec![graph]).await;
    let Disposition::Link { target, basis } = disposition(&plan, 0, 0) else {
        panic!("expected a link: {:?}", disposition(&plan, 0, 0));
    };
    assert_eq!(
        (target.human_id.as_str(), *basis),
        (stored.as_str(), LinkBasis::ExternalId)
    );
    let outcome = commit_import(&workspace, &session, &plan, &Provenance::default(), &mut RunToEnd)
        .await
        .expect("commit");
    assert_eq!(outcome.resolved.len(), 1);
    assert!(outcome.created.is_empty(), "nothing created: {:?}", outcome.created);
    assert_eq!(vitni_app::list_persons(&workspace).await.expect("persons").len(), 1);
}

#[tokio::test]
async fn a_tag_links_by_its_case_folded_name() {
    let (workspace, _dir) = workspace().await;
    let existing = vitni_app::create_tag(&workspace, &human(), "Emigrant".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    let graph = RecordGraph {
        record: "T1".to_owned(),
        entities: vec![StagedEntity {
            local_id: 0,
            item: None,
            fields: EntityFields::Tag(StagedTag {
                name: " emigrant ".to_owned(),
            }),
        }],
        links: Vec::new(),
    };
    let plan = import(&workspace, &importer(dataset(1)), vec![graph]).await;
    let Disposition::Link { target, basis } = disposition(&plan, 0, 0) else {
        panic!("expected a link: {:?}", disposition(&plan, 0, 0));
    };
    assert_eq!((target.id.as_str(), *basis), (existing.as_str(), LinkBasis::TagName));
    assert_eq!(vitni_app::list_tags(&workspace).await.expect("tags").len(), 1);
}

#[tokio::test]
async fn a_similar_stored_person_plans_candidates_and_commits_as_new() {
    let (workspace, _dir) = workspace().await;
    stored_person(&workspace, "Ole", 1850, None).await;

    let plan = import(&workspace, &importer(dataset(1)), vec![individual("I1", "Ole")]).await;
    let Disposition::Candidates(similar) = disposition(&plan, 0, 0) else {
        panic!("expected candidates: {:?}", disposition(&plan, 0, 0));
    };
    assert_eq!(similar.len(), 1);
    assert_eq!(
        vitni_app::list_persons(&workspace).await.expect("persons").len(),
        2,
        "decided later"
    );
}

#[tokio::test]
async fn two_items_of_one_graph_are_never_matched_to_each_other() {
    let (workspace, _dir) = workspace().await;
    let graph = RecordGraph {
        record: "R1".to_owned(),
        entities: vec![
            person(0, None, "Ole", "Hansen"),
            person(1, Some("father"), "Ole", "Hansen"),
        ],
        links: Vec::new(),
    };
    let plan = plan(&workspace, &importer(dataset(1)), vec![graph]).await;
    assert_eq!(plan.counts().new, 2);
}

/// The best candidate's score for the child `I2` of a family whose father `I1` carries `father_uid`.
async fn childs_best_score(workspace: &Workspace, father_uid: Option<&str>) -> Option<f64> {
    let mut father = person(0, None, "Ole", "Hansen");
    if let Some(value) = father_uid {
        father = with_uid(father, value);
    }
    let graphs = vec![
        RecordGraph {
            record: "I1".to_owned(),
            entities: vec![father],
            links: Vec::new(),
        },
        individual("I2", "Hans"),
        place("plac:Mandal", "Mandal"),
        family("F1", "I1", "I2"),
    ];
    let plan = plan(workspace, &importer(dataset(9)), graphs).await;
    if let Disposition::Candidates(similar) = disposition(&plan, 1, 0) {
        similar.first().map(|best| best.assessment.score)
    } else {
        None
    }
}

#[tokio::test]
async fn a_resolved_father_raises_the_childs_match() {
    let (workspace, _dir) = workspace().await;
    let father = stored_person(&workspace, "Ole", 1820, Some("UID-F")).await;
    let child = stored_person(&workspace, "Hans", 1850, None).await;
    let session = human();
    let family = vitni_app::create_family(&workspace, &session, Provenance::default(), &[])
        .await
        .expect("family");
    vitni_app::family::add_partner(&workspace, &session, &family, &father, MutationMeta::default())
        .await
        .expect("partner");
    vitni_app::family::add_child(
        &workspace,
        &session,
        &family,
        &child,
        Vec::new(),
        MutationMeta::default(),
    )
    .await
    .expect("child");

    let unresolved = childs_best_score(&workspace, None).await.expect("candidates");
    let resolved = childs_best_score(&workspace, Some("UID-F")).await.expect("candidates");
    assert!(resolved > unresolved, "resolved {resolved} vs unresolved {unresolved}");
}

/// A person another dataset made, carrying `UID-1`, and an import of `I1` with that uid, an occupation
/// and a birth at Mandal.
async fn linked_elsewhere(workspace: &Workspace) -> ImportPlan {
    stored_person(workspace, "Ole", 1850, Some("UID-1")).await;
    let mut graphs = vec![individual("I1", "Ole"), place("plac:Mandal", "Mandal")];
    graphs[0].entities[0] = with_occupation(with_uid(graphs[0].entities[0].clone(), "UID-1"), "Farmer");
    import(workspace, &importer(dataset(2)), graphs).await
}

#[tokio::test]
async fn a_person_linked_elsewhere_withholds_the_rest_of_its_graph() {
    let (workspace, _dir) = workspace().await;
    let plan = linked_elsewhere(&workspace).await;

    assert_eq!(plan.entity(0, 0).expect("person").scope, WriteScope::Identity);
    assert_eq!(plan.entity(0, 1).expect("birth").scope, WriteScope::Withheld);
    assert_eq!(records(&workspace).await, [1, 1, 0, 0], "only the stored birth");
    let persons = vitni_app::list_persons(&workspace).await.expect("persons");
    let view = workspace
        .store()
        .find_person(&persons[0].human_id)
        .await
        .expect("find")
        .expect("person");
    assert!(view.facts().is_empty(), "the other dataset's person keeps its contents");
}

#[tokio::test]
async fn a_place_only_withheld_events_reach_is_withheld() {
    let (workspace, _dir) = workspace().await;
    let plan = linked_elsewhere(&workspace).await;
    assert_eq!(plan.entity(1, 0).expect("place").scope, WriteScope::Withheld);
}

#[tokio::test]
async fn a_person_linked_elsewhere_joins_no_family_its_record_names() {
    let (workspace, _dir) = workspace().await;
    stored_person(&workspace, "Ole", 1850, Some("UID-1")).await;
    let mut household = family("F1", "I2", "I3");
    household.links.clear();
    household.entities[0].item = Some("family".to_owned());
    let mut graphs = vec![individual("I1", "Ole"), household];
    graphs[0].entities[0] = with_uid(graphs[0].entities[0].clone(), "UID-1");
    graphs[0].links.push(StagedLink {
        item: Some("household".to_owned()),
        link: LinkKind::Partner {
            family: EntityRef::Origin {
                kind: MatchableKind::Family,
                record: "F1".to_owned(),
                item: Some("family".to_owned()),
            },
            person: EntityRef::Local(0),
        },
    });

    let plan = import(&workspace, &importer(dataset(2)), graphs).await;

    assert_eq!(plan.entity(0, 0).expect("person").scope, WriteScope::Identity);
    assert_eq!(plan.entity(1, 0).expect("family").scope, WriteScope::Withheld);
    assert_eq!(
        records(&workspace).await,
        [1, 1, 0, 0],
        "no family for the other dataset's person"
    );
}

#[tokio::test]
async fn a_link_to_an_origin_nothing_holds_is_left_out() {
    let (workspace, _dir) = workspace().await;
    let plan = import(&workspace, &importer(dataset(1)), vec![individual("I1", "Ole")]).await;
    assert!(plan.links[1].dangling, "no graph or record is plac:Mandal");
    assert_eq!(records(&workspace).await, [1, 1, 0, 0]);
}

#[tokio::test]
async fn every_write_carries_its_entity_origin_and_the_run() {
    let (workspace, _dir) = workspace().await;
    let session = importer(dataset(1));
    import(&workspace, &session, tree()).await;
    let run = session.import_run().expect("run").id();

    let events = workspace.store().created_origins("event").await.expect("origins");
    let mut found: Vec<(String, Option<String>)> = events
        .iter()
        .map(|(_, origin)| {
            assert_eq!((&origin.dataset, origin.run), (&dataset(1), run));
            (origin.record.clone(), origin.item.clone())
        })
        .collect();
    found.sort();
    let birth = Some("event:BIRT:0".to_owned());
    assert_eq!(found, [("I1".to_owned(), birth.clone()), ("I2".to_owned(), birth)]);
}

#[tokio::test]
async fn a_records_origin_names_the_dataset_record_it_was_imported_from() {
    let (workspace, _dir) = workspace().await;
    let session = importer(dataset(1));
    import(&workspace, &session, tree()).await;
    let keyed = stored_person(&workspace, "Per", 1850, None).await;

    let mut imported = Vec::new();
    for person in vitni_app::list_persons(&workspace).await.expect("persons") {
        let origin = record_origin(&workspace, MatchableKind::Person, &person.human_id)
            .await
            .expect("origin");
        imported.push((person.human_id, origin.map(|o| (o.dataset, o.record, o.item))));
    }
    imported.sort();
    let place = vitni_app::list_places(&workspace).await.expect("places");
    let place_origin = record_origin(&workspace, MatchableKind::Place, &place[0].human_id)
        .await
        .expect("place origin")
        .expect("imported place");

    let from = |record: &str| Some((dataset(1), record.to_owned(), None));
    assert_eq!(
        imported,
        [
            ("I0001".to_owned(), from("I1")),
            ("I0002".to_owned(), from("I2")),
            (keyed, None),
        ],
        "a keyboard record has no origin"
    );
    assert_eq!(place_origin.record, "plac:Mandal");
    let missing = record_origin(&workspace, MatchableKind::Person, "I9999").await;
    assert!(
        matches!(missing, Err(vitni_app::AppError::PersonNotFound(_))),
        "{missing:?}"
    );
}

/// Stops a commit before its `limit`-th write.
struct StopAt {
    limit: u32,
}

impl CommitControl for StopAt {
    fn proceed(&mut self, done: u32, _total: u32) -> bool {
        done < self.limit
    }
}

#[tokio::test]
async fn an_interrupted_commit_finishes_on_re_run_with_no_duplicates() {
    let (clean, _clean_dir) = workspace().await;
    let plan = import(&clean, &importer(dataset(1)), tree()).await;
    let expected = records(&clean).await;
    let steps = plan.counts().new + u32::try_from(plan.links.iter().filter(|link| link.writes).count()).expect("n");

    for limit in 0..=steps {
        let (workspace, _dir) = workspace().await;
        let session = importer(dataset(1));
        let first = plan_import(&workspace, &session, tree(), None).await.expect("plan");
        let outcome = commit_import(
            &workspace,
            &session,
            &first,
            &Provenance::default(),
            &mut StopAt { limit },
        )
        .await
        .expect("commit");
        assert_eq!(outcome.interrupted, limit < steps, "stopped at {limit} of {steps}");

        import(&workspace, &importer(dataset(1)), tree()).await;
        assert_eq!(records(&workspace).await, expected, "after stopping at {limit}");

        let before = events(&workspace).await;
        let third = import(&workspace, &importer(dataset(1)), tree()).await;
        assert_eq!(third.counts().unchanged, 6, "stopped at {limit}: {:?}", third.counts());
        assert_eq!(events(&workspace).await, before, "stopped at {limit}");
    }
}

#[tokio::test]
async fn a_record_a_sibling_resolved_onto_is_never_a_candidate() {
    let (workspace, _dir) = workspace().await;
    stored_person(&workspace, "Ole", 1850, Some("UID-1")).await;
    let graph = RecordGraph {
        record: "R1".to_owned(),
        entities: vec![
            person(0, None, "Ole", "Hansen"),
            with_uid(person(1, Some("father"), "Ole", "Hansen"), "UID-1"),
        ],
        links: Vec::new(),
    };
    let plan = plan(&workspace, &importer(dataset(1)), vec![graph]).await;
    assert!(matches!(disposition(&plan, 0, 1), Disposition::Link { .. }));
    assert_eq!(
        disposition(&plan, 0, 0),
        &Disposition::New,
        "the father's record is taken"
    );
}

#[tokio::test]
async fn two_records_with_one_external_id_import_one_person() {
    let (workspace, _dir) = workspace().await;
    let graphs = vec![
        RecordGraph {
            record: "I1".to_owned(),
            entities: vec![with_uid(person(0, None, "Ole", "Hansen"), "UID-1")],
            links: Vec::new(),
        },
        RecordGraph {
            record: "I2".to_owned(),
            entities: vec![with_occupation(
                with_uid(person(0, None, "Ole", "Hansen"), "UID-1"),
                "Farmer",
            )],
            links: Vec::new(),
        },
    ];
    let plan = import(&workspace, &importer(dataset(1)), graphs).await;
    assert_eq!(disposition(&plan, 1, 0), &Disposition::Duplicate { of: 0 });
    let persons = vitni_app::list_persons(&workspace).await.expect("persons");
    assert_eq!(persons.len(), 1, "{persons:?}");
}

#[tokio::test]
async fn two_tags_of_one_name_import_one_tag() {
    let (workspace, _dir) = workspace().await;
    let tag = |record: &str, name: &str| RecordGraph {
        record: record.to_owned(),
        entities: vec![StagedEntity {
            local_id: 0,
            item: None,
            fields: EntityFields::Tag(StagedTag { name: name.to_owned() }),
        }],
        links: Vec::new(),
    };
    let plan = import(
        &workspace,
        &importer(dataset(1)),
        vec![tag("T1", "Emigrant"), tag("T2", "emigrant")],
    )
    .await;
    assert_eq!(disposition(&plan, 1, 0), &Disposition::Duplicate { of: 0 });
    assert_eq!(vitni_app::list_tags(&workspace).await.expect("tags").len(), 1);
}

/// Plans `graphs` for review as `session`.
async fn review(workspace: &Workspace, session: &Session, graphs: Vec<RecordGraph>) -> ImportReview {
    ImportReview::plan(workspace, session, graphs, None)
        .await
        .expect("plan")
}

/// Commits `review` as `session`, the identity decisions made as the human operator.
async fn commit_review(workspace: &Workspace, session: &Session, review: ImportReview) -> CommitOutcome {
    review
        .commit(workspace, session, &human(), &Provenance::default(), &mut RunToEnd)
        .await
        .expect("commit")
}

/// The human id the commit gave the own entity of the graph `graph`.
fn committed_id(outcome: &CommitOutcome, graph: usize) -> String {
    outcome
        .entities
        .iter()
        .find(|entity| entity.graph == graph && entity.local_id == 0)
        .map(|entity| entity.human_id.clone())
        .expect("committed")
}

fn decided() -> IdentityDecision {
    IdentityDecision {
        provenance: Provenance {
            rationale: Some("same farm, same year".to_owned()),
            ..Provenance::default()
        },
        assessment: None,
    }
}

#[tokio::test]
async fn a_record_without_candidates_asks_nothing() {
    let (workspace, _dir) = workspace().await;
    let review = review(&workspace, &importer(dataset(1)), vec![individual("I1", "Ole")]).await;
    assert_eq!(review.next_question(&workspace).await.expect("question"), None);
}

#[tokio::test]
async fn a_candidate_is_asked_about_with_both_records_labelled() {
    let (workspace, _dir) = workspace().await;
    let stored = stored_person(&workspace, "Ole", 1850, None).await;
    let review = review(&workspace, &importer(dataset(1)), vec![individual("I1", "Ole")]).await;

    let question = review
        .next_question(&workspace)
        .await
        .expect("question")
        .expect("a candidate");
    assert_eq!(question.kind, MatchableKind::Person);
    assert_eq!(question.candidate.human_id, stored);
    assert_eq!(question.candidate_label, "Ole Hansen");
    assert_eq!(question.incoming_label, "Ole Hansen");
    assert_eq!(
        question.incoming_origin.as_ref().map(|origin| origin.record.as_str()),
        Some("I1")
    );
    assert_eq!((question.position, question.total), (1, 1));
}

#[tokio::test]
async fn a_person_decided_same_is_imported_and_merged_into_the_candidate() {
    let (workspace, _dir) = workspace().await;
    let stored = stored_person(&workspace, "Ole", 1850, None).await;
    let session = importer(dataset(1));
    let mut review = review(&workspace, &session, vec![individual("I1", "Ole")]).await;
    review
        .answer(&workspace, &session, PairAnswer::Same(decided()))
        .await
        .expect("answer");
    assert_eq!(review.next_question(&workspace).await.expect("question"), None);

    let outcome = commit_review(&workspace, &session, review).await;
    let imported = committed_id(&outcome, 0);
    assert_ne!(imported, stored, "the record keeps its own persona");
    assert_eq!(
        vitni_app::pair_decision(&workspace, &stored, &imported)
            .await
            .expect("decision"),
        Some(PairDecision::SameCluster)
    );
    assert_eq!(outcome.deferred, 0);
}

#[tokio::test]
async fn a_place_decided_same_is_reused_and_resolves_so_on_the_next_run() {
    let (workspace, _dir) = workspace().await;
    let new = NewPlace {
        human_id: None,
        place_type: PlaceType::City,
        name: Some("Mandal".to_owned()),
    };
    let stored = vitni_app::create_place(&workspace, &human(), new, Provenance::default(), &[])
        .await
        .expect("place");
    let session = importer(dataset(1));
    let mut review = review(&workspace, &session, vec![place("plac:Mandal", "Mandal")]).await;
    let question = review
        .next_question(&workspace)
        .await
        .expect("question")
        .expect("a candidate");
    assert_eq!(
        (question.kind, question.candidate.human_id.as_str()),
        (MatchableKind::Place, stored.as_str())
    );
    review
        .answer(&workspace, &session, PairAnswer::Same(decided()))
        .await
        .expect("answer");

    let outcome = commit_review(&workspace, &session, review).await;
    assert_eq!(vitni_app::list_places(&workspace).await.expect("places").len(), 1);
    assert_eq!(committed_id(&outcome, 0), stored);
    assert_eq!(
        outcome.resolved.iter().map(|item| item.decision).collect::<Vec<_>>(),
        vec![ResolutionDecision::Matched]
    );

    let run = session.import_run().expect("a run");
    run.ensure_started(workspace.store()).await.expect("start");
    vitni_app::finish_import_run(
        &workspace,
        &human(),
        run.id(),
        outcome.resolved,
        ImportCounts::default(),
    )
    .await
    .expect("finish");
    let again = plan(&workspace, &importer(dataset(1)), vec![place("plac:Mandal", "Mandal")]).await;
    let Disposition::Link { target, basis } = disposition(&again, 0, 0) else {
        panic!("expected a link: {:?}", disposition(&again, 0, 0));
    };
    assert_eq!(
        (target.human_id.as_str(), *basis),
        (stored.as_str(), LinkBasis::Recorded)
    );
}

#[tokio::test]
async fn the_position_moves_on_past_a_reused_record() {
    let (workspace, _dir) = workspace().await;
    let new = NewPlace {
        human_id: None,
        place_type: PlaceType::City,
        name: Some("Mandal".to_owned()),
    };
    vitni_app::create_place(&workspace, &human(), new, Provenance::default(), &[])
        .await
        .expect("place");
    stored_person(&workspace, "Ole", 1850, None).await;
    let session = importer(dataset(1));
    let graphs = vec![place("plac:Mandal", "Mandal"), individual("I1", "Ole")];
    let mut review = review(&workspace, &session, graphs).await;
    let first = review
        .next_question(&workspace)
        .await
        .expect("question")
        .expect("a candidate");
    assert_eq!((first.kind, first.position, first.total), (MatchableKind::Place, 1, 2));
    review
        .answer(&workspace, &session, PairAnswer::Same(decided()))
        .await
        .expect("answer");

    let second = review
        .next_question(&workspace)
        .await
        .expect("question")
        .expect("a candidate");
    assert_eq!(
        (second.kind, second.position, second.total),
        (MatchableKind::Person, 2, 2)
    );
}

#[tokio::test]
async fn not_the_same_asks_the_next_candidate_and_records_each_distinction() {
    let (workspace, _dir) = workspace().await;
    let first = stored_person(&workspace, "Ole", 1850, None).await;
    let second = stored_person(&workspace, "Ole", 1850, None).await;
    let session = importer(dataset(1));
    let mut review = review(&workspace, &session, vec![individual("I1", "Ole")]).await;

    let mut asked = Vec::new();
    while let Some(question) = review.next_question(&workspace).await.expect("question") {
        asked.push(question.candidate.human_id);
        review
            .answer(&workspace, &session, PairAnswer::Distinct(decided()))
            .await
            .expect("answer");
    }
    asked.sort();
    assert_eq!(asked, vec![first.clone(), second.clone()]);

    let outcome = commit_review(&workspace, &session, review).await;
    let imported = committed_id(&outcome, 0);
    for stored in [first, second] {
        assert_eq!(
            vitni_app::pair_decision(&workspace, &imported, &stored)
                .await
                .expect("decision"),
            Some(PairDecision::Distinct)
        );
    }
    assert_eq!(outcome.deferred, 0, "every candidate was decided");
}

#[tokio::test]
async fn decide_later_imports_as_new_and_leaves_the_pair_undecided() {
    let (workspace, _dir) = workspace().await;
    let stored = stored_person(&workspace, "Ole", 1850, None).await;
    let session = importer(dataset(1));
    let mut review = review(&workspace, &session, vec![individual("I1", "Ole")]).await;
    review
        .answer(&workspace, &session, PairAnswer::Later)
        .await
        .expect("answer");
    assert_eq!(review.next_question(&workspace).await.expect("question"), None);

    let outcome = commit_review(&workspace, &session, review).await;
    let imported = committed_id(&outcome, 0);
    assert_eq!(outcome.deferred, 1);
    assert_eq!(
        vitni_app::pair_decision(&workspace, &imported, &stored)
            .await
            .expect("decision"),
        None
    );
}

/// A census `title` by Statistisk sentralbyrå, its own record.
fn census(record: &str, title: &str) -> RecordGraph {
    RecordGraph {
        record: record.to_owned(),
        entities: vec![StagedEntity {
            local_id: 0,
            item: None,
            fields: EntityFields::Source(StagedSource {
                title: Some(title.to_owned()),
                author: Some("Statistisk sentralbyrå".to_owned()),
                pub_info: Some("Kristiania".to_owned()),
                abbrev: None,
                restrictions: BTreeSet::default(),
            }),
        }],
        links: Vec::new(),
    }
}

/// A stored census `title` by Statistisk sentralbyrå, entered at the keyboard.
async fn stored_census(workspace: &Workspace, title: &str) -> String {
    let session = human();
    let new = NewSource {
        human_id: None,
        title: Some(title.to_owned()),
    };
    let source = vitni_app::create_source(workspace, &session, new, Provenance::default(), &[])
        .await
        .expect("source");
    let author = "Statistisk sentralbyrå".to_owned();
    vitni_app::set_source_author(workspace, &session, &source, author, MutationMeta::default())
        .await
        .expect("author");
    let pub_info = "Kristiania".to_owned();
    vitni_app::set_source_pub_info(workspace, &session, &source, pub_info, MutationMeta::default())
        .await
        .expect("publication");
    source
}

/// A stored place named `name`, entered at the keyboard.
async fn stored_place(workspace: &Workspace, name: &str) -> String {
    let new = NewPlace {
        human_id: None,
        place_type: PlaceType::City,
        name: Some(name.to_owned()),
    };
    vitni_app::create_place(workspace, &human(), new, Provenance::default(), &[])
        .await
        .expect("place")
}

#[tokio::test]
async fn the_plan_summary_counts_each_kind_by_disposition() {
    let (workspace, _dir) = workspace().await;
    import(&workspace, &importer(dataset(1)), tree()).await;

    let review = review(&workspace, &importer(dataset(1)), tree()).await;
    let summary = review.summary();
    let kinds: Vec<(MatchableKind, PlanCounts)> = summary.kinds.iter().map(|row| (row.kind, row.counts)).collect();
    let unchanged = |n| PlanCounts {
        unchanged: n,
        ..PlanCounts::default()
    };
    assert_eq!(
        kinds,
        vec![
            (MatchableKind::Place, unchanged(1)),
            (MatchableKind::Person, unchanged(2)),
            (MatchableKind::Family, unchanged(1)),
            (MatchableKind::Event, unchanged(2)),
        ]
    );
    assert_eq!(summary.candidates, 0);
}

#[tokio::test]
async fn a_probable_question_counts_the_probable_pairs_of_its_kind() {
    let (workspace, _dir) = workspace().await;
    stored_census(&workspace, "Folketelling 1865").await;
    stored_census(&workspace, "Folketelling 1875").await;
    let graphs = vec![census("S1", "Folketelling 1865"), census("S2", "Folketelling 1875")];
    let review = review(&workspace, &importer(dataset(1)), graphs).await;
    assert_eq!(review.summary().candidates, 2);

    let question = review
        .next_question(&workspace)
        .await
        .expect("question")
        .expect("a candidate");
    assert_eq!(
        question.assessment.band,
        MatchBand::Probable,
        "{:?}",
        question.assessment
    );
    assert_eq!(
        question.group,
        Some(MatchGroup {
            band: MatchBand::Probable,
            remaining: 2
        })
    );
}

#[tokio::test]
async fn same_for_the_group_reuses_every_probable_source_each_with_its_resolution() {
    let (workspace, _dir) = workspace().await;
    let first = stored_census(&workspace, "Folketelling 1865").await;
    let second = stored_census(&workspace, "Folketelling 1875").await;
    let session = importer(dataset(1));
    let graphs = vec![census("S1", "Folketelling 1865"), census("S2", "Folketelling 1875")];
    let mut review = review(&workspace, &session, graphs).await;
    review
        .answer_group(&workspace, &session, decided())
        .await
        .expect("answer");
    assert_eq!(review.next_question(&workspace).await.expect("question"), None);

    let outcome = commit_review(&workspace, &session, review).await;
    assert_eq!(vitni_app::list_sources(&workspace).await.expect("sources").len(), 2);
    assert_eq!((committed_id(&outcome, 0), committed_id(&outcome, 1)), (first, second));
    assert_eq!(
        outcome.resolved.iter().map(|item| item.decision).collect::<Vec<_>>(),
        vec![ResolutionDecision::Matched, ResolutionDecision::Matched]
    );
}

#[tokio::test]
async fn same_for_the_group_merges_every_probable_person() {
    let (workspace, _dir) = workspace().await;
    let ole = stored_person(&workspace, "Ole", 1850, None).await;
    let hans = stored_person(&workspace, "Hans", 1850, None).await;
    let session = importer(dataset(1));
    let graphs = vec![individual("I1", "Ole"), individual("I2", "Hans")];
    let mut review = review(&workspace, &session, graphs).await;
    let question = review
        .next_question(&workspace)
        .await
        .expect("question")
        .expect("a candidate");
    assert_eq!(question.group.map(|group| group.remaining), Some(2));
    review
        .answer_group(&workspace, &session, decided())
        .await
        .expect("answer");
    assert_eq!(review.next_question(&workspace).await.expect("question"), None);

    let outcome = commit_review(&workspace, &session, review).await;
    for (graph, stored) in [(0, ole), (1, hans)] {
        let imported = committed_id(&outcome, graph);
        assert_eq!(
            vitni_app::pair_decision(&workspace, &stored, &imported)
                .await
                .expect("decision"),
            Some(PairDecision::SameCluster)
        );
    }
    assert_eq!(outcome.deferred, 0);
}

#[tokio::test]
async fn deferring_the_rest_imports_every_candidate_as_new_for_later() {
    let (workspace, _dir) = workspace().await;
    stored_place(&workspace, "Mandal").await;
    stored_person(&workspace, "Ole", 1850, None).await;
    let session = importer(dataset(1));
    let graphs = vec![place("plac:Mandal", "Mandal"), individual("I1", "Ole")];
    let mut review = review(&workspace, &session, graphs).await;
    review.defer_rest();
    assert_eq!(review.next_question(&workspace).await.expect("question"), None);

    let outcome = commit_review(&workspace, &session, review).await;
    assert_eq!(outcome.deferred, 2);
    assert_eq!(vitni_app::list_places(&workspace).await.expect("places").len(), 2);
}

#[tokio::test]
async fn a_reviewed_commit_stops_when_its_control_does() {
    let (workspace, _dir) = workspace().await;
    let session = importer(dataset(1));
    let review = review(&workspace, &session, tree()).await;
    let outcome = review
        .commit(
            &workspace,
            &session,
            &human(),
            &Provenance::default(),
            &mut StopAt { limit: 1 },
        )
        .await
        .expect("commit");
    assert!(outcome.interrupted);
}

#[tokio::test]
async fn same_for_the_group_on_a_pair_outside_any_group_decides_nothing() {
    let (workspace, _dir) = workspace().await;
    stored_census(&workspace, "Folketelling 1865").await;
    stored_census(&workspace, "Folketelling 1875").await;
    let session = importer(dataset(1));
    let mut bare = census("S1", "Folketelling 1865");
    if let EntityFields::Source(source) = &mut bare.entities[0].fields {
        source.author = None;
        source.pub_info = None;
    }
    let graphs = vec![bare, census("S2", "Folketelling 1875")];
    let mut review = review(&workspace, &session, graphs).await;
    let first = review
        .next_question(&workspace)
        .await
        .expect("question")
        .expect("a candidate");
    assert_eq!((first.assessment.band, first.group), (MatchBand::Possible, None));

    review
        .answer_group(&workspace, &session, decided())
        .await
        .expect("answer");
    let again = review
        .next_question(&workspace)
        .await
        .expect("question")
        .expect("still asked");
    assert_eq!(again.incoming_origin, first.incoming_origin, "nothing was decided");
    review
        .answer(&workspace, &session, PairAnswer::Later)
        .await
        .expect("answer");
    let second = review
        .next_question(&workspace)
        .await
        .expect("question")
        .expect("S2 is still open");
    assert_eq!(second.group.map(|group| group.remaining), Some(1));
}
