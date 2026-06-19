//! Serializes the intermediate [`Tree`] back to the minimal GEDCOM subset.

use std::fmt::Write as _;

use crate::model::Tree;

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
    }

    for family in &tree.families {
        let _ = writeln!(out, "0 @{}@ FAM", family.xref);
        for (index, partner) in family.partners.iter().enumerate() {
            let tag = PARTNER_TAGS.get(index).copied().unwrap_or("HUSB");
            let _ = writeln!(out, "1 {tag} @{partner}@");
        }
        for child in &family.children {
            let _ = writeln!(out, "1 CHIL @{child}@");
        }
    }

    out.push_str("0 TRLR\n");
    out
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
