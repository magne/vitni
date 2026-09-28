//! `vitni backup create` / `vitni backup restore` end to end (ADR 0041): a backup restores into a new
//! registered workspace holding the same records, and a refusal is reported without side effects.

#![expect(clippy::unwrap_used, reason = "tests abort on setup failure")]

use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

/// Builds a `vitni` command isolated to `dir`, in English, selecting the workspace `workspace`.
fn vitni(dir: &Path, workspace: &str) -> Command {
    let mut cmd = Command::cargo_bin("vitni-cli").unwrap();
    cmd.env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("VITNI_WORKSPACE", workspace)
        .env_remove("LANGUAGE")
        .env_remove("VITNI_LANGUAGE")
        .env_remove("LC_MESSAGES")
        .env("LC_ALL", "C")
        .env("LANG", "C");
    cmd
}

/// A `gen` workspace at `<dir>/ws` holding Ada Lovelace, backed up to `<dir>/gen.vitni-backup`.
fn backed_up(dir: &Path) {
    vitni(dir, "gen")
        .arg("init")
        .arg("gen")
        .arg(dir.join("ws"))
        .assert()
        .success();
    vitni(dir, "gen")
        .args(["person", "create", "--given", "Ada", "--surname", "Lovelace"])
        .assert()
        .success();
    vitni(dir, "gen")
        .args(["backup", "create"])
        .arg(dir.join("gen.vitni-backup"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Backed up 2 events to").and(predicate::str::contains("gen.vitni-backup")));
}

#[test]
fn a_backup_restores_into_a_new_workspace_with_the_same_records() {
    let dir = TempDir::new().unwrap();
    backed_up(dir.path());

    vitni(dir.path(), "gen")
        .args(["backup", "restore"])
        .arg(dir.path().join("gen.vitni-backup"))
        .arg("--new")
        .arg("copy")
        .arg(dir.path().join("copy"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Restored 2 events into workspace \"copy\""));

    vitni(dir.path(), "copy")
        .args(["person", "show", "I0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Ada Lovelace"));
}

#[test]
fn a_backup_never_overwrites_a_file() {
    let dir = TempDir::new().unwrap();
    backed_up(dir.path());
    vitni(dir.path(), "gen")
        .args(["backup", "create"])
        .arg(dir.path().join("gen.vitni-backup"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn a_file_that_is_not_a_backup_is_refused() {
    let dir = TempDir::new().unwrap();
    let bogus = dir.path().join("notes.txt");
    std::fs::write(&bogus, "hello").unwrap();
    vitni(dir.path(), "gen")
        .args(["backup", "restore"])
        .arg(&bogus)
        .arg("--new")
        .arg("copy")
        .arg(dir.path().join("copy"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a vitni backup"));
    assert!(!dir.path().join("copy").exists());
}

#[test]
fn missing_media_is_reported_after_a_restore() {
    let dir = TempDir::new().unwrap();
    vitni(dir.path(), "gen")
        .arg("init")
        .arg("gen")
        .arg(dir.path().join("ws"))
        .assert()
        .success();
    std::fs::write(dir.path().join("ws/media/scan.png"), b"scan").unwrap();
    vitni(dir.path(), "gen")
        .args(["media", "create", "--path", "media/scan.png"])
        .assert()
        .success();
    vitni(dir.path(), "gen")
        .args(["backup", "create"])
        .arg(dir.path().join("gen.vitni-backup"))
        .assert()
        .success();
    vitni(dir.path(), "gen")
        .args(["backup", "restore"])
        .arg(dir.path().join("gen.vitni-backup"))
        .arg("--new")
        .arg("copy")
        .arg(dir.path().join("copy"))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Media file missing after the restore: media/scan.png",
        ));
}
