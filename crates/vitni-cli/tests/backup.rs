//! `vitni backup create` / `vitni backup restore` end to end (ADR 0041): a backup restores into a new
//! registered workspace holding the same records, and a refusal is reported without side effects.
//! `--replace --yes` restores over the open workspace instead (ADR 0044).

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

/// Adds Charles Babbage to `gen` after its backup, so a replace has something to discard.
fn diverged(dir: &Path) {
    backed_up(dir);
    vitni(dir, "gen")
        .args(["person", "create", "--given", "Charles", "--surname", "Babbage"])
        .assert()
        .success();
}

fn backups(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join("ws/backups"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_replace_without_yes_names_what_it_would_discard_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    diverged(dir.path());

    vitni(dir.path(), "gen")
        .args(["backup", "restore", "--replace"])
        .arg(dir.path().join("gen.vitni-backup"))
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Replacing workspace \"gen\" discards its 4 events")
                .and(predicate::str::contains("--yes"))
                .and(predicate::str::contains("backed up into its backups folder first")),
        );

    assert_eq!(backups(dir.path()), Vec::<String>::new());
    vitni(dir.path(), "gen")
        .args(["person", "show", "I0002"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Charles Babbage"));
}

#[test]
fn a_replace_restores_over_the_open_workspace_after_backing_it_up() {
    let dir = TempDir::new().unwrap();
    diverged(dir.path());

    vitni(dir.path(), "gen")
        .args(["backup", "restore", "--replace", "--yes"])
        .arg(dir.path().join("gen.vitni-backup"))
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Replaced workspace \"gen\" with 2 events")
                .and(predicate::str::contains("backed up first to"))
                .and(predicate::str::contains("gen-pre-restore-")),
        );

    vitni(dir.path(), "gen")
        .args(["person", "show", "I0002"])
        .assert()
        .failure();
    let saved = backups(dir.path());
    assert_eq!(saved.len(), 1, "{saved:?}");

    vitni(dir.path(), "gen")
        .args(["backup", "restore"])
        .arg(dir.path().join("ws/backups").join(&saved[0]))
        .arg("--new")
        .arg("before")
        .arg(dir.path().join("before"))
        .assert()
        .success();
    vitni(dir.path(), "before")
        .args(["person", "show", "I0002"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Charles Babbage"));
}

#[test]
fn restore_needs_exactly_one_of_new_and_replace() {
    let dir = TempDir::new().unwrap();
    backed_up(dir.path());
    let archive = dir.path().join("gen.vitni-backup");
    vitni(dir.path(), "gen")
        .args(["backup", "restore"])
        .arg(&archive)
        .assert()
        .failure();
    vitni(dir.path(), "gen")
        .args(["backup", "restore", "--replace", "--yes", "--new", "copy"])
        .arg(dir.path().join("copy"))
        .arg(&archive)
        .assert()
        .failure();
    assert!(!dir.path().join("copy").exists());
}
