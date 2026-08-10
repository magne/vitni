//! `issue-sync` — guards the `docs/issues.md` ↔ GitHub Issues linkage (`docs/issue-tracking.md` §5).
//!
//! Sync runs one way: the doc is the backlog of record, an issue is a working copy of one bullet, and
//! a filed bullet carries its number as a trailing `— #142`. This check keeps that linkage honest.
//!
//! Two modes, deliberately split by what they cost:
//!
//! - **Offline** (default; runs in `cargo xtask check`, so prek and CI cover it): validate the doc's
//!   own invariants — every issue reference is well-formed, no number is claimed by two bullets, and
//!   every bullet under a backlog H2 sits inside an `###` area so it has an `area/*` label to inherit.
//!   `## Bugs` is a pointer section rather than a backlog H2, so a bullet there is reported as
//!   misplaced — it belongs under the `###` area it affects. No network, no token.
//! - **Online** (`--online`): additionally reconcile against `gh issue list` and report drift in both
//!   directions — a bullet pointing at a closed issue, an open issue whose bullet is gone, an open
//!   issue with no bullet at all. Needs `gh` authenticated, so it is a scheduled/manual job rather
//!   than a per-commit gate.
//!
//! Drift is *reported*, never auto-fixed: which side is wrong is a judgement call. A bullet pointing
//! at a closed issue might mean the work landed and the bullet should go, or that the issue was closed
//! by mistake.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The backlog, relative to the repo root (`cargo xtask` runs there).
const ISSUES_DOC: &str = "docs/issues.md";
/// H2 sections that hold decisions rather than work — bullets there are never filed.
const NON_BACKLOG_H2: [&str; 1] = ["Decided — no action needed"];
/// H2 sections that hold prose pointing elsewhere — a bullet there is misplaced, not arealess.
const POINTER_H2: [&str; 1] = ["Bugs"];

/// One backlog bullet: where it sits and which issue (if any) it claims.
#[derive(Debug, PartialEq, Eq)]
pub struct Bullet {
    /// 1-based line number in the doc.
    pub line: usize,
    /// The enclosing `##` section.
    pub section: String,
    /// The enclosing `###` area, if any.
    pub area: Option<String>,
    /// The bold title.
    pub title: String,
    /// The issue number from a trailing `— #N`, if filed.
    pub issue: Option<u32>,
}

pub fn run() -> Result<()> {
    let online = std::env::args().any(|arg| arg == "--online");
    let path = Path::new(ISSUES_DOC);
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let bullets = parse(&text);

    let mut problems = offline_problems(&bullets);
    let filed: BTreeMap<u32, &Bullet> = bullets
        .iter()
        .filter_map(|bullet| bullet.issue.map(|number| (number, bullet)))
        .collect();

    println!(
        "issue-sync: {} bullets, {} filed, {} unfiled.",
        bullets.len(),
        filed.len(),
        bullets.len() - filed.len()
    );

    if online {
        problems.extend(online_problems(&filed)?);
    } else {
        println!("Offline mode — re-run with `--online` to reconcile against GitHub.");
    }

    if problems.is_empty() {
        println!("issue-sync: no drift.");
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  {problem}");
    }
    bail!("issue-sync: {} problem(s)", problems.len());
}

/// Parses every `- **Title**` bullet, tracking the enclosing `##`/`###` headings.
/// A bullet spans its `- **Title**` line plus any indented continuation lines, and the issue
/// reference sits at the very end of that block — the natural place to write it, since most bullets
/// are several lines of prose. So each bullet is finalized only once the next bullet or heading
/// starts (or the text ends), reading the reference off the block's last line.
#[must_use]
pub fn parse(text: &str) -> Vec<Bullet> {
    let mut bullets: Vec<Bullet> = Vec::new();
    let mut section = String::new();
    let mut area = None;
    // The last line of the currently-open bullet block, or `None` once that block is closed.
    // `Option` rather than a plain `String` so closing *consumes* it: a bullet followed by a blank
    // line and then a heading would otherwise be closed twice, the second pass overwriting the
    // reference found by the first with `None`.
    let mut tail: Option<String> = None;

    for (index, line) in text.lines().enumerate() {
        if line.starts_with("  ") && !line.trim().is_empty() {
            if tail.is_some() {
                tail = Some(line.to_owned());
            }
            continue;
        }
        // Anything that is not a continuation ends the open bullet's block.
        close(&mut bullets, &mut tail);

        if let Some(rest) = line.strip_prefix("## ") {
            rest.trim().clone_into(&mut section);
            area = None;
        } else if let Some(rest) = line.strip_prefix("### ") {
            area = Some(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix("- **") {
            let Some(title) = rest.split("**").next() else {
                continue;
            };
            bullets.push(Bullet {
                line: index + 1,
                section: section.clone(),
                area: area.clone(),
                title: title.to_owned(),
                issue: None,
            });
            tail = Some(line.to_owned());
        }
    }
    close(&mut bullets, &mut tail);
    bullets
}

/// Applies the reference on a finished block's last line to the bullet that owns it.
///
/// Consumes `tail`, which makes a second call a no-op — needed because a bullet followed by a blank
/// line *and then* a heading reaches the end-of-block path twice, and the second pass would otherwise
/// overwrite the reference the first one found.
fn close(bullets: &mut [Bullet], tail: &mut Option<String>) {
    if let (Some(bullet), Some(last)) = (bullets.last_mut(), tail.take()) {
        bullet.issue = issue_ref(&last);
    }
}

/// Extracts a trailing `— #N` / `- #N` issue reference from the last line of a bullet's block.
#[must_use]
pub fn issue_ref(line: &str) -> Option<u32> {
    let hash = line.rfind(" #")?;
    let digits: String = line[hash + 2..].chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    // Only a reference at end-of-line counts; `#142` mid-sentence is prose, not a linkage.
    if line[hash + 2 + digits.len()..].trim().is_empty() {
        digits.parse().ok()
    } else {
        None
    }
}

/// The doc's own invariants — no network needed.
#[must_use]
pub fn offline_problems(bullets: &[Bullet]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut claimed: BTreeMap<u32, usize> = BTreeMap::new();

    for bullet in bullets {
        let decided = NON_BACKLOG_H2.contains(&bullet.section.as_str());

        if let Some(number) = bullet.issue {
            if let Some(first) = claimed.insert(number, bullet.line) {
                problems.push(format!(
                    "{ISSUES_DOC}:{}: issue #{number} is already claimed by the bullet on line {first}",
                    bullet.line
                ));
            }
            if decided {
                problems.push(format!(
                    "{ISSUES_DOC}:{}: \"{}\" is under *{}* but references #{number}; \
                     decisions are not filed as issues",
                    bullet.line, bullet.title, bullet.section
                ));
            }
        }

        if let Some(problem) = placement_problem(bullet, decided) {
            problems.push(problem);
        }
    }
    problems
}

/// Where a bullet sits: it belongs under an `###` area, and never under a pointer H2.
fn placement_problem(bullet: &Bullet, decided: bool) -> Option<String> {
    if decided || bullet.section.is_empty() || bullet.area.is_some() {
        return None;
    }
    if POINTER_H2.contains(&bullet.section.as_str()) {
        return Some(format!(
            "{ISSUES_DOC}:{}: \"{}\" is under `## {}`, which is a pointer section: \
             move it under the `###` area it affects, where it inherits that area/* label \
             plus type/bug",
            bullet.line, bullet.title, bullet.section
        ));
    }
    Some(format!(
        "{ISSUES_DOC}:{}: \"{}\" is directly under `## {}` with no `###` area, \
         so it has no area/* label to inherit",
        bullet.line, bullet.title, bullet.section
    ))
}

/// Reconciles filed bullets against GitHub in both directions.
fn online_problems(filed: &BTreeMap<u32, &Bullet>) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    let open = fetch_open_issues()?;

    for (number, bullet) in filed {
        if !open.contains_key(number) {
            problems.push(format!(
                "{ISSUES_DOC}:{}: \"{}\" references #{number}, which is not open \
                 (closed, or never existed) — delete the bullet or reopen the issue",
                bullet.line, bullet.title
            ));
        }
    }
    for (number, title) in &open {
        if !filed.contains_key(number) {
            problems.push(format!(
                "issue #{number} (\"{title}\") is open with no bullet in {ISSUES_DOC}"
            ));
        }
    }
    Ok(problems)
}

/// Open issue numbers and titles, via `gh`.
fn fetch_open_issues() -> Result<BTreeMap<u32, String>> {
    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--state",
            "open",
            "--limit",
            "500",
            "--json",
            "number,title",
        ])
        .output()
        .context("running `gh issue list` (is the gh CLI installed and authenticated?)")?;
    if !output.status.success() {
        bail!(
            "`gh issue list` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(parse_issue_list(&String::from_utf8_lossy(&output.stdout)))
}

/// Parses `gh issue list --json number,title`. Hand-rolled: `xtask` carries no JSON dependency.
#[must_use]
pub fn parse_issue_list(json: &str) -> BTreeMap<u32, String> {
    let mut issues = BTreeMap::new();
    for object in json.split('{').skip(1) {
        let object = object.split('}').next().unwrap_or_default();
        let Some(number) = field(object, "number").and_then(|value| value.parse().ok()) else {
            continue;
        };
        issues.insert(number, field(object, "title").unwrap_or_default());
    }
    issues
}

/// Extracts one field's value (string or number) from a flat JSON object body.
fn field(object: &str, name: &str) -> Option<String> {
    let start = object.find(&format!("\"{name}\":"))? + name.len() + 3;
    let rest = object[start..].trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        let mut value = String::new();
        let mut chars = quoted.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '"' => break,
                '\\' => value.push(chars.next().unwrap_or('\\')),
                other => value.push(other),
            }
        }
        Some(value)
    } else {
        Some(rest.chars().take_while(char::is_ascii_digit).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{Bullet, issue_ref, offline_problems, parse, parse_issue_list};

    #[test]
    fn a_trailing_reference_is_extracted() {
        assert_eq!(issue_ref("- **Thing** — some detail — #142"), Some(142));
    }

    #[test]
    fn a_mid_sentence_hash_is_not_a_reference() {
        assert_eq!(
            issue_ref("- **Thing** — as #142 showed, this is prose"),
            None,
            "only an end-of-line reference is a linkage"
        );
    }

    #[test]
    fn an_unfiled_bullet_has_no_reference() {
        assert_eq!(issue_ref("- **Thing** — not filed yet"), None);
    }

    #[test]
    fn a_bare_hash_with_no_digits_is_not_a_reference() {
        assert_eq!(issue_ref("- **Thing** — see #"), None);
    }

    #[test]
    fn headings_are_tracked_onto_each_bullet() {
        let doc = "## Records & data model\n### Places\n- **A thing** — detail\n";
        let bullets = parse(doc);
        assert_eq!(bullets.len(), 1);
        assert_eq!(bullets[0].section, "Records & data model");
        assert_eq!(bullets[0].area.as_deref(), Some("Places"));
        assert_eq!(bullets[0].title, "A thing");
        assert_eq!(bullets[0].line, 3);
    }

    #[test]
    fn a_reference_at_the_end_of_a_multi_line_bullet_is_found() {
        // The real doc writes the reference after several lines of prose, not on the title line.
        let doc = "## S\n### A\n- **Thing** — first line\n  more prose here\n  final line — #142\n";
        assert_eq!(parse(doc)[0].issue, Some(142));
    }

    #[test]
    fn a_reference_survives_a_blank_line_then_a_heading() {
        // Regression: an area's *last* bullet is followed by a blank line and then the next heading.
        // Closing the block on both of those would reset the reference found by the first pass.
        let doc = "## S\n### A\n- **Last in area** — x\n  detail — #195\n\n### B\n- **Other** — y\n";
        let bullets = parse(doc);
        assert_eq!(bullets[0].issue, Some(195));
        assert_eq!(bullets[1].issue, None);
    }

    #[test]
    fn a_reference_survives_end_of_file_after_a_blank_line() {
        let doc = "## S\n### A\n- **Only** — x\n  detail — #7\n\n";
        assert_eq!(parse(doc)[0].issue, Some(7));
    }

    #[test]
    fn a_reference_does_not_leak_onto_the_following_bullet() {
        let doc = "## S\n### A\n- **One** — x — #1\n- **Two** — y\n";
        let bullets = parse(doc);
        assert_eq!(bullets[0].issue, Some(1));
        assert_eq!(bullets[1].issue, None);
    }

    #[test]
    fn a_blank_line_ends_a_bullet_block() {
        let doc = "## S\n### A\n- **One** — x\n\n  #99 in a later paragraph\n";
        assert_eq!(
            parse(doc)[0].issue,
            None,
            "a blank line closes the block, so later prose is not the bullet's reference"
        );
    }

    #[test]
    fn a_new_h2_clears_the_area() {
        let doc = "## One\n### Area\n- **A** — x\n## Two\n- **B** — y\n";
        let bullets = parse(doc);
        assert_eq!(bullets[1].section, "Two");
        assert_eq!(bullets[1].area, None, "an H2 resets the area, it does not inherit");
    }

    #[test]
    fn a_duplicate_issue_reference_is_flagged() {
        let doc = "## S\n### A\n- **One** — x — #7\n- **Two** — y — #7\n";
        let problems = offline_problems(&parse(doc));
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("already claimed"), "{problems:?}");
    }

    #[test]
    fn a_bullet_with_no_area_is_flagged() {
        let doc = "## Records & data model\n- **Homeless** — x\n";
        let problems = offline_problems(&parse(doc));
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("no `###` area"), "{problems:?}");
    }

    #[test]
    fn a_decided_bullet_needs_no_area() {
        let doc = "## Decided — no action needed\n- **By design** — x\n";
        assert!(
            offline_problems(&parse(doc)).is_empty(),
            "decisions are not filed, so they need no area"
        );
    }

    #[test]
    fn a_bullet_under_bugs_is_told_to_move_to_its_area() {
        let doc = "## Bugs\n- **Something is broken.** Details here.\n";
        let problems = offline_problems(&parse(doc));
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("move it under the `###` area"), "{problems:?}");
        assert!(
            !problems[0].contains("label to inherit"),
            "the generic arealess text is the confusing one this replaces: {problems:?}"
        );
    }

    #[test]
    fn a_bug_bullet_under_its_area_is_clean() {
        let doc = "## Frontend & interaction\n### Geography & map\n- **Markers mislabel** — x — #232\n";
        assert!(
            offline_problems(&parse(doc)).is_empty(),
            "an open bug lives under the `###` area it affects"
        );
    }

    #[test]
    fn a_filed_bullet_under_bugs_is_still_flagged_once() {
        let doc = "## Bugs\n- **Something is broken** — detail — #7\n";
        let problems = offline_problems(&parse(doc));
        assert_eq!(problems.len(), 1, "the reference neither suppresses nor duplicates it");
        assert!(problems[0].contains("move it under the `###` area"), "{problems:?}");
    }

    #[test]
    fn a_filed_decided_bullet_is_flagged() {
        let doc = "## Decided — no action needed\n### Group\n- **By design** — x — #9\n";
        let problems = offline_problems(&parse(doc));
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("decisions are not filed"), "{problems:?}");
    }

    #[test]
    fn a_clean_doc_has_no_problems() {
        let doc = "## Records & data model\n### Places\n- **A** — x — #1\n- **B** — y\n";
        assert!(offline_problems(&parse(doc)).is_empty());
    }

    #[test]
    fn the_issue_list_json_is_parsed() {
        let json = r#"[{"number":12,"title":"Bulk export is CLI-only"},{"number":13,"title":"x"}]"#;
        let issues = parse_issue_list(json);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[&12], "Bulk export is CLI-only");
    }

    #[test]
    fn an_escaped_quote_in_a_title_round_trips() {
        let issues = parse_issue_list(r#"[{"number":1,"title":"the \"thing\""}]"#);
        assert_eq!(issues[&1], r#"the "thing""#);
    }

    #[test]
    fn an_empty_issue_list_parses() {
        assert!(parse_issue_list("[]").is_empty());
    }

    #[test]
    fn a_bullet_outside_any_section_is_not_flagged_for_area() {
        // Intro bullets above the first `##` (the doc's own conventions list) are not backlog items.
        let bullets = parse("- **Not an item** — intro prose\n");
        assert_eq!(bullets[0].section, "");
        assert!(offline_problems(&bullets).is_empty());
    }

    #[test]
    fn bullet_equality_covers_the_fields_the_check_reads() {
        let a = Bullet {
            line: 1,
            section: "S".into(),
            area: None,
            title: "T".into(),
            issue: None,
        };
        let b = Bullet {
            line: 1,
            section: "S".into(),
            area: None,
            title: "T".into(),
            issue: Some(1),
        };
        assert_ne!(a, b);
    }
}
