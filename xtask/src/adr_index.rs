//! `adr-index` — guards `docs/adr/README.md`, the index of architecture decisions.
//!
//! The index is how a reader finds the ADR that bears on a change without opening all of them, so it
//! is only useful while it is complete: every `docs/adr/NNNN-*.md` must have exactly one row linking
//! it, and no row may link a file that does not exist.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// The ADR directory, relative to the repo root (`cargo xtask` runs there).
const ADR_DIR: &str = "docs/adr";
/// The index, inside [`ADR_DIR`].
const INDEX: &str = "README.md";

pub fn run() -> Result<()> {
    let dir = Path::new(ADR_DIR);
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if is_adr(&name) {
            files.push(name);
        }
    }
    let index_path = dir.join(INDEX);
    let index = std::fs::read_to_string(&index_path).with_context(|| format!("reading {}", index_path.display()))?;
    let found = problems(&files, &index);
    if !found.is_empty() {
        for problem in &found {
            eprintln!("{problem}");
        }
        bail!("adr-index: {} problem(s) in {ADR_DIR}/{INDEX}", found.len());
    }
    println!("adr-index: {} ADRs, each indexed once.", files.len());
    Ok(())
}

/// Whether `name` is an ADR file: four digits, a dash, and a `.md` extension.
fn is_adr(name: &str) -> bool {
    let Some((number, rest)) = name.split_once('-') else {
        return false;
    };
    let markdown = Path::new(rest).extension().is_some_and(|extension| extension == "md");
    number.len() == 4 && number.bytes().all(|byte| byte.is_ascii_digit()) && markdown
}

/// The ADR files the index's table rows link, in order, one per row that links one.
fn indexed(index: &str) -> Vec<String> {
    let mut links = Vec::new();
    for line in index.lines() {
        let Some(row) = line.trim_start().strip_prefix('|') else {
            continue;
        };
        let Some(start) = row.find("](") else {
            continue;
        };
        let target = &row[start + 2..];
        let Some(end) = target.find(')') else {
            continue;
        };
        let target = &target[..end];
        if is_adr(target) {
            links.push(target.to_owned());
        }
    }
    links
}

/// Every way the index disagrees with the ADR files on disk.
#[must_use]
pub fn problems(files: &[String], index: &str) -> Vec<String> {
    let mut rows: BTreeMap<String, u32> = BTreeMap::new();
    for link in indexed(index) {
        *rows.entry(link).or_default() += 1;
    }
    let mut found = Vec::new();
    let mut sorted = files.to_vec();
    sorted.sort();
    for file in &sorted {
        match rows.get(file) {
            None => found.push(format!("{ADR_DIR}/{file} has no row in {ADR_DIR}/{INDEX} — add one")),
            Some(1) => {}
            Some(count) => found.push(format!("{ADR_DIR}/{file} has {count} rows in {ADR_DIR}/{INDEX}")),
        }
    }
    for link in rows.keys() {
        if !files.contains(link) {
            found.push(format!("{ADR_DIR}/{INDEX} links {link}, which does not exist"));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::{is_adr, problems};

    fn files() -> Vec<String> {
        vec!["0001-first.md".to_owned(), "0002-second.md".to_owned()]
    }

    const COMPLETE: &str = "\
| ADR | Decision | Keywords |
| --- | --- | --- |
| [0001](0001-first.md) First | One. | a |
| [0002](0002-second.md) Second | Two. | b |
";

    #[test]
    fn a_complete_index_has_no_problems() {
        assert_eq!(problems(&files(), COMPLETE), Vec::<String>::new());
    }

    #[test]
    fn an_adr_with_no_row_is_reported() {
        let index = COMPLETE.replace("| [0002](0002-second.md) Second | Two. | b |\n", "");
        let found = problems(&files(), &index);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("0002-second.md has no row"), "{found:?}");
    }

    #[test]
    fn a_row_linking_a_missing_file_is_reported() {
        let index = format!("{COMPLETE}| [0003](0003-third.md) Third | Three. | c |\n");
        let found = problems(&files(), &index);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("0003-third.md, which does not exist"), "{found:?}");
    }

    #[test]
    fn an_adr_indexed_twice_is_reported() {
        let index = format!("{COMPLETE}| [0001](0001-first.md) Again | One. | a |\n");
        let found = problems(&files(), &index);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("0001-first.md has 2 rows"), "{found:?}");
    }

    #[test]
    fn links_outside_table_rows_are_not_rows() {
        let index = format!("See [0001](0001-first.md) for the start.\n\n{COMPLETE}");
        assert_eq!(problems(&files(), &index), Vec::<String>::new());
    }

    #[test]
    fn only_numbered_markdown_files_are_adrs() {
        assert!(is_adr("0042-test-fixture-provenance.md"));
        assert!(!is_adr("README.md"));
        assert!(!is_adr("042-short.md"));
        assert!(!is_adr("0042-notes.txt"));
    }
}
