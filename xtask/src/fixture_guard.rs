//! `cargo xtask fixture-guard` — every committed fixture declares where it came from (ADR 0042 §1–3).
//!
//! A tracked file under any `tests/fixtures/` tree, or under the matching corpus, must be covered by a
//! `PROVENANCE.toml` in its directory or an ancestor; the nearest one wins. It declares the `origin`
//! and what that origin requires:
//!
//! ```toml
//! origin = "generated"                      # or "invented", "licensed", "transcribed-facts"
//! generator = "cargo xtask backup-fixture"  # generated: the xtask that writes the files
//! # licensed: licence + attribution; transcribed-facts: sources, a list of https URLs or a statement
//!
//! [files."other.toml"]                      # optional: a different origin for one file in this directory
//! origin = "invented"
//! ```
//!
//! The guard also fails if a page the external fixture manifest lists (ADR 0042 §3) is tracked: those
//! pages are fetched into `target/`, never committed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use crate::fetch_fixtures;

/// The declaration's file name.
const DECLARATION: &str = "PROVENANCE.toml";

/// The matching evaluation corpus, the one fixture tree not under a `tests/fixtures/` directory.
const CORPUS: &str = "crates/vitni-core/matching/corpus";

/// A fixture's origin and what that origin requires (ADR 0042 §1–2).
#[derive(Deserialize, Debug)]
#[serde(tag = "origin", rename_all = "kebab-case", deny_unknown_fields)]
enum Origin {
    Invented {},
    Generated { generator: String },
    Licensed { licence: String, attribution: String },
    TranscribedFacts { sources: Sources },
}

/// Where transcribed facts came from: the records' URLs, or a statement that the files list them.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum Sources {
    Urls(Vec<String>),
    Statement(String),
}

impl Origin {
    /// Checks what serde cannot: that no required value is blank and that every source is a URL.
    fn validate(&self) -> Result<()> {
        match self {
            Origin::Invented {} => Ok(()),
            Origin::Generated { generator } => required("generator", generator),
            Origin::Licensed { licence, attribution } => {
                required("licence", licence)?;
                required("attribution", attribution)
            }
            Origin::TranscribedFacts {
                sources: Sources::Statement(statement),
            } => required("sources", statement),
            Origin::TranscribedFacts {
                sources: Sources::Urls(urls),
            } => {
                ensure!(!urls.is_empty(), "`sources` lists no URL");
                for url in urls {
                    let host = url.strip_prefix("https://").unwrap_or_default();
                    ensure!(!host.is_empty(), "source {url:?} is not an https URL");
                }
                Ok(())
            }
        }
    }
}

fn required(key: &str, value: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "`{key}` is empty");
    Ok(())
}

/// The file names a valid `PROVENANCE.toml` gives their own declaration under `[files."<name>"]`.
fn parse(text: &str) -> Result<BTreeSet<String>> {
    let mut table: toml::Table = toml::from_str(text).context("not valid TOML")?;
    let files = match table.remove("files") {
        None => toml::Table::new(),
        Some(toml::Value::Table(files)) => files,
        Some(other) => bail!(
            "`files` must be a table of per-file declarations, not a {}",
            other.type_str()
        ),
    };
    parse_origin(toml::Value::Table(table)).context("the directory's declaration")?;
    let mut names = BTreeSet::new();
    for (name, value) in files {
        parse_origin(value).with_context(|| format!("the declaration for {name:?}"))?;
        names.insert(name);
    }
    Ok(names)
}

fn parse_origin(value: toml::Value) -> Result<()> {
    let origin: Origin = value.try_into()?;
    origin.validate()
}

/// The outcome of a guard run.
#[derive(Debug)]
struct Report {
    fixtures: usize,
    declarations: usize,
    problems: Vec<String>,
}

/// Runs the guard from the repository root.
pub fn run() -> Result<()> {
    let report = check_checkout(Path::new("."))?;
    if !report.problems.is_empty() {
        for problem in &report.problems {
            eprintln!("  error: {problem}");
        }
        bail!(
            "fixture-guard found {} problem(s): declare each fixture's origin in a {DECLARATION} (ADR 0042)",
            report.problems.len()
        );
    }
    println!(
        "fixture-guard: ok ({} fixtures, {} declarations)",
        report.fixtures, report.declarations
    );
    Ok(())
}

/// Checks the checkout at `root`: its tracked files, their declarations and the external manifest.
fn check_checkout(root: &Path) -> Result<Report> {
    let tracked = tracked_files(root)?;
    let mut declarations = BTreeMap::new();
    for path in &tracked {
        if path.file_name().is_some_and(|name| name == DECLARATION) {
            let full = root.join(path);
            let text = fs::read_to_string(&full).with_context(|| format!("reading {}", full.display()))?;
            declarations.insert(path.clone(), text);
        }
    }
    Ok(check_tree(&tracked, &declarations, &external_paths(root)?))
}

/// Every file git tracks under `root`, relative to it.
fn tracked_files(root: &Path) -> Result<BTreeSet<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .context("running `git ls-files` (is git installed, and is this a checkout?)")?;
    ensure!(
        output.status.success(),
        "`git ls-files` failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let listing = String::from_utf8(output.stdout).context("`git ls-files` printed a non-UTF-8 path")?;
    let mut tracked = BTreeSet::new();
    for path in listing.split('\0') {
        if !path.is_empty() {
            tracked.insert(PathBuf::from(path));
        }
    }
    Ok(tracked)
}

/// Where `cargo xtask fetch-fixtures` saves each page the external manifest lists, relative to `root`.
fn external_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let manifest = root.join(fetch_fixtures::MANIFEST);
    let text = fs::read_to_string(&manifest).with_context(|| format!("reading {}", manifest.display()))?;
    let pages = fetch_fixtures::load_pages(&text).with_context(|| format!("parsing {}", manifest.display()))?;
    let mut paths = Vec::new();
    for page in pages {
        paths.push(fetch_fixtures::output_path(
            Path::new(fetch_fixtures::OUT_DIR),
            &page.id,
        ));
    }
    Ok(paths)
}

/// Checks the `tracked` files against the `declarations` (each tracked `PROVENANCE.toml` and its text)
/// and the `external` paths that must not be tracked.
fn check_tree(tracked: &BTreeSet<PathBuf>, declarations: &BTreeMap<PathBuf, String>, external: &[PathBuf]) -> Report {
    let mut problems = Vec::new();
    for (path, text) in declarations {
        match parse(text) {
            Ok(files) => problems.extend(stale_entries(path, &files, tracked)),
            Err(error) => problems.push(format!("{}: {error:#}", path.display())),
        }
    }
    let mut fixtures = 0;
    for path in tracked {
        if !is_fixture(path) {
            continue;
        }
        fixtures += 1;
        if !is_covered(path, declarations) {
            problems.push(format!(
                "{} has no {DECLARATION} in its directory or an ancestor",
                path.display()
            ));
        }
    }
    for path in external {
        if tracked.contains(path) {
            problems.push(format!(
                "{} is an external fixture page (ADR 0042 §3), fetched by `cargo xtask fetch-fixtures`; untrack it",
                path.display()
            ));
        }
    }
    Report {
        fixtures,
        declarations: declarations.len(),
        problems,
    }
}

/// The `[files."<name>"]` tables of the declaration at `path` that name no tracked file beside it.
fn stale_entries(path: &Path, files: &BTreeSet<String>, tracked: &BTreeSet<PathBuf>) -> Vec<String> {
    let dir = path.parent().unwrap_or(Path::new(""));
    let mut stale = Vec::new();
    for name in files {
        let target = dir.join(name);
        let beside = target.parent() == Some(dir) && name != DECLARATION;
        if !beside || !tracked.contains(&target) {
            stale.push(format!(
                "{}: [files.{name:?}] names no tracked file in this directory",
                path.display()
            ));
        }
    }
    stale
}

/// Whether `path` is a fixture: inside a `tests/fixtures/` tree or the matching corpus, and not a
/// declaration itself.
fn is_fixture(path: &Path) -> bool {
    if path.file_name().is_some_and(|name| name == DECLARATION) {
        return false;
    }
    if path.starts_with(CORPUS) {
        return true;
    }
    let Some(dir) = path.parent() else {
        return false;
    };
    let components: Vec<_> = dir.components().map(std::path::Component::as_os_str).collect();
    components
        .windows(2)
        .any(|pair| pair[0] == "tests" && pair[1] == "fixtures")
}

/// Whether a declaration sits in `path`'s directory or an ancestor. Only the nearest one applies, and
/// a malformed one is reported where it is parsed.
fn is_covered(path: &Path, declarations: &BTreeMap<PathBuf, String>) -> bool {
    path.ancestors()
        .skip(1)
        .any(|dir| declarations.contains_key(&dir.join(DECLARATION)))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    use super::{Report, check_checkout, check_tree};

    /// Runs the guard over `files` plus the `declarations` (path, text), which are tracked too.
    fn guard(files: &[&str], declarations: &[(&str, &str)], external: &[&str]) -> Report {
        let mut tracked: BTreeSet<PathBuf> = files.iter().map(PathBuf::from).collect();
        let mut texts = BTreeMap::new();
        for (path, text) in declarations {
            tracked.insert(PathBuf::from(path));
            texts.insert(PathBuf::from(path), (*text).to_owned());
        }
        let external: Vec<PathBuf> = external.iter().map(PathBuf::from).collect();
        check_tree(&tracked, &texts, &external)
    }

    fn problems(files: &[&str], declarations: &[(&str, &str)]) -> Vec<String> {
        guard(files, declarations, &[]).problems
    }

    /// Asserts exactly one problem, mentioning every one of `needles`.
    fn assert_one_problem(problems: &[String], needles: &[&str]) {
        assert_eq!(problems.len(), 1, "{problems:#?}");
        for needle in needles {
            assert!(problems[0].contains(needle), "{needle:?} not in {:?}", problems[0]);
        }
    }

    const INVENTED: &str = "origin = \"invented\"\n";

    #[test]
    fn an_uncovered_fixture_fails_naming_it() {
        let problems = problems(
            &[
                "crates/a/tests/fixtures/x/covered.bin",
                "crates/b/tests/fixtures/new.bin",
            ],
            &[("crates/a/tests/fixtures/PROVENANCE.toml", INVENTED)],
        );
        assert_one_problem(&problems, &["crates/b/tests/fixtures/new.bin", "PROVENANCE.toml"]);
    }

    #[test]
    fn a_declaration_covers_its_own_directory_and_every_one_below() {
        let report = guard(
            &["c/tests/fixtures/a.html", "c/tests/fixtures/deep/er/b.html"],
            &[("c/tests/fixtures/PROVENANCE.toml", INVENTED)],
            &[],
        );
        assert!(report.problems.is_empty(), "{:#?}", report.problems);
        assert_eq!(report.fixtures, 2, "the declaration itself is not a fixture");
    }

    #[test]
    fn a_declaration_above_the_fixture_tree_covers_it() {
        let problems = problems(&["c/tests/fixtures/a.html"], &[("c/PROVENANCE.toml", INVENTED)]);
        assert!(problems.is_empty(), "{problems:#?}");
    }

    #[test]
    fn the_matching_corpus_is_a_fixture_tree() {
        let problems = problems(&["crates/vitni-core/matching/corpus/new.toml"], &[]);
        assert_one_problem(&problems, &["crates/vitni-core/matching/corpus/new.toml"]);
    }

    #[test]
    fn files_outside_a_fixture_tree_need_no_declaration() {
        let problems = problems(
            &[
                "c/src/fixtures/a.rs",
                "c/tests/a.rs",
                "c/tests/external/manifest.toml",
                "fixtures/a",
            ],
            &[],
        );
        assert!(problems.is_empty(), "{problems:#?}");
    }

    #[test]
    fn the_nearest_declaration_wins() {
        let problems = problems(
            &["c/tests/fixtures/gen/a.bin"],
            &[
                ("c/tests/fixtures/PROVENANCE.toml", INVENTED),
                ("c/tests/fixtures/gen/PROVENANCE.toml", "origin = \"generated\"\n"),
            ],
        );
        assert_one_problem(&problems, &["c/tests/fixtures/gen/PROVENANCE.toml", "generator"]);
    }

    #[test]
    fn every_origin_with_what_it_requires_is_accepted() {
        let declarations = [
            INVENTED,
            "origin = \"generated\"\ngenerator = \"cargo xtask backup-fixture\"\n",
            "origin = \"licensed\"\nlicence = \"CC-BY-4.0\"\nattribution = \"Someone\"\n",
            "origin = \"transcribed-facts\"\nsources = [\"https://example.org/record/1\"]\n",
            "origin = \"transcribed-facts\"\nsources = \"each pair's `source` lists its records' URLs\"\n",
        ];
        for text in declarations {
            let problems = problems(&["c/tests/fixtures/a"], &[("c/tests/fixtures/PROVENANCE.toml", text)]);
            assert!(problems.is_empty(), "{text}: {problems:#?}");
        }
    }

    #[test]
    fn an_origin_missing_what_it_requires_is_rejected() {
        let declarations = [
            ("origin = \"licensed\"\nlicence = \"CC-BY-4.0\"\n", "attribution"),
            ("origin = \"licensed\"\nattribution = \"Someone\"\n", "licence"),
            ("origin = \"transcribed-facts\"\n", "sources"),
            ("generator = \"cargo xtask x\"\n", "origin"),
        ];
        for (text, missing) in declarations {
            let problems = problems(&["c/tests/fixtures/a"], &[("c/tests/fixtures/PROVENANCE.toml", text)]);
            assert_one_problem(&problems, &["c/tests/fixtures/PROVENANCE.toml", missing]);
        }
    }

    #[test]
    fn a_malformed_declaration_is_rejected() {
        let declarations = [
            "origin = \"found-it-online\"\n",
            "origin = \"invented\"\nlicence = \"MIT\"\n",
            "origin = \"invented\"\nnotes = \"a typo'd key\"\n",
            "origin = \"generated\"\ngenerator = \"  \"\n",
            "origin = \"licensed\"\nlicence = \"\"\nattribution = \"Someone\"\n",
            "origin = \"transcribed-facts\"\nsources = []\n",
            "origin = \"transcribed-facts\"\nsources = [\"digitalarkivet.no/source/1\"]\n",
            "origin = \"transcribed-facts\"\nsources = [\"http://example.org/plain\"]\n",
            "origin = \"transcribed-facts\"\nsources = \"\"\n",
            "origin = \"invented\"\nfiles = \"not a table\"\n",
            "origin = [",
        ];
        for text in declarations {
            let problems = problems(&["c/tests/fixtures/a"], &[("c/tests/fixtures/PROVENANCE.toml", text)]);
            assert_one_problem(&problems, &["c/tests/fixtures/PROVENANCE.toml"]);
        }
    }

    #[test]
    fn a_per_file_table_overrides_the_origin_of_that_file() {
        let text = "origin = \"invented\"\n[files.\"real.toml\"]\norigin = \"transcribed-facts\"\n";
        let problems = problems(
            &["c/tests/fixtures/real.toml", "c/tests/fixtures/made-up.toml"],
            &[("c/tests/fixtures/PROVENANCE.toml", text)],
        );
        assert_one_problem(&problems, &["c/tests/fixtures/PROVENANCE.toml", "real.toml", "sources"]);
    }

    #[test]
    fn a_per_file_table_naming_no_tracked_file_in_its_directory_is_rejected() {
        let text = "origin = \"invented\"\n[files.\"deep/a.toml\"]\norigin = \"invented\"\n\
                    [files.\"gone.toml\"]\norigin = \"invented\"\n";
        let problems = problems(
            &["c/tests/fixtures/deep/a.toml"],
            &[("c/tests/fixtures/PROVENANCE.toml", text)],
        );
        assert_eq!(problems.len(), 2, "{problems:#?}");
        assert!(problems[0].contains("deep/a.toml"), "{problems:#?}");
        assert!(problems[1].contains("gone.toml"), "{problems:#?}");
    }

    #[test]
    fn a_per_file_table_cannot_nest_files() {
        let text = "origin = \"invented\"\n[files.\"a\"]\norigin = \"invented\"\n[files.\"a\".files.\"b\"]\n\
                    origin = \"invented\"\n";
        let problems = problems(&["c/tests/fixtures/a"], &[("c/tests/fixtures/PROVENANCE.toml", text)]);
        assert_one_problem(&problems, &["c/tests/fixtures/PROVENANCE.toml", "files"]);
    }

    #[test]
    fn a_tracked_external_page_fails_naming_it() {
        let report = guard(
            &["target/external-fixtures/da/census-person.html"],
            &[],
            &[
                "target/external-fixtures/da/census-person.html",
                "target/external-fixtures/da/viewer.html",
            ],
        );
        assert_one_problem(
            &report.problems,
            &["target/external-fixtures/da/census-person.html", "external"],
        );
    }

    #[test]
    fn the_repository_passes() {
        let report = check_checkout(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).unwrap();
        assert!(report.problems.is_empty(), "{:#?}", report.problems);
        assert!(report.fixtures > 0 && report.declarations > 0, "{report:?}");
    }
}
