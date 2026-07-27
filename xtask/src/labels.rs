//! `cargo xtask labels` — reconcile GitHub's issue labels with `.github/labels.toml`.
//!
//! The taxonomy lives in the repo so it is reviewable and cannot drift silently
//! (`docs/issue-tracking.md` §2). This command reports the difference and, with `--apply`, creates
//! missing labels and updates ones whose colour or description has drifted.
//!
//! **Never deletes.** A label on GitHub but absent from the file is reported as `extra` and left
//! alone: GitHub's defaults are deliberately reused, and Dependabot applies some of them itself.
//! Deleting a label silently strips it from every issue that carries it, which is not a side effect a
//! reconcile command should have.
//!
//! Requires the `gh` CLI, authenticated. Dry-run by default — `--apply` is the only mutating path.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// The declared taxonomy, relative to the repo root (`cargo xtask` runs there).
const LABEL_FILE: &str = ".github/labels.toml";

/// One label as declared in `.github/labels.toml`.
#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Label {
    name: String,
    color: String,
    #[serde(default)]
    description: String,
}

/// The `.github/labels.toml` document: a flat `[[label]]` array.
#[derive(Debug, Deserialize)]
struct LabelFile {
    label: Vec<Label>,
}

/// What reconciling one declared label against GitHub would do.
#[derive(Debug, PartialEq, Eq)]
enum Action {
    /// Absent on GitHub.
    Create,
    /// Present but the colour or description differs.
    Update,
    /// Already matches.
    Unchanged,
}

pub fn run() -> Result<()> {
    let apply = std::env::args().any(|arg| arg == "--apply");
    let declared = load_declared()?;
    let live = fetch_live()?;
    let plan = plan(&declared, &live);

    let mut created = 0_usize;
    let mut updated = 0_usize;
    for (label, action) in &plan {
        match action {
            Action::Create => {
                println!("create  {}", label.name);
                if apply {
                    gh_label(label, false)?;
                }
                created += 1;
            }
            Action::Update => {
                println!("update  {}", label.name);
                if apply {
                    gh_label(label, true)?;
                }
                updated += 1;
            }
            Action::Unchanged => {}
        }
    }

    let mut extra: Vec<&String> = live
        .keys()
        .filter(|name| !declared.iter().any(|label| &&label.name == name))
        .collect();
    extra.sort();
    for name in &extra {
        println!("extra   {name} (left alone)");
    }

    println!(
        "\nlabels: {} declared, {created} to create, {updated} to update, {} extra on GitHub.",
        declared.len(),
        extra.len()
    );
    if !apply && (created > 0 || updated > 0) {
        println!("Dry run — re-run with `--apply` to make these changes.");
    }
    Ok(())
}

/// Parses `.github/labels.toml`, rejecting a duplicate name (which would make the plan ambiguous).
fn load_declared() -> Result<Vec<Label>> {
    let path = Path::new(LABEL_FILE);
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: LabelFile = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;

    let mut seen = BTreeSet::new();
    for label in &parsed.label {
        if !seen.insert(label.name.clone()) {
            bail!("{} declares `{}` twice", path.display(), label.name);
        }
    }
    Ok(parsed.label)
}

/// The labels currently on GitHub, keyed by name, as `(colour, description)`.
fn fetch_live() -> Result<BTreeMap<String, (String, String)>> {
    let output = Command::new("gh")
        .args(["label", "list", "--limit", "200", "--json", "name,color,description"])
        .output()
        .context("running `gh label list` (is the gh CLI installed and authenticated?)")?;
    if !output.status.success() {
        bail!(
            "`gh label list` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(parse_live(&String::from_utf8_lossy(&output.stdout)))
}

/// Parses `gh label list --json name,color,description` output.
///
/// Hand-rolled rather than pulling in `serde_json`: the shape is three known string fields, and
/// `xtask` deliberately carries almost no dependencies.
fn parse_live(json: &str) -> BTreeMap<String, (String, String)> {
    let mut live = BTreeMap::new();
    for object in json.split('{').skip(1) {
        let object = object.split('}').next().unwrap_or_default();
        let name = json_field(object, "name");
        if name.is_empty() {
            continue;
        }
        live.insert(name, (json_field(object, "color"), json_field(object, "description")));
    }
    live
}

/// Extracts one string field's value from a flat JSON object body, unescaping `\"` and `\\`.
fn json_field(object: &str, field: &str) -> String {
    let needle = format!("\"{field}\":\"");
    let Some(start) = object.find(&needle) else {
        return String::new();
    };
    let rest = &object[start + needle.len()..];
    let mut value = String::new();
    let mut chars = rest.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => break,
            '\\' => value.push(chars.next().unwrap_or('\\')),
            other => value.push(other),
        }
    }
    value
}

/// Pairs each declared label with the action that would bring GitHub in line with it.
fn plan<'a>(declared: &'a [Label], live: &BTreeMap<String, (String, String)>) -> Vec<(&'a Label, Action)> {
    let mut plan = Vec::new();
    for label in declared {
        let action = match live.get(&label.name) {
            None => Action::Create,
            Some((color, description)) => {
                // GitHub reports colours without the leading `#`, case-insensitively.
                let same = color.eq_ignore_ascii_case(&label.color) && description == &label.description;
                if same { Action::Unchanged } else { Action::Update }
            }
        };
        plan.push((label, action));
    }
    plan
}

/// Creates or updates one label through `gh`.
fn gh_label(label: &Label, exists: bool) -> Result<()> {
    let mut command = Command::new("gh");
    command.args([
        "label",
        "create",
        &label.name,
        "--color",
        &label.color,
        "--description",
        &label.description,
    ]);
    if exists {
        // `--force` turns create into upsert; `gh label edit` cannot create.
        command.arg("--force");
    }
    let output = command.output().context("running `gh label create`")?;
    if !output.status.success() {
        bail!(
            "`gh label create {}` failed: {}",
            label.name,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Action, Label, json_field, parse_live, plan};

    fn label(name: &str, color: &str, description: &str) -> Label {
        Label {
            name: name.to_owned(),
            color: color.to_owned(),
            description: description.to_owned(),
        }
    }

    #[test]
    fn a_label_absent_from_github_is_created() {
        let declared = vec![label("area/records/tags", "1d76db", "Tag aggregate")];
        let live = parse_live("[]");
        assert_eq!(plan(&declared, &live)[0].1, Action::Create);
    }

    #[test]
    fn a_matching_label_is_unchanged() {
        let declared = vec![label("type/bug", "d73a4a", "Incorrect behavior")];
        let live = parse_live(r#"[{"name":"type/bug","color":"d73a4a","description":"Incorrect behavior"}]"#);
        assert_eq!(plan(&declared, &live)[0].1, Action::Unchanged);
    }

    #[test]
    fn a_colour_case_difference_is_not_a_change() {
        let declared = vec![label("type/bug", "D73A4A", "Incorrect behavior")];
        let live = parse_live(r#"[{"name":"type/bug","color":"d73a4a","description":"Incorrect behavior"}]"#);
        assert_eq!(
            plan(&declared, &live)[0].1,
            Action::Unchanged,
            "GitHub reports colours lowercase; a case difference is not drift"
        );
    }

    #[test]
    fn a_drifted_description_is_updated() {
        let declared = vec![label("type/bug", "d73a4a", "Incorrect behavior")];
        let live = parse_live(r#"[{"name":"type/bug","color":"d73a4a","description":"stale"}]"#);
        assert_eq!(plan(&declared, &live)[0].1, Action::Update);
    }

    #[test]
    fn an_escaped_quote_in_a_description_round_trips() {
        let live = parse_live(r#"[{"name":"a","color":"fff","description":"say \"hi\""}]"#);
        assert_eq!(live["a"].1, r#"say "hi""#);
    }

    #[test]
    fn a_missing_field_yields_an_empty_string() {
        assert_eq!(json_field(r#""name":"a""#, "description"), "");
    }

    #[test]
    fn a_label_with_no_description_parses() {
        let live = parse_live(r#"[{"name":"a","color":"fff","description":""}]"#);
        assert_eq!(live["a"], ("fff".to_owned(), String::new()));
    }
}
