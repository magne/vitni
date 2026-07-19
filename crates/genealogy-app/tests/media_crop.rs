//! Media-ref crop/caption plumbing (ADR 0017 §9): the six `attach_*_media` use-cases record a
//! per-use crop + caption, the `MediaRefSummary` joins the media object's path + MIME, and the six
//! `update_*_media_ref` corrections supersede the attach assertion non-destructively.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use genealogy_app::{
    AppDefaults, MediaRefInput, MutationMeta, NewCitation, NewEvent, NewMedia, NewPerson, NewPlace, NewSource,
    OperatorConfig, PersonNameParts, Provenance, Rect, Session, Workspace, WorkspaceDefaults, attach_citation_media,
    attach_family_media, attach_person_media, change_log_for_person, create_citation, create_event, create_family,
    create_media, create_person, create_place, create_source, import_attach_event_media, import_attach_place_media,
    import_attach_source_media, set_media_mime, show_citation, show_event, show_family, show_person, show_place,
    show_source, update_person_media_ref, update_place_media_ref, update_source_media_ref,
};
use genealogy_core::enums::{EventType, EvidenceLevel, PlaceType};
use genealogy_core::ids::AgentId;
use genealogy_core::provenance::{Agent, AgentKind};
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

/// Creates a media object with a file path and MIME, returning its `human_id`.
async fn media_with_path(ws: &Workspace, session: &Session) -> String {
    let media = create_media(
        ws,
        session,
        NewMedia {
            human_id: None,
            path: Some("photos/group.jpg".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create media");
    set_media_mime(ws, session, &media, "image/jpeg".to_owned(), MutationMeta::default())
        .await
        .expect("set mime");
    media
}

fn person(given: &str) -> NewPerson {
    NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some("Lovelace".to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
    }
}

fn face() -> Rect {
    Rect {
        left: 10,
        top: 20,
        width: 30,
        height: 40,
    }
}

fn crop_input() -> MediaRefInput {
    MediaRefInput {
        crop: Some(face()),
        caption: Some("Ada's face".to_owned()),
    }
}

#[tokio::test]
async fn person_media_attach_round_trips_crop_caption_path_and_mime() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let subject = create_person(&ws, &session, person("Ada"), Provenance::default(), &[])
        .await
        .expect("person");
    let media = media_with_path(&ws, &session).await;

    attach_person_media(&ws, &session, &subject, &media, crop_input(), MutationMeta::default())
        .await
        .expect("attach with crop");

    let summary = show_person(&ws, &subject).await.expect("show").expect("person");
    let attached = summary.media.first().expect("one media ref");
    assert_eq!(attached.crop, Some(face()));
    assert_eq!(attached.caption.as_deref(), Some("Ada's face"));
    assert_eq!(attached.path.as_deref(), Some("photos/group.jpg"));
    assert_eq!(attached.mime.as_deref(), Some("image/jpeg"));
    assert_eq!(attached.human_id, media);
}

#[tokio::test]
async fn person_media_default_input_leaves_the_crop_and_caption_empty() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let subject = create_person(&ws, &session, person("Ada"), Provenance::default(), &[])
        .await
        .expect("person");
    let media = media_with_path(&ws, &session).await;

    attach_person_media(
        &ws,
        &session,
        &subject,
        &media,
        MediaRefInput::default(),
        MutationMeta::default(),
    )
    .await
    .expect("attach default");

    let summary = show_person(&ws, &subject).await.expect("show").expect("person");
    let attached = summary.media.first().expect("one media ref");
    assert_eq!(attached.crop, None, "default input records no crop");
    assert_eq!(attached.caption, None, "default input records no caption");
    // The media object's path + MIME are still joined for the gallery.
    assert_eq!(attached.path.as_deref(), Some("photos/group.jpg"));
    assert_eq!(attached.mime.as_deref(), Some("image/jpeg"));
}

#[tokio::test]
async fn updating_a_person_media_ref_supersedes_and_keeps_both_in_history() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let subject = create_person(&ws, &session, person("Ada"), Provenance::default(), &[])
        .await
        .expect("person");
    let media = media_with_path(&ws, &session).await;
    attach_person_media(
        &ws,
        &session,
        &subject,
        &media,
        MediaRefInput::default(),
        MutationMeta::default(),
    )
    .await
    .expect("attach");

    let original = show_person(&ws, &subject).await.expect("show").expect("person");
    let attach_assertion = original.media.first().expect("one media ref").assertion_id.clone();

    let recrop = Rect {
        left: 5,
        top: 5,
        width: 50,
        height: 50,
    };
    update_person_media_ref(
        &ws,
        &session,
        &subject,
        &attach_assertion,
        MediaRefInput {
            crop: Some(recrop),
            caption: Some("recropped".to_owned()),
        },
        MutationMeta::default(),
    )
    .await
    .expect("update media ref");

    let after = show_person(&ws, &subject).await.expect("show").expect("person");
    assert_eq!(after.media.len(), 1, "supersede replaces rather than appends");
    let row = after.media.first().expect("one media ref");
    assert_eq!(row.crop, Some(recrop), "the new crop is readable");
    assert_eq!(row.caption.as_deref(), Some("recropped"));
    assert_eq!(row.human_id, media, "the same media object stays attached");
    assert_ne!(
        row.assertion_id, attach_assertion,
        "the surviving row carries a new assertion id"
    );

    let log = change_log_for_person(&ws, &subject).await.expect("log");
    let attaches = log.iter().filter(|entry| entry.event_type == "MediaAttached").count();
    assert_eq!(
        attaches, 2,
        "the audit trail keeps both the original and the replacement attach"
    );
    assert!(
        log.iter().any(|entry| entry.event_type == "AssertionSuperseded"),
        "the correction is recorded as a supersession"
    );
}

#[tokio::test]
async fn updating_an_unknown_media_assertion_is_rejected() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let subject = create_person(&ws, &session, person("Ada"), Provenance::default(), &[])
        .await
        .expect("person");
    let bogus = Uuid::from_u128(0xdead).to_string();

    let result = update_person_media_ref(&ws, &session, &subject, &bogus, crop_input(), MutationMeta::default()).await;
    assert!(
        result.is_err(),
        "an assertion id that names no media attachment is rejected"
    );
}

#[tokio::test]
async fn family_media_attach_round_trips_crop_and_caption() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let family = create_family(&ws, &session, Provenance::default(), &[])
        .await
        .expect("family");
    let media = media_with_path(&ws, &session).await;

    attach_family_media(&ws, &session, &family, &media, crop_input(), MutationMeta::default())
        .await
        .expect("attach with crop");

    let summary = show_family(&ws, &family).await.expect("show").expect("family");
    let attached = summary.media.first().expect("one media ref");
    assert_eq!(attached.crop, Some(face()));
    assert_eq!(attached.caption.as_deref(), Some("Ada's face"));
    assert_eq!(attached.mime.as_deref(), Some("image/jpeg"));
}

#[tokio::test]
async fn citation_media_attach_round_trips_crop_and_caption() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let source = create_source(
        &ws,
        &session,
        NewSource {
            human_id: None,
            title: Some("1850 Census".to_owned()),
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
            source,
            page: Some("p. 14".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("citation");
    let media = media_with_path(&ws, &session).await;

    attach_citation_media(&ws, &session, &citation, &media, crop_input(), MutationMeta::default())
        .await
        .expect("attach with crop");

    let summary = show_citation(&ws, &citation).await.expect("show").expect("citation");
    let attached = summary.media.first().expect("one media ref");
    assert_eq!(attached.crop, Some(face()));
    assert_eq!(attached.caption.as_deref(), Some("Ada's face"));
}

#[tokio::test]
async fn event_media_attach_round_trips_crop_and_caption() {
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
    .expect("event");
    let media = media_with_path(&ws, &session).await;

    import_attach_event_media(&ws, &session, &event, &media, crop_input())
        .await
        .expect("attach with crop");

    let summary = show_event(&ws, &event).await.expect("show").expect("event");
    let attached = summary.media.first().expect("one media ref");
    assert_eq!(attached.crop, Some(face()));
    assert_eq!(attached.caption.as_deref(), Some("Ada's face"));
    assert_eq!(attached.path.as_deref(), Some("photos/group.jpg"));
}

#[tokio::test]
async fn source_media_ref_gains_a_crop_through_the_update_use_case() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let source = create_source(
        &ws,
        &session,
        NewSource {
            human_id: None,
            title: Some("Parish Register".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("source");
    let media = media_with_path(&ws, &session).await;
    import_attach_source_media(&ws, &session, &source, &media)
        .await
        .expect("attach");

    let attach_assertion = show_source(&ws, &source)
        .await
        .expect("show")
        .expect("source")
        .media
        .first()
        .expect("one media ref")
        .assertion_id
        .clone();
    update_source_media_ref(
        &ws,
        &session,
        &source,
        &attach_assertion,
        crop_input(),
        MutationMeta::default(),
    )
    .await
    .expect("update media ref");

    let summary = show_source(&ws, &source).await.expect("show").expect("source");
    let attached = summary.media.first().expect("one media ref");
    assert_eq!(attached.crop, Some(face()));
    assert_eq!(attached.caption.as_deref(), Some("Ada's face"));
}

#[tokio::test]
async fn place_media_ref_gains_a_crop_through_the_update_use_case() {
    let (ws, _dir) = workspace().await;
    let session = session();
    let place = create_place(
        &ws,
        &session,
        NewPlace {
            human_id: None,
            place_type: PlaceType::City,
            name: Some("Oslo".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");
    let media = media_with_path(&ws, &session).await;
    import_attach_place_media(&ws, &session, &place, &media)
        .await
        .expect("attach");

    let attach_assertion = show_place(&ws, &place)
        .await
        .expect("show")
        .expect("place")
        .media
        .first()
        .expect("one media ref")
        .assertion_id
        .clone();
    update_place_media_ref(
        &ws,
        &session,
        &place,
        &attach_assertion,
        crop_input(),
        MutationMeta::default(),
    )
    .await
    .expect("update media ref");

    let summary = show_place(&ws, &place).await.expect("show").expect("place");
    let attached = summary.media.first().expect("one media ref");
    assert_eq!(attached.crop, Some(face()));
    assert_eq!(attached.caption.as_deref(), Some("Ada's face"));
}
