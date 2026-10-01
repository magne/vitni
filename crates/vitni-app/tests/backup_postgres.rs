//! The ADR 0041 exit test: a backup restores into the other engine, and the projections equal the
//! original's row for row — SQLite into Postgres, and Postgres back into SQLite.
//!
//! Needs a Docker daemon (`test-containers-util` reuses one `vitni-pg` container and gives each
//! test its own database), so it compiles only under `--features postgres` and CI runs it in the
//! native `postgres` job.

#![cfg(feature = "postgres")]
#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::path::Path;

use sqlx::migrate::Migrator;
use test_containers_util::sqlx_pg::PostgresTestDb;
use time::macros::datetime;
use uuid::Uuid;
use vitni_app::{
    AppDefaults, BackupRequest, NewNote, NewPerson, OperatorConfig, PersonNameParts, Provenance, RestoreRequest,
    Session, Workspace, WorkspaceDefaults, create_backup, create_note, create_person, restore_backup,
};
use vitni_core::enums::EvidenceLevel;
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};

const CONTAINER: &str = "vitni-pg";

/// An empty migrator: the schema is created by `Store::open`, but the test helper requires one.
static MIGRATIONS: Migrator = sqlx::migrate!("../vitni-db/migrations");

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

async fn workspace(dir: &Path, database_url: Option<&str>) -> Workspace {
    Workspace::init(dir, &operator(), &AppDefaults::default(), database_url).expect("init");
    Workspace::open(dir, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open")
}

async fn seed(ws: &Workspace) {
    for (given, surname) in [("Ada", "Lovelace"), ("Charles", "Babbage"), ("Mary", "Somerville")] {
        let person = NewPerson {
            human_id: None,
            name: Some(PersonNameParts::simple(
                Some(given.to_owned()),
                Some(surname.to_owned()),
            )),
            evidence_level: EvidenceLevel::Conclusion,
            external_ids: Vec::new(),
        };
        create_person(ws, &session(), person, Provenance::default(), &[])
            .await
            .expect("person");
    }
    let note = NewNote {
        human_id: None,
        text: Some("Corresponded about the Analytical Engine.".to_owned()),
    };
    create_note(ws, &session(), note, Provenance::default(), &[])
        .await
        .expect("note");
}

async fn back_up(ws: &Workspace, destination: &Path) {
    let request = BackupRequest {
        workspace_name: "gen",
        destination,
        with_media: false,
        created_at: datetime!(2026-09-28 12:00:00 UTC),
    };
    create_backup(ws, &request).await.expect("backup");
}

async fn restore(home: &Path, archive: &Path, name: &str, database_url: Option<&str>) -> Workspace {
    let config = home.join("config.toml");
    let dir = home.join(name);
    let request = RestoreRequest {
        config_path: &config,
        archive,
        name,
        dir: Some(&dir),
        database_url,
    };
    restore_backup(&request).await.expect("restore");
    Workspace::open(&dir, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open restored")
}

async fn assert_same_workspace(left: &Workspace, right: &Workspace) {
    assert_eq!(
        left.store().read_raw_events(None, 1000).await.expect("rows"),
        right.store().read_raw_events(None, 1000).await.expect("rows"),
        "the logs are identical"
    );
    let projections = left.store().projection_rows().await.expect("projections");
    assert!(!projections.is_empty(), "{projections:?}");
    assert_eq!(
        projections,
        right.store().projection_rows().await.expect("projections"),
        "projections equal row for row"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sqlite_backup_restores_into_postgres() {
    let home = tempfile::tempdir().expect("tempdir");
    let source = workspace(&home.path().join("source"), None).await;
    seed(&source).await;
    let archive = home.path().join("gen.vitni-backup");
    back_up(&source, &archive).await;

    let db = PostgresTestDb::create(CONTAINER, &MIGRATIONS, None, None).await;
    let restored = restore(home.path(), &archive, "pg", Some(db.dsn())).await;
    assert_same_workspace(&source, &restored).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_postgres_backup_restores_into_sqlite() {
    let home = tempfile::tempdir().expect("tempdir");
    let db = PostgresTestDb::create(CONTAINER, &MIGRATIONS, None, None).await;
    let source = workspace(&home.path().join("source"), Some(db.dsn())).await;
    seed(&source).await;
    let archive = home.path().join("gen.vitni-backup");
    back_up(&source, &archive).await;

    let restored = restore(home.path(), &archive, "local", None).await;
    assert_same_workspace(&source, &restored).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restore_into_a_database_that_holds_events_is_refused() {
    let home = tempfile::tempdir().expect("tempdir");
    let source = workspace(&home.path().join("source"), None).await;
    seed(&source).await;
    let archive = home.path().join("gen.vitni-backup");
    back_up(&source, &archive).await;

    let db = PostgresTestDb::create(CONTAINER, &MIGRATIONS, None, None).await;
    let occupied = workspace(&home.path().join("occupied"), Some(db.dsn())).await;
    seed(&occupied).await;
    let config = home.path().join("config.toml");
    let dir = home.path().join("second");
    let request = RestoreRequest {
        config_path: &config,
        archive: &archive,
        name: "second",
        dir: Some(&dir),
        database_url: Some(db.dsn()),
    };
    let result = restore_backup(&request).await;
    assert!(
        matches!(
            result,
            Err(vitni_app::AppError::Backup(vitni_app::BackupError::DatabaseNotEmpty))
        ),
        "{result:?}"
    );
    assert!(!dir.exists(), "the half-made workspace was rolled back");
    assert_eq!(
        occupied.store().event_count().await.expect("count"),
        8,
        "the occupied database is untouched"
    );
}

/// Rewrites the archive at `from` into `to` with two extra, correctly listed media members, `media/a`
/// and `media/a/b`. The archive passes every check, but extracting the second fails because the
/// first is a file — a failure that comes after the rows are already in the database.
fn with_colliding_media(from: &Path, to: &Path) {
    use std::fmt::Write as _;
    use std::io::{Read, Write};

    use sha2::Digest;

    let mut archive = zip::ZipArchive::new(std::fs::File::open(from).expect("open")).expect("zip");
    let mut members = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).expect("member");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).expect("read");
        members.push((file.name().to_owned(), bytes));
    }
    let extra = [
        ("media/a".to_owned(), b"one".to_vec()),
        ("media/a/b".to_owned(), b"two".to_vec()),
    ];
    for (name, bytes) in &mut members {
        if name == "manifest.json" {
            let mut manifest: serde_json::Value = serde_json::from_slice(bytes).expect("manifest");
            for (extra_name, extra_bytes) in &extra {
                let mut digest = String::new();
                for byte in sha2::Sha256::digest(extra_bytes) {
                    let _ = write!(digest, "{byte:02x}");
                }
                manifest["members"][extra_name] = format!("sha256:{digest}").into();
            }
            *bytes = serde_json::to_vec(&manifest).expect("bytes");
        }
    }
    members.extend(extra);
    let mut writer = zip::ZipWriter::new(std::fs::File::create(to).expect("create"));
    for (name, bytes) in members {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .expect("start");
        writer.write_all(&bytes).expect("write");
    }
    writer.finish().expect("finish");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restore_that_fails_after_inserting_leaves_the_database_empty() {
    let home = tempfile::tempdir().expect("tempdir");
    let source = workspace(&home.path().join("source"), None).await;
    seed(&source).await;
    let archive = home.path().join("gen.vitni-backup");
    back_up(&source, &archive).await;
    let colliding = home.path().join("colliding.vitni-backup");
    with_colliding_media(&archive, &colliding);

    let db = PostgresTestDb::create(CONTAINER, &MIGRATIONS, None, None).await;
    let config = home.path().join("config.toml");
    let dir = home.path().join("pg");
    let request = RestoreRequest {
        config_path: &config,
        archive: &colliding,
        name: "pg",
        dir: Some(&dir),
        database_url: Some(db.dsn()),
    };
    let result = restore_backup(&request).await;
    assert!(
        matches!(
            result,
            Err(vitni_app::AppError::Backup(vitni_app::BackupError::Archive(_)))
        ),
        "{result:?}"
    );
    assert!(!dir.exists(), "the half-made workspace was rolled back");

    let restored = restore(home.path(), &archive, "pg", Some(db.dsn())).await;
    assert_same_workspace(&source, &restored).await;
}
