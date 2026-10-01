//! The reference sweep of ADR 0039 §5: once a record is merged, every reference-bearing read names its
//! cluster's root, never the member — in families, events, associations, pedigrees and backlinks.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, IdentityDecision, MutationMeta, NewCitation, NewEvent, NewNote, NewParticipation, NewPerson,
    NewResearchNote, NewResearchNoteSubject, NewSource, OperatorConfig, PersonNameParts, Provenance, Session,
    Workspace, WorkspaceDefaults, add_child, add_partner, add_person_citation, ancestors, assert_association,
    assert_participation, attach_person_note, create_citation, create_event, create_family, create_note, create_person,
    create_research_note, create_source, descendants, families_for_person, list_events, list_families,
    list_family_rows, list_research_notes_about, merge_persons, show_event, show_family, show_note, show_person,
    show_research_note, show_source,
};
use vitni_core::enums::{AssociationRole, ChildParentRelationship, EventType, EvidenceLevel, ParticipantRole};
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
        name: Some(PersonNameParts::simple(Some(given.to_owned()), Some("Dahl".to_owned()))),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("person")
}

/// The fixture: `member` is a partner and a parent in one family, a child in another, a participant
/// in an event, the other side of an association, and cites a source and a note — then is merged into
/// `root`.
struct Fixture {
    ws: Workspace,
    _dir: tempfile::TempDir,
    root: String,
    member: String,
    member_id: String,
    partner: String,
    child: String,
    partner_family: String,
    child_family: String,
    event: String,
    source: String,
    note: String,
    research_note: String,
}

/// A source cited by `member`, a note attached to it, and a research note about it.
async fn evidence_about(ws: &Workspace, member: &str) -> (String, String, String) {
    let s = session();
    let meta = MutationMeta::default;
    let source = create_source(
        ws,
        &s,
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
        ws,
        &s,
        NewCitation {
            human_id: None,
            source: source.clone(),
            page: Some("p. 7".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("citation");
    add_person_citation(ws, &s, member, &citation, meta())
        .await
        .expect("cite");
    let note = create_note(
        ws,
        &s,
        NewNote {
            human_id: None,
            text: Some("Confirmed 1801".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("note");
    attach_person_note(ws, &s, member, &note, meta()).await.expect("note");
    let research_note = create_research_note(
        ws,
        &s,
        NewResearchNote {
            human_id: None,
            subjects: vec![NewResearchNoteSubject::Person(member.to_owned())],
            title: Some("Which Peder?".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("research note");

    (source, note, research_note)
}

async fn fixture() -> Fixture {
    let (ws, dir) = workspace().await;
    let s = session();
    let meta = MutationMeta::default;
    let root = person(&ws, "Per").await;
    let member = person(&ws, "Peder").await;
    let partner = person(&ws, "Marit").await;
    let child = person(&ws, "Ola").await;

    let partner_family = create_family(&ws, &s, Provenance::default(), &[])
        .await
        .expect("family");
    add_partner(&ws, &s, &partner_family, &member, meta())
        .await
        .expect("partner");
    add_partner(&ws, &s, &partner_family, &partner, meta())
        .await
        .expect("partner");
    let relationships = vec![(member.clone(), ChildParentRelationship::Birth)];
    add_child(&ws, &s, &partner_family, &child, relationships, meta())
        .await
        .expect("child");
    let child_family = create_family(&ws, &s, Provenance::default(), &[])
        .await
        .expect("family");
    add_child(&ws, &s, &child_family, &member, Vec::new(), meta())
        .await
        .expect("child");

    let event = create_event(
        &ws,
        &s,
        NewEvent {
            human_id: None,
            event_type: EventType::Baptism,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("event");
    assert_participation(
        &ws,
        &s,
        &member,
        &event,
        NewParticipation::with_role(ParticipantRole::Primary),
        meta(),
    )
    .await
    .expect("participation");
    assert_association(&ws, &s, &partner, &member, AssociationRole::Godparent, meta())
        .await
        .expect("association");

    let (source, note, research_note) = evidence_about(&ws, &member).await;

    let member_id = ws
        .store()
        .find_person(&member)
        .await
        .expect("find")
        .and_then(|view| view.person_id())
        .expect("member id")
        .to_string();
    merge_persons(&ws, &s, &root, &member, IdentityDecision::default())
        .await
        .expect("merge");
    Fixture {
        ws,
        _dir: dir,
        root,
        member,
        member_id,
        partner,
        child,
        partner_family,
        child_family,
        event,
        source,
        note,
        research_note,
    }
}

impl Fixture {
    /// Asserts a read names the root and never the member, by `human_id` or by aggregate id.
    fn names_the_root(&self, what: &str, dto: &impl std::fmt::Debug) {
        let rendered = format!("{dto:?}");
        assert!(
            !rendered.contains(&format!("\"{}\"", self.member)),
            "{what} names the member {}: {rendered}",
            self.member
        );
        assert!(
            !rendered.contains(&self.member_id),
            "{what} names the member's id: {rendered}"
        );
        assert!(
            rendered.contains(&format!("\"{}\"", self.root)),
            "{what} does not name the root {}: {rendered}",
            self.root
        );
    }
}

#[tokio::test]
async fn every_reference_to_a_member_reads_as_its_root() {
    let f = fixture().await;
    let ws = &f.ws;
    f.names_the_root(
        "the partner family",
        &show_family(ws, &f.partner_family).await.expect("family"),
    );
    f.names_the_root(
        "the child family",
        &show_family(ws, &f.child_family).await.expect("family"),
    );
    f.names_the_root("the family list", &list_families(ws).await.expect("families"));
    f.names_the_root("the family rows", &list_family_rows(ws).await.expect("rows"));
    f.names_the_root(
        "the partner's families",
        &families_for_person(ws, &f.partner).await.expect("families"),
    );
    f.names_the_root("the event", &show_event(ws, &f.event).await.expect("event"));
    f.names_the_root("the event list", &list_events(ws).await.expect("events"));
    f.names_the_root(
        "the partner's associations",
        &show_person(ws, &f.partner).await.expect("partner"),
    );
    f.names_the_root(
        "the child's ancestors",
        &ancestors(ws, &f.child, 2).await.expect("ancestors"),
    );
    f.names_the_root(
        "the source's backers",
        &show_source(ws, &f.source).await.expect("source"),
    );
    f.names_the_root("the note's backlinks", &show_note(ws, &f.note).await.expect("note"));
    let research_note = show_research_note(ws, &f.research_note)
        .await
        .expect("note")
        .expect("exists");
    f.names_the_root("the research note's subject", &research_note.subject_refs);
    let about = list_research_notes_about(ws, NewResearchNoteSubject::Person(f.root.clone()))
        .await
        .expect("notes about the root");
    assert_eq!(about.len(), 1, "the root reads the notes about its members: {about:?}");
}

#[tokio::test]
async fn the_root_reads_the_members_families_and_descendants() {
    let f = fixture().await;
    let families = families_for_person(&f.ws, &f.root).await.expect("families");
    let mut ids: Vec<_> = families.iter().map(|family| family.family_human_id.clone()).collect();
    ids.sort();
    let mut expected = vec![f.partner_family.clone(), f.child_family.clone()];
    expected.sort();
    assert_eq!(ids, expected);
    let chart = descendants(&f.ws, &f.root, 1).await.expect("descendants");
    assert!(format!("{chart:?}").contains(&format!("\"{}\"", f.child)), "{chart:?}");
}
