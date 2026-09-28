//! The golden backup fixtures (ADR 0041 §5), the compatibility guard of the backup format.
//!
//! Every committed archive under `tests/fixtures/backup/v<format>/` must restore and reach its
//! recorded projection digest, and the current format's archive must hold every event variant of
//! every aggregate. A breaking event change turns a fixture red, which forces a format bump, an
//! upgrader and a new fixture; a new variant fails the coverage test until the fixture includes it.
//! `cargo xtask backup-fixture` regenerates the current archive and every digest.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use uuid::Uuid;
use vitni_app::backup::current_format_version;
use vitni_app::{OperatorConfig, RestoreRequest, Workspace, WorkspaceDefaults, projection_digest, restore_backup};
use vitni_core::ids::AgentId;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/backup");

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: None,
        email: None,
    }
}

/// Every fixture directory holding an archive, oldest format first.
fn fixture_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(FIXTURES).expect("the fixtures directory") {
        let path = entry.expect("an entry").path();
        if path.join("workspace.vitni-backup").is_file() {
            dirs.push(path);
        }
    }
    dirs.sort();
    dirs
}

fn current_fixture() -> PathBuf {
    Path::new(FIXTURES)
        .join(format!("v{}", current_format_version()))
        .join("workspace.vitni-backup")
}

#[tokio::test]
async fn every_fixture_restores_to_its_recorded_projections() {
    let dirs = fixture_dirs();
    assert!(!dirs.is_empty(), "no fixtures under {FIXTURES}");
    for dir in dirs {
        let home = tempfile::tempdir().expect("tempdir");
        let config = home.path().join("config.toml");
        let target = home.path().join("restored");
        let archive = dir.join("workspace.vitni-backup");
        let request = RestoreRequest {
            config_path: &config,
            archive: &archive,
            name: "fixture",
            dir: Some(&target),
            database_url: None,
        };
        let report = restore_backup(&request)
            .await
            .unwrap_or_else(|error| panic!("{} does not restore: {error}", dir.display()));
        assert!(
            report.media_missing.is_empty(),
            "{}: {:?}",
            dir.display(),
            report.media_missing
        );
        assert!(
            report.media_mismatched.is_empty(),
            "{}: {:?}",
            dir.display(),
            report.media_mismatched
        );
        let workspace = Workspace::open(&target, &operator(), &WorkspaceDefaults::default())
            .await
            .expect("open the restored fixture");
        let expected = fs::read_to_string(dir.join("projection-digest.txt")).expect("the recorded digest");
        assert_eq!(
            projection_digest(&workspace).await.expect("digest"),
            expected.trim(),
            "{} restores to different projections; rerun `cargo xtask backup-fixture` if the projections \
             changed on purpose",
            dir.display()
        );
    }
}

#[test]
fn the_current_format_has_a_fixture() {
    assert!(
        current_fixture().is_file(),
        "no fixture for the current format; run `cargo xtask backup-fixture`"
    );
}

#[test]
fn the_current_fixture_holds_every_event_variant_of_every_aggregate() {
    let file = fs::File::open(current_fixture()).expect("the current fixture");
    let mut archive = zip::ZipArchive::new(file).expect("a zip");
    let mut events = String::new();
    archive
        .by_name("events.jsonl")
        .expect("the log")
        .read_to_string(&mut events)
        .expect("utf-8");
    let mut present = BTreeSet::new();
    for line in events.lines() {
        let row: serde_json::Value = serde_json::from_str(line).expect("a row");
        present.insert((
            row["aggregate_type"].as_str().expect("aggregate_type").to_owned(),
            row["event_type"].as_str().expect("event_type").to_owned(),
        ));
    }
    let mut missing = Vec::new();
    for (aggregate, variants) in vitni_db::event_variants() {
        for variant in variants {
            if !present.contains(&(aggregate.to_owned(), (*variant).to_owned())) {
                missing.push(format!("{aggregate}::{variant}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "the current backup fixture lacks {} event variant(s): {}; add them to `xtask/src/backup_fixture/` and \
         run `cargo xtask backup-fixture`",
        missing.len(),
        missing.join(", ")
    );
}
