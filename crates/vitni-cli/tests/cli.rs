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
