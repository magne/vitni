//! Workspace backup and restore (ADR 0041): an archive of the event log restores into a new
//! workspace whose projections equal the original's row for row, and a damaged, foreign or
//! out-of-window archive is refused before anything is created. A restore can also replace the open
//! workspace (ADR 0044), behind an automatic pre-restore backup.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]
#![expect(clippy::panic, reason = "an unexpected outcome aborts the test")]

use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use time::macros::datetime;
use uuid::Uuid;
use vitni_app::backup::{
    BackupError, BackupRequest, ReplaceRequest, RestoreRequest, create_backup, replace_backup, restore_backup,
};
use vitni_app::{
    AppDefaults, AppError, DatasetId, ImportCounts, NewImportRun, NewMedia, NewNote, NewPerson, OperatorConfig,
    PersonNameParts, Provenance, RecordOrigin, Session, Workspace, WorkspaceDefaults, create_media, create_note,
    create_person, finish_import_run, list_datasets, list_import_runs, read_pre_restore_backup,
    save_pre_restore_backup, start_import_run,
};
use vitni_core::enums::EvidenceLevel;
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

/// A workspace under `home/source` holding a person, a note and a media object whose file is in
/// the media library.
async fn seeded_workspace(home: &Path) -> Workspace {
    let dir = home.join("source");
    Workspace::init(&dir, &operator(), &AppDefaults::default(), None).expect("init");
    let ws = Workspace::open(&dir, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open");
    let name = PersonNameParts::simple(Some("Ada".to_owned()), Some("Lovelace".to_owned()));
    let person = NewPerson {
        human_id: None,
        name: Some(name),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    create_person(&ws, &session(), person, Provenance::default(), &[])
        .await
        .expect("person");
    let note = NewNote {
        human_id: None,
        text: Some("Born in London.".to_owned()),
    };
    create_note(&ws, &session(), note, Provenance::default(), &[])
        .await
        .expect("note");
    fs::create_dir_all(ws.media_root().join("portraits")).expect("media dir");
    fs::write(ws.media_root().join("portraits/ada.jpg"), b"not really a jpeg").expect("media file");
    let media = NewMedia {
        human_id: None,
        path: Some("media/portraits/ada.jpg".to_owned()),
    };
    create_media(&ws, &session(), media, Provenance::default(), &[])
        .await
        .expect("media");
    ws
}

fn backup_request(destination: &Path, with_media: bool) -> BackupRequest<'_> {
    BackupRequest {
        workspace_name: "gen",
        destination,
        with_media,
        created_at: datetime!(2026-09-28 12:00:00 UTC),
    }
}

fn restore_request<'a>(config_path: &'a Path, archive: &'a Path, target: &'a Path) -> RestoreRequest<'a> {
    RestoreRequest {
        config_path,
        archive,
        name: "restored",
        dir: Some(target),
        database_url: None,
    }
}

async fn open(dir: &Path) -> Workspace {
    Workspace::open(dir, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open restored")
}

fn backup_error(result: Result<impl std::fmt::Debug, AppError>) -> BackupError {
    match result {
        Err(AppError::Backup(error)) => error,
        other => panic!("expected a backup error, got {other:?}"),
    }
}

fn checksum(bytes: &[u8]) -> String {
    let mut text = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// Reads every member of the archive at `path` into `(name, bytes)` pairs, in archive order.
fn members(path: &Path) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path).expect("open archive")).expect("zip");
    let mut out = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).expect("member");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).expect("read member");
        out.push((file.name().to_owned(), bytes));
    }
    out
}

fn member(path: &Path, name: &str) -> Vec<u8> {
    members(path)
        .into_iter()
        .find(|(member, _)| member == name)
        .map(|(_, bytes)| bytes)
        .expect("member present")
}

/// Writes `members` as a new archive at `path`.
fn write_members(path: &Path, members: &[(String, Vec<u8>)]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).expect("create archive"));
    for (name, bytes) in members {
        writer
            .start_file(name.as_str(), zip::write::SimpleFileOptions::default())
            .expect("start member");
        writer.write_all(bytes).expect("write member");
    }
    writer.finish().expect("finish archive");
}

/// Copies the archive at `from` to `to`, passing every member through `edit`.
fn rewrite(from: &Path, to: &Path, edit: impl Fn(&str, Vec<u8>) -> Vec<u8>) {
    let edited: Vec<_> = members(from)
        .into_iter()
        .map(|(name, bytes)| {
            let bytes = edit(&name, bytes);
            (name, bytes)
        })
        .collect();
    write_members(to, &edited);
}

/// Edits the manifest's JSON in place.
fn edit_manifest(bytes: &[u8], edit: impl Fn(&mut serde_json::Value)) -> Vec<u8> {
    let mut manifest: serde_json::Value = serde_json::from_slice(bytes).expect("manifest json");
    edit(&mut manifest);
    serde_json::to_vec(&manifest).expect("manifest bytes")
}

struct Fixture {
    home: tempfile::TempDir,
    config: PathBuf,
    source: Workspace,
    archive: PathBuf,
}

async fn backed_up(with_media: bool) -> Fixture {
    let home = tempfile::tempdir().expect("tempdir");
    let source = seeded_workspace(home.path()).await;
    let archive = home.path().join("gen.vitni-backup");
    create_backup(&source, &backup_request(&archive, with_media))
        .await
        .expect("backup");
    let config = home.path().join("config.toml");
    Fixture {
        home,
        config,
        source,
        archive,
    }
}

#[tokio::test]
async fn a_restore_reproduces_every_event_and_projection() {
    let fixture = backed_up(false).await;
    let target = fixture.home.path().join("restored");
    let report = restore_backup(&restore_request(&fixture.config, &fixture.archive, &target))
        .await
        .expect("restore");
    assert_eq!(
        report.events, 7,
        "person (2), note (2) and media (3, the checksum included) events"
    );
    assert_eq!(report.workspace.name, "restored");
    assert_eq!(report.format_version, "0.1");

    let restored = open(&target).await;
    let source_rows = fixture.source.store().read_raw_events(None, 1000).await.expect("rows");
    let restored_rows = restored.store().read_raw_events(None, 1000).await.expect("rows");
    assert_eq!(restored_rows, source_rows, "every row is inserted as stored");
    assert_eq!(
        restored.store().projection_rows().await.expect("projections"),
        fixture.source.store().projection_rows().await.expect("projections"),
        "projections equal row for row"
    );
    assert_eq!(
        report.media_missing,
        ["media/portraits/ada.jpg"],
        "without --with-media the library file is reported missing"
    );
}

#[tokio::test]
async fn with_media_the_library_files_come_back() {
    let fixture = backed_up(true).await;
    let target = fixture.home.path().join("restored");
    let report = restore_backup(&restore_request(&fixture.config, &fixture.archive, &target))
        .await
        .expect("restore");
    assert_eq!(report.media_restored, 1);
    assert!(report.media_missing.is_empty(), "{:?}", report.media_missing);
    assert!(report.media_mismatched.is_empty(), "{:?}", report.media_mismatched);
    assert_eq!(
        fs::read(target.join("media/portraits/ada.jpg")).expect("restored file"),
        b"not really a jpeg"
    );
}

#[tokio::test]
async fn media_outside_the_library_is_verified_where_it_lives() {
    let home = tempfile::tempdir().expect("tempdir");
    let source = seeded_workspace(home.path()).await;
    let scan = home.path().join("scans/census.png");
    fs::create_dir_all(scan.parent().expect("parent")).expect("dir");
    fs::write(&scan, b"census scan").expect("write");
    let media = NewMedia {
        human_id: None,
        path: Some(scan.display().to_string()),
    };
    create_media(&source, &session(), media, Provenance::default(), &[])
        .await
        .expect("media");
    let archive = home.path().join("gen.vitni-backup");
    create_backup(&source, &backup_request(&archive, true))
        .await
        .expect("backup");
    let names: Vec<String> = members(&archive).into_iter().map(|(name, _)| name).collect();
    assert!(
        !names.iter().any(|name| name.contains("census")),
        "only library files are archived: {names:?}"
    );

    let config = home.path().join("config.toml");
    let target = home.path().join("restored");
    let report = restore_backup(&restore_request(&config, &archive, &target))
        .await
        .expect("restore");
    assert!(report.media_missing.is_empty(), "{:?}", report.media_missing);
    assert!(report.media_mismatched.is_empty(), "{:?}", report.media_mismatched);

    fs::write(&scan, b"a different scan").expect("overwrite");
    let again = home.path().join("again");
    let mut request = restore_request(&config, &archive, &again);
    request.name = "again";
    let report = restore_backup(&request).await.expect("restore again");
    assert_eq!(report.media_mismatched, [scan.display().to_string()]);
}

#[tokio::test]
async fn the_archive_holds_the_manifest_the_log_and_a_portable_workspace_manifest() {
    let fixture = backed_up(true).await;
    let names: Vec<String> = members(&fixture.archive).into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        [
            "events.jsonl",
            "workspace.toml",
            "media.json",
            "media/portraits/ada.jpg",
            "manifest.json"
        ]
    );

    let manifest: serde_json::Value =
        serde_json::from_slice(&member(&fixture.archive, "manifest.json")).expect("manifest");
    assert_eq!(manifest["format_version"], "0.1");
    assert_eq!(manifest["app_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest["workspace"], "gen");
    assert_eq!(manifest["event_count"], 7);
    assert_eq!(manifest["created_at"], "2026-09-28T12:00:00Z");
    let events = member(&fixture.archive, "events.jsonl");
    assert_eq!(manifest["members"]["events.jsonl"], checksum(&events));
    assert_eq!(
        manifest["members"]["media/portraits/ada.jpg"],
        checksum(b"not really a jpeg")
    );
    assert!(manifest["members"].get("manifest.json").is_none());

    let first: serde_json::Value =
        serde_json::from_slice(events.split(|b| *b == b'\n').next().expect("a line")).expect("row");
    for column in [
        "aggregate_type",
        "aggregate_id",
        "sequence",
        "event_type",
        "event_version",
        "payload",
        "metadata",
    ] {
        assert!(first.get(column).is_some(), "row has {column}: {first}");
    }

    let toml = String::from_utf8(member(&fixture.archive, "workspace.toml")).expect("utf-8");
    assert!(!toml.contains("database_url"), "machine-local: {toml}");
    assert!(toml.contains("[operators."), "{toml}");

    let media: serde_json::Value = serde_json::from_slice(&member(&fixture.archive, "media.json")).expect("media");
    assert_eq!(
        media,
        serde_json::json!([{
            "path": "media/portraits/ada.jpg",
            "checksum": checksum(b"not really a jpeg"),
            "size": 17,
        }])
    );
}

#[tokio::test]
async fn two_backups_of_one_workspace_are_byte_identical() {
    let fixture = backed_up(true).await;
    let second = fixture.home.path().join("second.vitni-backup");
    create_backup(&fixture.source, &backup_request(&second, true))
        .await
        .expect("backup");
    assert_eq!(
        fs::read(&second).expect("read"),
        fs::read(&fixture.archive).expect("read")
    );
}

#[tokio::test]
async fn a_backup_never_overwrites_an_existing_file() {
    let fixture = backed_up(false).await;
    let error = backup_error(create_backup(&fixture.source, &backup_request(&fixture.archive, false)).await);
    assert!(matches!(error, BackupError::DestinationExists(_)), "{error:?}");
}

#[tokio::test]
async fn a_missing_media_file_is_reported_by_the_backup() {
    let home = tempfile::tempdir().expect("tempdir");
    let source = seeded_workspace(home.path()).await;
    fs::remove_file(source.media_root().join("portraits/ada.jpg")).expect("remove");
    let archive = home.path().join("gen.vitni-backup");
    let report = create_backup(&source, &backup_request(&archive, true))
        .await
        .expect("backup");
    assert_eq!(report.events, 7);
    assert_eq!(report.media_missing, ["media/portraits/ada.jpg"]);
    assert_eq!(report.media_files, 0);
}

/// Asserts a refused restore registered nothing and created no directory.
fn assert_nothing_created(home: &Path, target: &Path) {
    assert!(!target.exists(), "no workspace directory was created");
    let config = fs::read_to_string(home.join("config.toml")).unwrap_or_default();
    assert!(!config.contains("restored"), "nothing was registered: {config}");
}

#[tokio::test]
async fn a_damaged_member_is_refused_before_anything_is_created() {
    let fixture = backed_up(false).await;
    let damaged = fixture.home.path().join("damaged.vitni-backup");
    rewrite(&fixture.archive, &damaged, |name, mut bytes| {
        if name == "events.jsonl" {
            bytes.extend_from_slice(b"\n");
        }
        bytes
    });
    let target = fixture.home.path().join("restored");
    let error = backup_error(restore_backup(&restore_request(&fixture.config, &damaged, &target)).await);
    assert!(
        matches!(&error, BackupError::ChecksumMismatch(member) if member == "events.jsonl"),
        "{error:?}"
    );
    assert_nothing_created(fixture.home.path(), &target);
}

#[tokio::test]
async fn a_member_missing_from_the_archive_is_refused() {
    let fixture = backed_up(false).await;
    let partial = fixture.home.path().join("partial.vitni-backup");
    let kept: Vec<_> = members(&fixture.archive)
        .into_iter()
        .filter(|(name, _)| name != "media.json")
        .collect();
    write_members(&partial, &kept);
    let target = fixture.home.path().join("restored");
    let error = backup_error(restore_backup(&restore_request(&fixture.config, &partial, &target)).await);
    assert!(
        matches!(&error, BackupError::MissingMember(member) if member == "media.json"),
        "{error:?}"
    );
    assert_nothing_created(fixture.home.path(), &target);
}

#[tokio::test]
async fn an_unlisted_member_is_refused() {
    let fixture = backed_up(false).await;
    let extra = fixture.home.path().join("extra.vitni-backup");
    let mut all = members(&fixture.archive);
    all.push(("media/../../escape.txt".to_owned(), b"x".to_vec()));
    write_members(&extra, &all);
    let target = fixture.home.path().join("restored");
    let error = backup_error(restore_backup(&restore_request(&fixture.config, &extra, &target)).await);
    assert!(matches!(&error, BackupError::UnexpectedMember(_)), "{error:?}");
    assert_nothing_created(fixture.home.path(), &target);
    assert!(!fixture.home.path().join("escape.txt").exists());
}

#[tokio::test]
async fn a_file_that_is_not_a_backup_is_refused() {
    let home = tempfile::tempdir().expect("tempdir");
    let bogus = home.path().join("notes.vitni-backup");
    fs::write(&bogus, b"plain text").expect("write");
    let target = home.path().join("restored");
    let config = home.path().join("config.toml");
    let error = backup_error(restore_backup(&restore_request(&config, &bogus, &target)).await);
    assert!(matches!(error, BackupError::NotABackup(_)), "{error:?}");
    assert_nothing_created(home.path(), &target);
}

#[tokio::test]
async fn a_newer_format_is_refused_with_both_versions_named() {
    let fixture = backed_up(false).await;
    let newer = fixture.home.path().join("newer.vitni-backup");
    rewrite(&fixture.archive, &newer, |name, bytes| {
        if name == "manifest.json" {
            return edit_manifest(&bytes, |manifest| manifest["format_version"] = "0.9".into());
        }
        bytes
    });
    let target = fixture.home.path().join("restored");
    let error = backup_error(restore_backup(&restore_request(&fixture.config, &newer, &target)).await);
    let BackupError::FormatTooNew { found, current } = &error else {
        panic!("expected FormatTooNew, got {error:?}");
    };
    assert_eq!((found.as_str(), current.as_str()), ("0.9", "0.1"));
    assert_nothing_created(fixture.home.path(), &target);
}

#[tokio::test]
async fn a_format_no_release_wrote_is_refused() {
    let fixture = backed_up(false).await;
    let unknown = fixture.home.path().join("unknown.vitni-backup");
    rewrite(&fixture.archive, &unknown, |name, bytes| {
        if name == "manifest.json" {
            return edit_manifest(&bytes, |manifest| manifest["format_version"] = "0.0".into());
        }
        bytes
    });
    let target = fixture.home.path().join("restored");
    let error = backup_error(restore_backup(&restore_request(&fixture.config, &unknown, &target)).await);
    assert!(
        matches!(&error, BackupError::UnknownFormat(found) if found == "0.0"),
        "{error:?}"
    );
}

#[tokio::test]
async fn an_event_that_does_not_decode_is_refused_with_its_line() {
    let fixture = backed_up(false).await;
    let events = String::from_utf8(member(&fixture.archive, "events.jsonl")).expect("utf-8");
    let mut lines: Vec<String> = events.lines().map(str::to_owned).collect();
    let mut row: serde_json::Value = serde_json::from_str(&lines[1]).expect("row");
    row["payload"]["type"] = "NoSuchVariant".into();
    lines[1] = row.to_string();
    let edited = format!("{}\n", lines.join("\n")).into_bytes();
    let digest = checksum(&edited);
    let broken = fixture.home.path().join("broken.vitni-backup");
    rewrite(&fixture.archive, &broken, |name, bytes| match name {
        "events.jsonl" => edited.clone(),
        "manifest.json" => edit_manifest(&bytes, |manifest| {
            manifest["members"]["events.jsonl"] = digest.clone().into();
        }),
        _ => bytes,
    });
    let target = fixture.home.path().join("restored");
    let error = backup_error(restore_backup(&restore_request(&fixture.config, &broken, &target)).await);
    assert!(matches!(&error, BackupError::InvalidEvent { line: 2, .. }), "{error:?}");
    assert_nothing_created(fixture.home.path(), &target);
}

#[tokio::test]
async fn an_event_count_that_disagrees_with_the_log_is_refused() {
    let fixture = backed_up(false).await;
    let short = fixture.home.path().join("short.vitni-backup");
    rewrite(&fixture.archive, &short, |name, bytes| {
        if name == "manifest.json" {
            return edit_manifest(&bytes, |manifest| manifest["event_count"] = 8.into());
        }
        bytes
    });
    let target = fixture.home.path().join("restored");
    let error = backup_error(restore_backup(&restore_request(&fixture.config, &short, &target)).await);
    assert!(
        matches!(error, BackupError::EventCountMismatch { expected: 8, found: 7 }),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_non_empty_target_directory_is_refused() {
    let fixture = backed_up(false).await;
    let target = fixture.home.path().join("restored");
    fs::create_dir_all(&target).expect("dir");
    fs::write(target.join("keep.txt"), b"mine").expect("write");
    let error = backup_error(restore_backup(&restore_request(&fixture.config, &fixture.archive, &target)).await);
    assert!(matches!(error, BackupError::TargetNotEmpty(_)), "{error:?}");
    assert_eq!(fs::read(target.join("keep.txt")).expect("kept"), b"mine");
}

#[tokio::test]
async fn an_empty_existing_target_directory_is_used() {
    let fixture = backed_up(false).await;
    let target = fixture.home.path().join("restored");
    fs::create_dir_all(&target).expect("dir");
    restore_backup(&restore_request(&fixture.config, &fixture.archive, &target))
        .await
        .expect("restore");
    assert!(target.join("workspace.toml").exists());
}

#[tokio::test]
async fn a_manifest_that_does_not_list_the_log_is_refused() {
    let fixture = backed_up(false).await;
    let unlisted = fixture.home.path().join("unlisted.vitni-backup");
    rewrite(&fixture.archive, &unlisted, |name, bytes| {
        if name == "manifest.json" {
            return edit_manifest(&bytes, |manifest| {
                if let Some(members) = manifest["members"].as_object_mut() {
                    members.remove("events.jsonl");
                }
            });
        }
        bytes
    });
    let target = fixture.home.path().join("restored");
    let error = backup_error(restore_backup(&restore_request(&fixture.config, &unlisted, &target)).await);
    assert!(
        matches!(&error, BackupError::UnexpectedMember(member) if member == "events.jsonl"),
        "{error:?}"
    );
    assert_nothing_created(fixture.home.path(), &target);
}

/// The record origin on every event of `workspace`, in log order.
async fn origins(workspace: &Workspace) -> Vec<Option<vitni_core::origin::RecordOrigin>> {
    #[derive(serde::Deserialize)]
    struct Header {
        context: vitni_core::provenance::EventContext,
    }
    let rows = workspace.store().read_raw_events(None, 1000).await.expect("rows");
    rows.into_iter()
        .map(|row| {
            let header: Header = serde_json::from_value(row.payload).expect("decode envelope");
            header.context.origin.map(|origin| *origin)
        })
        .collect()
}

#[tokio::test]
async fn a_backup_keeps_every_record_origin_and_import_run() {
    let home = tempfile::tempdir().expect("tempdir");
    let dir = home.path().join("source");
    Workspace::init(&dir, &operator(), &AppDefaults::default(), None).expect("init");
    let source = Workspace::open(&dir, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open");
    let dataset = DatasetId::lineage("gedcom", Uuid::from_u128(5));
    let run = NewImportRun {
        plugin: "gedcom-import".to_owned(),
        plugin_version: "0.1.0".to_owned(),
        dataset: dataset.clone(),
        dataset_label: "tree.ged".to_owned(),
        source_label: "tree.ged".to_owned(),
        source_path: None,
        file_asserted_at: None,
        unreadable_file_date: None,
        dataset_hint: None,
    };
    let run = start_import_run(&source, &session(), run).await.expect("start");
    let importer = Session::software("gedcom-import", "0.1.0");
    let provenance = Provenance {
        origin: Some(RecordOrigin {
            dataset,
            record: "I1".to_owned(),
            item: None,
            digest: None,
            run,
        }),
        ..Provenance::default()
    };
    let person = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(Some("Ada".to_owned()), None)),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    create_person(&source, &importer, person, provenance, &[])
        .await
        .expect("person");
    finish_import_run(&source, &session(), run, Vec::new(), ImportCounts::default())
        .await
        .expect("finish");

    let archive = home.path().join("gen.vitni-backup");
    create_backup(&source, &backup_request(&archive, false))
        .await
        .expect("backup");
    let target = home.path().join("restored");
    let config = home.path().join("config.toml");
    restore_backup(&restore_request(&config, &archive, &target))
        .await
        .expect("restore");
    let restored = open(&target).await;

    let kept = origins(&restored).await;
    assert_eq!(kept, origins(&source).await, "every origin survives, event for event");
    assert_eq!(
        kept.iter().flatten().count(),
        2,
        "the person's creation and name carry theirs"
    );
    assert_eq!(
        list_import_runs(&restored).await.expect("runs"),
        list_import_runs(&source).await.expect("runs"),
        "the run comes back with its operator, dataset and end"
    );
    assert_eq!(
        list_datasets(&restored).await.expect("datasets"),
        list_datasets(&source).await.expect("datasets")
    );
}

#[tokio::test]
async fn a_restore_keeps_the_workspace_id() {
    let fixture = backed_up(false).await;
    let target = fixture.home.path().join("restored");
    restore_backup(&restore_request(&fixture.config, &fixture.archive, &target))
        .await
        .expect("restore");

    assert_eq!(
        open(&target).await.id(),
        fixture.source.id(),
        "a restore continues the same tree"
    );
}

#[tokio::test]
async fn a_backup_taken_before_workspace_ids_restores_with_an_id_of_its_own() {
    let home = tempfile::tempdir().expect("tempdir");
    let source = seeded_workspace(home.path()).await;
    let manifest = source.dir().join("workspace.toml");
    let text = fs::read_to_string(&manifest).expect("manifest");
    let mut without_id = String::new();
    for line in text.lines().filter(|line| !line.starts_with("id = ")) {
        without_id.push_str(line);
        without_id.push('\n');
    }
    assert_ne!(without_id, text, "the manifest carried an id");
    fs::write(&manifest, without_id).expect("write manifest");
    let archive = home.path().join("old.vitni-backup");
    create_backup(&source, &backup_request(&archive, false))
        .await
        .expect("backup");
    let target = home.path().join("restored");

    restore_backup(&restore_request(&home.path().join("config.toml"), &archive, &target))
        .await
        .expect("restore");

    let restored = fs::read_to_string(target.join("workspace.toml")).expect("restored manifest");
    let restored_id = open(&target).await.id();
    assert!(restored.contains(&format!("id = \"{restored_id}\"")), "{restored}");
}

/// A second workspace under `home/target` holding one person of its own, to be replaced.
async fn replace_target(home: &Path) -> Workspace {
    let dir = home.join("target");
    Workspace::init(&dir, &operator(), &AppDefaults::default(), None).expect("init");
    let ws = open(&dir).await;
    let person = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some("Charles".to_owned()),
            Some("Babbage".to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    create_person(&ws, &session(), person, Provenance::default(), &[])
        .await
        .expect("person");
    ws
}

fn replace_request(archive: &Path) -> ReplaceRequest<'_> {
    ReplaceRequest {
        archive,
        workspace_name: "target",
        now: datetime!(2026-10-03 09:30:15 UTC),
    }
}

/// The pre-restore backup `replace_request` names in `ws`'s `backups/`.
fn pre_restore_path(ws: &Workspace) -> PathBuf {
    ws.dir()
        .join("backups")
        .join("target-pre-restore-20261003T093015Z.vitni-backup")
}

/// Every stored row and projection row of `ws`, the state a replace must leave or keep.
async fn state(ws: &Workspace) -> (Vec<vitni_db::RawEvent>, Vec<vitni_db::ProjectionRow>) {
    (
        ws.store().read_raw_events(None, 1000).await.expect("rows"),
        ws.store().projection_rows().await.expect("projections"),
    )
}

#[tokio::test]
async fn a_replace_leaves_the_archives_rows_and_projections() {
    let fixture = backed_up(false).await;
    let target = replace_target(fixture.home.path()).await;

    let report = replace_backup(&target, &replace_request(&fixture.archive))
        .await
        .expect("replace");

    assert_eq!(report.events, 7);
    assert_eq!(report.format_version, "0.1");
    assert_eq!(
        state(&target).await,
        state(&fixture.source).await,
        "row for row the archive's"
    );
    assert_eq!(
        report.pre_restore_backup.as_deref(),
        Some(pre_restore_path(&target).as_path())
    );
    assert_eq!(report.media_missing, ["media/portraits/ada.jpg"]);
}

#[tokio::test]
async fn the_pre_restore_backup_restores_the_previous_state() {
    let fixture = backed_up(false).await;
    let target = replace_target(fixture.home.path()).await;
    let before = state(&target).await;

    replace_backup(&target, &replace_request(&fixture.archive))
        .await
        .expect("replace");
    let back = fixture.home.path().join("back");
    restore_backup(&restore_request(&fixture.config, &pre_restore_path(&target), &back))
        .await
        .expect("restore the pre-restore backup");

    assert_eq!(state(&open(&back).await).await, before);
}

#[tokio::test]
async fn a_failed_pre_restore_backup_aborts_the_replace() {
    let fixture = backed_up(false).await;
    let target = replace_target(fixture.home.path()).await;
    let before = state(&target).await;
    fs::write(pre_restore_path(&target), b"in the way").expect("block the backup");

    let error = backup_error(replace_backup(&target, &replace_request(&fixture.archive)).await);

    match error {
        BackupError::PreRestoreBackup { path, .. } => assert_eq!(path, pre_restore_path(&target)),
        other => panic!("expected the pre-restore backup to fail, got {other:?}"),
    }
    assert_eq!(state(&target).await, before, "nothing was replaced");
}

#[tokio::test]
async fn a_workspace_can_switch_the_pre_restore_backup_off() {
    let fixture = backed_up(false).await;
    let target = replace_target(fixture.home.path()).await;
    assert!(read_pre_restore_backup(target.dir()).expect("read"), "on by default");
    save_pre_restore_backup(target.dir(), false).expect("switch off");

    let report = replace_backup(&target, &replace_request(&fixture.archive))
        .await
        .expect("replace");

    assert_eq!(report.pre_restore_backup, None);
    assert_eq!(fs::read_dir(target.dir().join("backups")).expect("backups").count(), 0);
    assert!(
        !read_pre_restore_backup(target.dir()).expect("read"),
        "the replace keeps this copy's setting, not the archive's"
    );
    assert_eq!(state(&target).await, state(&fixture.source).await);
}

#[tokio::test]
async fn a_damaged_archive_replaces_nothing_and_backs_nothing_up() {
    let fixture = backed_up(false).await;
    let target = replace_target(fixture.home.path()).await;
    let before = state(&target).await;
    let damaged = fixture.home.path().join("damaged.vitni-backup");
    rewrite(&fixture.archive, &damaged, |name, mut bytes| {
        if name == "events.jsonl" {
            bytes.push(b' ');
        }
        bytes
    });

    let error = backup_error(replace_backup(&target, &replace_request(&damaged)).await);

    assert!(matches!(error, BackupError::ChecksumMismatch(_)), "{error:?}");
    assert_eq!(state(&target).await, before);
    assert_eq!(fs::read_dir(target.dir().join("backups")).expect("backups").count(), 0);
}

#[tokio::test]
async fn a_replace_takes_the_archives_id_and_keeps_this_copys_database() {
    let fixture = backed_up(false).await;
    let target = replace_target(fixture.home.path()).await;
    let manifest = || fs::read_to_string(target.dir().join("workspace.toml")).expect("manifest");
    let database_line = manifest()
        .lines()
        .find(|line| line.starts_with("database_url"))
        .expect("database_url")
        .to_owned();

    replace_backup(&target, &replace_request(&fixture.archive))
        .await
        .expect("replace");

    assert_eq!(open(target.dir()).await.id(), fixture.source.id());
    assert!(manifest().contains(&database_line), "{}", manifest());
}

#[tokio::test]
async fn a_replace_adds_archived_media_but_never_overwrites_a_file() {
    let fixture = backed_up(true).await;
    let target = replace_target(fixture.home.path()).await;

    let report = replace_backup(&target, &replace_request(&fixture.archive))
        .await
        .expect("replace");
    assert_eq!(report.media_restored, 1);
    let file = target.media_root().join("portraits/ada.jpg");
    assert_eq!(fs::read(&file).expect("restored file"), b"not really a jpeg");

    fs::write(&file, b"edited since").expect("edit");
    save_pre_restore_backup(target.dir(), false).expect("switch off");
    let again = replace_backup(&target, &replace_request(&fixture.archive))
        .await
        .expect("replace again");
    assert_eq!(again.media_restored, 0);
    assert_eq!(again.media_mismatched, ["media/portraits/ada.jpg"]);
    assert_eq!(fs::read(&file).expect("kept file"), b"edited since");
}
