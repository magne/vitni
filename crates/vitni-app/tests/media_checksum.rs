//! Media checksums are recorded from the file's bytes (#359): creating a media object, or pointing one
//! at a new file, hashes the file when it is in the workspace media library and records the digest on
//! the aggregate — no operator types it.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::fs;

use uuid::Uuid;
use vitni_app::{
    AppDefaults, MutationMeta, NewMedia, OperatorConfig, Provenance, Session, Workspace, WorkspaceDefaults,
    change_log_for_media, create_media, set_media_file_path, show_media,
};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};

/// SHA-256 of `b"abc"` (FIPS 180-2 test vector), the same one the plugin host's `media-store` test uses.
const ABC: &str = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// SHA-256 of `b"xyz"`.
const XYZ: &str = "sha256:3608bca1e44ea6c4d268eb6db02260269892c0b42b86bbf1e77a6fa16c3c9282";

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

/// Writes `bytes` at `rel` below the workspace media root.
fn library_file(ws: &Workspace, rel: &str, bytes: &[u8]) {
    let target = ws.media_root().join(rel);
    fs::create_dir_all(target.parent().expect("a parent directory")).expect("create dirs");
    fs::write(target, bytes).expect("write file");
}

async fn create(ws: &Workspace, path: &str) -> String {
    create_media(
        ws,
        &session(),
        NewMedia {
            human_id: None,
            path: Some(path.to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("create media")
}

async fn checksum(ws: &Workspace, human_id: &str) -> Option<String> {
    show_media(ws, human_id)
        .await
        .expect("show")
        .expect("media exists")
        .checksum
}

async fn checksum_events(ws: &Workspace, human_id: &str) -> usize {
    let log = change_log_for_media(ws, human_id).await.expect("change log");
    let mut count = 0;
    for entry in log {
        if entry.event_type == "ChecksumSet" {
            count += 1;
        }
    }
    count
}

#[tokio::test]
async fn creating_media_over_a_library_file_records_its_checksum() {
    let (ws, _dir) = workspace().await;
    library_file(&ws, "portraits/ada.jpg", b"abc");
    let media = create(&ws, "media/portraits/ada.jpg").await;
    assert_eq!(checksum(&ws, &media).await.as_deref(), Some(ABC));
}

#[tokio::test]
async fn a_bare_root_relative_path_is_hashed_like_the_stored_form() {
    let (ws, _dir) = workspace().await;
    library_file(&ws, "02_folketelling/1920 bergstøl.png", b"abc");
    let media = create(&ws, "02_folketelling/1920 bergstøl.png").await;
    assert_eq!(checksum(&ws, &media).await.as_deref(), Some(ABC));
}

#[tokio::test]
async fn a_file_not_on_this_machine_creates_the_record_without_a_checksum() {
    let (ws, _dir) = workspace().await;
    let media = create(&ws, "media/portraits/missing.jpg").await;
    assert_eq!(checksum(&ws, &media).await, None);
    assert_eq!(checksum_events(&ws, &media).await, 0);
}

#[tokio::test]
async fn a_file_outside_the_media_library_is_not_hashed() {
    let (ws, dir) = workspace().await;
    let outside = dir.path().join("outside.jpg");
    fs::write(&outside, b"abc").expect("write file");
    let media = create(&ws, outside.to_str().expect("utf-8 path")).await;
    assert_eq!(checksum(&ws, &media).await, None);
}

#[tokio::test]
async fn pointing_media_at_a_different_file_records_the_new_checksum() {
    let (ws, _dir) = workspace().await;
    library_file(&ws, "a.jpg", b"abc");
    library_file(&ws, "b.jpg", b"xyz");
    let media = create(&ws, "media/a.jpg").await;
    set_media_file_path(
        &ws,
        &session(),
        &media,
        "media/b.jpg".to_owned(),
        MutationMeta::default(),
    )
    .await
    .expect("set path");
    assert_eq!(checksum(&ws, &media).await.as_deref(), Some(XYZ));
    assert_eq!(checksum_events(&ws, &media).await, 2);
}

#[tokio::test]
async fn a_moved_file_with_the_same_bytes_records_no_new_checksum() {
    let (ws, _dir) = workspace().await;
    library_file(&ws, "a.jpg", b"abc");
    library_file(&ws, "moved/a.jpg", b"abc");
    let media = create(&ws, "media/a.jpg").await;
    set_media_file_path(
        &ws,
        &session(),
        &media,
        "media/moved/a.jpg".to_owned(),
        MutationMeta::default(),
    )
    .await
    .expect("set path");
    assert_eq!(checksum(&ws, &media).await.as_deref(), Some(ABC));
    assert_eq!(checksum_events(&ws, &media).await, 1);
}

#[tokio::test]
async fn pointing_media_at_a_missing_file_keeps_the_recorded_checksum() {
    let (ws, _dir) = workspace().await;
    library_file(&ws, "a.jpg", b"abc");
    let media = create(&ws, "media/a.jpg").await;
    set_media_file_path(
        &ws,
        &session(),
        &media,
        "media/gone.jpg".to_owned(),
        MutationMeta::default(),
    )
    .await
    .expect("set path");
    assert_eq!(checksum(&ws, &media).await.as_deref(), Some(ABC));
}
