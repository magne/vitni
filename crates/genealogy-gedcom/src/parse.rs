//! A minimal GEDCOM reader for the spike subset: `INDI` (with `NAME`) and `FAM` (`HUSB`/`WIFE`/
//! `CHIL`). Unknown tags are skipped, so a richer file still imports its persons and families.

use crate::model::{Date, Event, EventKind, Family, Individual, Sex, Tree};

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
    // The level-1 event the current level-2 lines (DATE/PLAC) belong to, as an index into the
    // current record's `events`. Cleared by the next level-1 line or a new level-0 record.
    let mut event: Option<usize> = None;

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
                event = None;
            }
            1 => event = apply_level1(&mut tree, &current, &parsed),
            _ => apply_level2(&mut tree, &current, event, &parsed),
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
        _ => Current::Other,
    }
}

/// Applies a level-1 line to the current record. Returns the index of a newly-opened event (so its
/// level-2 `DATE`/`PLAC` can attach), or `None` for a non-event tag.
fn apply_level1(tree: &mut Tree, current: &Current, parsed: &Line<'_>) -> Option<usize> {
    match current {
        Current::Individual(index) => {
            let individual = tree.individuals.get_mut(*index)?;
            if let Some(kind) = individual_event_kind(parsed.tag) {
                individual.events.push(Event {
                    kind,
                    date: None,
                    place: None,
                });
                return Some(individual.events.len() - 1);
            }
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
            None
        }
        Current::Family(index) => {
            let family = tree.families.get_mut(*index)?;
            if let Some(kind) = family_event_kind(parsed.tag) {
                family.events.push(Event {
                    kind,
                    date: None,
                    place: None,
                });
                return Some(family.events.len() - 1);
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
            None
        }
        Current::Other => None,
    }
}

/// Applies a level-2 line (`DATE`/`PLAC`) to the current event, if one is open.
fn apply_level2(tree: &mut Tree, current: &Current, event: Option<usize>, parsed: &Line<'_>) {
    let Some(event_index) = event else { return };
    let events = match current {
        Current::Individual(index) => tree.individuals.get_mut(*index).map(|i| &mut i.events),
        Current::Family(index) => tree.families.get_mut(*index).map(|f| &mut f.events),
        Current::Other => None,
    };
    let Some(event) = events.and_then(|events| events.get_mut(event_index)) else {
        return;
    };
    match parsed.tag {
        "DATE" => event.date = parse_date(parsed.value),
        "PLAC" => event.place = non_empty(parsed.value),
        _ => {}
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
