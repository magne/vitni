//! End-to-end CLI tests driving the `vitni-cli` binary against a temp named workspace.
//!
//! `HOME`/`XDG_*` point at a temp dir so the global config bootstraps in isolation, and
//! `VITNI_WORKSPACE` selects the workspace by name. This exercises the whole stack: arg parsing
//! → name resolution → app use-case → SQLite store → projection → rendered output.

#![expect(clippy::unwrap_used, reason = "tests abort on setup failure")]

use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;
use vitni_app::{LocaleOverrides, save_locale_overrides};

/// Builds a `vitni` command isolated to `dir`, selecting the workspace named `gen`.
fn vitni(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("vitni-cli").unwrap();
    cmd.env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("VITNI_WORKSPACE", "gen")
        // Pin the locale so output is the English fallback regardless of the host locale.
        // `LANGUAGE` outranks `LC_ALL` in the locale negotiation, so clear it explicitly; clear the
        // app-scoped `VITNI_LANGUAGE` override too (ADR 0015) so a dev machine can't regress the
        // English-pinned assertions.
        .env_remove("LANGUAGE")
        .env_remove("VITNI_LANGUAGE")
        .env_remove("LC_MESSAGES")
        .env("LC_ALL", "C")
        .env("LANG", "C");
    cmd
}

/// Like [`vitni`], but leaves `LANGUAGE` intact so the two env-precedence tests below can assert
/// that a bare `LANGUAGE` is (and is not) outranked. `VITNI_LANGUAGE` is still cleared so a dev
/// machine that sets it can't regress the assertions; the test that needs it re-sets it explicitly.
fn vitni_env_language(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("vitni-cli").unwrap();
    cmd.env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("VITNI_WORKSPACE", "gen")
        .env_remove("VITNI_LANGUAGE")
        .env_remove("LC_MESSAGES")
        .env("LC_ALL", "C")
        .env("LANG", "C");
    cmd
}

/// Configures the `gen` workspace's UI language to Norwegian (ADR 0015 §4 fixture).
fn set_ui_language_norwegian(dir: &Path) {
    save_locale_overrides(
        &dir.join("ws"),
        LocaleOverrides {
            ui_language: Some("no".parse().unwrap()),
            ..Default::default()
        },
    )
    .unwrap();
}

/// Initializes the `gen` workspace at `<dir>/ws`.
fn init(dir: &Path) {
    vitni(dir).arg("init").arg("gen").arg(dir.join("ws")).assert().success();
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

    vitni(dir.path())
        .args(["person", "create", "--given", "Ada", "--surname", "Lovelace"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0001"));

    vitni(dir.path())
        .args(["person", "show", "I0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("I0001").and(predicate::str::contains("Ada Lovelace")));

    vitni(dir.path())
        .args(["person", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("I0001").and(predicate::str::contains("Ada Lovelace")));
}

#[test]
fn person_create_born_records_a_dated_birth_the_person_is_primary_in() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["person", "create", "--given", "Ada", "--born", "1852-03"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0001"));

    vitni(dir.path())
        .args(["event", "show", "E0001"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("type: birth")
                .and(predicate::str::contains("date: March 1852"))
                .and(predicate::str::contains("participants: 1")),
        );
}

#[test]
fn person_create_rejects_a_birth_date_it_cannot_read() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["person", "create", "--given", "Ada", "--born", "spring"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--born"));
    vitni(dir.path())
        .args(["person", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Ada").not());
}

/// A create that looks like a stored record says so on stderr, and still creates it (ADR 0038 §8).
#[test]
fn creating_a_likely_duplicate_hints_at_the_stored_record_on_stderr() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args([
            "person",
            "create",
            "--given",
            "Guldbrand",
            "--surname",
            "Olsen",
            "--born",
            "1852",
        ])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
    vitni(dir.path())
        .args([
            "person",
            "create",
            "--given",
            "Kari",
            "--surname",
            "Hansen",
            "--born",
            "1900",
        ])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
    vitni(dir.path())
        .args([
            "person",
            "create",
            "--given",
            "Gulbrand",
            "--surname",
            "Olsøn",
            "--born",
            "1852",
        ])
        .assert()
        .success()
        .stdout(predicate::eq("Created I0003\n"))
        .stderr(
            predicate::str::contains("Possibly the same as I0001 (")
                .and(predicate::str::contains("vitni match show person I0003 I0001"))
                .and(predicate::str::contains("I0002").not()),
        );

    vitni(dir.path())
        .args(["place", "create", "--type", "farm", "--name", "Nordaas"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
    vitni(dir.path())
        .args(["place", "create", "--type", "farm", "--name", "Nordås"])
        .assert()
        .success()
        .stderr(predicate::str::contains("vitni match show place P0002 P0001"));
}

#[test]
fn second_create_gets_the_next_id() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0001"));

    vitni(dir.path())
        .args(["person", "create", "--given", "Alan", "--surname", "Turing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0002"));
}

#[test]
fn output_is_localized_to_the_requested_locale() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .env("LC_ALL", "nb_NO.UTF-8")
        .env("LANG", "nb_NO.UTF-8")
        .args(["person", "create", "--given", "Ada", "--surname", "Lovelace"])
        // `vitni()` already clears LANGUAGE, so LC_ALL drives the negotiation here.
        .assert()
        .success()
        .stdout(predicate::str::contains("Opprettet I0001"));
}

#[test]
fn place_create_show_list_round_trip() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["place", "create", "--type", "parish", "--name", "Vågå"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created P0001"));

    vitni(dir.path())
        .args(["place", "show", "P0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("P0001").and(predicate::str::contains("Vågå")));

    vitni(dir.path())
        .args(["place", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("P0001").and(predicate::str::contains("parish")));
}

#[test]
fn source_create_show_list_round_trip() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["source", "create", "--title", "Folketelling 1801"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created S0001"));

    vitni(dir.path())
        .args(["source", "show", "S0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("S0001").and(predicate::str::contains("Folketelling 1801")));

    vitni(dir.path())
        .args(["source", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("S0001"));
}

#[test]
fn citation_against_a_source_round_trips() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["source", "create", "--title", "Folketelling 1801"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created S0001"));

    vitni(dir.path())
        .args(["citation", "create", "--source", "S0001", "--page", "p. 42"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created C0001"));

    vitni(dir.path())
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

    vitni(dir.path())
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

    vitni(dir.path())
        .args(["source", "create", "--title", "Parish register"])
        .assert()
        .success();
    vitni(dir.path())
        .args(["citation", "create", "--source", "S0001", "--page", "fol. 3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created C0001"));
    vitni(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success();
    vitni(dir.path())
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

    vitni(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success();
    vitni(dir.path())
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

    vitni(dir.path())
        .args(["place", "create", "--type", "parish", "--name", "Vågå"])
        .assert()
        .success();
    vitni(dir.path())
        .args(["event", "create", "--type", "birth"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created E0001"));
    vitni(dir.path())
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
    vitni(dir.path())
        .args(["event", "link-place", "E0001", "P0001"])
        .assert()
        .success();

    vitni(dir.path())
        .args(["event", "show", "E0001"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("E0001")
                .and(predicate::str::contains("birth"))
                // The date is rendered by ICU4X for the locale (en: "March 12, 1847").
                .and(predicate::str::contains("March 12, 1847"))
                .and(predicate::str::contains("P0001")),
        );

    // A person participates in the event, carrying an age, an attribute, and a note (ADR 0019).
    vitni(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success();
    vitni(dir.path())
        .args(["note", "create", "--text", "a witness note"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created N0001"));
    vitni(dir.path())
        .args([
            "person",
            "add-participation",
            "I0001",
            "--event",
            "E0001",
            "--role",
            "primary",
            "--age-years",
            "25",
            "--age-months",
            "3",
            "--attribute",
            "occupation=farmer",
            "--note",
            "N0001",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated I0001"));
}

#[test]
fn event_linking_an_unknown_place_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["event", "create", "--type", "marriage"])
        .assert()
        .success();
    vitni(dir.path())
        .args(["event", "link-place", "E0001", "P9999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("P9999"));
}

#[test]
fn participation_in_an_unknown_event_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success();
    vitni(dir.path())
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
fn event_date_is_localized_by_icu_in_each_locale() {
    // A full date renders with ICU4X's locale-specific month name and order:
    // en "March 12, 1847" vs nb "12. mars 1847".
    let dir = TempDir::new().unwrap();
    init(dir.path());
    vitni(dir.path())
        .args(["event", "create", "--type", "baptism"])
        .assert()
        .success();
    vitni(dir.path())
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

    vitni(dir.path())
        .args(["event", "show", "E0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("March 12, 1847"));

    vitni(dir.path())
        .env("LC_ALL", "nb_NO.UTF-8")
        .env("LANG", "nb_NO.UTF-8")
        .args(["event", "show", "E0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("12. mars 1847"));
}

#[test]
fn place_aggregate_ids_are_independent_of_persons() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    // Person and Place allocate from separate human-id sequences.
    vitni(dir.path())
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0001"));
    vitni(dir.path())
        .args(["place", "create", "--type", "farm"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created P0001"));
}

/// The succession write path (#196, ADR 0026 §3): a many→one merge names the survivor with `--to` and
/// the other ceasing places with `--from`; the positional `HUMAN_ID` is the anchor and is added to the
/// ceasing set by the command, so the operator never has to repeat it.
#[test]
fn place_assert_succession_merges_two_municipalities_into_one() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    for name in ["Aker", "Kristiania", "Oslo"] {
        vitni(dir.path())
            .args(["place", "create", "--type", "municipality", "--name", name])
            .assert()
            .success();
    }

    vitni(dir.path())
        .args([
            "place",
            "assert-succession",
            "P0001",
            "--to",
            "P0003",
            "--from",
            "P0002",
            "--kind",
            "merged",
            "--year",
            "1948",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated P0001"));
}

/// A split names many resulting places through a repeated `--to`, and needs no `--from` at all.
#[test]
fn place_assert_succession_splits_one_county_into_two() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    for name in ["Old County", "North County", "South County"] {
        vitni(dir.path())
            .args(["place", "create", "--type", "county", "--name", name])
            .assert()
            .success();
    }

    vitni(dir.path())
        .args([
            "place",
            "assert-succession",
            "P0001",
            "--to",
            "P0002",
            "--to",
            "P0003",
            "--kind",
            "split",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated P0001"));
}

#[test]
fn place_assert_succession_to_an_unknown_place_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["place", "create", "--type", "parish", "--name", "Vågå"])
        .assert()
        .success();

    vitni(dir.path())
        .args([
            "place",
            "assert-succession",
            "P0001",
            "--to",
            "P9999",
            "--kind",
            "renamed",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("P9999"));
}

#[test]
fn show_of_an_unknown_person_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["person", "show", "I0404"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("I0404"));
}

#[test]
fn configured_ui_language_outranks_plain_language_env() {
    // The bug fix (ADR 0015 §4): a configured `ui_language` beats a bare `LANGUAGE` in the env.
    let dir = TempDir::new().unwrap();
    init(dir.path());
    set_ui_language_norwegian(dir.path());

    vitni_env_language(dir.path())
        .env("LANGUAGE", "en")
        .args(["person", "create", "--given", "Ada", "--surname", "Lovelace"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Opprettet I0001"));
}

#[test]
fn vitni_language_env_outranks_configured_ui_language() {
    // `VITNI_LANGUAGE` is the explicit, app-scoped override; it beats the configured Norwegian.
    let dir = TempDir::new().unwrap();
    init(dir.path());
    set_ui_language_norwegian(dir.path());

    vitni_env_language(dir.path())
        .env("VITNI_LANGUAGE", "en")
        .args(["person", "create", "--given", "Ada"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created I0001"));
}

#[test]
fn an_unknown_workspace_name_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .env("VITNI_WORKSPACE", "nope")
        .args(["person", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("nope"));
}

#[test]
fn plugin_trust_add_list_remove_round_trips_through_config() {
    // The client-scope pinned-publisher store (ADR 0014 §3): add, see it listed with a short
    // fingerprint (never the raw 64-hex key), then remove it.
    let dir = TempDir::new().unwrap();
    init(dir.path());
    let key = "a".repeat(64);

    vitni(dir.path())
        .args(["plugin", "trust", "add", "acme", &key])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pinned publisher acme"));
    vitni(dir.path())
        .args(["plugin", "trust", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("acme").and(predicate::str::contains("aaaaaaaaaaaaaaaa")));
    vitni(dir.path())
        .args(["plugin", "trust", "remove", "acme"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Unpinned publisher acme"));
    vitni(dir.path())
        .args(["plugin", "trust", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No publisher is pinned"));
}

#[test]
fn plugin_list_finds_the_embedded_fleet_outside_the_source_tree() {
    // The embedded layer (ADR 0014 §4) resolves independently of the working directory: run from a
    // temp dir with no override, the dev fallback still finds the built fleet (`cargo xtask
    // build-plugins`, which CI runs before tests).
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .current_dir(dir.path())
        .env_remove("VITNI_PLUGIN_DIR")
        .args(["plugin", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gedcom-import"));
}

#[test]
fn plugin_trust_add_rejects_a_malformed_key() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["plugin", "trust", "add", "acme", "not-a-key"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("acme"));
}

#[test]
fn plugin_grant_then_revoke_round_trips_through_the_manifest() {
    // Grant/revoke edit the workspace manifest's approved-capability set (ADR 0014 §5); no plugin
    // discovery is needed, so this exercises the mutation path without built bundles.
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["plugin", "grant", "gedcom-import", "query", "commands"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Saved grants for gedcom-import").and(predicate::str::contains("commands")));
    vitni(dir.path())
        .args(["plugin", "revoke", "gedcom-import", "commands"])
        .assert()
        .success()
        .stdout(predicate::str::contains("query").and(predicate::str::contains("commands").not()));
}

const TREE: &str = "\
0 HEAD
1 SOUR test
1 FILE tree.ged
0 @I1@ INDI
1 NAME John /Smith/
0 TRLR
";

/// Runs `vitni import gedcom-import <file> --into gen --yes` plus `extra`.
fn import_tree(dir: &Path, extra: &[&str]) -> assert_cmd::assert::Assert {
    let file = dir.join("tree.ged");
    std::fs::write(&file, TREE).unwrap();
    vitni(dir)
        .args(["import", "gedcom-import"])
        .arg(&file)
        .args(["--into", "gen", "--yes"])
        .args(extra)
        .assert()
}

/// Runs `vitni import gedcom-import <file> --into gen` with `text` as the file, plus `extra`.
fn import_text(dir: &Path, text: &str, extra: &[&str]) -> assert_cmd::assert::Assert {
    let file = dir.join("tree.ged");
    std::fs::write(&file, text).unwrap();
    vitni(dir)
        .args(["import", "gedcom-import"])
        .arg(&file)
        .args(["--into", "gen"])
        .args(extra)
        .assert()
}

#[test]
fn a_re_export_naming_no_dataset_is_imported_into_the_one_proposed_when_confirmed() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    import_tree(dir.path(), &[]).success();

    // `y` confirms the import into a non-empty workspace; the closed stdin then declines the proposal.
    let mut command = vitni(dir.path());
    command
        .args(["import", "gedcom-import"])
        .arg(dir.path().join("tree.ged"))
        .args(["--into", "gen"]);
    assert_cmd::Command::from_std(command)
        .write_stdin("y\n")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("looks like a later export of \"tree.ged\": 1 of its 1")
                .and(predicate::str::contains("--dataset"))
                .and(predicate::str::contains("--new-dataset")),
        );

    import_tree(dir.path(), &[])
        .success()
        .stderr(predicate::str::contains("Importing into \"tree.ged\""));
    let datasets = vitni(dir.path()).args(["import-run", "datasets"]).assert().success();
    let stdout = String::from_utf8(datasets.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
}

#[test]
fn a_file_proposed_no_dataset_must_name_one_even_with_yes() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    import_tree(dir.path(), &[]).success();
    let unrelated = TREE.replace("1 FILE tree.ged", "1 FILE other.ged");

    import_text(dir.path(), &unrelated, &["--yes"]).failure().stderr(
        predicate::str::contains("--dataset")
            .and(predicate::str::contains("--new-dataset"))
            .and(predicate::str::contains("tree.ged")),
    );
    // The same file into its own dataset is already on record: it writes nothing, not even a run.
    import_tree(dir.path(), &["--dataset", "tree.ged"]).success();
    import_text(dir.path(), &unrelated, &["--yes", "--new-dataset"]).success();

    let datasets = vitni(dir.path()).args(["import-run", "datasets"]).assert().success();
    let stdout = String::from_utf8(datasets.get_output().stdout.clone()).unwrap();
    assert_eq!(
        stdout.lines().filter(|line| line.contains("tree.ged  1 run")).count(),
        2,
        "{stdout}"
    );
}

#[test]
fn dataset_and_new_dataset_are_mutually_exclusive() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    import_tree(dir.path(), &["--dataset", "tree.ged", "--new-dataset"]).failure();
}

#[test]
fn a_new_workspace_is_not_created_for_an_import_naming_a_dataset() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("tree.ged");
    std::fs::write(&file, TREE).unwrap();
    let target = dir.path().join("fresh");
    vitni(dir.path())
        .args(["import", "gedcom-import"])
        .arg(&file)
        .arg("--new")
        .arg("fresh")
        .arg(&target)
        .args(["--dataset", "tree.ged"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--dataset"));
    assert!(!target.exists(), "nothing is created when the dataset cannot exist");
}

/// The stdout of `assert`.
fn stdout(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}

/// TREE declaring `export` as its export date and `birth` as John's birth date.
fn dated_tree(export: &str, birth: &str) -> String {
    TREE.replace("1 SOUR test\n", &format!("1 SOUR test\n1 DATE {export}\n"))
        .replace(
            "1 NAME John /Smith/\n",
            &format!("1 NAME John /Smith/\n1 BIRT\n2 DATE {birth}\n"),
        )
}

#[test]
fn a_re_import_that_writes_nothing_says_its_export_date_could_not_be_read() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    let first = stdout(&import_text(dir.path(), &dated_tree("1 JAN 2000", "5 APR 1970"), &["--yes"]).success());
    assert!(!first.contains("cannot be read"), "{first}");

    let again = stdout(
        &import_text(
            dir.path(),
            &dated_tree("30 FEB 2100", "6 APR 1970"),
            &["--yes", "--dataset", "tree.ged"],
        )
        .success(),
    );
    assert!(
        again.contains(
            "tree.ged declares an export date that cannot be read (30 FEB 2100), so this import only added to records and replaced no value."
        ),
        "{again}"
    );
    let runs = stdout(&vitni(dir.path()).args(["import-run", "list"]).assert().success());
    assert_eq!(
        runs.lines().count(),
        1,
        "the re-import wrote nothing, so it left no run: {runs}"
    );
}

#[test]
fn a_plan_of_a_re_import_reports_every_record_unchanged_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    import_tree(dir.path(), &[]).success();

    let plan = import_tree(dir.path(), &["--plan"]).success();
    let text = stdout(&plan);
    assert!(text.contains("Plan for tree.ged:"), "{text}");
    assert!(text.contains("persons: 1 unchanged"), "{text}");
    assert!(
        text.contains("Every record is already on record: importing this file writes nothing."),
        "{text}"
    );
    assert!(!text.contains("Imported"), "{text}");

    let json = stdout(&import_tree(dir.path(), &["--plan", "--json"]).success());
    let plan: serde_json::Value = serde_json::from_str(json.trim()).unwrap();
    let kinds = plan["kinds"].as_array().unwrap();
    assert!(!kinds.is_empty(), "{json}");
    for kind in kinds {
        for count in ["new", "updated", "linked", "candidates"] {
            assert_eq!(kind[count], 0, "{count} in {kind}");
        }
        assert!(kind["unchanged"].as_u64().unwrap() > 0, "{kind}");
    }

    let runs = stdout(&vitni(dir.path()).args(["import-run", "list"]).assert().success());
    assert_eq!(runs.lines().count(), 1, "a plan records no run: {runs}");
}

#[test]
fn a_plan_of_a_re_import_names_what_it_changes_on_each_record() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    import_tree(dir.path(), &[]).success();
    let farmer = TREE.replace("1 NAME John /Smith/\n", "1 NAME John /Smith/\n1 OCCU Farmer\n");

    let text = stdout(&import_text(dir.path(), &farmer, &["--yes", "--dataset", "tree.ged", "--plan"]).success());
    assert!(text.contains("persons: 1 updated"), "{text}");
    assert!(text.contains("John Smith (I0001): updates occupation"), "{text}");

    let json = stdout(
        &import_text(
            dir.path(),
            &farmer,
            &["--yes", "--dataset", "tree.ged", "--plan", "--json"],
        )
        .success(),
    );
    let plan: serde_json::Value = serde_json::from_str(json.trim()).unwrap();
    assert_eq!(plan["records"][0]["change"], "updates", "{json}");
    assert_eq!(
        plan["records"][0]["fields"][0], "person.FactAsserted.Occupation",
        "{json}"
    );
}

#[test]
fn a_plan_of_a_first_import_shows_its_records_as_new_and_writes_none_of_them() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    let text = stdout(&import_tree(dir.path(), &["--plan"]).success());
    assert!(text.contains("persons: 1 new"), "{text}");
    vitni(dir.path())
        .args(["person", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Smith").not());
}

#[test]
fn json_needs_plan_and_plan_needs_an_existing_workspace() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    import_tree(dir.path(), &["--json"]).failure();
    import_tree(dir.path(), &["--plan", "--defer-matches"]).failure();

    let file = dir.path().join("tree.ged");
    let target = dir.path().join("fresh");
    vitni(dir.path())
        .args(["import", "gedcom-import"])
        .arg(&file)
        .arg("--new")
        .arg("fresh")
        .arg(&target)
        .arg("--plan")
        .assert()
        .failure();
    assert!(!target.exists(), "a plan creates no workspace");
}

#[test]
fn deferring_the_matches_imports_a_second_tree_of_the_same_people_as_new() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    import_tree(dir.path(), &[]).success();
    let other = TREE.replace("1 FILE tree.ged", "1 FILE other.ged");
    import_text(dir.path(), &other, &["--yes", "--new-dataset", "--defer-matches"])
        .success()
        .stdout(predicate::str::contains("Imported 1 record(s)"));
    let persons = stdout(&vitni(dir.path()).args(["person", "list"]).assert().success());
    assert_eq!(persons.matches("Smith").count(), 2, "{persons}");
}

#[test]
fn a_json_plan_of_a_file_without_records_is_still_json() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    let empty = "0 HEAD\n1 SOUR test\n1 FILE empty.ged\n0 TRLR\n";
    let json = stdout(&import_text(dir.path(), empty, &["--yes", "--plan", "--json"]).success());
    let plan: serde_json::Value = serde_json::from_str(json.trim()).unwrap();
    assert_eq!(plan["kinds"].as_array().map(Vec::len), Some(0), "{json}");
}

/// Imports `TREE`, then a second export of it as another tree, every possible match left for later;
/// returns the second run's id (runs are listed oldest first).
fn import_deferred_copy(dir: &Path) -> String {
    import_tree(dir, &[]).success();
    let other = TREE.replace("1 FILE tree.ged", "1 FILE other.ged");
    import_text(dir, &other, &["--yes", "--new-dataset", "--defer-matches"]).success();
    let runs = stdout(&vitni(dir).args(["import-run", "list"]).assert().success());
    let line = runs.lines().last().unwrap();
    line.split_whitespace().next().unwrap().to_owned()
}

/// The `match list` lines of `extra`.
fn match_lines(dir: &Path, extra: &[&str]) -> Vec<String> {
    let text = stdout(&vitni(dir).args(["match", "list"]).args(extra).assert().success());
    text.lines().map(str::to_owned).collect()
}

#[test]
fn deferred_pairs_are_listed_under_their_run_and_emptied_by_deciding_them() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    let run = import_deferred_copy(dir.path());

    let lines = match_lines(dir.path(), &["--run", &run]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let fields: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(fields[0], "person", "{lines:?}");
    let (a, b) = (fields[1], fields[2]);
    assert!(lines[0].contains('%'), "{lines:?}");

    let shown = stdout(
        &vitni(dir.path())
            .args(["match", "show", "person", a, b])
            .assert()
            .success(),
    );
    assert!(shown.contains("given name: same"), "{shown}");

    vitni(dir.path())
        .args([
            "match",
            "same",
            "person",
            a,
            b,
            "--confidence",
            "high",
            "--rationale",
            "one export",
        ])
        .assert()
        .success();
    assert_eq!(
        match_lines(dir.path(), &["--run", &run]),
        ["No possible matches."],
        "the decided pair has left the queue"
    );
    vitni(dir.path())
        .args(["person", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Smith").count(1));
}

#[test]
fn a_pair_marked_distinct_leaves_the_queue_and_cannot_be_merged() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    import_deferred_copy(dir.path());
    let json = stdout(&vitni(dir.path()).args(["match", "list", "--json"]).assert().success());
    let queue: serde_json::Value = serde_json::from_str(json.trim()).unwrap();
    let pairs = queue.as_array().unwrap();
    assert_eq!(pairs.len(), 1, "{json}");
    assert_eq!(pairs[0]["kind"], "person");
    assert!(pairs[0]["score"].as_u64().unwrap() <= 100, "{json}");
    let (a, b) = (pairs[0]["a"].as_str().unwrap(), pairs[0]["b"].as_str().unwrap());

    vitni(dir.path())
        .args(["match", "distinct", "person", a, b])
        .assert()
        .success();
    assert_eq!(match_lines(dir.path(), &[]), ["No possible matches."]);
    vitni(dir.path())
        .args(["match", "same", "person", a, b])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already"));
}

#[test]
fn the_queue_is_filtered_by_kind_and_band() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    import_deferred_copy(dir.path());
    assert_eq!(match_lines(dir.path(), &["--kind", "place"]), ["No possible matches."]);
    assert_eq!(match_lines(dir.path(), &["--kind", "person"]).len(), 1);
    assert_eq!(
        match_lines(dir.path(), &["--band", "deterministic"]),
        ["No possible matches."]
    );
    vitni(dir.path())
        .args(["match", "list", "--kind", "tag"])
        .assert()
        .failure();
    vitni(dir.path())
        .args(["match", "list", "--run", "not-a-run"])
        .assert()
        .failure();
}

/// Runs `statement` against the `gen` workspace's database with no command running, returning the
/// first column of its first row, if any.
fn sql(dir: &Path, statement: &str) -> Option<String> {
    let url = format!("sqlite://{}", dir.join("ws").join("vitni.sqlite3").display());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        let value = sqlx::query_scalar(statement).fetch_optional(&pool).await.unwrap();
        pool.close().await;
        value
    })
}

#[test]
fn a_workspace_with_an_unreadable_event_opens_warns_and_names_it() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    for given in ["Ada", "Bob"] {
        vitni(dir.path())
            .args(["person", "create", "--given", given])
            .assert()
            .success();
    }
    let ada = sql(dir.path(), "SELECT view_id FROM person_view WHERE human_id = 'I0001'").unwrap();
    sql(
        dir.path(),
        &format!(
            "UPDATE events SET payload = '{{\"type\":\"NoSuchEvent\"}}' \
             WHERE aggregate_id = '{ada}' AND sequence = 2 RETURNING aggregate_id"
        ),
    );
    let named = format!("  person event {ada} #2 (");
    let warning = "warning: one event in this workspace could not be read";

    let rebuilt = vitni(dir.path()).arg("rebuild").assert().success();
    let stderr = String::from_utf8(rebuilt.get_output().stderr.clone()).unwrap();
    assert_eq!(stderr.matches(warning).count(), 1, "warned once: {stderr}");
    assert!(stderr.contains(&named), "names the event: {stderr}");
    assert!(
        stderr.contains("update vitni; otherwise restore an earlier backup"),
        "{stderr}"
    );

    vitni(dir.path())
        .args(["person", "list"])
        .assert()
        .success()
        .stderr(predicate::str::contains(warning).and(predicate::str::contains(named.as_str())))
        .stdout(
            predicate::str::contains("I0001")
                .and(predicate::str::contains("I0002"))
                .and(predicate::str::contains("Bob")),
        );

    let backup = dir.path().join("gen.vitni-backup");
    vitni(dir.path())
        .args(["backup", "create"])
        .arg(&backup)
        .assert()
        .success();
    vitni(dir.path())
        .args(["backup", "restore"])
        .arg(&backup)
        .args(["--new", "copy"])
        .arg(dir.path().join("copy"))
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!("person event {ada} #2 (")));
}

/// Runs `args` to success and returns the id from its trailing `Created <id>` line.
fn create_record(dir: &Path, args: &[&str]) -> String {
    let created = vitni(dir).args(args).assert().success();
    let stdout = stdout(&created);
    let line = stdout.lines().find(|line| line.starts_with("Created ")).unwrap();
    line.trim_start_matches("Created ").trim().to_owned()
}

/// Creates one record of `aggregate` (with whatever it depends on) and returns its id, or `None` for a
/// name with no restrictions.
fn try_create_restrictable(dir: &Path, aggregate: &str) -> Option<String> {
    let id = match aggregate {
        "person" => create_record(dir, &["person", "create", "--given", "Ada"]),
        "family" => create_record(dir, &["family", "create"]),
        "place" => create_record(dir, &["place", "create", "--type", "farm", "--name", "Nordås"]),
        "source" => create_record(dir, &["source", "create", "--title", "Folketelling 1801"]),
        "citation" => {
            let source = create_record(dir, &["source", "create", "--title", "Parish register"]);
            create_record(dir, &["citation", "create", "--source", &source])
        }
        "event" => create_record(dir, &["event", "create", "--type", "birth"]),
        "dna-test" => {
            let person = create_record(dir, &["person", "create", "--given", "Ada"]);
            create_record(dir, &["dna-test", "create", "--person", &person])
        }
        "dna-match" => {
            let ada = create_record(dir, &["person", "create", "--given", "Ada"]);
            let bob = create_record(dir, &["person", "create", "--given", "Bob"]);
            let test_a = create_record(dir, &["dna-test", "create", "--person", &ada]);
            let test_b = create_record(dir, &["dna-test", "create", "--person", &bob]);
            create_record(
                dir,
                &[
                    "dna-match",
                    "observe",
                    "--test-a",
                    &test_a,
                    "--test-b",
                    &test_b,
                    "--provider",
                    "ancestry-dna",
                    "--shared-cm",
                    "850.5",
                ],
            )
        }
        "repository" => create_record(dir, &["repository", "create", "--name", "Statsarkivet"]),
        "note" => create_record(dir, &["note", "create", "--text", "a witness note"]),
        "media" => create_record(dir, &["media", "create", "--path", "scan.png"]),
        "tag" => create_record(dir, &["tag", "create", "--name", "Living"]),
        "research-note" => {
            let person = create_record(dir, &["person", "create", "--given", "Ada"]);
            let subject = format!("person:{person}");
            create_record(
                dir,
                &[
                    "research-note",
                    "create",
                    "--subject",
                    &subject,
                    "--title",
                    "Who was Ada?",
                ],
            )
        }
        _ => return None,
    };
    Some(id)
}

/// [`try_create_restrictable`] for a name known to be restrictable.
fn create_restrictable(dir: &Path, aggregate: &str) -> String {
    try_create_restrictable(dir, aggregate).unwrap()
}

#[test]
fn set_restrictions_writes_and_clears_restrictions_on_every_aggregate() {
    let aggregates = [
        "person",
        "family",
        "place",
        "source",
        "citation",
        "event",
        "dna-test",
        "dna-match",
        "repository",
        "note",
        "media",
        "tag",
        "research-note",
    ];
    for aggregate in aggregates {
        let dir = TempDir::new().unwrap();
        init(dir.path());
        let id = create_restrictable(dir.path(), aggregate);

        vitni(dir.path())
            .args([aggregate, "set-restrictions", &id, "--restriction", "privacy,locked"])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!("Updated {id}")));
        let shown = vitni(dir.path()).args([aggregate, "show", &id]).assert().success();
        assert!(
            stdout(&shown).trim_end().ends_with("[locked, privacy]"),
            "{aggregate}: {}",
            stdout(&shown)
        );
        let listed = vitni(dir.path()).args([aggregate, "list"]).assert().success();
        assert!(
            stdout(&listed).contains("[locked, privacy]"),
            "{aggregate}: {}",
            stdout(&listed)
        );

        vitni(dir.path())
            .args([aggregate, "set-restrictions", &id, "--clear"])
            .assert()
            .success();
        let shown = vitni(dir.path()).args([aggregate, "show", &id]).assert().success();
        let shown = stdout(&shown);
        assert!(
            !shown.contains("locked") && !shown.contains("privacy"),
            "{aggregate}: {shown}"
        );
    }
}

#[test]
fn list_prints_the_window_offset_and_limit_name_on_every_paged_aggregate() {
    let aggregates = [
        "person",
        "family",
        "place",
        "source",
        "citation",
        "event",
        "dna-test",
        "dna-match",
        "repository",
        "note",
        "media",
        "research-note",
    ];
    for aggregate in aggregates {
        let dir = TempDir::new().unwrap();
        init(dir.path());
        let ids: Vec<String> = (0..3).map(|_| create_restrictable(dir.path(), aggregate)).collect();
        let listed = |window: &[&str]| {
            let assert = vitni(dir.path())
                .args([aggregate, "list"])
                .args(window)
                .assert()
                .success();
            let out = stdout(&assert);
            let shown: Vec<&str> = ids.iter().map(String::as_str).filter(|id| out.contains(*id)).collect();
            (shown.join(" "), out)
        };

        assert_eq!(
            listed(&["--limit", "2"]).0,
            format!("{} {}", ids[0], ids[1]),
            "{aggregate}"
        );
        assert_eq!(listed(&["--offset", "1", "--limit", "1"]).0, ids[1], "{aggregate}");
        assert_eq!(listed(&["--offset", "2"]).0, ids[2], "{aggregate}");
        let (shown, out) = listed(&["--offset", "3"]);
        assert!(shown.is_empty(), "{aggregate}: {out}");
        assert!(out.contains("Nothing to list past the first 3."), "{aggregate}: {out}");
        vitni(dir.path())
            .args([aggregate, "list", "--limit", "0"])
            .assert()
            .failure();
    }
}

#[test]
fn set_restrictions_accepts_the_flag_repeated() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    let id = create_restrictable(dir.path(), "person");

    vitni(dir.path())
        .args(["person", "set-restrictions", &id, "--restriction", "confidential"])
        .args(["--restriction", "privacy"])
        .assert()
        .success();
    vitni(dir.path())
        .args(["person", "show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("[confidential, privacy]"));
}

#[test]
fn set_restrictions_requires_a_restriction_or_clear() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    let id = create_restrictable(dir.path(), "person");

    vitni(dir.path())
        .args(["person", "set-restrictions", &id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--restriction"));
    vitni(dir.path())
        .args(["person", "set-restrictions", &id, "--clear", "--restriction", "privacy"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
    vitni(dir.path())
        .args(["person", "set-restrictions", &id, "--restriction", "secret"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("secret"));
}

#[test]
fn set_restrictions_on_an_unknown_record_fails_naming_it() {
    let dir = TempDir::new().unwrap();
    init(dir.path());

    vitni(dir.path())
        .args(["note", "set-restrictions", "N9999", "--restriction", "privacy"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("N9999"));
}

#[test]
fn restrictions_render_in_the_workspace_language() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    let id = create_restrictable(dir.path(), "source");
    set_ui_language_norwegian(dir.path());

    vitni(dir.path())
        .args(["source", "set-restrictions", &id, "--restriction", "privacy"])
        .assert()
        .success();
    let shown = vitni(dir.path()).args(["source", "show", &id]).assert().success();
    assert!(
        stdout(&shown).trim_end().ends_with("[personvern]"),
        "{}",
        stdout(&shown)
    );
}

#[test]
fn every_attach_verb_takes_the_human_id_the_cli_printed() {
    let verbs = [
        ("event", "attach-note", "--note"),
        ("event", "attach-media", "--media"),
        ("place", "attach-note", "--note"),
        ("place", "attach-media", "--media"),
        ("source", "attach-note", "--note"),
        ("source", "attach-media", "--media"),
        ("repository", "attach-note", "--note"),
        ("media", "attach-note", "--note"),
        ("dna-test", "attach-note", "--note"),
        ("dna-match", "attach-note", "--note"),
    ];
    for (aggregate, verb, flag) in verbs {
        let dir = TempDir::new().unwrap();
        init(dir.path());
        let owner = create_restrictable(dir.path(), aggregate);
        let attached = match flag {
            "--note" => create_record(dir.path(), &["note", "create", "--text", "Banns read"]),
            _ => create_record(dir.path(), &["media", "create", "--path", "register.png"]),
        };

        vitni(dir.path())
            .args([aggregate, verb, flag, &attached, &owner])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!("Updated {owner}")));
    }
}

#[test]
fn an_attach_verb_naming_an_unknown_note_or_media_fails() {
    let dir = TempDir::new().unwrap();
    init(dir.path());
    let event = create_record(dir.path(), &["event", "create", "--type", "birth"]);

    vitni(dir.path())
        .args(["event", "attach-note", "--note", "N9999", &event])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no note with human_id \"N9999\""));
    vitni(dir.path())
        .args(["event", "attach-media", "--media", "O9999", &event])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no media with human_id \"O9999\""));
}
