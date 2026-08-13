//! Framework-free boundary guard (ADR 0008 §§2,4): `vitni-ui` holds all presentation logic and
//! **no framework types**. Dependencies flow one way — `vitni-app → vitni-ui →
//! vitni-ui-<framework>` — so no `dioxus::` (or future `slint::`) type may appear at or below
//! this crate. Until now that boundary was doc-comment only; these tests make it executable.
//!
//! Two independent checks, each factored into a pure function unit-tested in isolation:
//!
//! 1. **Dependency-tree guard** — `vitni-ui`'s transitive dependency closure (resolved offline
//!    from `Cargo.lock` via `cargo metadata`) contains no banned framework crate.
//! 2. **Source scan** — no `.rs` file under `src/` references a framework namespace (`dioxus::` /
//!    `slint::`), the cheap honest stand-in for a public-API scan (no nightly rustdoc-json).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use cargo_metadata::{Metadata, MetadataCommand, PackageId};

/// The framework crates that must never enter `vitni-ui`. A package is a match if its name is
/// exactly one of these or belongs to its `-*` family (e.g. `dioxus-core`, `slint-macros`).
const BANNED_FRAMEWORKS: &[&str] = &["dioxus", "slint"];

/// True when `name` is a banned framework crate or a member of its `-*` family.
fn framework_matches(name: &str, banned: &[&str]) -> bool {
    banned
        .iter()
        .any(|prefix| name == *prefix || name.strip_prefix(prefix).is_some_and(|rest| rest.starts_with('-')))
}

/// Collect every banned framework crate reachable from `root` in a package-name dependency graph.
///
/// `graph` maps each package name to the names of its direct dependencies. Returns the sorted,
/// de-duplicated set of banned crates in `root`'s transitive closure. Pure over a plain map so it is
/// unit-testable with a hand-built graph; the `cargo_metadata` adapter feeds it the real resolve.
fn framework_crates_in_graph(graph: &BTreeMap<String, Vec<String>>, root: &str, banned: &[&str]) -> Vec<String> {
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<&str> = vec![root];
    while let Some(pkg) = stack.pop() {
        if !visited.insert(pkg) {
            continue;
        }
        if pkg != root && framework_matches(pkg, banned) {
            found.insert(pkg.to_string());
        }
        if let Some(deps) = graph.get(pkg) {
            for dep in deps {
                stack.push(dep);
            }
        }
    }
    found.into_iter().collect()
}

/// Resolve `root`'s transitive dependency closure from `meta`'s resolve graph and return any banned
/// framework crate in it. Package edges are keyed by name (versions merge — adequate for detecting a
/// banned *name*); the resolve graph is produced offline from `Cargo.lock`.
fn framework_crates_in_closure(meta: &Metadata, root: &str, banned: &[&str]) -> Vec<String> {
    let id_to_name: HashMap<&PackageId, String> = meta.packages.iter().map(|p| (&p.id, p.name.to_string())).collect();
    let resolve = meta.resolve.as_ref().expect("cargo metadata resolve graph is present");
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in &resolve.nodes {
        let name = id_to_name[&node.id].clone();
        let deps = node
            .deps
            .iter()
            .filter_map(|dep| id_to_name.get(&dep.pkg).cloned())
            .collect::<Vec<_>>();
        graph.entry(name).or_default().extend(deps);
    }
    framework_crates_in_graph(&graph, root, banned)
}

/// Return the banned framework namespaces referenced as a path (`name::`) in the *code* of `source`.
///
/// Line comments (`//`, `//!`, `///`) are skipped: a doc comment that discusses the boundary — as
/// this crate's own module doc does — is not a leak. Only a real path reference in code counts.
fn framework_tokens_in_source(source: &str, banned: &[&str]) -> Vec<String> {
    let mut hits: BTreeSet<String> = BTreeSet::new();
    for line in source.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for prefix in banned {
            if line.contains(&format!("{prefix}::")) {
                hits.insert((*prefix).to_string());
            }
        }
    }
    hits.into_iter().collect()
}

/// Every `.rs` file under `dir`, recursively.
fn rust_sources(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).expect("read source directory");
    for entry in entries {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            files.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files
}

#[test]
fn matcher_flags_a_crate_and_its_family_but_not_a_lookalike() {
    assert!(framework_matches("dioxus", BANNED_FRAMEWORKS));
    assert!(framework_matches("dioxus-core", BANNED_FRAMEWORKS));
    assert!(framework_matches("slint-macros", BANNED_FRAMEWORKS));
    assert!(!framework_matches("dioxuslike", BANNED_FRAMEWORKS));
    assert!(!framework_matches("vitni-ui", BANNED_FRAMEWORKS));
}

#[test]
fn graph_walk_detects_a_framework_crate_in_the_closure() {
    let mut graph = BTreeMap::new();
    graph.insert("vitni-ui".to_string(), vec!["vitni-app".to_string()]);
    graph.insert("vitni-app".to_string(), vec!["dioxus".to_string()]);
    graph.insert("dioxus".to_string(), vec!["dioxus-core".to_string()]);
    graph.insert("dioxus-core".to_string(), Vec::new());

    let found = framework_crates_in_graph(&graph, "vitni-ui", BANNED_FRAMEWORKS);
    assert_eq!(found, vec!["dioxus".to_string(), "dioxus-core".to_string()]);
}

#[test]
fn graph_walk_is_empty_for_a_clean_closure() {
    let mut graph = BTreeMap::new();
    graph.insert("vitni-ui".to_string(), vec!["vitni-app".to_string()]);
    graph.insert("vitni-app".to_string(), vec!["serde".to_string()]);
    graph.insert("serde".to_string(), Vec::new());

    assert!(framework_crates_in_graph(&graph, "vitni-ui", BANNED_FRAMEWORKS).is_empty());
}

#[test]
fn source_scan_detects_a_framework_use() {
    let hits = framework_tokens_in_source("use dioxus::prelude::*;\n", BANNED_FRAMEWORKS);
    assert_eq!(hits, vec!["dioxus".to_string()]);
}

#[test]
fn source_scan_ignores_a_framework_reference_in_a_comment() {
    let clean = "//! No `dioxus::` type appears here (ADR 0008).\nuse vitni_app::Workspace;\n";
    assert!(framework_tokens_in_source(clean, BANNED_FRAMEWORKS).is_empty());
}

#[test]
fn vitni_ui_dependency_closure_is_framework_free() {
    let meta = MetadataCommand::new().exec().expect("run cargo metadata");
    let found = framework_crates_in_closure(&meta, "vitni-ui", BANNED_FRAMEWORKS);
    assert!(found.is_empty(), "vitni-ui pulls banned framework crates: {found:?}");
}

#[test]
fn vitni_ui_sources_reference_no_framework() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: BTreeSet<String> = BTreeSet::new();
    for file in rust_sources(&src) {
        let contents = std::fs::read_to_string(&file).expect("read source file");
        for token in framework_tokens_in_source(&contents, BANNED_FRAMEWORKS) {
            offenders.insert(format!("{}: {token}", file.display()));
        }
    }
    assert!(
        offenders.is_empty(),
        "framework references in vitni-ui/src: {offenders:?}"
    );
}
