//! `adr-index` — guards `docs/adr/README.md`, the index of architecture decisions.
//!
//! The index is how a reader finds the ADR that bears on a change without opening all of them, so it
//! is only useful while it is complete: every `docs/adr/NNNN-*.md` must have exactly one row linking
//! it, and no row may link a file that does not exist. Its Relations column is only useful while both
//! ends agree: a row's `Supersedes 0009` needs `Superseded by` that row in 0009's row, and the reverse.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};

/// The ADR directory, relative to the repo root (`cargo xtask` runs there).
const ADR_DIR: &str = "docs/adr";
/// The index, inside [`ADR_DIR`].
const INDEX: &str = "README.md";
/// The Relations cell of an ADR that changes no other and is changed by none.
const NO_RELATIONS: &str = "—";

/// How a later ADR changes an earlier one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Supersedes,
    Amends,
    Extends,
}

/// Which end of a relation a row states: the ADR that makes the change, or the one it changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Changer,
    Changed,
}

/// Each clause opening the Relations column accepts, and what it states.
const VERBS: [(&str, Kind, End); 6] = [
    ("Supersedes ", Kind::Supersedes, End::Changer),
    ("Amends ", Kind::Amends, End::Changer),
    ("Extends ", Kind::Extends, End::Changer),
    ("Superseded by ", Kind::Supersedes, End::Changed),
    ("Amended by ", Kind::Amends, End::Changed),
    ("Extended by ", Kind::Extends, End::Changed),
];

/// One relation, as `(changer, kind, changed)` ADR numbers.
type Relation = (String, Kind, String);

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
    println!(
        "adr-index: {} ADRs, each indexed once, every relation stated at both ends.",
        files.len()
    );
    Ok(())
}

/// Whether `name` is an ADR file: four digits, a dash, and a `.md` extension.
fn is_adr(name: &str) -> bool {
    let Some((number, rest)) = name.split_once('-') else {
        return false;
    };
    let markdown = Path::new(rest).extension().is_some_and(|extension| extension == "md");
    is_number(number) && markdown
}

/// Whether `word` is an ADR number: exactly four digits.
fn is_number(word: &str) -> bool {
    word.len() == 4 && word.bytes().all(|byte| byte.is_ascii_digit())
}

/// One table row that links an ADR: the file it links, and its Relations cell when it has one.
struct Row<'a> {
    file: String,
    relations: Option<&'a str>,
}

/// The index's table rows that link an ADR file, in order.
fn rows(index: &str) -> Vec<Row<'_>> {
    let mut rows = Vec::new();
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
            let relations = row.split('|').nth(2).map(str::trim);
            rows.push(Row {
                file: target.to_owned(),
                relations,
            });
        }
    }
    rows
}

/// Every way the index disagrees with the ADR files on disk, or with itself.
#[must_use]
pub fn problems(files: &[String], index: &str) -> Vec<String> {
    let rows = rows(index);
    let mut found = coverage(files, &rows);
    found.extend(relation_problems(&rows));
    found
}

/// Every ADR file without exactly one row, and every row linking a file that does not exist.
fn coverage(files: &[String], rows: &[Row<'_>]) -> Vec<String> {
    let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
    for row in rows {
        *counts.entry(&row.file).or_default() += 1;
    }
    let mut found = Vec::new();
    let mut sorted = files.to_vec();
    sorted.sort();
    for file in &sorted {
        match counts.get(file.as_str()) {
            None => found.push(format!("{ADR_DIR}/{file} has no row in {ADR_DIR}/{INDEX} — add one")),
            Some(1) => {}
            Some(count) => found.push(format!("{ADR_DIR}/{file} has {count} rows in {ADR_DIR}/{INDEX}")),
        }
    }
    for link in counts.keys() {
        if !files.iter().any(|file| file == link) {
            found.push(format!("{ADR_DIR}/{INDEX} links {link}, which does not exist"));
        }
    }
    found
}

/// Every Relations cell that does not parse, names an ADR with no row, or lacks its inverse.
fn relation_problems(rows: &[Row<'_>]) -> Vec<String> {
    let numbers: BTreeSet<&str> = rows.iter().map(|row| &row.file[..4]).collect();
    let mut found = Vec::new();
    let mut by_changer = BTreeSet::new();
    let mut by_changed = BTreeSet::new();
    for row in rows {
        let number = &row.file[..4];
        let Some(cell) = row.relations else {
            found.push(format!(
                "{ADR_DIR}/{INDEX}: the row for {number} has no Relations column"
            ));
            continue;
        };
        for (end, relation) in parse_relations(number, cell, &mut found) {
            let other = match end {
                End::Changer => &relation.2,
                End::Changed => &relation.0,
            };
            if !numbers.contains(other.as_str()) {
                found.push(format!(
                    "{ADR_DIR}/{INDEX}: the row for {number} names {other}, which has no row"
                ));
            }
            match end {
                End::Changer => by_changer.insert(relation),
                End::Changed => by_changed.insert(relation),
            };
        }
    }
    for (changer, kind, changed) in by_changer.difference(&by_changed) {
        found.push(format!(
            "{ADR_DIR}/{INDEX}: {changer} {} {changed}, but the row for {changed} does not say {} {changer}",
            verb(*kind, End::Changer).trim_end(),
            verb(*kind, End::Changed).trim_end(),
        ));
    }
    for (changer, kind, changed) in by_changed.difference(&by_changer) {
        found.push(format!(
            "{ADR_DIR}/{INDEX}: {changed} is {} {changer}, but the row for {changer} does not say {} {changed}",
            verb(*kind, End::Changed).trim_end().to_lowercase(),
            verb(*kind, End::Changer).trim_end(),
        ));
    }
    found
}

/// The clause opening for `kind` seen from `end`, with its trailing space.
fn verb(kind: Kind, end: End) -> &'static str {
    let mut opening = "";
    for (text, verb_kind, verb_end) in VERBS {
        if verb_kind == kind && verb_end == end {
            opening = text;
        }
    }
    opening
}

/// The relations row `number`'s Relations `cell` states, each with the end it states; a clause that does
/// not parse is pushed onto `found` instead.
fn parse_relations(number: &str, cell: &str, found: &mut Vec<String>) -> Vec<(End, Relation)> {
    let mut relations = Vec::new();
    if cell == NO_RELATIONS {
        return relations;
    }
    for clause in cell.split(';').map(str::trim) {
        let Some((rest, kind, end)) = VERBS
            .iter()
            .find_map(|(text, kind, end)| clause.strip_prefix(text).map(|rest| (rest, *kind, *end)))
        else {
            found.push(format!(
                "{ADR_DIR}/{INDEX}: the row for {number} has the relation `{clause}`, which opens with none of {}",
                VERBS.map(|(text, _, _)| text.trim_end()).join(", "),
            ));
            continue;
        };
        let targets = targets(rest);
        if targets.is_empty() {
            found.push(format!(
                "{ADR_DIR}/{INDEX}: the row for {number} has the relation `{clause}`, naming no ADR"
            ));
        }
        for target in targets {
            let relation = match end {
                End::Changer => (number.to_owned(), kind, target),
                End::Changed => (target, kind, number.to_owned()),
            };
            relations.push((end, relation));
        }
    }
    relations
}

/// The ADR numbers a clause names: its words that are four digits, punctuation aside.
fn targets(clause: &str) -> Vec<String> {
    let mut numbers = Vec::new();
    for word in clause.split_whitespace() {
        let word = word.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        if is_number(word) {
            numbers.push(word.to_owned());
        }
    }
    numbers
}

#[cfg(test)]
mod tests {
    use super::{is_adr, problems};

    fn files() -> Vec<String> {
        vec!["0001-first.md".to_owned(), "0002-second.md".to_owned()]
    }

    const COMPLETE: &str = "\
| ADR | Decision | Relations | Keywords |
| --- | --- | --- | --- |
| [0001](0001-first.md) First | One. | Superseded by 0002 (§1: the rest stands) | a |
| [0002](0002-second.md) Second | Two. | Supersedes 0001 §1 | b |
";

    #[test]
    fn a_complete_index_has_no_problems() {
        assert_eq!(problems(&files(), COMPLETE), Vec::<String>::new());
    }

    #[test]
    fn an_index_without_relations_has_no_problems() {
        let index = COMPLETE
            .replace("Superseded by 0002 (§1: the rest stands)", "—")
            .replace("Supersedes 0001 §1", "—");
        assert_eq!(problems(&files(), &index), Vec::<String>::new());
    }

    #[test]
    fn an_adr_with_no_row_is_reported() {
        let index = COMPLETE
            .replace(
                "| [0002](0002-second.md) Second | Two. | Supersedes 0001 §1 | b |\n",
                "",
            )
            .replace("Superseded by 0002 (§1: the rest stands)", "—");
        let found = problems(&files(), &index);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("0002-second.md has no row"), "{found:?}");
    }

    #[test]
    fn a_row_linking_a_missing_file_is_reported() {
        let index = format!("{COMPLETE}| [0003](0003-third.md) Third | Three. | — | c |\n");
        let found = problems(&files(), &index);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("0003-third.md, which does not exist"), "{found:?}");
    }

    #[test]
    fn an_adr_indexed_twice_is_reported() {
        let index = format!("{COMPLETE}| [0001](0001-first.md) Again | One. | — | a |\n");
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
    fn a_relation_stated_only_by_the_later_adr_is_reported() {
        let index = COMPLETE.replace("Superseded by 0002 (§1: the rest stands)", "—");
        let found = problems(&files(), &index);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("0002 Supersedes 0001, but the row for 0001 does not say Superseded by 0002"),
            "{found:?}"
        );
    }

    #[test]
    fn a_relation_stated_only_by_the_earlier_adr_is_reported() {
        let index = COMPLETE.replace("Supersedes 0001 §1", "—");
        let found = problems(&files(), &index);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("0001 is superseded by 0002, but the row for 0002 does not say Supersedes 0001"),
            "{found:?}"
        );
    }

    #[test]
    fn inverses_of_a_different_kind_do_not_match() {
        let index = COMPLETE.replace("Supersedes 0001 §1", "Amends 0001 §1");
        let found = problems(&files(), &index);
        assert_eq!(found.len(), 2, "{found:?}");
    }

    #[test]
    fn an_unknown_verb_is_reported() {
        let index = COMPLETE.replace("Supersedes 0001 §1", "Replaces 0001 §1");
        let found = problems(&files(), &index);
        assert!(
            found
                .iter()
                .any(|problem| problem.contains("`Replaces 0001 §1`, which opens with none of")),
            "{found:?}"
        );
    }

    #[test]
    fn a_relation_naming_no_adr_is_reported() {
        let index = COMPLETE.replace("Supersedes 0001 §1", "Supersedes 0001 §1; Extends the data model");
        let found = problems(&files(), &index);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("`Extends the data model`, naming no ADR"),
            "{found:?}"
        );
    }

    #[test]
    fn a_relation_naming_an_adr_with_no_row_is_reported() {
        let index = COMPLETE.replace("Supersedes 0001 §1", "Supersedes 0001 §1; Extends 0009");
        let found = problems(&files(), &index);
        assert!(
            found
                .iter()
                .any(|problem| problem.contains("names 0009, which has no row")),
            "{found:?}"
        );
    }

    #[test]
    fn a_row_without_a_relations_column_is_reported() {
        let index = COMPLETE.replace(
            "| [0002](0002-second.md) Second | Two. | Supersedes 0001 §1 | b |",
            "| [0002](0002-second.md) Second",
        );
        let found = problems(&files(), &index);
        assert!(
            found
                .iter()
                .any(|problem| problem.contains("the row for 0002 has no Relations column")),
            "{found:?}"
        );
    }

    #[test]
    fn only_numbered_markdown_files_are_adrs() {
        assert!(is_adr("0042-test-fixture-provenance.md"));
        assert!(!is_adr("README.md"));
        assert!(!is_adr("042-short.md"));
        assert!(!is_adr("0042-notes.txt"));
    }
}
