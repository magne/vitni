//! `licence-check` — guards the per-crate licence split (ADR 0034 §1): the permissive crates are
//! only honestly permissive while **no permissive crate reaches an `AGPL-3.0-or-later` one**. One
//! new dependency edge would make a published `MIT OR Apache-2.0` declaration false, which is a
//! licence claim the project cannot honour rather than a mere inconsistency.
//!
//! Manifests are read directly instead of through `cargo metadata`, for one reason: the `plugins/*`
//! crates are **excluded from the workspace** (ADR 0011) and so are invisible to a root
//! `cargo metadata`, yet `plugins/digitalarkivet-import` is exactly the crate whose licence differs
//! from its siblings'. Reading `Cargo.toml` covers workspace and plugin crates the same way.
//!
//! Three failures are reported, each with the dependency path that produced it:
//!
//! 1. a permissive crate that reaches a copyleft crate,
//! 2. a crate that declares no licence at all (what every `plugins/*` manifest did before ADR 0034),
//! 3. a crate that declares a licence this project does not use, which means the split has grown a
//!    case nobody decided.
//!
//! `dev-dependencies` are deliberately not followed: they are linked into test binaries only and
//! never into a distributed artifact, so they cannot taint its licence.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// The licence the commodity-interop crates carry (the workspace default).
const PERMISSIVE: &str = "MIT OR Apache-2.0";
/// The licence the application crates carry.
const COPYLEFT: &str = "AGPL-3.0-or-later";
/// Manifest directories scanned, each holding one crate per child directory.
const CRATE_DIRS: [&str; 2] = ["crates", "plugins"];
/// Manifests that are not under a scanned directory.
const EXTRA_MANIFESTS: [&str; 1] = ["xtask/Cargo.toml"];
/// Dependency tables that affect the distributed artifact.
const LINKED_DEP_TABLES: [&str; 2] = ["dependencies", "build-dependencies"];

/// One crate manifest, reduced to what the licence direction depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Crate {
    /// Package name, as other manifests refer to it.
    name: String,
    /// The declared licence, or `None` when the manifest declares none.
    licence: Option<String>,
    /// Names of the in-repo crates this one links.
    deps: Vec<String>,
}

/// Runs the `licence-check` command (see module docs).
pub fn run() -> Result<()> {
    let root = read_manifest(Path::new("Cargo.toml"))?;
    let workspace_licence = workspace_licence(&root)?;
    let workspace_paths = workspace_path_dependencies(&root);

    let mut crates: BTreeMap<String, Crate> = BTreeMap::new();
    for manifest in manifest_paths()? {
        let value = read_manifest(&manifest)?;
        let parsed = parse_crate(&value, &workspace_licence, &workspace_paths)
            .with_context(|| format!("reading {}", manifest.display()))?;
        println!(
            "licence-check: {:<32} {}",
            parsed.name,
            parsed.licence.as_deref().unwrap_or("(none declared)")
        );
        crates.insert(parsed.name.clone(), parsed);
    }

    let problems = problems(&crates);
    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("  error: {problem}");
        }
        bail!("licence-check found {} problem(s) (see above)", problems.len());
    }
    println!(
        "\nlicence-check: {} crates, no permissive crate reaches copyleft.",
        crates.len()
    );
    Ok(())
}

/// Every crate manifest in the repository, sorted.
fn manifest_paths() -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = EXTRA_MANIFESTS.iter().map(PathBuf::from).collect();
    for dir in CRATE_DIRS {
        let dir = Path::new(dir);
        for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let entry = entry.with_context(|| format!("reading an entry under {}", dir.display()))?;
            let manifest = entry.path().join("Cargo.toml");
            if manifest.is_file() {
                paths.push(manifest);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

/// Parses `path` as TOML.
fn read_manifest(path: &Path) -> Result<toml::Value> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// The `[workspace.package] license` every crate inherits with `license.workspace = true`.
fn workspace_licence(root: &toml::Value) -> Result<String> {
    root.get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("license"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .context("the root Cargo.toml declares no [workspace.package] license")
}

/// The `[workspace.dependencies]` entries that point at an in-repo path, i.e. the ones a member
/// crate's `dep = { workspace = true }` resolves to another crate of this repository.
fn workspace_path_dependencies(root: &toml::Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(table) = root
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
    else {
        return names;
    };
    for (name, spec) in table {
        if spec.get("path").is_some() {
            names.insert(name.clone());
        }
    }
    names
}

/// Reduces one parsed manifest to the name, licence and in-repo dependencies the check needs.
///
/// `workspace_licence` resolves `license.workspace = true`; `workspace_paths` names the
/// `[workspace.dependencies]` entries that resolve to an in-repo crate, so that
/// `dep = { workspace = true }` is recognised as an internal edge.
fn parse_crate(manifest: &toml::Value, workspace_licence: &str, workspace_paths: &BTreeSet<String>) -> Result<Crate> {
    let package = manifest.get("package").context("manifest has no [package] table")?;
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .context("[package] has no name")?
        .to_owned();

    let licence = match package.get("license") {
        None => None,
        Some(value) if value.as_str().is_some() => value.as_str().map(str::to_owned),
        Some(value) if value.get("workspace").and_then(toml::Value::as_bool) == Some(true) => {
            Some(workspace_licence.to_owned())
        }
        Some(_) => bail!("{name}: [package] license is neither a string nor `license.workspace = true`"),
    };

    let mut deps = Vec::new();
    for table in LINKED_DEP_TABLES {
        let Some(entries) = manifest.get(table).and_then(toml::Value::as_table) else {
            continue;
        };
        for (key, spec) in entries {
            let dependency = spec.get("package").and_then(toml::Value::as_str).unwrap_or(key);
            let by_path = spec.get("path").is_some();
            let by_workspace =
                spec.get("workspace").and_then(toml::Value::as_bool) == Some(true) && workspace_paths.contains(key);
            if by_path || by_workspace {
                deps.push(dependency.to_owned());
            }
        }
    }
    deps.sort();
    deps.dedup();

    Ok(Crate { name, licence, deps })
}

/// Every licence problem in `crates`, as human-readable lines: an undeclared or unrecognised
/// licence, or a permissive crate that reaches a copyleft one (reported with the path that gets
/// there, since the offending edge is rarely the direct one).
fn problems(crates: &BTreeMap<String, Crate>) -> Vec<String> {
    let mut problems = Vec::new();
    for entry in crates.values() {
        match entry.licence.as_deref() {
            None => problems.push(format!(
                "{}: declares no licence — every crate must pick a side of the split (ADR 0034 §1)",
                entry.name
            )),
            Some(PERMISSIVE) => {
                if let Some(path) = copyleft_reachable_from(crates, &entry.name) {
                    problems.push(format!(
                        "{} is {PERMISSIVE} but reaches {COPYLEFT} code: {}",
                        entry.name,
                        path.join(" -> ")
                    ));
                }
            }
            Some(COPYLEFT) => {}
            Some(other) => problems.push(format!(
                "{}: licence {other:?} is neither {PERMISSIVE:?} nor {COPYLEFT:?} — the split has no case for it",
                entry.name
            )),
        }
    }
    problems
}

/// The shortest dependency path from `root` to a copyleft crate, or `None` if it reaches none.
///
/// Breadth-first so the reported path is the shortest one; a dependency naming a crate outside
/// `crates` (an external one) is simply not traversed.
fn copyleft_reachable_from(crates: &BTreeMap<String, Crate>, root: &str) -> Option<Vec<String>> {
    let mut came_from: BTreeMap<&str, &str> = BTreeMap::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    seen.insert(root);
    queue.push_back(root);

    while let Some(name) = queue.pop_front() {
        let entry = crates.get(name)?;
        if name != root && entry.licence.as_deref() == Some(COPYLEFT) {
            return Some(trace(&came_from, root, name));
        }
        for dep in &entry.deps {
            if !crates.contains_key(dep.as_str()) || !seen.insert(dep.as_str()) {
                continue;
            }
            let Some((dep_key, _)) = crates.get_key_value(dep.as_str()) else {
                continue;
            };
            came_from.insert(dep_key.as_str(), name);
            queue.push_back(dep_key.as_str());
        }
    }
    None
}

/// Rebuilds the `root -> … -> target` path from a breadth-first search's predecessor map.
fn trace(came_from: &BTreeMap<&str, &str>, root: &str, target: &str) -> Vec<String> {
    let mut path = vec![target.to_owned()];
    let mut node = target;
    while node != root {
        let Some(previous) = came_from.get(node) else {
            break;
        };
        path.push((*previous).to_owned());
        node = previous;
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crate_of(name: &str, licence: &str, deps: &[&str]) -> (String, Crate) {
        (
            name.to_owned(),
            Crate {
                name: name.to_owned(),
                licence: Some(licence.to_owned()),
                deps: deps.iter().map(|dep| (*dep).to_owned()).collect(),
            },
        )
    }

    fn graph(entries: Vec<(String, Crate)>) -> BTreeMap<String, Crate> {
        entries.into_iter().collect()
    }

    #[test]
    fn a_permissive_crate_reaching_copyleft_is_reported_with_its_path() {
        let crates = graph(vec![
            crate_of("vitni-gedcom", PERMISSIVE, &["vitni-interchange"]),
            crate_of("vitni-interchange", PERMISSIVE, &["vitni-core"]),
            crate_of("vitni-core", COPYLEFT, &[]),
        ]);

        let problems = problems(&crates);

        // Both permissive crates are reported, not only the one holding the offending edge: each
        // one's own declaration is false, and which edge to cut is the reader's call.
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(
            problems[0].contains("vitni-gedcom -> vitni-interchange -> vitni-core"),
            "{}",
            problems[0]
        );
        assert!(
            problems[1].contains("vitni-interchange -> vitni-core"),
            "{}",
            problems[1]
        );
    }

    #[test]
    fn the_shipped_direction_permissive_into_copyleft_is_fine() {
        let crates = graph(vec![
            crate_of("vitni-app", COPYLEFT, &["vitni-core", "vitni-gedcom"]),
            crate_of("vitni-core", COPYLEFT, &[]),
            crate_of("vitni-gedcom", PERMISSIVE, &["vitni-interchange"]),
            crate_of("vitni-interchange", PERMISSIVE, &[]),
        ]);

        assert!(problems(&crates).is_empty());
    }

    #[test]
    fn an_undeclared_licence_is_reported() {
        let mut crates = graph(vec![crate_of("vitni-plugin-api", PERMISSIVE, &[])]);
        crates
            .get_mut("vitni-plugin-api")
            .expect("the crate was just inserted")
            .licence = None;

        let problems = problems(&crates);

        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("declares no licence"), "{}", problems[0]);
    }

    #[test]
    fn a_licence_outside_the_split_is_reported() {
        let crates = graph(vec![crate_of("vitni-core", "GPL-2.0-only", &[])]);

        let problems = problems(&crates);

        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("the split has no case for it"), "{}", problems[0]);
    }

    #[test]
    fn a_dependency_cycle_does_not_hang_the_walk() {
        let crates = graph(vec![
            crate_of("a", PERMISSIVE, &["b"]),
            crate_of("b", PERMISSIVE, &["a"]),
        ]);

        assert!(problems(&crates).is_empty());
    }

    #[test]
    fn an_external_dependency_is_not_traversed() {
        let crates = graph(vec![crate_of("vitni-i18n", PERMISSIVE, &["i18n-embed"])]);

        assert!(problems(&crates).is_empty());
    }

    #[test]
    fn a_manifest_inherits_the_workspace_licence_and_resolves_both_dependency_forms() {
        let manifest: toml::Value = toml::from_str(
            r#"
            [package]
            name = "vitni-gedcom"
            license.workspace = true

            [dependencies]
            vitni-interchange = { workspace = true }
            quick-xml = { workspace = true }
            local-thing = { path = "../local-thing" }
            renamed = { path = "../other", package = "vitni-other" }
            "#,
        )
        .expect("the fixture manifest parses");
        let workspace_paths = ["vitni-interchange".to_owned()].into_iter().collect();

        let parsed = parse_crate(&manifest, PERMISSIVE, &workspace_paths).expect("the fixture manifest is well-formed");

        assert_eq!(parsed.name, "vitni-gedcom");
        assert_eq!(parsed.licence.as_deref(), Some(PERMISSIVE));
        assert_eq!(parsed.deps, ["local-thing", "vitni-interchange", "vitni-other"]);
    }

    #[test]
    fn an_explicit_licence_overrides_the_workspace_default_and_dev_dependencies_are_ignored() {
        let manifest: toml::Value = toml::from_str(
            r#"
            [package]
            name = "vitni-core"
            license = "AGPL-3.0-or-later"

            [dev-dependencies]
            vitni-db = { workspace = true }
            "#,
        )
        .expect("the fixture manifest parses");
        let workspace_paths = ["vitni-db".to_owned()].into_iter().collect();

        let parsed = parse_crate(&manifest, PERMISSIVE, &workspace_paths).expect("the fixture manifest is well-formed");

        assert_eq!(parsed.licence.as_deref(), Some(COPYLEFT));
        assert!(parsed.deps.is_empty());
    }

    #[test]
    fn the_workspace_licence_and_its_path_dependencies_are_read_from_the_root_manifest() {
        let root: toml::Value = toml::from_str(
            r#"
            [workspace.package]
            license = "MIT OR Apache-2.0"

            [workspace.dependencies]
            vitni-core = { path = "crates/vitni-core" }
            serde = "1"
            "#,
        )
        .expect("the fixture root manifest parses");

        assert_eq!(workspace_licence(&root).expect("a licence is declared"), PERMISSIVE);
        assert_eq!(
            workspace_path_dependencies(&root),
            ["vitni-core".to_owned()].into_iter().collect()
        );
    }
}
