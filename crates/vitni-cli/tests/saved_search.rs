//! End-to-end tests of `--saved` on the paged list verbs and the `saved-search` verb (ADR 0052 §8-10).
//!
//! Stored entries are written straight into `workspace.toml` where a test needs one the `save`
//! verb would refuse (a stale clause) or where the test is about `--saved` alone.

#![expect(clippy::unwrap_used, reason = "tests abort on setup failure")]

use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

/// Builds a `vitni-cli` command isolated to `dir`, selecting the workspace named `gen`.
fn vitni(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("vitni-cli").unwrap();
    cmd.env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("VITNI_WORKSPACE", "gen")
        .env_remove("LANGUAGE")
        .env_remove("VITNI_LANGUAGE")
        .env_remove("LC_MESSAGES")
        .env("LC_ALL", "C")
        .env("LANG", "C");
    cmd
}

fn manifest(dir: &Path) -> std::path::PathBuf {
    dir.join("ws").join("workspace.toml")
}

/// Initializes the `gen` workspace and creates Ola Olsen (I0001, b. 1790), Kari Olsen (I0002, b. 1810)
/// and Per Hansen (I0003, b. 1805).
fn workspace_with_people() -> TempDir {
    let dir = TempDir::new().unwrap();
    vitni(dir.path())
        .arg("init")
        .arg("gen")
        .arg(dir.path().join("ws"))
        .assert()
        .success();
    for (given, surname, born) in [
        ("Ola", "Olsen", "1790"),
        ("Kari", "Olsen", "1810-05"),
        ("Per", "Hansen", "1805"),
    ] {
        vitni(dir.path())
            .args([
                "person",
                "create",
                "--given",
                given,
                "--surname",
                surname,
                "--born",
                born,
            ])
            .assert()
            .success();
    }
    dir
}

/// Appends one `[[saved_searches]]` entry to `workspace.toml`; `query` is the body of its `query` table.
fn write_search(dir: &Path, n: u8, name: &str, kind: &str, query: &str) {
    let path = manifest(dir);
    let text = std::fs::read_to_string(&path).unwrap();
    let entry = format!(
        "\n[[saved_searches]]\nid = \"00000000-0000-7000-8000-0000000000{n:02}\"\nname = \"{name}\"\n\
         [saved_searches.query]\nkind = \"{kind}\"\n{query}\n"
    );
    std::fs::write(&path, format!("{text}{entry}")).unwrap();
}

fn clause(column: &str, op: &str, value_type: &str, value: &str) -> String {
    format!(
        "[[saved_searches.query.filters]]\ncolumn = \"{column}\"\nop = \"{op}\"\n\
         [saved_searches.query.filters.value]\ntype = \"{value_type}\"\nvalue = {value}\n"
    )
}

fn text_clause(column: &str, op: &str, value: &str) -> String {
    clause(column, op, "text", &format!("\"{value}\""))
}

fn sort(column: &str, direction: &str) -> String {
    format!("[saved_searches.query.sort]\ncolumn = \"{column}\"\ndirection = \"{direction}\"\n")
}

/// Ola and Kari Olsen, oldest first.
fn write_olsens(dir: &Path) {
    let query = format!(
        "{}{}",
        text_clause("surname", "contains", "ols"),
        sort("birth", "ascending")
    );
    write_search(dir, 1, "Olsens", "person", &query);
}

/// The record ids `person list` prints with `args`, in order.
fn listed(dir: &Path, args: &[&str]) -> Vec<String> {
    let assert = vitni(dir).args(["person", "list"]).args(args).assert().success();
    String::from_utf8(assert.get_output().stdout.clone())
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|id| id.chars().next().is_some_and(|first| first.is_ascii_uppercase()))
        .map(str::to_owned)
        .collect()
}

#[test]
fn saved_applies_the_stored_query_and_matches_the_name_in_any_case() {
    let dir = workspace_with_people();
    write_olsens(dir.path());

    assert_eq!(listed(dir.path(), &["--saved", "Olsens"]), ["I0001", "I0002"]);
    assert_eq!(listed(dir.path(), &["--saved", "oLSENS"]), ["I0001", "I0002"]);
}

#[test]
fn filter_beside_saved_appends_for_that_run_only() {
    let dir = workspace_with_people();
    write_olsens(dir.path());
    let before = std::fs::read_to_string(manifest(dir.path())).unwrap();

    assert_eq!(
        listed(dir.path(), &["--saved", "Olsens", "--filter", "birth:before:1806"]),
        ["I0001"]
    );

    assert_eq!(std::fs::read_to_string(manifest(dir.path())).unwrap(), before);
    assert_eq!(listed(dir.path(), &["--saved", "Olsens"]), ["I0001", "I0002"]);
}

#[test]
fn search_beside_saved_replaces_the_stored_search_for_that_run_only() {
    let dir = workspace_with_people();
    write_search(dir.path(), 1, "Pers", "person", "search = \"per\"");
    let before = std::fs::read_to_string(manifest(dir.path())).unwrap();

    assert_eq!(listed(dir.path(), &["--saved", "Pers"]), ["I0003"]);
    assert_eq!(listed(dir.path(), &["--saved", "Pers", "--search", "ola"]), ["I0001"]);

    assert_eq!(std::fs::read_to_string(manifest(dir.path())).unwrap(), before);
    assert_eq!(listed(dir.path(), &["--saved", "Pers"]), ["I0003"]);
}

#[test]
fn sort_beside_saved_replaces_the_stored_sort_for_that_run_only() {
    let dir = workspace_with_people();
    write_olsens(dir.path());
    let before = std::fs::read_to_string(manifest(dir.path())).unwrap();

    assert_eq!(
        listed(dir.path(), &["--saved", "Olsens", "--sort", "birth:desc"]),
        ["I0002", "I0001"]
    );

    assert_eq!(std::fs::read_to_string(manifest(dir.path())).unwrap(), before);
    assert_eq!(listed(dir.path(), &["--saved", "Olsens"]), ["I0001", "I0002"]);
}

#[test]
fn an_unknown_saved_name_fails_naming_it() {
    let dir = workspace_with_people();

    vitni(dir.path())
        .args(["person", "list", "--saved", "Nobody"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Nobody").and(predicate::str::contains("with id").not()));
}

#[test]
fn saved_on_another_kinds_verb_fails_naming_both_kinds() {
    let dir = workspace_with_people();
    write_olsens(dir.path());

    vitni(dir.path())
        .args(["place", "list", "--saved", "Olsens"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("person").and(predicate::str::contains("place")));
}

#[test]
fn a_stale_clause_fails_naming_the_saved_search_and_the_clause() {
    for (column, op) in [("zodiac", "is"), ("surname", "resembles"), ("birth", "contains")] {
        let dir = workspace_with_people();
        let query = format!(
            "{}{}",
            text_clause("surname", "contains", "ols"),
            text_clause(column, op, "x")
        );
        write_search(dir.path(), 1, "Drifted", "person", &query);

        vitni(dir.path())
            .args(["person", "list", "--saved", "Drifted"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Drifted").and(predicate::str::contains(format!("(`{column}:{op}`)"))));
    }
}

#[test]
fn a_stale_clause_naming_neither_column_nor_operator_says_so() {
    let dir = workspace_with_people();
    let query =
        "[[saved_searches.query.filters]]\n[saved_searches.query.filters.value]\ntype = \"text\"\nvalue = \"x\"\n";
    write_search(dir.path(), 1, "Bare", "person", query);

    vitni(dir.path())
        .args(["person", "list", "--saved", "Bare"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("clause 1: names no column or operator").and(predicate::str::contains("``").not()),
        );
}

#[test]
fn a_stale_clause_missing_its_column_or_operator_says_which_without_an_empty_name() {
    let value = "[saved_searches.query.filters.value]\ntype = \"text\"\nvalue = \"x\"\n";
    let cases = [
        (
            "NoColumn",
            "op = \"contains\"\n",
            "clause 1 (`contains`): names no column",
        ),
        (
            "NoOp",
            "column = \"surname\"\n",
            "clause 1 (`surname`): names no operator",
        ),
        (
            "EmptyColumn",
            "column = \"\"\nop = \"contains\"\n",
            "clause 1 (`contains`): names no column",
        ),
        (
            "EmptyOp",
            "column = \"surname\"\nop = \"\"\n",
            "clause 1 (`surname`): names no operator",
        ),
    ];
    for (name, keys, expected) in cases {
        let dir = workspace_with_people();
        let query = format!("[[saved_searches.query.filters]]\n{keys}{value}");
        write_search(dir.path(), 1, name, "person", &query);

        vitni(dir.path())
            .args(["person", "list", "--saved", name])
            .assert()
            .failure()
            .stderr(predicate::str::contains(expected).and(predicate::str::contains("``").not()));
    }
}

#[test]
fn an_empty_result_under_saved_prints_the_no_match_message() {
    let dir = workspace_with_people();
    write_search(
        dir.path(),
        1,
        "Nobody",
        "person",
        &text_clause("surname", "is", "nobody"),
    );

    vitni(dir.path())
        .args(["person", "list", "--saved", "Nobody"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing matches the query.").and(predicate::str::contains("yet").not()));
}

#[test]
fn saved_search_list_shows_each_search_with_its_kind_and_marks_unreadable_entries() {
    let dir = workspace_with_people();
    write_olsens(dir.path());
    write_search(dir.path(), 2, "Fana", "place", "");
    let path = manifest(dir.path());
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!("{text}\n[[saved_searches]]\nname = \"Broken\"\nid = 42\n[saved_searches.query]\nkind = \"galaxy\"\n"),
    )
    .unwrap();

    vitni(dir.path())
        .args(["saved-search", "list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Olsens")
                .and(predicate::str::contains("person"))
                .and(predicate::str::contains("Fana"))
                .and(predicate::str::contains("place"))
                .and(predicate::str::contains("saved_searches[2]")),
        );
}

#[test]
fn saved_search_save_stores_a_query_that_saved_then_applies() {
    let dir = workspace_with_people();

    vitni(dir.path())
        .args([
            "saved-search",
            "save",
            "Olsens",
            "--kind",
            "person",
            "--filter",
            "surname:contains:ols",
            "--sort",
            "birth:desc",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Olsens"));

    assert_eq!(listed(dir.path(), &["--saved", "olsens"]), ["I0002", "I0001"]);
}

#[test]
fn saved_search_save_refuses_a_duplicate_name_ignoring_case() {
    let dir = workspace_with_people();
    vitni(dir.path())
        .args(["saved-search", "save", "Olsens", "--kind", "person"])
        .assert()
        .success();

    vitni(dir.path())
        .args(["saved-search", "save", "OLSENS", "--kind", "person"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn saved_search_delete_removes_the_entry_and_an_unknown_name_fails() {
    let dir = workspace_with_people();
    write_olsens(dir.path());

    vitni(dir.path())
        .args(["saved-search", "delete", "olsens"])
        .assert()
        .success();
    vitni(dir.path())
        .args(["person", "list", "--saved", "Olsens"])
        .assert()
        .failure();

    vitni(dir.path())
        .args(["saved-search", "delete", "Olsens"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Olsens"));
}

#[test]
fn saved_search_list_prints_a_nameless_unreadable_entry_without_a_dangling_separator() {
    let dir = workspace_with_people();
    write_olsens(dir.path());
    let path = manifest(dir.path());
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!("{text}\n[[saved_searches]]\nid = 42\n[saved_searches.query]\nkind = \"galaxy\"\n"),
    )
    .unwrap();

    vitni(dir.path())
        .args(["saved-search", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("saved_searches[1]").and(predicate::str::contains("[1] :").not()));
}
