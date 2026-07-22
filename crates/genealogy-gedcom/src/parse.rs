//! A GEDCOM reader: line stream → a generic node tree → the typed [`Tree`] model.
//!
//! GEDCOM is a line-oriented, level-nested format. We first build a small generic tree of
//! [`Node`]s (so arbitrarily-deep structures like `NAME`/`ADDR` sub-records and the full `DATE`
//! grammar are reachable), then interpret the `INDI`/`FAM`/`SOUR` records into the model. Unknown
//! tags are skipped, so a richer file still imports what we understand.

use genealogy_interchange::parse_age;

use crate::model::{
    Address, Association, AssociationKind, Calendar, ChildRef, Citation, Date, DateModifier, DatePoint, DateQuality,
    Event, EventAssociation, EventKind, Fact, FactKind, Family, Individual, MediaObject, Name, NameKind, Place,
    Restriction, Sex, Source, Tree,
};

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

/// One node in the generic GEDCOM tree: a tag, its value, and nested children.
#[derive(Debug, Default)]
struct Node {
    /// The record cross-reference id (only on level-0 records), e.g. `I0001`.
    xref: Option<String>,
    /// The tag (e.g. `INDI`, `NAME`, `DATE`).
    tag: String,
    /// The line value (text, or a `@pointer@`).
    value: String,
    /// Nested sub-records.
    children: Vec<Node>,
}

impl Node {
    /// The first child with `tag`, if any.
    fn child(&self, tag: &str) -> Option<&Node> {
        self.children.iter().find(|node| node.tag == tag)
    }

    /// The value of the first child with `tag`, trimmed and non-empty.
    fn child_value(&self, tag: &str) -> Option<String> {
        self.child(tag).and_then(|node| non_empty(&node.value))
    }

    /// The node's value plus any `CONT`/`CONC` continuation children joined as GEDCOM defines
    /// (`CONT` starts a new line, `CONC` concatenates).
    fn full_value(&self) -> String {
        let mut text = self.value.clone();
        for child in &self.children {
            match child.tag.as_str() {
                "CONT" => {
                    text.push('\n');
                    text.push_str(&child.value);
                }
                "CONC" => text.push_str(&child.value),
                _ => {}
            }
        }
        text
    }
}

/// Parses GEDCOM `text` into a [`Tree`].
///
/// # Errors
/// [`GedcomError::InvalidLevel`] if a non-empty line has no numeric level.
pub fn parse(text: &str) -> Result<Tree, GedcomError> {
    let forest = build_forest(text)?;
    let mut tree = Tree::default();
    for node in &forest {
        match node.tag.as_str() {
            "INDI" => tree.individuals.push(individual(node)),
            "FAM" => tree.families.push(family(node)),
            "SOUR" => tree.sources.push(source(node)),
            _ => {}
        }
    }
    Ok(tree)
}

/// Builds the generic node forest from the line stream, nesting by level.
fn build_forest(text: &str) -> Result<Vec<Node>, GedcomError> {
    // Many exports prepend a UTF-8 byte-order mark; strip it so the first line parses.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut roots: Vec<Node> = Vec::new();
    // `path` holds the index of the current node at each ancestor level, so the next line attaches
    // under the right parent.
    let mut path: Vec<usize> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let parsed = parse_line(line).ok_or_else(|| GedcomError::InvalidLevel {
            line: index + 1,
            text: line.to_owned(),
        })?;
        let depth = usize::from(parsed.level).min(path.len());
        path.truncate(depth);
        let siblings = children_at(&mut roots, &path);
        siblings.push(Node {
            xref: parsed.xref.map(str::to_owned),
            tag: parsed.tag.to_owned(),
            value: parsed.value.to_owned(),
            children: Vec::new(),
        });
        let pushed = siblings.len() - 1;
        path.push(pushed);
    }
    Ok(roots)
}

/// Navigates from `roots` along `path` to the children vector a new node should be appended to.
fn children_at<'a>(roots: &'a mut Vec<Node>, path: &[usize]) -> &'a mut Vec<Node> {
    let mut nodes = roots;
    for &index in path {
        nodes = &mut nodes[index].children;
    }
    nodes
}

/// Interprets an `INDI` record node.
fn individual(node: &Node) -> Individual {
    let mut individual = Individual {
        xref: node.xref.clone().unwrap_or_default(),
        ..Individual::default()
    };
    for child in &node.children {
        if let Some(kind) = event_kind(&child.tag) {
            individual.events.push(event(child, kind));
            continue;
        }
        if let Some(kind) = fact_kind(&child.tag) {
            individual.facts.push(Fact {
                kind,
                value: non_empty(&child.full_value()),
                date: child.child("DATE").and_then(|date| parse_date(&date.value)),
            });
            continue;
        }
        match child.tag.as_str() {
            "NAME" => individual.name = Some(name(child)),
            "SEX" => individual.sex = Some(parse_sex(&child.value)),
            "_UID" => individual.uid = non_empty(&child.value),
            "RESN" => individual.restrictions = parse_resn(&child.full_value()),
            "ASSO" => {
                if let Some(other_xref) = unwrap_xref(&child.value) {
                    individual.associations.push(Association {
                        other_xref: other_xref.to_owned(),
                        role: child.child_value("ROLE").map(|role| association_kind(&role)),
                    });
                }
            }
            "SOUR" => {
                if let Some(source_xref) = unwrap_xref(&child.value) {
                    individual.citations.push(Citation {
                        source_xref: source_xref.to_owned(),
                        page: child.child_value("PAGE"),
                    });
                }
            }
            "OBJE" => individual.media.push(MediaObject {
                file: child.child_value("FILE"),
                title: child.child_value("TITL"),
                mime: child.child_value("FORM"),
            }),
            "NOTE" => {
                if let Some(text) = non_empty(&child.full_value()) {
                    individual.notes.push(text);
                }
            }
            _ => {}
        }
    }
    individual
}

/// Interprets a `FAM` record node.
fn family(node: &Node) -> Family {
    let mut family = Family {
        xref: node.xref.clone().unwrap_or_default(),
        ..Family::default()
    };
    for child in &node.children {
        if let Some(kind) = event_kind(&child.tag) {
            family.events.push(event(child, kind));
            continue;
        }
        match child.tag.as_str() {
            "HUSB" | "WIFE" => {
                if let Some(xref) = unwrap_xref(&child.value) {
                    family.partners.push(xref.to_owned());
                }
            }
            "CHIL" => {
                if let Some(xref) = unwrap_xref(&child.value) {
                    family.children.push(ChildRef {
                        xref: xref.to_owned(),
                        father_relationship: child.child_value("_FREL"),
                        mother_relationship: child.child_value("_MREL"),
                    });
                }
            }
            "_UID" => family.uid = non_empty(&child.value),
            "RESN" => family.restrictions = parse_resn(&child.full_value()),
            _ => {}
        }
    }
    family
}

/// Parses a GEDCOM v7 `RESN` value (a comma-separated list of `CONFIDENTIAL`/`LOCKED`/`PRIVACY`)
/// into the restriction set, ignoring unrecognized tokens.
fn parse_resn(value: &str) -> Vec<Restriction> {
    let mut restrictions = Vec::new();
    for token in value.split(',') {
        let restriction = match token.trim().to_ascii_uppercase().as_str() {
            "CONFIDENTIAL" => Restriction::Confidential,
            "LOCKED" => Restriction::Locked,
            "PRIVACY" => Restriction::Privacy,
            _ => continue,
        };
        if !restrictions.contains(&restriction) {
            restrictions.push(restriction);
        }
    }
    restrictions
}

/// Interprets a top-level `SOUR` record node.
fn source(node: &Node) -> Source {
    Source {
        xref: node.xref.clone().unwrap_or_default(),
        title: node.child_value("TITL"),
        author: node.child_value("AUTH"),
        pub_info: node.child_value("PUBL"),
    }
}

/// Interprets an event node (`BIRT`/`DEAT`/`MARR`/…) into an [`Event`], reading the participant ages
/// (`2 AGE` on an `INDI` event, `HUSB`/`WIFE` `3 AGE` on a `FAM` event) and event-level `ASSO`
/// witnesses (data-model §17).
fn event(node: &Node, kind: EventKind) -> Event {
    Event {
        kind,
        date: node.child("DATE").and_then(|date| parse_date(&date.value)),
        place: place(node),
        address: address(node),
        age: node.child_value("AGE").and_then(|value| parse_age(&value)),
        husband_age: partner_age(node, "HUSB"),
        wife_age: partner_age(node, "WIFE"),
        associations: node
            .children
            .iter()
            .filter(|c| c.tag == "ASSO")
            .filter_map(event_association)
            .collect(),
    }
}

/// Interprets an event's `PLAC` node into a [`Place`]: its name, plus the point (`MAP.LATI`/`LONG`)
/// when present (ADR 0024 §4). A blank/absent `PLAC` yields `None`, matching `child_value`.
fn place(node: &Node) -> Option<Place> {
    let plac = node.child("PLAC")?;
    let name = non_empty(&plac.value)?;
    let map = plac.child("MAP");
    Some(Place {
        name,
        latitude: map.and_then(|m| m.child_value("LATI")),
        longitude: map.and_then(|m| m.child_value("LONG")),
    })
}

/// Reads a `FAM`-event partner's age (`HUSB`/`WIFE` → `AGE`).
fn partner_age(node: &Node, tag: &str) -> Option<genealogy_interchange::Age> {
    node.child(tag)
        .and_then(|partner| partner.child_value("AGE"))
        .and_then(|value| parse_age(&value))
}

/// Interprets an event-level `ASSO` node into an [`EventAssociation`] (xref, `ROLE`, nested `SOUR`
/// citations, and `NOTE`s). A malformed `ASSO` without an xref pointer is skipped.
fn event_association(node: &Node) -> Option<EventAssociation> {
    let other_xref = unwrap_xref(&node.value)?.to_owned();
    let citations = node
        .children
        .iter()
        .filter(|c| c.tag == "SOUR")
        .filter_map(|c| {
            unwrap_xref(&c.value).map(|source_xref| Citation {
                source_xref: source_xref.to_owned(),
                page: c.child_value("PAGE"),
            })
        })
        .collect();
    let notes = node
        .children
        .iter()
        .filter(|c| c.tag == "NOTE")
        .filter_map(|c| non_empty(&c.full_value()))
        .collect();
    Some(EventAssociation {
        other_xref,
        role: node.child_value("ROLE").map(|role| association_kind(&role)),
        citations,
        notes,
    })
}

/// Interprets a `NAME` node and its sub-records into a [`Name`].
fn name(node: &Node) -> Name {
    let (given, surname) = parse_slash_name(&node.value);
    let mut name = Name {
        given,
        surname,
        ..Name::default()
    };
    // Structured sub-records override the slash form when present.
    if let Some(value) = node.child_value("GIVN") {
        name.given = Some(value);
    }
    if let Some(value) = node.child_value("SURN") {
        name.surname = Some(value);
    }
    name.surname_prefix = node.child_value("SPFX");
    name.nickname = node.child_value("NICK");
    name.prefix = node.child_value("NPFX");
    name.suffix = node.child_value("NSFX");
    name.name_type = node.child_value("TYPE").map(|value| name_kind(&value));
    name
}

/// Interprets the `ADDR` structure (and the contact subtags beside it) into an [`Address`].
fn address(node: &Node) -> Option<Address> {
    let addr = node.child("ADDR");
    let mut address = Address::default();
    if let Some(addr) = addr {
        for line in addr.full_value().lines() {
            let line = line.trim();
            if !line.is_empty() {
                address.lines.push(line.to_owned());
            }
        }
        for tag in ["ADR1", "ADR2", "ADR3"] {
            if let Some(value) = addr.child_value(tag) {
                address.lines.push(value);
            }
        }
        address.locality = addr.child_value("CITY");
        address.region = addr.child_value("STAE");
        address.postal_code = addr.child_value("POST");
        address.country = addr.child_value("CTRY");
    }
    // The contact subtags sit beside `ADDR`, under the event/record node.
    address.phone = node.child_value("PHON");
    address.email = node.child_value("EMAIL");
    address.fax = node.child_value("FAX");
    address.www = node.child_value("WWW");
    if address.is_empty() { None } else { Some(address) }
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

/// Maps an event tag to its kind, or `None` if the tag is not an event we model.
fn event_kind(tag: &str) -> Option<EventKind> {
    let kind = match tag {
        "BIRT" => EventKind::Birth,
        "DEAT" => EventKind::Death,
        "MARR" => EventKind::Marriage,
        "BAPM" => EventKind::Baptism,
        "CHR" => EventKind::Christening,
        "BURI" => EventKind::Burial,
        "CREM" => EventKind::Cremation,
        "CENS" => EventKind::Census,
        "RESI" => EventKind::Residence,
        "IMMI" => EventKind::Immigration,
        "EMIG" => EventKind::Emigration,
        "ADOP" => EventKind::Adoption,
        "CONF" => EventKind::Confirmation,
        "BARM" => EventKind::BarMitzvah,
        "BASM" => EventKind::BasMitzvah,
        "FCOM" => EventKind::FirstCommunion,
        "GRAD" => EventKind::Graduation,
        "NATU" => EventKind::Naturalization,
        "ORDN" => EventKind::Ordination,
        "PROB" => EventKind::Probate,
        "RETI" => EventKind::Retirement,
        "WILL" => EventKind::Will,
        "ENGA" => EventKind::Engagement,
        "ANUL" => EventKind::Annulment,
        "DIV" => EventKind::Divorce,
        "DIVF" => EventKind::DivorceFiled,
        "MARB" => EventKind::MarriageBanns,
        "MARC" => EventKind::MarriageContract,
        "MARL" => EventKind::MarriageLicense,
        "MARS" => EventKind::MarriageSettlement,
        _ => return None,
    };
    Some(kind)
}

/// Maps an INDI-attribute tag to its fact kind, or `None` if the tag is not a fact we model.
fn fact_kind(tag: &str) -> Option<FactKind> {
    let kind = match tag {
        "OCCU" => FactKind::Occupation,
        "RELI" => FactKind::Religion,
        "EDUC" => FactKind::Education,
        "CAST" => FactKind::Caste,
        "DSCR" => FactKind::PhysicalDescription,
        "ETHN" => FactKind::Ethnicity,
        "IDNO" => FactKind::NationalId,
        "NATI" => FactKind::Nationality,
        "NCHI" => FactKind::NumberOfChildren,
        "NMR" => FactKind::NumberOfMarriages,
        "PROP" => FactKind::Property,
        "SSN" => FactKind::SocialSecurityNumber,
        "TITL" => FactKind::NobilityTitle,
        _ => return None,
    };
    Some(kind)
}

/// Maps a GEDCOM `ASSO.ROLE` token to an [`AssociationKind`] (unknown roles kept verbatim).
fn association_kind(role: &str) -> AssociationKind {
    match role.to_ascii_uppercase().as_str() {
        "CLERGY" => AssociationKind::Clergy,
        "FRIEND" => AssociationKind::Friend,
        "GODP" => AssociationKind::Godparent,
        "NGHBR" => AssociationKind::Neighbour,
        "OFFICIATOR" => AssociationKind::Officiator,
        "WITN" => AssociationKind::Witness,
        "CHIL" => AssociationKind::Child,
        "FATH" => AssociationKind::Father,
        "MOTH" => AssociationKind::Mother,
        "PARENT" => AssociationKind::Parent,
        "HUSB" => AssociationKind::Husband,
        "WIFE" => AssociationKind::Wife,
        "SPOU" => AssociationKind::Spouse,
        "MULTIPLE" => AssociationKind::Multiple,
        _ => AssociationKind::Other(role.to_owned()),
    }
}

/// Maps a GEDCOM `NAME.TYPE` token to a [`NameKind`] (unknown types kept verbatim).
fn name_kind(value: &str) -> NameKind {
    match value.to_ascii_uppercase().as_str() {
        "BIRTH" => NameKind::BirthName,
        "MARRIED" => NameKind::MarriedName,
        "MAIDEN" => NameKind::Maiden,
        "IMMIGRANT" => NameKind::Immigrant,
        "PROFESSIONAL" => NameKind::Professional,
        "AKA" => NameKind::AlsoKnownAs,
        "RELIGIOUS" => NameKind::ReligiousName,
        _ => NameKind::Other(value.to_owned()),
    }
}

/// Parses a `SEX` value (`M`/`F`/`X`, else unknown).
fn parse_sex(value: &str) -> Sex {
    match value.trim() {
        "M" => Sex::Male,
        "F" => Sex::Female,
        "X" => Sex::Intersex,
        _ => Sex::Unknown,
    }
}

/// Parses a `NAME` value (`Given /Surname/`) into its given and surname parts.
fn parse_slash_name(value: &str) -> (Option<String>, Option<String>) {
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

/// Parses a GEDCOM `DATE` value into a structured [`Date`], retaining the verbatim text. An
/// unparseable date becomes [`DateModifier::TextOnly`].
fn parse_date(value: &str) -> Option<Date> {
    let original = value.trim().to_owned();
    if original.is_empty() {
        return None;
    }
    let (calendar, rest) = strip_calendar(&original);
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut new_year_begins = None;
    let quality = quality_of(&tokens);
    let modifier = parse_modifier(&tokens, &original, &mut new_year_begins);
    Some(Date {
        calendar,
        quality,
        modifier,
        new_year_begins,
        original,
    })
}

/// Reads the `EST`/`CAL` quality keyword from the leading token (else `Normal`).
fn quality_of(tokens: &[&str]) -> DateQuality {
    match tokens.first().map(|token| token.to_ascii_uppercase()).as_deref() {
        Some("EST") => DateQuality::Estimated,
        Some("CAL") => DateQuality::Calculated,
        _ => DateQuality::Normal,
    }
}

/// Strips a leading calendar escape (`@#DJULIAN@`, …), returning the calendar and the remainder.
fn strip_calendar(value: &str) -> (Calendar, String) {
    let trimmed = value.trim_start();
    if let Some(after) = trimmed.strip_prefix("@#D")
        && let Some((name, rest)) = after.split_once('@')
    {
        let calendar = match name.trim().to_ascii_uppercase().as_str() {
            "JULIAN" => Calendar::Julian,
            "HEBREW" => Calendar::Hebrew,
            "FRENCH R" | "FRENCH_R" => Calendar::FrenchRepublican,
            "ISLAMIC" => Calendar::Islamic,
            "SWEDISH" => Calendar::Swedish,
            _ => Calendar::Gregorian,
        };
        return (calendar, rest.trim().to_owned());
    }
    (Calendar::Gregorian, value.trim().to_owned())
}

/// Parses the modifier keyword (if any) and the date point(s); falls back to `TextOnly`.
fn parse_modifier(tokens: &[&str], original: &str, new_year_begins: &mut Option<u8>) -> DateModifier {
    let keyword = tokens.first().map(|token| token.to_ascii_uppercase());
    match keyword.as_deref() {
        Some("BET") => {
            if let Some(and) = tokens.iter().position(|token| token.eq_ignore_ascii_case("AND"))
                && let (Some(start), Some(end)) = (
                    parse_point(&tokens[1..and], new_year_begins),
                    parse_point(&tokens[and + 1..], new_year_begins),
                )
            {
                return DateModifier::Range { start, end };
            }
            DateModifier::TextOnly(original.to_owned())
        }
        Some("FROM") => {
            if let Some(to) = tokens.iter().position(|token| token.eq_ignore_ascii_case("TO")) {
                if let (Some(start), Some(end)) = (
                    parse_point(&tokens[1..to], new_year_begins),
                    parse_point(&tokens[to + 1..], new_year_begins),
                ) {
                    return DateModifier::Span { start, end };
                }
                return DateModifier::TextOnly(original.to_owned());
            }
            point_or_text(&tokens[1..], new_year_begins, original, DateModifier::From)
        }
        Some("TO") => point_or_text(&tokens[1..], new_year_begins, original, DateModifier::To),
        Some("BEF") => point_or_text(&tokens[1..], new_year_begins, original, DateModifier::Before),
        Some("AFT") => point_or_text(&tokens[1..], new_year_begins, original, DateModifier::After),
        Some("ABT") => point_or_text(&tokens[1..], new_year_begins, original, DateModifier::About),
        Some("INT") => interpreted(&tokens[1..], new_year_begins, original),
        Some("EST" | "CAL") => {
            // Quality keywords; `quality_of` reads the keyword, so here we parse the point as exact.
            point_or_text(&tokens[1..], new_year_begins, original, DateModifier::Exact)
        }
        _ => point_or_text(tokens, new_year_begins, original, DateModifier::Exact),
    }
}

/// Builds `wrap(point)` if the tokens parse, else `TextOnly(original)`.
fn point_or_text(
    tokens: &[&str],
    new_year_begins: &mut Option<u8>,
    original: &str,
    wrap: fn(DatePoint) -> DateModifier,
) -> DateModifier {
    match parse_point(tokens, new_year_begins) {
        Some(point) => wrap(point),
        None => DateModifier::TextOnly(original.to_owned()),
    }
}

/// Parses a `INT <date> (phrase)` value into an [`DateModifier::Interpreted`].
fn interpreted(tokens: &[&str], new_year_begins: &mut Option<u8>, original: &str) -> DateModifier {
    let phrase_start = tokens.iter().position(|token| token.starts_with('('));
    let date_tokens = phrase_start.map_or(tokens, |index| &tokens[..index]);
    let phrase = phrase_start
        .map(|index| tokens[index..].join(" ").trim_matches(['(', ')']).to_owned())
        .unwrap_or_default();
    match parse_point(date_tokens, new_year_begins) {
        Some(date) => DateModifier::Interpreted { date, phrase },
        None => DateModifier::TextOnly(original.to_owned()),
    }
}

/// Parses `[DAY] [MON] YEAR` tokens into a [`DatePoint`]; the year is required. Dual years
/// (`1735/6`) set `new_year_begins` and keep the first year.
fn parse_point(tokens: &[&str], new_year_begins: &mut Option<u8>) -> Option<DatePoint> {
    let mut point = DatePoint::default();
    for token in tokens {
        if let Some(month) = month_number(token) {
            point.month = Some(month);
        } else if let Some(year) = parse_year(token, new_year_begins) {
            point.year = Some(year);
        } else if let Ok(number) = token.parse::<u8>()
            && (1..=31).contains(&number)
            && point.day.is_none()
            && point.year.is_none()
        {
            point.day = Some(number);
        }
    }
    point.year.map(|_| point)
}

/// Parses a year token, handling a dual year (`1735/6` → 1735 + dual-dating marker).
fn parse_year(token: &str, new_year_begins: &mut Option<u8>) -> Option<i32> {
    if let Some((first, _second)) = token.split_once('/') {
        let year = first.parse::<i32>().ok()?;
        // Old-style years began on Lady Day (25 March); record the month as the dual-dating marker.
        *new_year_begins = Some(3);
        return Some(year);
    }
    // A bare 1–31 is a day, not a year; only treat larger numbers (or any 4-digit value) as a year.
    let year = token.parse::<i32>().ok()?;
    if (1..=31).contains(&year) { None } else { Some(year) }
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
