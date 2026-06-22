//! A minimal GEDCOM reader for the spike subset: `INDI` (with `NAME`) and `FAM` (`HUSB`/`WIFE`/
//! `CHIL`). Unknown tags are skipped, so a richer file still imports its persons and families.

use crate::model::{Citation, Date, Event, EventKind, Family, Individual, Sex, Source, Tree};

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
    Source(usize),
    Other,
}

/// The level-1 child the current level-2 lines belong to, within the current record.
enum Open {
    /// An event, by index into the record's `events`.
    Event(usize),
    /// A citation, by index into the individual's `citations`.
    Citation(usize),
    /// No open child (the level-1 line was a leaf).
    None,
}

/// Parses GEDCOM `text` into a [`Tree`], skipping tags outside the spike subset.
///
/// # Errors
/// [`GedcomError::InvalidLevel`] if a non-empty line has no numeric level.
pub fn parse(text: &str) -> Result<Tree, GedcomError> {
    let mut tree = Tree::default();
    let mut current = Current::Other;
    // The level-1 child the current level-2 lines belong to. Cleared by the next level-1 line or a
    // new level-0 record.
    let mut open = Open::None;

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

        match parsed.level {
            0 => {
                current = begin_record(&mut tree, &parsed);
                open = Open::None;
            }
            1 => open = apply_level1(&mut tree, &current, &parsed),
            _ => apply_level2(&mut tree, &current, &open, &parsed),
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
                events: Vec::new(),
                citations: Vec::new(),
            });
            Current::Individual(tree.individuals.len() - 1)
        }
        (Some(xref), "FAM") => {
            tree.families.push(Family {
                xref: xref.to_owned(),
                uid: None,
                partners: Vec::new(),
                children: Vec::new(),
                events: Vec::new(),
            });
            Current::Family(tree.families.len() - 1)
        }
        (Some(xref), "SOUR") => {
            tree.sources.push(Source {
                xref: xref.to_owned(),
                title: None,
            });
            Current::Source(tree.sources.len() - 1)
        }
        _ => Current::Other,
    }
}

/// Applies a level-1 line to the current record, returning the level-1 child it opened (an event or
/// a citation whose level-2 lines follow), or [`Open::None`] for a leaf.
fn apply_level1(tree: &mut Tree, current: &Current, parsed: &Line<'_>) -> Open {
    match current {
        Current::Individual(index) => {
            let Some(individual) = tree.individuals.get_mut(*index) else {
                return Open::None;
            };
            if let Some(kind) = individual_event_kind(parsed.tag) {
                individual.events.push(Event {
                    kind,
                    date: None,
                    place: None,
                });
                return Open::Event(individual.events.len() - 1);
            }
            match parsed.tag {
                "NAME" => {
                    let (given, surname) = parse_name(parsed.value);
                    individual.given = given;
                    individual.surname = surname;
                }
                "SEX" => individual.sex = Some(parse_sex(parsed.value)),
                "_UID" => individual.uid = non_empty(parsed.value),
                "SOUR" => {
                    if let Some(source_xref) = unwrap_xref(parsed.value) {
                        individual.citations.push(Citation {
                            source_xref: source_xref.to_owned(),
                            page: None,
                        });
                        return Open::Citation(individual.citations.len() - 1);
                    }
                }
                _ => {}
            }
            Open::None
        }
        Current::Family(index) => {
            let Some(family) = tree.families.get_mut(*index) else {
                return Open::None;
            };
            if let Some(kind) = family_event_kind(parsed.tag) {
                family.events.push(Event {
                    kind,
                    date: None,
                    place: None,
                });
                return Open::Event(family.events.len() - 1);
            }
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
            Open::None
        }
        Current::Source(index) => {
            if let Some(source) = tree.sources.get_mut(*index)
                && parsed.tag == "TITL"
            {
                source.title = non_empty(parsed.value);
            }
            Open::None
        }
        Current::Other => Open::None,
    }
}

/// Applies a level-2 line to the open level-1 child: `DATE`/`PLAC` for an event, `PAGE` for a
/// citation.
fn apply_level2(tree: &mut Tree, current: &Current, open: &Open, parsed: &Line<'_>) {
    match open {
        Open::Event(event_index) => {
            let events = match current {
                Current::Individual(index) => tree.individuals.get_mut(*index).map(|i| &mut i.events),
                Current::Family(index) => tree.families.get_mut(*index).map(|f| &mut f.events),
                Current::Source(_) | Current::Other => None,
            };
            let Some(event) = events.and_then(|events| events.get_mut(*event_index)) else {
                return;
            };
            match parsed.tag {
                "DATE" => event.date = parse_date(parsed.value),
                "PLAC" => event.place = non_empty(parsed.value),
                _ => {}
            }
        }
        Open::Citation(citation_index) => {
            let Current::Individual(index) = current else { return };
            let Some(citation) = tree
                .individuals
                .get_mut(*index)
                .and_then(|i| i.citations.get_mut(*citation_index))
            else {
                return;
            };
            if parsed.tag == "PAGE" {
                citation.page = non_empty(parsed.value);
            }
        }
        Open::None => {}
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

/// Maps an individual-level event tag to its kind, or `None` if the tag is not an event.
fn individual_event_kind(tag: &str) -> Option<EventKind> {
    match tag {
        "BIRT" => Some(EventKind::Birth),
        "DEAT" => Some(EventKind::Death),
        "CHR" | "BAPM" => Some(EventKind::Baptism),
        "BURI" => Some(EventKind::Burial),
        "CENS" => Some(EventKind::Census),
        "RESI" => Some(EventKind::Residence),
        "IMMI" => Some(EventKind::Immigration),
        "EMIG" => Some(EventKind::Emigration),
        _ => None,
    }
}

/// Maps a family-level event tag to its kind, or `None` if the tag is not an event.
fn family_event_kind(tag: &str) -> Option<EventKind> {
    match tag {
        "MARR" => Some(EventKind::Marriage),
        _ => None,
    }
}

/// Parses a GEDCOM `DATE` value into a [`Date`], best-effort: a leading modifier (`ABT`, `BEF`,
/// `AFT`, `BET`, `EST`, `CAL`, `ABT`, `FROM`, `TO`) is skipped, then `[DAY] [MON] YEAR` is read. The
/// year is required; an unparseable value yields `None`. (Full GEDCOM date grammar is a refinement.)
fn parse_date(value: &str) -> Option<Date> {
    let mut year = None;
    let mut month = None;
    let mut day = None;
    for token in value.split_whitespace() {
        if let Some(m) = month_number(token) {
            month = Some(m);
        } else if let Ok(number) = token.parse::<i32>() {
            match u8::try_from(number) {
                Ok(small) if (1..=31).contains(&small) && day.is_none() && year.is_none() => day = Some(small),
                _ => year = Some(number),
            }
        }
    }
    year.map(|year| Date { year, month, day })
}

/// Maps a GEDCOM month abbreviation to its number.
fn month_number(token: &str) -> Option<u8> {
    let months = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    months
        .iter()
        .position(|month| month.eq_ignore_ascii_case(token))
        .and_then(|index| u8::try_from(index).ok())
        .map(|index| index + 1)
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
