//! End-to-end CLI tests driving the `genealogy` binary against a temp named workspace.
//!
//! `HOME`/`XDG_*` point at a temp dir so the global config bootstraps in isolation, and
//! `GENEALOGY_WORKSPACE` selects the workspace by name. This exercises the whole stack: arg parsing
//! → name resolution → app use-case → SQLite store → projection → rendered output.

#![expect(clippy::unwrap_used, reason = "tests abort on setup failure")]

use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

/// Builds a `genealogy` command isolated to `dir`, selecting the workspace named `gen`.
fn genealogy(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("genealogy").unwrap();
    cmd.env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("GENEALOGY_WORKSPACE", "gen")
        // Pin the locale so output is the English fallback regardless of the host locale.
        // `LANGUAGE` outranks `LC_ALL` in the locale negotiation, so clear it explicitly.
        .env_remove("LANGUAGE")
        .env_remove("LC_MESSAGES")
        .env("LC_ALL", "C")
        .env("LANG", "C");
    cmd
}

/// Initializes the `gen` workspace at `<dir>/ws`.
fn init(dir: &Path) {
    genealogy(dir)
        .arg("init")
        .arg("gen")
        .arg(dir.join("ws"))
        .assert()
        .success();
}

#[test]
fn init_builds_the_workspace_directory_tree() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    let ws = dir.path().join("ws");
    assert!(ws.join("workspace.toml").is_file(), "manifest");
    assert!(ws.join("exports").is_dir());
    assert!(ws.join("backups").is_dir());
    assert!(ws.join("media").is_dir());
}

#[test]
fn init_create_show_list_round_trip() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["person", "create", "--given", "Ada", "--surname", "Lovelace"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0001"));

    genealogy(dir.path())
        .args(["person", "show", "I0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("I0001").and(predicate::str::contains("Ada Lovelace")));

    genealogy(dir.path())
        .args(["person", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("I0001").and(predicate::str::contains("Ada Lovelace")));
}

#[test]
fn second_create_gets_the_next_id() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0001"));

    genealogy(dir.path())
        .args(["person", "create", "--given", "Alan", "--surname", "Turing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0002"));
}

#[test]
fn output_is_localized_to_the_requested_locale() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .env("LC_ALL", "nb_NO.UTF-8")
        .env("LANG", "nb_NO.UTF-8")
        .args(["person", "create", "--given", "Ada", "--surname", "Lovelace"])
        // `genealogy()` already clears LANGUAGE, so LC_ALL drives the negotiation here.
        .assert()
        .success()
        .stdout(predicate::str::contains("Opprettet I0001"));
}

#[test]
fn place_create_show_list_round_trip() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["place", "create", "--type", "parish", "--name", "Vågå"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created P0001"));

    genealogy(dir.path())
        .args(["place", "show", "P0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("P0001").and(predicate::str::contains("Vågå")));

    genealogy(dir.path())
        .args(["place", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("P0001").and(predicate::str::contains("parish")));
}

#[test]
fn source_create_show_list_round_trip() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["source", "create", "--title", "Folketelling 1801"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created S0001"));

    genealogy(dir.path())
        .args(["source", "show", "S0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("S0001").and(predicate::str::contains("Folketelling 1801")));

    genealogy(dir.path())
        .args(["source", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("S0001"));
}

#[test]
fn place_aggregate_ids_are_independent_of_persons() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    // Person and Place allocate from separate human-id sequences.
    genealogy(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0001"));
    genealogy(dir.path())
        .args(["place", "create", "--type", "farm"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created P0001"));
}

#[test]
fn show_of_an_unknown_person_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["person", "show", "I0404"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("I0404"));
}

#[test]
fn an_unknown_workspace_name_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .env("GENEALOGY_WORKSPACE", "nope")
        .args(["person", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("nope"));
}
