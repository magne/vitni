//! A minimal GEDCOM reader for the spike subset: `INDI` (with `NAME`) and `FAM` (`HUSB`/`WIFE`/
//! `CHIL`). Unknown tags are skipped, so a richer file still imports its persons and families.

use crate::model::{Family, Individual, Sex, Tree};

/// A GEDCOM parse failure.
#[derive(Debug, thiserror::Error)]
pub enum GedcomError {
    /// A line did not start with a numeric level.
    #[error("line {line}: missing or invalid level: {text:?}")]
    InvalidLevel {
        /// The 1-based line number.
        line: usize,
        /// The offending line text.
        text: String,
    },
}

/// The record a level-1 line currently applies to.
enum Current {
    Individual(usize),
    Family(usize),
    Other,
}

/// Parses GEDCOM `text` into a [`Tree`], skipping tags outside the spike subset.
///
/// # Errors
/// [`GedcomError::InvalidLevel`] if a non-empty line has no numeric level.
pub fn parse(text: &str) -> Result<Tree, GedcomError> {
    let mut tree = Tree::default();
    let mut current = Current::Other;

    // Many exports prepend a UTF-8 byte-order mark; strip it so the first line parses.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let parsed = parse_line(line).ok_or_else(|| GedcomError::InvalidLevel {
            line: index + 1,
            text: line.to_owned(),
        })?;

        if parsed.level == 0 {
            current = begin_record(&mut tree, &parsed);
        } else {
            apply_subrecord(&mut tree, &current, &parsed);
        }
    }

    Ok(tree)
}

/// Starts a new level-0 record, returning what subsequent level-1 lines apply to.
fn begin_record(tree: &mut Tree, parsed: &Line<'_>) -> Current {
    match (parsed.xref, parsed.tag) {
        (Some(xref), "INDI") => {
            tree.individuals.push(Individual {
                xref: xref.to_owned(),
                uid: None,
                given: None,
                surname: None,
                sex: None,
            });
            Current::Individual(tree.individuals.len() - 1)
        }
        (Some(xref), "FAM") => {
            tree.families.push(Family {
                xref: xref.to_owned(),
                uid: None,
                partners: Vec::new(),
                children: Vec::new(),
            });
            Current::Family(tree.families.len() - 1)
        }
        _ => Current::Other,
    }
}

/// Applies a level-1 line to the current record.
fn apply_subrecord(tree: &mut Tree, current: &Current, parsed: &Line<'_>) {
    match current {
        Current::Individual(index) => {
            let Some(individual) = tree.individuals.get_mut(*index) else {
                return;
            };
            match parsed.tag {
                "NAME" => {
                    let (given, surname) = parse_name(parsed.value);
                    individual.given = given;
                    individual.surname = surname;
                }
                "SEX" => individual.sex = Some(parse_sex(parsed.value)),
                "_UID" => individual.uid = non_empty(parsed.value),
                _ => {}
            }
        }
        Current::Family(index) => {
            let Some(family) = tree.families.get_mut(*index) else {
                return;
            };
            match parsed.tag {
                "HUSB" | "WIFE" => {
                    if let Some(xref) = unwrap_xref(parsed.value) {
                        family.partners.push(xref.to_owned());
                    }
                }
                "CHIL" => {
                    if let Some(xref) = unwrap_xref(parsed.value) {
                        family.children.push(xref.to_owned());
                    }
                }
                "_UID" => family.uid = non_empty(parsed.value),
                _ => {}
            }
        }
        Current::Other => {}
    }
}

/// One decomposed GEDCOM line: `LEVEL [@XREF@] TAG [VALUE]`.
struct Line<'a> {
    level: u8,
    xref: Option<&'a str>,
    tag: &'a str,
    value: &'a str,
}

/// Decomposes a non-empty GEDCOM line; returns `None` if the level is not numeric.
fn parse_line(line: &str) -> Option<Line<'_>> {
    let (level_text, rest) = line.split_once(' ').unwrap_or((line, ""));
    let level: u8 = level_text.parse().ok()?;

    if let Some(after_at) = rest.strip_prefix('@') {
        let (xref, tag_value) = after_at.split_once('@')?;
        let (tag, value) = split_tag_value(tag_value.trim_start());
        return Some(Line {
            level,
            xref: Some(xref),
            tag,
            value,
        });
    }

    let (tag, value) = split_tag_value(rest);
    Some(Line {
        level,
        xref: None,
        tag,
        value,
    })
}

/// Splits a tag from its optional value (the first whitespace-delimited token is the tag).
fn split_tag_value(text: &str) -> (&str, &str) {
    match text.split_once(' ') {
        Some((tag, value)) => (tag, value.trim()),
        None => (text, ""),
    }
}

/// Extracts an xref from a pointer value like `@I0001@`.
fn unwrap_xref(value: &str) -> Option<&str> {
    value.strip_prefix('@')?.strip_suffix('@')
}

/// Parses a `SEX` value (`M`/`F`, else unknown).
fn parse_sex(value: &str) -> Sex {
    match value.trim() {
        "M" => Sex::Male,
        "F" => Sex::Female,
        _ => Sex::Unknown,
    }
}

/// Parses a `NAME` value (`Given /Surname/`) into its given and surname parts.
fn parse_name(value: &str) -> (Option<String>, Option<String>) {
    let Some(slash) = value.find('/') else {
        return (non_empty(value), None);
    };
    let given = &value[..slash];
    let rest = &value[slash + 1..];
    let surname = match rest.find('/') {
        Some(end) => &rest[..end],
        None => rest,
    };
    (non_empty(given), non_empty(surname))
}

/// Trims `text`, returning `None` if it is empty.
fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
