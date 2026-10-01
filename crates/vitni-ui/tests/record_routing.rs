//! Edits on a merged place, source, citation, repository, note or media detail are routed to the
//! record that owns the row (ADR 0039 §5): an undo of a member's row is written to that member's
//! stream, the pane keeps showing the root, and the row names the member it came from.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    Agent, AgentId, AgentKind, AppDefaults, IdentityDecision, MutationMeta, NewCitation, NewMedia, NewNote, NewPlace,
    NewRepository, NewSource, OperatorConfig, PlaceType, Provenance, Session, Url, Workspace, WorkspaceDefaults,
    add_citation_attribute, add_media_attribute, add_note_translation, add_place_name, add_repository_url,
    add_source_attribute, create_citation, create_media, create_note, create_place, create_repository, create_source,
    merge_citations, merge_media, merge_notes, merge_places, merge_repositories, merge_sources, show_citation,
    show_media, show_note, show_place, show_repository, show_source,
};
use vitni_ui::{
    CitationDetail, CitationEdit, Localizer, MediaDetail, MediaEdit, NoteDetail, NoteEdit, PlaceDetail, PlaceEdit,
    ProvenanceDraft, RepositoryDetail, RepositoryEdit, SourceDetail, SourceEdit, dispatch_citation_edit,
    dispatch_media_edit, dispatch_note_edit, dispatch_place_edit, dispatch_repository_edit, dispatch_source_edit,
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

async fn workspace() -> (Workspace, tempfile::TempDir, Localizer) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ws");
    Workspace::init(&path, &operator(), &AppDefaults::default(), None).expect("init");
    let ws = Workspace::open(&path, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    let loc = Localizer::for_workspace(&path, None);
    (ws, dir, loc)
}

async fn new_place(ws: &Workspace) -> String {
    let new = NewPlace {
        human_id: None,
        place_type: PlaceType::Farm,
        name: None,
    };
    create_place(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("place")
}

async fn place_row(ws: &Workspace, human_id: &str) {
    add_place_name(ws, &session(), human_id, "Hougen".to_owned(), MutationMeta::default())
        .await
        .expect("name");
}

async fn new_source(ws: &Workspace) -> String {
    let new = NewSource {
        human_id: None,
        title: None,
    };
    create_source(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("source")
}

async fn source_row(ws: &Workspace, human_id: &str) {
    let (key, value) = ("film".to_owned(), "8123".to_owned());
    add_source_attribute(ws, &session(), human_id, key, value, MutationMeta::default())
        .await
        .expect("attribute");
}

async fn new_citation(ws: &Workspace) -> String {
    let source = new_source(ws).await;
    let new = NewCitation {
        human_id: None,
        source,
        page: None,
    };
    create_citation(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("citation")
}

async fn citation_row(ws: &Workspace, human_id: &str) {
    let (key, value) = ("line".to_owned(), "4".to_owned());
    add_citation_attribute(ws, &session(), human_id, key, value, MutationMeta::default())
        .await
        .expect("attribute");
}

async fn new_repository(ws: &Workspace) -> String {
    let new = NewRepository {
        human_id: None,
        name: None,
    };
    create_repository(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("repository")
}

async fn repository_row(ws: &Workspace, human_id: &str) {
    let url = Url {
        url_type: None,
        href: "https://example.org/archive".to_owned(),
        description: None,
    };
    add_repository_url(ws, &session(), human_id, url, MutationMeta::default())
        .await
        .expect("url");
}

async fn new_note(ws: &Workspace) -> String {
    let new = NewNote {
        human_id: None,
        text: Some("Born at the farm".to_owned()),
    };
    create_note(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("note")
}

async fn note_row(ws: &Workspace, human_id: &str) {
    let (language, text) = ("nb".to_owned(), "Født på garden".to_owned());
    add_note_translation(ws, &session(), human_id, language, text, None, MutationMeta::default())
        .await
        .expect("translation");
}

async fn new_media(ws: &Workspace) -> String {
    let new = NewMedia {
        human_id: None,
        path: None,
    };
    create_media(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("media")
}

async fn media_row(ws: &Workspace, human_id: &str) {
    let (key, value) = ("scanner".to_owned(), "flatbed".to_owned());
    add_media_attribute(ws, &session(), human_id, key, value, MutationMeta::default())
        .await
        .expect("attribute");
}

/// One kind's routing: the member's row is attributed on the root's detail, and undoing it from the
/// root writes to the member.
macro_rules! routing_test {
    (
        $name:ident,
        new: $new:ident,
        row: $row:ident,
        merge: $merge:ident,
        show: $show:ident,
        detail: $Detail:ident,
        rows: $rows:ident,
        edit: $Edit:ident,
        dispatch: $dispatch:ident $(,)?
    ) => {
        #[tokio::test]
        async fn $name() {
            let (ws, _dir, loc) = workspace().await;
            let (root, member) = ($new(&ws).await, $new(&ws).await);
            $row(&ws, &member).await;
            $merge(&ws, &session(), &root, &member, IdentityDecision::default())
                .await
                .expect("merge");

            let summary = $show(&ws, &root).await.expect("show").expect("root");
            let detail = $Detail::from_summary(&summary, &loc);
            let row = detail.$rows.last().expect("the member's row");
            assert_eq!(
                row.merged_from.as_deref(),
                Some(member.as_str()),
                "the row names its member"
            );

            let edit = $Edit::UndoAssertion {
                human_id: root.clone(),
                assertion_id: row.assertion_id.clone(),
            };
            let target = $dispatch(&ws, &session(), &edit, &ProvenanceDraft::default())
                .await
                .expect("undo routed to the member");
            assert_eq!(target, root, "the pane reloads the root");
            let after = $show(&ws, &root).await.expect("show").expect("root");
            assert!(
                $Detail::from_summary(&after, &loc).$rows.is_empty(),
                "the member's row is undone"
            );
        }
    };
}

routing_test!(
    a_member_place_row_is_attributed_and_undone_on_the_member,
    new: new_place,
    row: place_row,
    merge: merge_places,
    show: show_place,
    detail: PlaceDetail,
    rows: names,
    edit: PlaceEdit,
    dispatch: dispatch_place_edit,
);

routing_test!(
    a_member_source_row_is_attributed_and_undone_on_the_member,
    new: new_source,
    row: source_row,
    merge: merge_sources,
    show: show_source,
    detail: SourceDetail,
    rows: attributes,
    edit: SourceEdit,
    dispatch: dispatch_source_edit,
);

routing_test!(
    a_member_citation_row_is_attributed_and_undone_on_the_member,
    new: new_citation,
    row: citation_row,
    merge: merge_citations,
    show: show_citation,
    detail: CitationDetail,
    rows: attributes,
    edit: CitationEdit,
    dispatch: dispatch_citation_edit,
);

routing_test!(
    a_member_repository_row_is_attributed_and_undone_on_the_member,
    new: new_repository,
    row: repository_row,
    merge: merge_repositories,
    show: show_repository,
    detail: RepositoryDetail,
    rows: urls,
    edit: RepositoryEdit,
    dispatch: dispatch_repository_edit,
);

routing_test!(
    a_member_note_row_is_attributed_and_undone_on_the_member,
    new: new_note,
    row: note_row,
    merge: merge_notes,
    show: show_note,
    detail: NoteDetail,
    rows: translations,
    edit: NoteEdit,
    dispatch: dispatch_note_edit,
);

routing_test!(
    a_member_media_row_is_attributed_and_undone_on_the_member,
    new: new_media,
    row: media_row,
    merge: merge_media,
    show: show_media,
    detail: MediaDetail,
    rows: attributes,
    edit: MediaEdit,
    dispatch: dispatch_media_edit,
);
