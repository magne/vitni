//! Person use-case integration tests: create / name / show / list against a temp workspace dir.
//!
//! These exercise the full application path — id generation, meta stamping, command execution
//! through the engine-neutral store, projection query — over a real on-disk workspace directory.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use genealogy_app::{
    AppDefaults, NewCitation, NewFact, NewPerson, NewSource, OperatorConfig, PersonNameParts, Provenance, Session,
    Workspace, WorkspaceDefaults, add_name, assert_association, assert_fact, create_citation, create_person,
    create_source, list_persons, show_person,
};
use genealogy_core::enums::{AssociationRole, EvidenceLevel, FactType};
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind, Confidence};
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

async fn workspace() -> (Workspace, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, dir)
}

fn new_person(given: &str, surname: &str) -> NewPerson {
    NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some(surname.to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
    }
}

#[tokio::test]
async fn create_auto_allocates_sequential_human_ids() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let first = create_person(&ws, &session, new_person("Ada", "Lovelace"))
        .await
        .expect("create");
    let second = create_person(&ws, &session, new_person("Alan", "Turing"))
        .await
        .expect("create");
    assert_eq!(first, "I0001");
    assert_eq!(second, "I0002");
}

#[tokio::test]
async fn create_honors_a_supplied_id_then_rejects_a_duplicate() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let supplied = NewPerson {
        human_id: Some("I0500".to_owned()),
        name: Some(PersonNameParts::simple(
            Some("Grace".to_owned()),
            Some("Hopper".to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
    };
    let assigned = create_person(&ws, &session, supplied.clone()).await.expect("create");
    assert_eq!(assigned, "I0500");

    let err = create_person(&ws, &session, supplied).await;
    assert!(matches!(err, Err(genealogy_app::AppError::HumanIdTaken(id)) if id == "I0500"));
}

#[tokio::test]
async fn show_reflects_an_added_name() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let id = create_person(
        &ws,
        &session,
        NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(Some("Ada".to_owned()), None)),
            evidence_level: EvidenceLevel::Conclusion,
        },
    )
    .await
    .expect("create");

    add_name(
        &ws,
        &session,
        &id,
        PersonNameParts::simple(Some("Augusta".to_owned()), Some("Lovelace".to_owned())),
        Provenance::default(),
        &[],
    )
    .await
    .expect("add name");

    let summary = show_person(&ws, &id).await.expect("show").expect("person exists");
    assert_eq!(summary.human_id, "I0001");
    // The first asserted name (given-only "Ada") is the display name.
    assert_eq!(summary.display_name.as_deref(), Some("Ada"));
}

#[tokio::test]
async fn list_returns_persons_in_human_id_order() {
    let (ws, _dir) = workspace().await;
    let session = session();
    create_person(&ws, &session, new_person("Ada", "Lovelace"))
        .await
        .expect("create");
    create_person(&ws, &session, new_person("Alan", "Turing"))
        .await
        .expect("create");

    let people = list_persons(&ws).await.expect("list");
    let ids: Vec<&str> = people.iter().map(|p| p.human_id.as_str()).collect();
    assert_eq!(ids, ["I0001", "I0002"]);
    assert_eq!(people[0].display_name.as_deref(), Some("Ada Lovelace"));
}

#[tokio::test]
async fn list_surfaces_facts_and_resolves_association_targets_to_human_ids() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let john = create_person(&ws, &session, new_person("John", "Smith"))
        .await
        .expect("create john");
    let jane = create_person(&ws, &session, new_person("Jane", "Doe"))
        .await
        .expect("create jane");

    assert_fact(
        &ws,
        &session,
        &john,
        NewFact {
            fact_type: FactType::Occupation,
            value: Some("Carpenter".to_owned()),
            date: None,
        },
        Provenance {
            confidence: Confidence::High,
            rationale: None,
        },
        &[],
    )
    .await
    .expect("assert fact");
    assert_association(&ws, &session, &john, &jane, AssociationRole::Witness)
        .await
        .expect("assert association");

    let summary = show_person(&ws, &john).await.expect("show").expect("john exists");
    assert_eq!(summary.facts.len(), 1, "the occupation fact surfaces");
    assert_eq!(summary.facts[0].fact.fact_type, FactType::Occupation);
    assert_eq!(summary.facts[0].fact.value.as_deref(), Some("Carpenter"));
    assert_eq!(
        summary.facts[0].confidence,
        Confidence::High,
        "the asserted confidence surfaces on the fact summary"
    );
    assert_eq!(summary.associations.len(), 1, "the association surfaces");
    assert_eq!(
        summary.associations[0].other.human_id, jane,
        "the association target resolves to its human_id"
    );
    assert_eq!(summary.associations[0].role, AssociationRole::Witness);
}

#[tokio::test]
async fn a_facts_citations_resolve_with_their_creation_provenance() {
    // Backs the "Why we believe" popover: a fact's per-claim citations resolve to full refs (source
    // title, page) carrying the creating operator + timestamp ("asserted by …").
    let (ws, _dir) = workspace().await;
    let session = session();

    let source = create_source(
        &ws,
        &session,
        NewSource {
            human_id: None,
            title: Some("1850 U.S. Census".to_owned()),
        },
    )
    .await
    .expect("create source");
    let citation = create_citation(
        &ws,
        &session,
        NewCitation {
            human_id: None,
            source: source.clone(),
            page: Some("p. 14".to_owned()),
        },
    )
    .await
    .expect("create citation");
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"))
        .await
        .expect("create person");
    assert_fact(
        &ws,
        &session,
        &person,
        NewFact {
            fact_type: FactType::Birth,
            value: None,
            date: None,
        },
        Provenance {
            confidence: Confidence::High,
            rationale: None,
        },
        std::slice::from_ref(&citation),
    )
    .await
    .expect("assert fact");

    let summary = show_person(&ws, &person).await.expect("show").expect("person exists");
    let fact = summary
        .facts
        .iter()
        .find(|fact| fact.fact.fact_type == FactType::Birth)
        .expect("the birth fact surfaces");
    assert_eq!(fact.citations.len(), 1, "the fact resolves its backing citation");
    let resolved = &fact.citations[0];
    assert_eq!(resolved.human_id, citation, "the resolved citation is the one attached");
    assert_eq!(resolved.source_title.as_deref(), Some("1850 U.S. Census"));
    assert_eq!(resolved.page.as_deref(), Some("p. 14"));
    assert_eq!(
        resolved.asserted_by.as_deref(),
        Some("Tester"),
        "the citation carries its creating operator"
    );
    assert!(
        resolved.asserted_at.is_some(),
        "the citation carries its creation timestamp"
    );
}

#[tokio::test]
async fn families_for_person_reports_partner_and_child_roles() {
    use genealogy_app::{
        ChildParentRelationship, PersonFamilyRole, add_child, add_partner, create_family, families_for_person,
    };

    let (ws, _dir) = workspace().await;
    let session = session();
    let parent = create_person(&ws, &session, new_person("Jane", "Doe"))
        .await
        .expect("create parent");
    let child = create_person(&ws, &session, new_person("Junior", "Doe"))
        .await
        .expect("create child");
    let family = create_family(&ws, &session).await.expect("create family");
    add_partner(&ws, &session, &family, &parent).await.expect("add partner");
    add_child(
        &ws,
        &session,
        &family,
        &child,
        vec![(parent.clone(), ChildParentRelationship::Birth)],
    )
    .await
    .expect("add child");

    let parent_families = families_for_person(&ws, &parent).await.expect("parent families");
    assert_eq!(parent_families.len(), 1);
    assert_eq!(parent_families[0].family_human_id, family);
    assert_eq!(parent_families[0].role, PersonFamilyRole::Partner);
    assert_eq!(
        parent_families[0].children,
        vec![(child.clone(), vec![(parent.clone(), ChildParentRelationship::Birth)])]
    );

    let child_families = families_for_person(&ws, &child).await.expect("child families");
    assert_eq!(child_families.len(), 1);
    assert_eq!(
        child_families[0].role,
        PersonFamilyRole::Child(vec![(parent.clone(), ChildParentRelationship::Birth)])
    );
    assert_eq!(child_families[0].partners, vec![parent.clone()]);
}

#[tokio::test]
async fn show_family_surfaces_partners_children_and_a_linked_event() {
    use genealogy_app::{
        ChildParentRelationship, EventType, NewEvent, add_child, add_partner, create_event, create_family,
        link_family_event, show_family,
    };

    let (ws, _dir) = workspace().await;
    let session = session();
    let partner_a = create_person(&ws, &session, new_person("Mary", "Doe"))
        .await
        .expect("a");
    let partner_b = create_person(&ws, &session, new_person("John", "Smith"))
        .await
        .expect("b");
    let child = create_person(&ws, &session, new_person("Jonathan", "Smith"))
        .await
        .expect("c");
    let family = create_family(&ws, &session).await.expect("family");
    add_partner(&ws, &session, &family, &partner_a)
        .await
        .expect("partner a");
    add_partner(&ws, &session, &family, &partner_b)
        .await
        .expect("partner b");
    add_child(
        &ws,
        &session,
        &family,
        &child,
        vec![
            (partner_a.clone(), ChildParentRelationship::Birth),
            (partner_b.clone(), ChildParentRelationship::Step),
        ],
    )
    .await
    .expect("child");
    let marriage = create_event(
        &ws,
        &session,
        NewEvent {
            human_id: None,
            event_type: EventType::Marriage,
        },
    )
    .await
    .expect("event");
    link_family_event(&ws, &session, &family, &marriage)
        .await
        .expect("link event");

    let summary = show_family(&ws, &family).await.expect("show").expect("family");
    assert!(!summary.id.is_empty(), "the stable family id is surfaced");
    assert_eq!(summary.partners.len(), 2);
    assert_eq!(summary.partners[0].name.as_deref(), Some("Mary Doe"));
    assert_eq!(summary.children.len(), 1);
    assert_eq!(
        summary.children[0].relationships,
        vec![
            (partner_a.clone(), ChildParentRelationship::Birth),
            (partner_b.clone(), ChildParentRelationship::Step),
        ],
        "per-partner relationships, by partner human_id"
    );
    assert_eq!(summary.events.len(), 1);
    assert_eq!(summary.events[0].human_id, marriage);
    assert_eq!(summary.events[0].event_type, Some(EventType::Marriage));
}

#[tokio::test]
async fn missing_person_and_empty_name_surface_distinct_errors() {
    let (ws, _dir) = workspace().await;
    let session = session();
    create_person(&ws, &session, new_person("Ada", "Lovelace"))
        .await
        .expect("create");

    let missing = add_name(
        &ws,
        &session,
        "I9999",
        PersonNameParts::simple(Some("X".to_owned()), None),
        Provenance::default(),
        &[],
    )
    .await;
    assert!(matches!(missing, Err(genealogy_app::AppError::PersonNotFound(id)) if id == "I9999"));

    let empty = add_name(
        &ws,
        &session,
        "I0001",
        PersonNameParts::simple(None, None),
        Provenance::default(),
        &[],
    )
    .await;
    assert!(matches!(empty, Err(genealogy_app::AppError::Domain(_))));
}
