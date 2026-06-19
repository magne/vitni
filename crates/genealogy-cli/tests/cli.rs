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
fn citation_against_a_source_round_trips() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["source", "create", "--title", "Folketelling 1801"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created S0001"));

    genealogy(dir.path())
        .args(["citation", "create", "--source", "S0001", "--page", "p. 42"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created C0001"));

    genealogy(dir.path())
        .args(["citation", "show", "C0001"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("C0001")
                .and(predicate::str::contains("S0001"))
                .and(predicate::str::contains("p. 42")),
        );
}

#[test]
fn citation_against_an_unknown_source_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["citation", "create", "--source", "S9999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("S9999"));
}

#[test]
fn a_name_can_be_backed_by_a_citation() {
    // The full evidence chain: source <- citation <- a person's name assertion.
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["source", "create", "--title", "Parish register"])
        .assert()
        .success();
    genealogy(dir.path())
        .args(["citation", "create", "--source", "S0001", "--page", "fol. 3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created C0001"));
    genealogy(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success();
    genealogy(dir.path())
        .args([
            "person",
            "add-name",
            "I0001",
            "--surname",
            "Lovelace",
            "--citation",
            "C0001",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated I0001"));
}

#[test]
fn a_name_citing_an_unknown_citation_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success();
    genealogy(dir.path())
        .args(["person", "add-name", "I0001", "--surname", "X", "--citation", "C9999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("C9999"));
}

#[test]
fn event_create_link_place_and_participation_round_trip() {
    // The full cross-aggregate slice: an event, dated, linked to a place, with a participant.
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["place", "create", "--type", "parish", "--name", "Vågå"])
        .assert()
        .success();
    genealogy(dir.path())
        .args(["event", "create", "--type", "birth"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created E0001"));
    genealogy(dir.path())
        .args([
            "event",
            "assert-date",
            "E0001",
            "--year",
            "1847",
            "--month",
            "3",
            "--day",
            "12",
        ])
        .assert()
        .success();
    genealogy(dir.path())
        .args(["event", "link-place", "E0001", "P0001"])
        .assert()
        .success();

    genealogy(dir.path())
        .args(["event", "show", "E0001"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("E0001")
                .and(predicate::str::contains("birth"))
                .and(predicate::str::contains("1847-03-12"))
                .and(predicate::str::contains("P0001")),
        );

    // A person participates in the event.
    genealogy(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success();
    genealogy(dir.path())
        .args([
            "person",
            "add-participation",
            "I0001",
            "--event",
            "E0001",
            "--role",
            "primary",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated I0001"));
}

#[test]
fn event_linking_an_unknown_place_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["event", "create", "--type", "marriage"])
        .assert()
        .success();
    genealogy(dir.path())
        .args(["event", "link-place", "E0001", "P9999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("P9999"));
}

#[test]
fn participation_in_an_unknown_event_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    genealogy(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success();
    genealogy(dir.path())
        .args([
            "person",
            "add-participation",
            "I0001",
            "--event",
            "E9999",
            "--role",
            "witness",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E9999"));
}

#[test]
fn event_date_renders_localized_month_is_deferred_but_digits_show() {
    // PR-scope: date rendering is the plain YYYY-MM-DD form (localized formatting lands later).
    let dir = TempDir::new().unwrap();
    init(dir.path());
    genealogy(dir.path())
        .args(["event", "create", "--type", "census"])
        .assert()
        .success();
    genealogy(dir.path())
        .args(["event", "assert-date", "E0001", "--year", "1801"])
        .assert()
        .success();
    genealogy(dir.path())
        .args(["event", "show", "E0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1801"));
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
