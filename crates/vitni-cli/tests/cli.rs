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
