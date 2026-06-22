//! Serializes the intermediate [`Tree`] back to the minimal GEDCOM subset.

use std::fmt::Write as _;

use crate::model::{Citation, Date, Event, EventKind, Sex, Tree};

/// The GEDCOM tags partners are emitted under, in order (first partner → `HUSB`, second → `WIFE`).
const PARTNER_TAGS: [&str; 2] = ["HUSB", "WIFE"];

/// Emits `tree` as a GEDCOM document (5.5-style header, the records, and a trailer).
#[must_use]
pub fn emit(tree: &Tree) -> String {
    let mut out = String::new();
    out.push_str("0 HEAD\n1 SOUR genealogy\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n");

    for individual in &tree.individuals {
        let _ = writeln!(out, "0 @{}@ INDI", individual.xref);
        if individual.given.is_some() || individual.surname.is_some() {
            let _ = writeln!(
                out,
                "1 NAME {}",
                name_value(individual.given.as_deref(), individual.surname.as_deref())
            );
        }
        if let Some(sex) = individual.sex {
            let _ = writeln!(out, "1 SEX {}", sex_value(sex));
        }
        if let Some(uid) = &individual.uid {
            let _ = writeln!(out, "1 _UID {uid}");
        }
        for event in &individual.events {
            emit_event(&mut out, event);
        }
        for citation in &individual.citations {
            emit_citation(&mut out, citation);
        }
    }

    for family in &tree.families {
        let _ = writeln!(out, "0 @{}@ FAM", family.xref);
        if let Some(uid) = &family.uid {
            let _ = writeln!(out, "1 _UID {uid}");
        }
        for (index, partner) in family.partners.iter().enumerate() {
            let tag = PARTNER_TAGS.get(index).copied().unwrap_or("HUSB");
            let _ = writeln!(out, "1 {tag} @{partner}@");
        }
        for child in &family.children {
            let _ = writeln!(out, "1 CHIL @{child}@");
        }
        for event in &family.events {
            emit_event(&mut out, event);
        }
    }

    for source in &tree.sources {
        let _ = writeln!(out, "0 @{}@ SOUR", source.xref);
        if let Some(title) = &source.title {
            let _ = writeln!(out, "1 TITL {title}");
        }
    }

    out.push_str("0 TRLR\n");
    out
}

/// Emits one citation (`1 SOUR @S..@`, then `2 PAGE` when present).
fn emit_citation(out: &mut String, citation: &Citation) {
    let _ = writeln!(out, "1 SOUR @{}@", citation.source_xref);
    if let Some(page) = &citation.page {
        let _ = writeln!(out, "2 PAGE {page}");
    }
}

/// Emits one event record (`1 TAG`, then `2 DATE`/`2 PLAC` when present).
fn emit_event(out: &mut String, event: &Event) {
    let _ = writeln!(out, "1 {}", event_tag(event.kind));
    if let Some(date) = event.date {
        let _ = writeln!(out, "2 DATE {}", date_value(date));
    }
    if let Some(place) = &event.place {
        let _ = writeln!(out, "2 PLAC {place}");
    }
}

/// The canonical GEDCOM tag for an event kind.
fn event_tag(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Birth => "BIRT",
        EventKind::Death => "DEAT",
        EventKind::Marriage => "MARR",
        EventKind::Baptism => "CHR",
        EventKind::Burial => "BURI",
        EventKind::Census => "CENS",
        EventKind::Residence => "RESI",
        EventKind::Immigration => "IMMI",
        EventKind::Emigration => "EMIG",
    }
}

/// Renders a `DATE` value: `[DAY] [MON] YEAR`, omitting absent parts.
fn date_value(date: Date) -> String {
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    let month = date
        .month
        .and_then(|m| MONTHS.get((m as usize).wrapping_sub(1)).copied());
    match (date.day, month) {
        (Some(day), Some(month)) => format!("{day} {month} {}", date.year),
        (None, Some(month)) => format!("{month} {}", date.year),
        _ => date.year.to_string(),
    }
}

/// Renders a GEDCOM `SEX` value.
fn sex_value(sex: Sex) -> &'static str {
    match sex {
        Sex::Male => "M",
        Sex::Female => "F",
        Sex::Unknown => "U",
    }
}

/// Renders a GEDCOM `NAME` value: `Given /Surname/`, omitting an absent part.
fn name_value(given: Option<&str>, surname: Option<&str>) -> String {
    match (given, surname) {
        (Some(given), Some(surname)) => format!("{given} /{surname}/"),
        (Some(given), None) => given.to_owned(),
        (None, Some(surname)) => format!("/{surname}/"),
        (None, None) => String::new(),
    }
}
