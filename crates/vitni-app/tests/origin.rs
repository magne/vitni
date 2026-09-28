//! Record origin threading (ADR 0037 §1): a [`Provenance`] carrying an origin stamps it on every event
//! a use-case writes, including the follow-up events a `create_*` issues after its creating one.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, NewCitation, NewEvent, NewMedia, NewNote, NewPerson, NewPlace, NewRepository, NewSource,
    OperatorConfig, PersonNameParts, Provenance, Session, Workspace, WorkspaceDefaults, create_citation, create_event,
    create_family, create_media, create_note, create_person, create_place, create_repository, create_source,
};
use vitni_core::enums::{EventType, EvidenceLevel, PlaceType};
use vitni_core::ids::{AgentId, ImportRunId};
use vitni_core::origin::{DatasetId, RecordOrigin};
use vitni_core::provenance::{Agent, AgentKind, EventContext};

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

fn origin() -> RecordOrigin {
    RecordOrigin {
        dataset: DatasetId::global("digitalarkivet"),
        record: "pf01073902000464".to_owned(),
        item: None,
        digest: None,
        run: ImportRunId::from_uuid(Uuid::from_u128(9)),
    }
}

fn provenance() -> Provenance {
    Provenance {
        origin: Some(origin()),
        ..Provenance::default()
    }
}

#[derive(serde::Deserialize)]
struct Header {
    context: EventContext,
}

#[tokio::test]
async fn every_event_a_create_writes_carries_the_callers_origin() {
    let (workspace, _dir) = workspace().await;
    let session = session();
    let person = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some("Ola".to_owned()),
            Some("Nordmann".to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    create_person(&workspace, &session, person, provenance(), &[])
        .await
        .expect("person");
    create_family(&workspace, &session, provenance(), &[])
        .await
        .expect("family");
    let event = NewEvent {
        human_id: None,
        event_type: EventType::Birth,
    };
    create_event(&workspace, &session, event, provenance(), &[])
        .await
        .expect("event");
    let place = NewPlace {
        human_id: None,
        place_type: PlaceType::Farm,
        name: Some("Haugen".to_owned()),
    };
    create_place(&workspace, &session, place, provenance(), &[])
        .await
        .expect("place");
    let source = NewSource {
        human_id: None,
        title: Some("Census 1865".to_owned()),
    };
    let source = create_source(&workspace, &session, source, provenance(), &[])
        .await
        .expect("source");
    let citation = NewCitation {
        human_id: None,
        source,
        page: Some("p. 4".to_owned()),
    };
    create_citation(&workspace, &session, citation, provenance(), &[])
        .await
        .expect("citation");
    let repository = NewRepository {
        human_id: None,
        name: Some("Arkivverket".to_owned()),
    };
    create_repository(&workspace, &session, repository, provenance(), &[])
        .await
        .expect("repository");
    let note = NewNote {
        human_id: None,
        text: Some("transcript".to_owned()),
    };
    create_note(&workspace, &session, note, provenance(), &[])
        .await
        .expect("note");
    let media = NewMedia {
        human_id: None,
        path: Some("scans/p4.jpg".to_owned()),
    };
    create_media(&workspace, &session, media, provenance(), &[])
        .await
        .expect("media");

    let events = workspace.store().read_recent_events(1000).await.expect("read log");
    assert!(events.len() >= 14, "every create and its follow-ups are in the log");
    for event in events {
        let header: Header = serde_json::from_str(&event.payload).expect("decode envelope");
        assert_eq!(
            header.context.origin.as_deref(),
            Some(&origin()),
            "{} {} lost its origin",
            event.aggregate_type,
            event.event_type
        );
    }
}
