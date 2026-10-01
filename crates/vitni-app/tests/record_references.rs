//! References to a merged place, source, citation, repository, note or media object (ADR 0039 §5):
//! every reader that names the record names its cluster's root instead, so a merge needs no
//! reference rewritten.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::str::FromStr;

use uuid::Uuid;
use vitni_app::{
    AppDefaults, GeoCoordinates, IdentityDecision, MediaRefInput, Microdegrees, MutationMeta, NewCitation, NewEvent,
    NewMedia, NewNote, NewPlace, NewRepository, NewSource, OperatorConfig, Provenance, Session, SourceMediaType,
    Workspace, WorkspaceDefaults, add_event_citation, assert_place_coordinates, assert_place_enclosed_by,
    attach_event_media, attach_event_note, attach_place_note, create_citation, create_event, create_media, create_note,
    create_place, create_repository, create_source, link_place, link_source_repository, list_event_rows,
    merge_citations, merge_media, merge_notes, merge_places, merge_repositories, merge_sources, show_citation,
    show_event, show_geography, show_media, show_note, show_place, show_repository, show_source,
};
use vitni_core::enums::{EventType, PlaceType};
use vitni_core::ids::{AgentId, MediaId, NoteId};
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

fn meta() -> MutationMeta<'static> {
    MutationMeta::default()
}

async fn place(ws: &Workspace, name: &str) -> String {
    let new = NewPlace {
        human_id: None,
        place_type: PlaceType::Farm,
        name: Some(name.to_owned()),
    };
    create_place(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create place")
}

async fn event(ws: &Workspace) -> String {
    let new = NewEvent {
        human_id: None,
        event_type: EventType::Baptism,
    };
    create_event(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create event")
}

async fn source(ws: &Workspace) -> String {
    let new = NewSource {
        human_id: None,
        title: Some("Ministerialbok".to_owned()),
    };
    create_source(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create source")
}

async fn citation(ws: &Workspace, source: &str) -> String {
    let new = NewCitation {
        human_id: None,
        source: source.to_owned(),
        page: Some("p. 3".to_owned()),
    };
    create_citation(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create citation")
}

async fn note(ws: &Workspace) -> (String, NoteId) {
    let new = NewNote {
        human_id: None,
        text: Some("Baptised at home".to_owned()),
    };
    let human_id = create_note(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create note");
    let id = show_note(ws, &human_id).await.expect("show").expect("note").id;
    (human_id, NoteId::from_uuid(Uuid::parse_str(&id).expect("uuid")))
}

async fn media(ws: &Workspace) -> (String, MediaId) {
    let new = NewMedia {
        human_id: None,
        path: Some("https://example.org/scan.jpg".to_owned()),
    };
    let human_id = create_media(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create media");
    let id = show_media(ws, &human_id).await.expect("show").expect("media").id;
    (human_id, MediaId::from_uuid(Uuid::parse_str(&id).expect("uuid")))
}

fn point(lat: &str, lon: &str) -> GeoCoordinates {
    GeoCoordinates {
        latitude: Microdegrees::from_str(lat).expect("lat"),
        longitude: Microdegrees::from_str(lon).expect("lon"),
    }
}

#[tokio::test]
async fn an_event_whose_place_was_merged_shows_the_survivor() {
    let (ws, _dir) = workspace().await;
    let (survivor, copy) = (place(&ws, "Haugen").await, place(&ws, "Hougen").await);
    assert_place_coordinates(&ws, &session(), &survivor, point("61.5", "9.0"), meta())
        .await
        .expect("coordinates");
    let baptism = event(&ws).await;
    link_place(&ws, &session(), &baptism, &copy, meta())
        .await
        .expect("link");
    merge_places(&ws, &session(), &survivor, &copy, IdentityDecision::default())
        .await
        .expect("merge");

    let shown = show_event(&ws, &baptism).await.expect("show").expect("event");
    assert_eq!(
        shown.place.expect("place").human_id,
        survivor,
        "the detail names the root"
    );
    let row = list_event_rows(&ws)
        .await
        .expect("rows")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(
        row.place.expect("place").human_id,
        survivor,
        "the list row names the root"
    );
    let geography = show_geography(&ws, None).await.expect("geography");
    assert!(geography.markers.iter().all(|marker| marker.human_id != copy));
    assert_eq!(
        geography
            .events
            .iter()
            .map(|pin| pin.place_human_id.as_str())
            .collect::<Vec<_>>(),
        vec![survivor.as_str()],
        "the event pins at the root's point"
    );
    let at_survivor = show_place(&ws, &survivor).await.expect("show").expect("place");
    assert_eq!(at_survivor.events.len(), 1, "the root's map lists the event");
}

#[tokio::test]
async fn a_place_enclosed_by_a_merged_place_names_the_root_as_its_jurisdiction() {
    let (ws, _dir) = workspace().await;
    let (parish, copy, farm) = (
        place(&ws, "Hof").await,
        place(&ws, "Hoff").await,
        place(&ws, "Haugen").await,
    );
    assert_place_enclosed_by(&ws, &session(), &farm, &copy, meta())
        .await
        .expect("enclose");
    merge_places(&ws, &session(), &parish, &copy, IdentityDecision::default())
        .await
        .expect("merge");
    let shown = show_place(&ws, &farm).await.expect("show").expect("place");
    assert_eq!(shown.enclosing.first().expect("enclosing").human_id, parish);
    assert!(shown.generated_title.contains("Hof"), "{}", shown.generated_title);
}

#[tokio::test]
async fn a_merged_citation_and_source_resolve_to_their_roots() {
    let (ws, _dir) = workspace().await;
    let (book, book_copy) = (source(&ws).await, source(&ws).await);
    let (cited, cited_copy) = (citation(&ws, &book).await, citation(&ws, &book_copy).await);
    let baptism = event(&ws).await;
    add_event_citation(&ws, &session(), &baptism, &cited_copy, meta())
        .await
        .expect("cite");
    merge_sources(&ws, &session(), &book, &book_copy, IdentityDecision::default())
        .await
        .expect("merge sources");
    merge_citations(&ws, &session(), &cited, &cited_copy, IdentityDecision::default())
        .await
        .expect("merge citations");

    let shown = show_event(&ws, &baptism).await.expect("show").expect("event");
    let citation = shown.citations.first().expect("citation");
    assert_eq!(citation.human_id, cited, "the event cites the root citation");
    assert_eq!(citation.source.as_ref().expect("source").human_id, book);
    let root = show_citation(&ws, &cited).await.expect("show").expect("citation");
    assert_eq!(root.source.expect("source").human_id, book);
    let book_shown = show_source(&ws, &book).await.expect("show").expect("source");
    let cited_by: Vec<&str> = book_shown
        .citations
        .iter()
        .map(|c| c.citation.human_id.as_str())
        .collect();
    assert_eq!(cited_by, vec![cited.as_str()], "the root source lists the cluster once");
}

#[tokio::test]
async fn a_source_held_by_a_merged_repository_names_the_root() {
    let (ws, _dir) = workspace().await;
    let new = |name: &str| NewRepository {
        human_id: None,
        name: Some(name.to_owned()),
    };
    let archive = create_repository(&ws, &session(), new("Arkivverket"), Provenance::default(), &[])
        .await
        .expect("repository");
    let copy = create_repository(&ws, &session(), new("Riksarkivet"), Provenance::default(), &[])
        .await
        .expect("repository");
    let book = source(&ws).await;
    link_source_repository(&ws, &session(), &book, &copy, None, SourceMediaType::Book, meta())
        .await
        .expect("link");
    merge_repositories(&ws, &session(), &archive, &copy, IdentityDecision::default())
        .await
        .expect("merge");

    let shown = show_source(&ws, &book).await.expect("show").expect("source");
    let held = shown.repositories.first().expect("repository");
    assert_eq!(held.repository.as_ref().expect("ref").human_id, archive);
    let root = show_repository(&ws, &archive).await.expect("show").expect("repository");
    assert_eq!(
        root.sources
            .iter()
            .map(|s| s.source.human_id.as_str())
            .collect::<Vec<_>>(),
        vec![book.as_str()]
    );
}

#[tokio::test]
async fn a_merged_note_and_media_object_resolve_to_their_roots() {
    let (ws, _dir) = workspace().await;
    let ((note_root, _), (note_copy, note_copy_id)) = (note(&ws).await, note(&ws).await);
    let ((media_root, _), (media_copy, media_copy_id)) = (media(&ws).await, media(&ws).await);
    let (baptism, farm) = (event(&ws).await, place(&ws, "Haugen").await);
    attach_event_note(&ws, &session(), &baptism, note_copy_id, meta())
        .await
        .expect("note");
    attach_place_note(&ws, &session(), &farm, note_copy_id, meta())
        .await
        .expect("place note");
    attach_event_media(
        &ws,
        &session(),
        &baptism,
        media_copy_id,
        MediaRefInput::default(),
        meta(),
    )
    .await
    .expect("media");
    merge_notes(&ws, &session(), &note_root, &note_copy, IdentityDecision::default())
        .await
        .expect("merge notes");
    merge_media(&ws, &session(), &media_root, &media_copy, IdentityDecision::default())
        .await
        .expect("merge media");

    let shown = show_event(&ws, &baptism).await.expect("show").expect("event");
    assert_eq!(shown.notes.first().expect("note").human_id, note_root);
    assert_eq!(shown.media.first().expect("media").human_id, media_root);
    let note_shown = show_note(&ws, &note_root).await.expect("show").expect("note");
    let mut users: Vec<&str> = note_shown.references.iter().map(|r| r.human_id.as_str()).collect();
    users.sort_unstable();
    let mut expected = vec![baptism.as_str(), farm.as_str()];
    expected.sort_unstable();
    assert_eq!(
        users, expected,
        "the root note lists every record that uses the cluster"
    );
    let media_shown = show_media(&ws, &media_root).await.expect("show").expect("media");
    assert_eq!(
        media_shown
            .used_by
            .iter()
            .map(|r| r.human_id.as_str())
            .collect::<Vec<_>>(),
        vec![baptism.as_str()]
    );
}

#[tokio::test]
async fn a_record_using_a_merged_place_is_listed_under_the_root() {
    let (ws, _dir) = workspace().await;
    let (survivor, copy) = (place(&ws, "Haugen").await, place(&ws, "Hougen").await);
    let (remark, remark_id) = note(&ws).await;
    attach_place_note(&ws, &session(), &copy, remark_id, meta())
        .await
        .expect("note");
    merge_places(&ws, &session(), &survivor, &copy, IdentityDecision::default())
        .await
        .expect("merge");
    let shown = show_note(&ws, &remark).await.expect("show").expect("note");
    assert_eq!(
        shown.references.iter().map(|r| r.human_id.as_str()).collect::<Vec<_>>(),
        vec![survivor.as_str()],
        "a note on the copy is used by the root"
    );
}
