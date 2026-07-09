//! Person use-case integration tests: create / name / show / list against a temp workspace dir.
//!
//! These exercise the full application path — id generation, meta stamping, command execution
//! through the engine-neutral store, projection query — over a real on-disk workspace directory.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;

use genealogy_app::{
    AppDefaults, Centimorgans, DnaProvider, MutationMeta, NewCitation, NewDnaMatch, NewDnaTest, NewEvent, NewFact,
    NewMedia, NewNote, NewPerson, NewPlace, NewRepository, NewSource, OperatorConfig, PersonNameParts, Provenance,
    Session, TagChangeSet, TagTarget, Workspace, WorkspaceDefaults, add_name, assert_association,
    assert_dna_test_haplogroup, assert_fact, attach_person_note, change_log_for_citation, change_log_for_dna_match,
    change_log_for_dna_test, change_log_for_event, change_log_for_family, change_log_for_media, change_log_for_note,
    change_log_for_person, change_log_for_place, change_log_for_repository, change_log_for_source, change_log_for_tag,
    commit_tag_change_set, create_citation, create_dna_test, create_event, create_media, create_note, create_person,
    create_place, create_repository, create_source, create_tag, list_persons, merge_persons, observe_dna_match,
    rename_tag, set_dna_match_status, set_dna_test_provider, set_event_description, set_family_restrictions,
    set_media_mime, set_note_text, set_page, set_place_code, set_repository_name, set_source_author, show_dna_test,
    show_person, tag_person, undo_assertion,
};
use genealogy_app::{EvidenceAnalysis, EvidenceKind, InformationKind, SourceQuality};
use genealogy_core::enums::{AssociationRole, EventType, EvidenceLevel, FactType, PlaceType, Restriction};
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

    let first = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("create");
    let second = create_person(&ws, &session, new_person("Alan", "Turing"), Provenance::default(), &[])
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
    let assigned = create_person(&ws, &session, supplied.clone(), Provenance::default(), &[])
        .await
        .expect("create");
    assert_eq!(assigned, "I0500");

    let err = create_person(&ws, &session, supplied, Provenance::default(), &[]).await;
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
        Provenance::default(),
        &[],
    )
    .await
    .expect("create");

    add_name(
        &ws,
        &session,
        &id,
        PersonNameParts::simple(Some("Augusta".to_owned()), Some("Lovelace".to_owned())),
        MutationMeta::default(),
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
    create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("create");
    create_person(&ws, &session, new_person("Alan", "Turing"), Provenance::default(), &[])
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
    let john = create_person(&ws, &session, new_person("John", "Smith"), Provenance::default(), &[])
        .await
        .expect("create john");
    let jane = create_person(&ws, &session, new_person("Jane", "Doe"), Provenance::default(), &[])
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
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::High,
                rationale: None,
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("assert fact");
    assert_association(
        &ws,
        &session,
        &john,
        &jane,
        AssociationRole::Witness,
        MutationMeta::default(),
    )
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
        Provenance::default(),
        &[],
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
        Provenance::default(),
        &[],
    )
    .await
    .expect("create citation");
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
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
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::High,
                rationale: None,
                evidence_analysis: None,
            },
            citations: std::slice::from_ref(&citation),
            supersedes: None,
        },
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
    let parent = create_person(&ws, &session, new_person("Jane", "Doe"), Provenance::default(), &[])
        .await
        .expect("create parent");
    let child = create_person(&ws, &session, new_person("Junior", "Doe"), Provenance::default(), &[])
        .await
        .expect("create child");
    let family = create_family(&ws, &session, Provenance::default(), &[])
        .await
        .expect("create family");
    add_partner(&ws, &session, &family, &parent, MutationMeta::default())
        .await
        .expect("add partner");
    add_child(
        &ws,
        &session,
        &family,
        &child,
        vec![(parent.clone(), ChildParentRelationship::Birth)],
        MutationMeta::default(),
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
    let partner_a = create_person(&ws, &session, new_person("Mary", "Doe"), Provenance::default(), &[])
        .await
        .expect("a");
    let partner_b = create_person(&ws, &session, new_person("John", "Smith"), Provenance::default(), &[])
        .await
        .expect("b");
    let child = create_person(
        &ws,
        &session,
        new_person("Jonathan", "Smith"),
        Provenance::default(),
        &[],
    )
    .await
    .expect("c");
    let family = create_family(&ws, &session, Provenance::default(), &[])
        .await
        .expect("family");
    add_partner(&ws, &session, &family, &partner_a, MutationMeta::default())
        .await
        .expect("partner a");
    add_partner(&ws, &session, &family, &partner_b, MutationMeta::default())
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
        MutationMeta::default(),
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
        Provenance::default(),
        &[],
    )
    .await
    .expect("event");
    link_family_event(&ws, &session, &family, &marriage, MutationMeta::default())
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
async fn a_partner_carries_its_assertion_id_and_can_be_retracted() {
    use genealogy_app::{add_partner, create_family, show_family, undo_family_assertion};

    let (ws, _dir) = workspace().await;
    let session = session();
    let partner = create_person(&ws, &session, new_person("Mary", "Doe"), Provenance::default(), &[])
        .await
        .expect("partner");
    let family = create_family(&ws, &session, Provenance::default(), &[])
        .await
        .expect("family");
    add_partner(&ws, &session, &family, &partner, MutationMeta::default())
        .await
        .expect("add partner");

    let summary = show_family(&ws, &family).await.expect("show").expect("family");
    assert_eq!(summary.partners.len(), 1);
    let assertion_id = summary.partners[0].assertion_id.clone();
    assert!(!assertion_id.is_empty(), "the partner's assertion id is surfaced");

    undo_family_assertion(&ws, &session, &family, &assertion_id, None)
        .await
        .expect("retract partner");

    let after = show_family(&ws, &family).await.expect("show").expect("family");
    assert!(
        after.partners.is_empty(),
        "retracting the partner removes it from the view"
    );
}

#[tokio::test]
async fn missing_person_and_empty_name_surface_distinct_errors() {
    let (ws, _dir) = workspace().await;
    let session = session();
    create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("create");

    let missing = add_name(
        &ws,
        &session,
        "I9999",
        PersonNameParts::simple(Some("X".to_owned()), None),
        MutationMeta::default(),
    )
    .await;
    assert!(matches!(missing, Err(genealogy_app::AppError::PersonNotFound(id)) if id == "I9999"));

    let empty = add_name(
        &ws,
        &session,
        "I0001",
        PersonNameParts::simple(None, None),
        MutationMeta::default(),
    )
    .await;
    assert!(matches!(empty, Err(genealogy_app::AppError::Domain(_))));
}

#[tokio::test]
async fn merge_links_the_merged_person_as_a_persona_of_the_survivor() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let survivor = create_person(&ws, &session, new_person("John", "Smith"), Provenance::default(), &[])
        .await
        .expect("create survivor");
    let merged = create_person(&ws, &session, new_person("John", "Smyth"), Provenance::default(), &[])
        .await
        .expect("create merged");

    let result = merge_persons(&ws, &session, &survivor, &merged, None)
        .await
        .expect("merge");
    assert_eq!(result.survivor.human_id, survivor);
    assert_eq!(result.merged_human_id, merged);
    assert!(
        result.survivor.merged.iter().any(|persona| persona.human_id == merged),
        "the survivor's summary lists the merged persona: {:?}",
        result.survivor.merged
    );

    // The merged person's own record is untouched — it still resolves.
    let merged_summary = show_person(&ws, &merged).await.expect("show").expect("still exists");
    assert_eq!(merged_summary.human_id, merged);
}

#[tokio::test]
async fn merging_a_person_with_itself_is_rejected_and_emits_no_event() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let solo = create_person(&ws, &session, new_person("Solo", "Person"), Provenance::default(), &[])
        .await
        .expect("create");

    let result = merge_persons(&ws, &session, &solo, &solo, None).await;
    assert!(matches!(result, Err(genealogy_app::AppError::Domain(_))));

    let log = change_log_for_person(&ws, &solo).await.expect("log");
    assert!(
        log.iter().all(|entry| entry.event_type != "PersonsMerged"),
        "a rejected self-merge emits no event: {log:?}"
    );
}

#[tokio::test]
async fn merge_with_an_unknown_human_id_surfaces_person_not_found() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let survivor = create_person(&ws, &session, new_person("John", "Smith"), Provenance::default(), &[])
        .await
        .expect("create");

    let missing_merged = merge_persons(&ws, &session, &survivor, "I9999", None).await;
    assert!(matches!(missing_merged, Err(genealogy_app::AppError::PersonNotFound(id)) if id == "I9999"));

    let missing_survivor = merge_persons(&ws, &session, "I9998", &survivor, None).await;
    assert!(matches!(missing_survivor, Err(genealogy_app::AppError::PersonNotFound(id)) if id == "I9998"));
}

#[tokio::test]
async fn undoing_a_merge_removes_the_persona_link() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let survivor = create_person(&ws, &session, new_person("John", "Smith"), Provenance::default(), &[])
        .await
        .expect("create survivor");
    let merged = create_person(&ws, &session, new_person("John", "Smyth"), Provenance::default(), &[])
        .await
        .expect("create merged");
    merge_persons(&ws, &session, &survivor, &merged, None)
        .await
        .expect("merge");

    let log = change_log_for_person(&ws, &survivor).await.expect("log");
    let merge_entry = log
        .iter()
        .find(|entry| entry.event_type == "PersonsMerged")
        .expect("merge entry logged");
    assert!(merge_entry.can_undo, "a merge assertion can be undone");
    undo_assertion(&ws, &session, &survivor, &merge_entry.assertion_id, None)
        .await
        .expect("undo merge");

    let after = show_person(&ws, &survivor).await.expect("show").expect("survivor");
    assert!(
        after.merged.iter().all(|persona| persona.human_id != merged),
        "undoing the merge removes the persona link: {:?}",
        after.merged
    );
}

#[tokio::test]
async fn a_persons_applied_tag_reflects_a_later_rename_and_recolour() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("create person");
    let tag = commit_tag_change_set(
        &ws,
        &session,
        TagChangeSet {
            target: TagTarget::New,
            name: "Ancestor".to_owned(),
            priority: 1,
            color: "#e5534b".to_owned(),
            provenance: Provenance::default(),
            citations: Vec::new(),
        },
    )
    .await
    .expect("create tag");
    tag_person(&ws, &session, &person, &tag, false, MutationMeta::default())
        .await
        .expect("apply tag");

    let before = show_person(&ws, &person).await.expect("show").expect("person");
    assert_eq!(before.tag_refs.len(), 1);
    assert_eq!(before.tag_refs[0].name, "Ancestor");
    assert_eq!(before.tag_refs[0].color.as_deref(), Some("#e5534b"));

    // Rename + recolour the tag.
    commit_tag_change_set(
        &ws,
        &session,
        TagChangeSet {
            target: TagTarget::Existing { id: tag.clone() },
            name: "Direct ancestor".to_owned(),
            priority: 1,
            color: "#2faa6a".to_owned(),
            provenance: Provenance::default(),
            citations: Vec::new(),
        },
    )
    .await
    .expect("edit tag");

    let after = show_person(&ws, &person).await.expect("show").expect("person");
    assert_eq!(after.tag_refs.len(), 1);
    assert_eq!(
        after.tag_refs[0].name, "Direct ancestor",
        "the applied-tag view reflects the rename"
    );
    assert_eq!(
        after.tag_refs[0].color.as_deref(),
        Some("#2faa6a"),
        "the applied-tag view reflects the recolour"
    );
    // The id is still carried (for the change-set diff) but never derived from a stale label.
    assert_eq!(after.tags, vec![tag]);
}

/// The PR24 acceptance round trip: an assertion made with a rationale, a backing citation, and an
/// Evidence Explained analysis surfaces all three back through the History change-log DTO.
#[tokio::test]
async fn a_fact_assertion_round_trips_rationale_citation_and_evidence_analysis_through_history() {
    let (ws, _dir) = workspace().await;
    let session = session();

    let source = create_source(
        &ws,
        &session,
        NewSource {
            human_id: None,
            title: Some("Parish register".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("source");
    let citation = create_citation(
        &ws,
        &session,
        NewCitation {
            human_id: None,
            source: source.clone(),
            page: Some("f. 3".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("citation");
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("person");

    let analysis = EvidenceAnalysis {
        source: SourceQuality::Original,
        information: InformationKind::Primary,
        evidence: EvidenceKind::Direct,
    };
    assert_fact(
        &ws,
        &session,
        &person,
        NewFact {
            fact_type: FactType::Birth,
            value: None,
            date: None,
        },
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::High,
                rationale: Some("baptism entry".to_owned()),
                evidence_analysis: Some(analysis),
            },
            citations: std::slice::from_ref(&citation),
            supersedes: None,
        },
    )
    .await
    .expect("assert fact");

    let log = change_log_for_person(&ws, &person).await.expect("log");
    let fact = log
        .iter()
        .find(|entry| entry.event_type == "FactAsserted")
        .expect("the fact assertion is logged");
    assert_eq!(
        fact.rationale.as_deref(),
        Some("baptism entry"),
        "the rationale round-trips"
    );
    assert_eq!(fact.confidence, Confidence::High, "the confidence round-trips");
    assert_eq!(
        fact.evidence_analysis,
        Some(analysis),
        "the evidence analysis round-trips"
    );
    assert_eq!(
        fact.citations.len(),
        1,
        "the backing citation round-trips through the change log"
    );
}

/// A non-create mutation called with `supersedes` wraps its command in a supersession: the projection
/// shows the replacement value (not a second, buried assertion) and the change log records it.
#[tokio::test]
async fn superseding_a_name_replaces_it_and_logs_a_supersession() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let person = create_person(
        &ws,
        &session,
        NewPerson {
            human_id: None,
            name: None,
            evidence_level: EvidenceLevel::Conclusion,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("person");
    add_name(
        &ws,
        &session,
        &person,
        PersonNameParts::simple(Some("Ada".to_owned()), Some("Lovelace".to_owned())),
        MutationMeta::default(),
    )
    .await
    .expect("first name");

    let target = change_log_for_person(&ws, &person)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == "NameAsserted")
        .expect("the name assertion is logged")
        .assertion_id;

    add_name(
        &ws,
        &session,
        &person,
        PersonNameParts::simple(Some("Augusta".to_owned()), Some("King".to_owned())),
        MutationMeta {
            provenance: Provenance::default(),
            citations: &[],
            supersedes: Some(&target),
        },
    )
    .await
    .expect("supersede name");

    let summary = show_person(&ws, &person).await.expect("show").expect("person");
    assert_eq!(
        summary.names.len(),
        1,
        "the supersession replaces the name rather than adding a second"
    );
    assert_eq!(
        summary.given.as_deref(),
        Some("Augusta"),
        "the replacement name is now primary"
    );
    assert!(
        change_log_for_person(&ws, &person)
            .await
            .expect("log")
            .iter()
            .any(|entry| entry.event_type == "AssertionSuperseded"),
        "the supersession is recorded in the change log"
    );
}

/// An unparseable `supersedes` id is rejected before any write (parsed like a history undo target).
#[tokio::test]
async fn superseding_with_an_unparseable_assertion_id_is_rejected() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("person");

    let rejected = add_name(
        &ws,
        &session,
        &person,
        PersonNameParts::simple(Some("X".to_owned()), None),
        MutationMeta {
            provenance: Provenance::default(),
            citations: &[],
            supersedes: Some("not-a-uuid"),
        },
    )
    .await;
    assert!(
        matches!(rejected, Err(genealogy_app::AppError::Db(_))),
        "an unparseable supersede id is a malformed-input error: {rejected:?}"
    );
}

/// Undo carries an operator rationale through to the retraction's provenance, replacing the default
/// "Undo" label.
#[tokio::test]
async fn undo_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("person");
    assert_fact(
        &ws,
        &session,
        &person,
        NewFact {
            fact_type: FactType::Birth,
            value: None,
            date: None,
        },
        MutationMeta::default(),
    )
    .await
    .expect("assert fact");

    let fact_assertion = change_log_for_person(&ws, &person)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == "FactAsserted")
        .expect("the fact assertion is logged")
        .assertion_id;
    undo_assertion(
        &ws,
        &session,
        &person,
        &fact_assertion,
        Some("entered in error".to_owned()),
    )
    .await
    .expect("undo");

    let retraction = change_log_for_person(&ws, &person)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == "AssertionRetracted")
        .expect("the retraction is logged");
    assert_eq!(
        retraction.rationale.as_deref(),
        Some("entered in error"),
        "the undo rationale replaces the default label"
    );
}

/// A merge carries an operator rationale through to the `PersonsMerged` provenance.
#[tokio::test]
async fn merge_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let survivor = create_person(&ws, &session, new_person("John", "Smith"), Provenance::default(), &[])
        .await
        .expect("survivor");
    let merged = create_person(&ws, &session, new_person("John", "Smyth"), Provenance::default(), &[])
        .await
        .expect("merged");

    merge_persons(
        &ws,
        &session,
        &survivor,
        &merged,
        Some("same individual per DNA match".to_owned()),
    )
    .await
    .expect("merge");

    let entry = change_log_for_person(&ws, &survivor)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == "PersonsMerged")
        .expect("the merge is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("same individual per DNA match"),
        "the merge rationale is recorded"
    );
}

// PR24 per-aggregate coverage: each test performs one non-create mutation with a caller-supplied
// `MutationMeta` rationale (and confidence, where the aggregate isn't Tag's flat signature) and
// asserts both round-trip through that aggregate's change log.

#[tokio::test]
async fn family_restrictions_records_the_supplied_rationale() {
    use genealogy_app::create_family;

    let (ws, _dir) = workspace().await;
    let session = session();
    let family = create_family(&ws, &session, Provenance::default(), &[])
        .await
        .expect("create family");

    let mut restrictions = BTreeSet::new();
    restrictions.insert(Restriction::Privacy);
    set_family_restrictions(
        &ws,
        &session,
        &family,
        restrictions,
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::High,
                rationale: Some("living partner, privacy requested".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("set restrictions");

    let log = change_log_for_family(&ws, &family).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "RestrictionsChanged")
        .expect("the restrictions change is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("living partner, privacy requested"),
        "the rationale round-trips"
    );
    assert_eq!(entry.confidence, Confidence::High, "the confidence round-trips");
}

#[tokio::test]
async fn event_description_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let event = create_event(
        &ws,
        &session,
        NewEvent {
            human_id: None,
            event_type: EventType::Marriage,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create event");

    set_event_description(
        &ws,
        &session,
        &event,
        "Wedding at St. Mary's".to_owned(),
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::Normal,
                rationale: Some("per parish register".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("set description");

    let log = change_log_for_event(&ws, &event).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "DescriptionSet")
        .expect("the description assertion is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("per parish register"),
        "the rationale round-trips"
    );
}

#[tokio::test]
async fn place_code_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::City,
            name: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create place");

    set_place_code(
        &ws,
        &session,
        &place,
        "OSLO".to_owned(),
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::High,
                rationale: Some("matches gazetteer entry".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("set code");

    let log = change_log_for_place(&ws, &place).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "CodeSet")
        .expect("the code assertion is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("matches gazetteer entry"),
        "the rationale round-trips"
    );
    assert_eq!(entry.confidence, Confidence::High, "the confidence round-trips");
}

#[tokio::test]
async fn source_author_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let source = create_source(
        &ws,
        &session,
        NewSource {
            human_id: None,
            title: Some("Parish register".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create source");

    set_source_author(
        &ws,
        &session,
        &source,
        "Rev. John Doe".to_owned(),
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::Normal,
                rationale: Some("title page attribution".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("set author");

    let log = change_log_for_source(&ws, &source).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "AuthorSet")
        .expect("the author assertion is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("title page attribution"),
        "the rationale round-trips"
    );
}

#[tokio::test]
async fn citation_page_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let source = create_source(
        &ws,
        &session,
        NewSource {
            human_id: None,
            title: Some("1850 U.S. Census".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create source");
    let citation = create_citation(
        &ws,
        &session,
        NewCitation {
            human_id: None,
            source: source.clone(),
            page: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create citation");

    set_page(
        &ws,
        &session,
        &citation,
        "p. 42".to_owned(),
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::High,
                rationale: Some("re-read the microfilm".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("set page");

    let log = change_log_for_citation(&ws, &citation).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "PageSet")
        .expect("the page assertion is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("re-read the microfilm"),
        "the rationale round-trips"
    );
    assert_eq!(entry.confidence, Confidence::High, "the confidence round-trips");
}

#[tokio::test]
async fn repository_name_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let repository = create_repository(
        &ws,
        &session,
        NewRepository {
            human_id: None,
            name: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create repository");

    set_repository_name(
        &ws,
        &session,
        &repository,
        "National Archives".to_owned(),
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::Normal,
                rationale: Some("per correspondence".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("set name");

    let log = change_log_for_repository(&ws, &repository).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "NameSet")
        .expect("the name assertion is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("per correspondence"),
        "the rationale round-trips"
    );
}

#[tokio::test]
async fn media_mime_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let media = create_media(
        &ws,
        &session,
        NewMedia {
            human_id: None,
            path: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create media");

    set_media_mime(
        &ws,
        &session,
        &media,
        "image/jpeg".to_owned(),
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::High,
                rationale: Some("inspected file header".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("set mime");

    let log = change_log_for_media(&ws, &media).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "MimeSet")
        .expect("the mime assertion is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("inspected file header"),
        "the rationale round-trips"
    );
    assert_eq!(entry.confidence, Confidence::High, "the confidence round-trips");
}

#[tokio::test]
async fn note_text_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let note = create_note(
        &ws,
        &session,
        NewNote {
            human_id: None,
            text: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create note");

    set_note_text(
        &ws,
        &session,
        &note,
        "Interviewed the family in 1998.".to_owned(),
        None,
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::Normal,
                rationale: Some("researcher's field notes".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("set text");

    let log = change_log_for_note(&ws, &note).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "RichTextSet")
        .expect("the text assertion is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("researcher's field notes"),
        "the rationale round-trips"
    );
}

// Tag mutations take a flat `Provenance` + citations (data-model §9: tags carry no supersede path),
// not a `MutationMeta` — the signature that makes this one aggregate special among the eleven.
#[tokio::test]
async fn tag_rename_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let tag = create_tag(&ws, &session, "Ancestor".to_owned(), Provenance::default(), &[])
        .await
        .expect("create tag");

    rename_tag(
        &ws,
        &session,
        &tag,
        "Direct ancestor".to_owned(),
        Provenance {
            confidence: Confidence::Normal,
            rationale: Some("clarified during review".to_owned()),
            evidence_analysis: None,
        },
        &[],
    )
    .await
    .expect("rename tag");

    let log = change_log_for_tag(&ws, &tag).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "TagRenamed")
        .expect("the rename is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("clarified during review"),
        "the rationale round-trips"
    );
}

#[tokio::test]
async fn dna_test_provider_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("create person");
    let dna_test = create_dna_test(
        &ws,
        &session,
        NewDnaTest { human_id: None, person },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create dna test");

    set_dna_test_provider(
        &ws,
        &session,
        &dna_test,
        DnaProvider::AncestryDna,
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::High,
                rationale: Some("kit label confirms vendor".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("set provider");

    let log = change_log_for_dna_test(&ws, &dna_test).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "ProviderSet")
        .expect("the provider assertion is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("kit label confirms vendor"),
        "the rationale round-trips"
    );
    assert_eq!(entry.confidence, Confidence::High, "the confidence round-trips");
}

#[tokio::test]
async fn dna_match_status_records_the_supplied_rationale() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let person_a = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("create person a");
    let person_b = create_person(&ws, &session, new_person("Alan", "Turing"), Provenance::default(), &[])
        .await
        .expect("create person b");
    let test_a = create_dna_test(
        &ws,
        &session,
        NewDnaTest {
            human_id: None,
            person: person_a,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create dna test a");
    let test_b = create_dna_test(
        &ws,
        &session,
        NewDnaTest {
            human_id: None,
            person: person_b,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create dna test b");
    let dna_match = observe_dna_match(
        &ws,
        &session,
        NewDnaMatch {
            human_id: None,
            test_a,
            test_b,
            provider: DnaProvider::AncestryDna,
            shared_cm: Centimorgans::from_hundredths(3_500),
            percent_shared: None,
            segment_count: 12,
            largest_segment_cm: Centimorgans::from_hundredths(800),
            predicted_relationship: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("observe match");

    set_dna_match_status(
        &ws,
        &session,
        &dna_match,
        true,
        MutationMeta {
            provenance: Provenance {
                confidence: Confidence::High,
                rationale: Some("confirmed via shared tree research".to_owned()),
                evidence_analysis: None,
            },
            citations: &[],
            supersedes: None,
        },
    )
    .await
    .expect("confirm match");

    let log = change_log_for_dna_match(&ws, &dna_match).await.expect("log");
    let entry = log
        .iter()
        .find(|entry| entry.event_type == "MatchConfirmed")
        .expect("the confirmation is logged");
    assert_eq!(
        entry.rationale.as_deref(),
        Some("confirmed via shared tree research"),
        "the rationale round-trips"
    );
    assert_eq!(entry.confidence, Confidence::High, "the confidence round-trips");
}

// --- PR29 step 2: assertion ids surface on collection-row DTOs (the read side of corrections) ---

/// A name summary carries the same `AssertionId` the change log records for its `NameAsserted` — the
/// id a per-row Edit/Retract targets (ADR 0004 §2).
#[tokio::test]
async fn add_name_summary_carries_the_change_log_assertion_id() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("person");
    add_name(
        &ws,
        &session,
        &person,
        PersonNameParts::simple(Some("Augusta".to_owned()), Some("King".to_owned())),
        MutationMeta::default(),
    )
    .await
    .expect("second name");

    let summary = show_person(&ws, &person).await.expect("show").expect("person");
    let logged: std::collections::BTreeSet<String> = change_log_for_person(&ws, &person)
        .await
        .expect("log")
        .into_iter()
        .filter(|entry| entry.event_type == "NameAsserted")
        .map(|entry| entry.assertion_id)
        .collect();
    let on_summary: std::collections::BTreeSet<String> =
        summary.names.iter().map(|name| name.assertion_id.clone()).collect();
    assert_eq!(
        on_summary, logged,
        "every name row carries its introducing assertion id"
    );
    assert!(summary.names.iter().all(|name| !name.assertion_id.is_empty()));
}

/// Superseding a name replaces the row with one carrying a *new* assertion id (not the retired one).
#[tokio::test]
async fn superseding_a_name_stamps_the_replacement_assertion_id() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("person");
    let original = show_person(&ws, &person)
        .await
        .expect("show")
        .expect("person")
        .names
        .first()
        .expect("one name")
        .assertion_id
        .clone();

    add_name(
        &ws,
        &session,
        &person,
        PersonNameParts::simple(Some("Augusta".to_owned()), Some("King".to_owned())),
        MutationMeta {
            provenance: Provenance::default(),
            citations: &[],
            supersedes: Some(&original),
        },
    )
    .await
    .expect("supersede");

    let summary = show_person(&ws, &person).await.expect("show").expect("person");
    assert_eq!(summary.names.len(), 1, "supersede replaces rather than appends");
    let replacement = &summary.names[0].assertion_id;
    assert_ne!(replacement, &original, "the surviving row carries a new assertion id");
    assert_eq!(summary.given.as_deref(), Some("Augusta"));
}

/// Undoing an attach assertion drops the attached row from the summary (Detach = retract the attach).
#[tokio::test]
async fn undoing_a_note_attach_drops_the_row() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("person");
    let note = create_note(
        &ws,
        &session,
        NewNote {
            human_id: None,
            text: None,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("note");
    attach_person_note(&ws, &session, &person, &note, MutationMeta::default())
        .await
        .expect("attach note");

    let attached = show_person(&ws, &person).await.expect("show").expect("person");
    let attach_assertion = attached.notes.first().expect("one note").assertion_id.clone();
    assert!(
        !attach_assertion.is_empty(),
        "the note row carries its attach assertion id"
    );

    undo_assertion(&ws, &session, &person, &attach_assertion, None)
        .await
        .expect("detach note");

    let after = show_person(&ws, &person).await.expect("show").expect("person");
    assert!(after.notes.is_empty(), "undoing the attach drops the note row");
}

/// Sibling smoke: a DNA-test haplogroup row carries the assertion id its change log records.
#[tokio::test]
async fn haplogroup_row_carries_its_assertion_id() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let person = create_person(&ws, &session, new_person("Ada", "Lovelace"), Provenance::default(), &[])
        .await
        .expect("person");
    let test = create_dna_test(
        &ws,
        &session,
        NewDnaTest { human_id: None, person },
        Provenance::default(),
        &[],
    )
    .await
    .expect("dna test");
    assert_dna_test_haplogroup(&ws, &session, &test, "R-M269".to_owned(), MutationMeta::default())
        .await
        .expect("haplogroup");

    let summary = show_dna_test(&ws, &test).await.expect("show").expect("dna test");
    let row = summary.haplogroups.first().expect("one haplogroup");
    let logged = change_log_for_dna_test(&ws, &test)
        .await
        .expect("log")
        .into_iter()
        .find(|entry| entry.event_type == "HaplogroupAsserted")
        .expect("haplogroup logged")
        .assertion_id;
    assert_eq!(row.assertion_id, logged, "the haplogroup row carries its assertion id");
}

#[tokio::test]
async fn citation_date_value_round_trips_the_full_grammar() {
    use genealogy_app::{
        Calendar, DateInput, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody,
        assert_citation_date_value, build_genealogical_date, show_citation,
    };

    let (ws, _dir) = workspace().await;
    let session = session();
    let source = create_source(
        &ws,
        &session,
        NewSource {
            human_id: None,
            title: Some("Parish register".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create source");
    let citation = create_citation(
        &ws,
        &session,
        NewCitation {
            human_id: None,
            source,
            page: Some("f. 18".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create citation");

    let date = build_genealogical_date(DateInput {
        calendar: Calendar::Julian,
        quality: DateQuality::Estimated,
        body: GenealogicalDateBody::Structured(DateModifier::Range {
            start: DatePoint {
                year: Some(1876),
                month: Some(6),
                day: Some(14),
            },
            end: DatePoint {
                year: Some(1880),
                month: None,
                day: None,
            },
        }),
        new_year_begins: None,
        original_text: Some("bet 1876 and 1880".to_owned()),
        time: None,
    });
    assert_citation_date_value(&ws, &session, &citation, date.clone(), MutationMeta::default())
        .await
        .expect("assert citation date");

    let summary = show_citation(&ws, &citation)
        .await
        .expect("show")
        .expect("citation exists");
    assert_eq!(
        summary.date.as_ref(),
        Some(&date),
        "the full date round-trips structurally"
    );
    let _: Option<GenealogicalDate> = summary.date;
}

#[tokio::test]
async fn media_date_value_round_trips_the_full_grammar() {
    use genealogy_app::{
        Calendar, DateInput, DateModifier, DatePoint, DateQuality, GenealogicalDateBody, assert_media_date_value,
        build_genealogical_date, show_media,
    };

    let (ws, _dir) = workspace().await;
    let session = session();
    let media = create_media(
        &ws,
        &session,
        NewMedia {
            human_id: None,
            path: Some("portrait.jpg".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create media");

    let date = build_genealogical_date(DateInput {
        calendar: Calendar::Julian,
        quality: DateQuality::Estimated,
        body: GenealogicalDateBody::Structured(DateModifier::About(DatePoint {
            year: Some(1876),
            month: Some(6),
            day: Some(14),
        })),
        new_year_begins: None,
        original_text: Some("abt 14 Jun 1876".to_owned()),
        time: None,
    });
    assert_media_date_value(&ws, &session, &media, date.clone(), MutationMeta::default())
        .await
        .expect("assert media date");

    let summary = show_media(&ws, &media).await.expect("show").expect("media exists");
    assert_eq!(
        summary.date.as_ref(),
        Some(&date),
        "the full date round-trips structurally"
    );
}
