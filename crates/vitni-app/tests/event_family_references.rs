//! The read side of event and family clusters (ADR 0039 §5): a merged event or family reads as one
//! record — its participants, partners, children and claims unioned — and every reference to a member
//! names the cluster's root.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;

use uuid::Uuid;
use vitni_app::{
    AppDefaults, IdentityDecision, MutationMeta, NewCitation, NewEvent, NewNote, NewParticipation, NewPerson,
    NewResearchNote, NewResearchNoteSubject, NewSource, OperatorConfig, PersonNameParts, Provenance, Session,
    Workspace, WorkspaceDefaults, add_child, add_event_citation, add_partner, ancestors, assert_participation,
    attach_event_note, attach_family_note, create_citation, create_event, create_family, create_note, create_person,
    create_research_note, create_source, create_tag, descendants, event_claim_owner, families_for_person,
    family_claim_owner, link_family_event, list_event_rows, list_events, list_families, list_family_rows,
    list_research_notes_about, merge_events, merge_families, remove_child, set_event_restrictions,
    set_family_restrictions, show_event, show_family, show_note, show_person, show_research_note, show_source,
    tag_event, tag_family, undo_event_assertion, workspace_counts,
};
use vitni_core::enums::{ChildParentRelationship, EventType, EvidenceLevel, ParticipantRole, Restriction};
use vitni_core::ids::AgentId;
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
            Some("Bakke".to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("person")
}

async fn marriage(ws: &Workspace) -> String {
    let new = NewEvent {
        human_id: None,
        event_type: EventType::Marriage,
    };
    create_event(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("event")
}

async fn participate(ws: &Workspace, person: &str, event: &str, role: ParticipantRole) {
    let new = NewParticipation::with_role(role);
    assert_participation(ws, &session(), person, event, new, MutationMeta::default())
        .await
        .expect("participation");
}

async fn family(ws: &Workspace, partners: [&str; 2], event: &str) -> String {
    let family = create_family(ws, &session(), Provenance::default(), &[])
        .await
        .expect("family");
    for partner in partners {
        add_partner(ws, &session(), &family, partner, MutationMeta::default())
            .await
            .expect("partner");
    }
    link_family_event(ws, &session(), &family, event, MutationMeta::default())
        .await
        .expect("link event");
    family
}

/// A citation of a parish register, returning the source and citation `human_id`s.
async fn citation(ws: &Workspace) -> (String, String) {
    let new = NewSource {
        human_id: None,
        title: Some("Parish register".to_owned()),
    };
    let source = create_source(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("source");
    let new = NewCitation {
        human_id: None,
        source: source.clone(),
        page: Some("p. 12".to_owned()),
    };
    let citation = create_citation(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("citation");
    (source, citation)
}

async fn note(ws: &Workspace, text: &str) -> String {
    let new = NewNote {
        human_id: None,
        text: Some(text.to_owned()),
    };
    create_note(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("note")
}

async fn research_note(ws: &Workspace, subject: NewResearchNoteSubject) -> String {
    let new = NewResearchNote {
        human_id: None,
        subjects: vec![subject],
        title: Some("Two copies of one marriage?".to_owned()),
    };
    create_research_note(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("research note")
}

/// Two copies of one marriage, each recorded from a different source: the root copy names the groom,
/// the member copy the bride and a witness, and each has its own family. The member event carries a
/// citation and a note, the member family a child and a note, and each member is the subject of a
/// research note — then both pairs are merged.
struct Fixture {
    ws: Workspace,
    _dir: tempfile::TempDir,
    groom: String,
    bride: String,
    witness: String,
    child: String,
    root_event: String,
    member_event: String,
    member_event_id: String,
    root_family: String,
    member_family: String,
    member_family_id: String,
    source: String,
    event_note: String,
    family_note: String,
    event_research: String,
    family_research: String,
}

async fn fixture() -> Fixture {
    let (ws, dir) = workspace().await;
    let s = session();
    let meta = MutationMeta::default;
    let (groom, bride, witness, child) = (
        person(&ws, "Hans").await,
        person(&ws, "Marte").await,
        person(&ws, "Lars").await,
        person(&ws, "Ola").await,
    );
    let (root_event, member_event) = (marriage(&ws).await, marriage(&ws).await);
    participate(&ws, &groom, &root_event, ParticipantRole::Primary).await;
    participate(&ws, &bride, &member_event, ParticipantRole::Primary).await;
    participate(&ws, &witness, &member_event, ParticipantRole::Witness).await;
    let (source, citation) = citation(&ws).await;
    add_event_citation(&ws, &s, &member_event, &citation, meta())
        .await
        .expect("cite");
    let event_note = note(&ws, "Banns read three Sundays").await;
    let note_id = ws
        .store()
        .find_note(&event_note)
        .await
        .expect("find")
        .and_then(|view| view.note_id())
        .expect("note id");
    attach_event_note(&ws, &s, &member_event, note_id, meta())
        .await
        .expect("note");

    let root_family = family(&ws, [&groom, &bride], &root_event).await;
    let member_family = family(&ws, [&groom, &bride], &member_event).await;
    let relationships = vec![(groom.clone(), ChildParentRelationship::Birth)];
    add_child(&ws, &s, &member_family, &child, relationships, meta())
        .await
        .expect("child");
    let family_note = note(&ws, "Household of 1865").await;
    attach_family_note(&ws, &s, &member_family, &family_note, meta())
        .await
        .expect("note");
    let event_research = research_note(&ws, NewResearchNoteSubject::Event(member_event.clone())).await;
    let family_research = research_note(&ws, NewResearchNoteSubject::Family(member_family.clone())).await;

    let member_event_id = ws
        .store()
        .find_event(&member_event)
        .await
        .expect("find")
        .and_then(|view| view.event_id())
        .expect("member event id")
        .to_string();
    let member_family_id = ws
        .store()
        .find_family(&member_family)
        .await
        .expect("find")
        .and_then(|view| view.family_id())
        .expect("member family id")
        .to_string();
    merge_events(&ws, &s, &root_event, &member_event, IdentityDecision::default())
        .await
        .expect("merge events");
    merge_families(&ws, &s, &root_family, &member_family, IdentityDecision::default())
        .await
        .expect("merge families");
    Fixture {
        ws,
        _dir: dir,
        groom,
        bride,
        witness,
        child,
        root_event,
        member_event,
        member_event_id,
        root_family,
        member_family,
        member_family_id,
        source,
        event_note,
        family_note,
        event_research,
        family_research,
    }
}

impl Fixture {
    /// Asserts a read names `root` and never `member`, by `human_id` or by aggregate id.
    fn names_root(what: &str, dto: &impl std::fmt::Debug, [root, member, member_id]: [&str; 3]) {
        let rendered = format!("{dto:?}");
        assert!(
            !rendered.contains(&format!("\"{member}\"")),
            "{what} names the member {member}: {rendered}"
        );
        assert!(
            !rendered.contains(member_id),
            "{what} names the member's id: {rendered}"
        );
        assert!(
            rendered.contains(&format!("\"{root}\"")),
            "{what} does not name the root {root}: {rendered}"
        );
    }

    fn names_the_root_event(&self, what: &str, dto: &impl std::fmt::Debug) {
        Self::names_root(what, dto, [&self.root_event, &self.member_event, &self.member_event_id]);
    }

    fn names_the_root_family(&self, what: &str, dto: &impl std::fmt::Debug) {
        Self::names_root(
            what,
            dto,
            [&self.root_family, &self.member_family, &self.member_family_id],
        );
    }
}

#[tokio::test]
async fn two_copies_of_one_marriage_merge_into_one_event_with_every_participant() {
    let f = fixture().await;
    let event = show_event(&f.ws, &f.root_event).await.expect("show").expect("event");
    let participants: BTreeSet<_> = event.participants.iter().map(|p| p.human_id.clone()).collect();
    assert_eq!(
        participants,
        BTreeSet::from([f.groom.clone(), f.bride.clone(), f.witness.clone()]),
        "{event:?}"
    );
    assert_eq!(event.merged.len(), 1, "{:?}", event.merged);
    assert_eq!(event.merged[0].human_id, f.member_event);
    assert_eq!(event.citations.len(), 1, "the member's citation is the root's too");
    assert_eq!(event.notes.len(), 1);
    assert_eq!(event.owner_of(event.notes[0].assertion_id.as_str()), f.member_event);
    let member = show_event(&f.ws, &f.member_event).await.expect("show").expect("event");
    assert_eq!(member, event, "a member reads as its cluster's root");
}

#[tokio::test]
async fn lists_and_counts_hide_merged_events_and_families() {
    let f = fixture().await;
    let events: Vec<_> = list_events(&f.ws)
        .await
        .expect("events")
        .into_iter()
        .map(|e| e.human_id)
        .collect();
    assert_eq!(events, std::slice::from_ref(&f.root_event));
    let rows: Vec<_> = list_event_rows(&f.ws)
        .await
        .expect("rows")
        .into_iter()
        .map(|e| e.human_id)
        .collect();
    assert_eq!(rows, std::slice::from_ref(&f.root_event));
    let families: Vec<_> = list_families(&f.ws)
        .await
        .expect("families")
        .into_iter()
        .map(|family| family.human_id)
        .collect();
    assert_eq!(families, std::slice::from_ref(&f.root_family));
    let rows: Vec<_> = list_family_rows(&f.ws)
        .await
        .expect("rows")
        .into_iter()
        .map(|family| family.human_id)
        .collect();
    assert_eq!(rows, std::slice::from_ref(&f.root_family));
    let counts = workspace_counts(&f.ws).await.expect("counts");
    assert_eq!((counts.event, counts.family), (1, 1));
}

#[tokio::test]
async fn a_merged_family_reads_its_members_children_and_claims() {
    let f = fixture().await;
    let family = show_family(&f.ws, &f.root_family).await.expect("show").expect("family");
    assert!(
        family.children.iter().any(|child| child.human_id == f.child),
        "{family:?}"
    );
    assert_eq!(family.notes.len(), 1);
    assert_eq!(family.owner_of(family.notes[0].assertion_id.as_str()), f.member_family);
    assert_eq!(family.merged.len(), 1);
    let member = show_family(&f.ws, &f.member_family)
        .await
        .expect("show")
        .expect("family");
    assert_eq!(member, family, "a member reads as its cluster's root");

    let ancestors = ancestors(&f.ws, &f.child, 1).await.expect("ancestors");
    let rendered = format!("{ancestors:?}");
    assert!(rendered.contains(&format!("\"{}\"", f.groom)), "{rendered}");
}

#[tokio::test]
async fn every_reference_to_a_member_event_or_family_reads_as_its_root() {
    let f = fixture().await;
    let ws = &f.ws;
    for who in [&f.bride, &f.witness] {
        f.names_the_root_event("a participant's events", &show_person(ws, who).await.expect("person"));
    }
    let family = show_family(ws, &f.root_family).await.expect("family");
    f.names_the_root_event("the family's events", &family);
    f.names_the_root_event("the family list", &list_families(ws).await.expect("families"));
    f.names_the_root_family("the family rows", &list_family_rows(ws).await.expect("rows"));
    let families = families_for_person(ws, &f.child).await.expect("families");
    assert_eq!(families.len(), 1, "{families:?}");
    f.names_the_root_family("the child's families", &families);
    let families = families_for_person(ws, &f.groom).await.expect("families");
    assert_eq!(families.len(), 1, "one family for both copies: {families:?}");
    f.names_the_root_event(
        "the source's backers",
        &show_source(ws, &f.source).await.expect("source"),
    );
    f.names_the_root_event(
        "the event note's backlinks",
        &show_note(ws, &f.event_note).await.expect("note"),
    );
    f.names_the_root_family(
        "the family note's backlinks",
        &show_note(ws, &f.family_note).await.expect("note"),
    );
    let research = show_research_note(ws, &f.event_research)
        .await
        .expect("note")
        .expect("exists");
    f.names_the_root_event("the event research subject", &research.subject_refs);
    let research = show_research_note(ws, &f.family_research)
        .await
        .expect("note")
        .expect("exists");
    f.names_the_root_family("the family research subject", &research.subject_refs);
    let about = list_research_notes_about(ws, NewResearchNoteSubject::Event(f.root_event.clone()))
        .await
        .expect("notes about the root event");
    assert_eq!(about.len(), 1, "the root reads the notes about its members: {about:?}");
    let about = list_research_notes_about(ws, NewResearchNoteSubject::Family(f.root_family.clone()))
        .await
        .expect("notes about the root family");
    assert_eq!(about.len(), 1, "{about:?}");
}

#[tokio::test]
async fn a_child_both_copies_name_is_one_child_in_the_charts() {
    let f = fixture().await;
    let relationships = vec![(f.groom.clone(), ChildParentRelationship::Birth)];
    add_child(
        &f.ws,
        &session(),
        &f.root_family,
        &f.child,
        relationships,
        MutationMeta::default(),
    )
    .await
    .expect("child");
    let chart = descendants(&f.ws, &f.groom, 1).await.expect("descendants");
    let rendered = format!("{chart:?}");
    assert_eq!(rendered.matches(&format!("\"{}\"", f.child)).count(), 1, "{rendered}");
    let families = families_for_person(&f.ws, &f.child).await.expect("families");
    assert_eq!(families.len(), 1, "{families:?}");
    assert_eq!(families[0].children.len(), 1, "{families:?}");
}

#[tokio::test]
async fn a_correction_of_a_member_row_is_written_to_the_member() {
    let f = fixture().await;
    let ws = &f.ws;
    let event = show_event(ws, &f.root_event).await.expect("show").expect("event");
    let note = event.notes[0].assertion_id.clone();
    assert_eq!(
        event_claim_owner(ws, &f.root_event, &note).await.expect("owner"),
        f.member_event
    );
    assert_eq!(
        event_claim_owner(ws, &f.member_event, &note).await.expect("owner"),
        f.member_event
    );
    undo_event_assertion(ws, &session(), &f.member_event, &note, None)
        .await
        .expect("undo the member's note");
    let event = show_event(ws, &f.root_event).await.expect("show").expect("event");
    assert!(event.notes.is_empty(), "{:?}", event.notes);

    let family = show_family(ws, &f.root_family).await.expect("show").expect("family");
    let note = family.notes[0].assertion_id.clone();
    assert_eq!(
        family_claim_owner(ws, &f.root_family, &note).await.expect("owner"),
        f.member_family
    );
    let own = family.partners[0].assertion_id.clone();
    assert_eq!(
        family_claim_owner(ws, &f.root_family, &own).await.expect("owner"),
        f.root_family
    );
}

#[tokio::test]
async fn removing_a_tag_or_child_from_a_cluster_removes_it_from_every_record() {
    let f = fixture().await;
    let ws = &f.ws;
    let tag = create_tag(ws, &session(), "Duplicate copy".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    tag_event(ws, &session(), &f.member_event, &tag, false, MutationMeta::default())
        .await
        .expect("tag member");
    tag_family(ws, &session(), &f.member_family, &tag, false, MutationMeta::default())
        .await
        .expect("tag member");
    let event = show_event(ws, &f.root_event).await.expect("show").expect("event");
    assert_eq!(event.tags.len(), 1, "the member's tag reads on the root");

    tag_event(ws, &session(), &f.root_event, &tag, true, MutationMeta::default())
        .await
        .expect("untag cluster");
    tag_family(ws, &session(), &f.root_family, &tag, true, MutationMeta::default())
        .await
        .expect("untag cluster");
    let event = show_event(ws, &f.root_event).await.expect("show").expect("event");
    assert!(event.tags.is_empty(), "{:?}", event.tags);
    let family = show_family(ws, &f.root_family).await.expect("show").expect("family");
    assert!(family.tags.is_empty(), "{:?}", family.tags);

    remove_child(ws, &session(), &f.root_family, &f.child, MutationMeta::default())
        .await
        .expect("remove the member's child");
    let family = show_family(ws, &f.root_family).await.expect("show").expect("family");
    assert!(family.children.is_empty(), "{:?}", family.children);
}

#[tokio::test]
async fn restricting_a_cluster_narrows_every_record() {
    let f = fixture().await;
    let ws = &f.ws;
    let private = BTreeSet::from([Restriction::Privacy]);
    set_event_restrictions(
        ws,
        &session(),
        &f.member_event,
        private.clone(),
        MutationMeta::default(),
    )
    .await
    .expect("restrict member");
    set_family_restrictions(ws, &session(), &f.member_family, private, MutationMeta::default())
        .await
        .expect("restrict member");
    let event = show_event(ws, &f.root_event).await.expect("show").expect("event");
    assert!(
        !event.restrictions.is_empty(),
        "the member's restriction reads on the root"
    );

    set_event_restrictions(ws, &session(), &f.root_event, BTreeSet::new(), MutationMeta::default())
        .await
        .expect("lift");
    set_family_restrictions(ws, &session(), &f.root_family, BTreeSet::new(), MutationMeta::default())
        .await
        .expect("lift");
    let event = show_event(ws, &f.root_event).await.expect("show").expect("event");
    assert!(event.restrictions.is_empty(), "{:?}", event.restrictions);
    let family = show_family(ws, &f.root_family).await.expect("show").expect("family");
    assert!(family.restrictions.is_empty(), "{:?}", family.restrictions);
}

#[tokio::test]
async fn a_new_relationship_for_a_members_child_is_written_to_the_member() {
    let f = fixture().await;
    let ws = &f.ws;
    vitni_app::assert_child_relationship(
        ws,
        &session(),
        &f.root_family,
        &f.child,
        &f.bride,
        ChildParentRelationship::Birth,
        MutationMeta::default(),
    )
    .await
    .expect("relate the member's child to the other partner");
    let family = show_family(ws, &f.root_family).await.expect("show").expect("family");
    let child = family.children.first().expect("child");
    assert_eq!(child.relationships.len(), 2, "{:?}", child.relationships);
    for link in &child.relationships {
        assert_eq!(family.owner_of(&link.assertion_id), f.member_family);
    }
}
