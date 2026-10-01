//! Edits on a merged event or family detail are routed to the record that owns the row (ADR 0039 §5):
//! an undo or a supersede of a member's row is written to that member's stream, while the pane keeps
//! showing the root.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    Address, Agent, AgentId, AgentKind, AppDefaults, EventType, EvidenceLevel, IdentityDecision, MutationMeta,
    NewEvent, NewNote, NewParticipation, NewPerson, OperatorConfig, ParticipantRole, PersonNameParts, Provenance,
    Session, Workspace, WorkspaceDefaults, assert_event_address, assert_participation, attach_family_note,
    create_event, create_family, create_note, create_person, link_family_event, merge_events, merge_families,
    show_event, show_family,
};
use vitni_ui::{
    EventDetail, EventEdit, FamilyDetail, FamilyEdit, Localizer, ProvenanceDraft, dispatch_event_edit,
    dispatch_family_edit,
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

fn address(locality: &str) -> Address {
    Address {
        locality: Some(locality.to_owned()),
        ..Address::default()
    }
}

/// Two merged marriages, the member holding an address and a participant, and two merged families,
/// the member holding a note and the link to the member marriage: `(workspace, dir, [root event, member
/// event, root family, member family])`.
async fn merged() -> (Workspace, tempfile::TempDir, [String; 4]) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ws");
    Workspace::init(&path, &operator(), &AppDefaults::default(), None).expect("init");
    let ws = Workspace::open(&path, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    let s = session();
    let mut events = Vec::new();
    for _ in 0..2 {
        let new = NewEvent {
            human_id: None,
            event_type: EventType::Marriage,
        };
        events.push(
            create_event(&ws, &s, new, Provenance::default(), &[])
                .await
                .expect("event"),
        );
    }
    assert_event_address(&ws, &s, &events[1], address("Mandal"), MutationMeta::default())
        .await
        .expect("address");
    let groom = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some("Hans".to_owned()),
            Some("Bakke".to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    let groom = create_person(&ws, &s, groom, Provenance::default(), &[])
        .await
        .expect("person");
    let role = NewParticipation::with_role(ParticipantRole::Primary);
    assert_participation(&ws, &s, &groom, &events[1], role, MutationMeta::default())
        .await
        .expect("participation");
    merge_events(&ws, &s, &events[0], &events[1], IdentityDecision::default())
        .await
        .expect("merge events");
    let mut families = Vec::new();
    for _ in 0..2 {
        families.push(
            create_family(&ws, &s, Provenance::default(), &[])
                .await
                .expect("family"),
        );
    }
    let note = NewNote {
        human_id: None,
        text: Some("Household of 1865".to_owned()),
    };
    let note = create_note(&ws, &s, note, Provenance::default(), &[])
        .await
        .expect("note");
    attach_family_note(&ws, &s, &families[1], &note, MutationMeta::default())
        .await
        .expect("note");
    link_family_event(&ws, &s, &families[1], &events[1], MutationMeta::default())
        .await
        .expect("link");
    merge_families(&ws, &s, &families[0], &families[1], IdentityDecision::default())
        .await
        .expect("merge families");
    let [root_event, member_event] = [events[0].clone(), events[1].clone()];
    let [root_family, member_family] = [families[0].clone(), families[1].clone()];
    (ws, dir, [root_event, member_event, root_family, member_family])
}

#[tokio::test]
async fn undoing_a_member_events_row_from_the_root_writes_to_the_member() {
    let (ws, _dir, [root, ..]) = merged().await;
    let assertion_id = show_event(&ws, &root).await.expect("show").expect("root").addresses[0]
        .assertion_id
        .clone();
    let edit = EventEdit::UndoAssertion {
        human_id: root.clone(),
        assertion_id,
    };
    let target = dispatch_event_edit(&ws, &session(), &edit, &ProvenanceDraft::default())
        .await
        .expect("undo routed to the member");
    assert_eq!(target, root, "the pane reloads the root");
    let event = show_event(&ws, &root).await.expect("show").expect("root");
    assert!(event.addresses.is_empty(), "{:?}", event.addresses);
}

#[tokio::test]
async fn superseding_a_member_events_row_from_the_root_replaces_it_on_the_member() {
    let (ws, _dir, [root, member, ..]) = merged().await;
    let summary = show_event(&ws, &root).await.expect("show").expect("root");
    let prov = ProvenanceDraft {
        supersedes: Some(summary.addresses[0].assertion_id.clone()),
        ..ProvenanceDraft::default()
    };
    let edit = EventEdit::AddAddress {
        human_id: root.clone(),
        address: address("Kristiansand"),
    };
    dispatch_event_edit(&ws, &session(), &edit, &prov)
        .await
        .expect("supersede routed to the member");
    let after = show_event(&ws, &root).await.expect("show").expect("root");
    let places: Vec<_> = after
        .addresses
        .iter()
        .map(|row| {
            (
                row.address.locality.clone().unwrap_or_default(),
                after.owner_of(&row.assertion_id).to_owned(),
            )
        })
        .collect();
    assert_eq!(places, [("Kristiansand".to_owned(), member)]);
}

#[tokio::test]
async fn undoing_a_member_familys_row_from_the_root_writes_to_the_member() {
    let (ws, _dir, [.., root, _member]) = merged().await;
    let assertion_id = show_family(&ws, &root).await.expect("show").expect("root").notes[0]
        .assertion_id
        .clone();
    let edit = FamilyEdit::UndoAssertion {
        human_id: root.clone(),
        assertion_id,
    };
    let target = dispatch_family_edit(&ws, &session(), &edit, &ProvenanceDraft::default())
        .await
        .expect("undo routed to the member");
    assert_eq!(target, root);
    let family = show_family(&ws, &root).await.expect("show").expect("root");
    assert!(family.notes.is_empty(), "{:?}", family.notes);
}

#[tokio::test]
async fn a_member_row_names_the_member_it_came_from() {
    let (ws, dir, [root_event, member_event, root_family, member_family]) = merged().await;
    let loc = Localizer::for_workspace(&dir.path().join("ws"), None);
    let summary = show_event(&ws, &root_event).await.expect("show").expect("root");
    let event = EventDetail::from_summary(&summary, &loc);
    assert_eq!(event.addresses[0].merged_from.as_deref(), Some(member_event.as_str()));
    assert_eq!(
        event.participants[0].merged_from.as_deref(),
        Some(member_event.as_str())
    );

    let summary = show_family(&ws, &root_family).await.expect("show").expect("root");
    let family = FamilyDetail::from_summary(&summary, &loc);
    assert_eq!(family.events[0].merged_from.as_deref(), Some(member_family.as_str()));
    assert_eq!(
        family.events[0].human_id, root_event,
        "the member marriage reads as the root"
    );
}
