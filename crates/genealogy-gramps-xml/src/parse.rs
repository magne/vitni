//! Parses Gramps XML (gzipped or plain) into a [`Database`].

use genealogy_interchange::{AssociationKind, Calendar, Date, DateModifier, DatePoint, DateQuality, EventKind, Name};
use thiserror::Error;

use crate::model::{
    Citation, Database, Event, Family, Gender, MediaObject, Note, Person, PersonRef, Place, Repository, Source, Tag,
};
use crate::xml::{Element, read_tree};

/// An error parsing a Gramps XML document.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GrampsError {
    /// The gzip stream could not be inflated.
    #[error("gzip inflate failed: {0}")]
    Gzip(String),
    /// The XML was malformed.
    #[error("malformed XML: {0}")]
    Xml(String),
}

/// Parses Gramps XML `bytes` (gzipped `.gramps` or plain XML) into a [`Database`].
///
/// # Errors
/// [`GrampsError::Gzip`] if a gzipped stream cannot be inflated, or [`GrampsError::Xml`] if the XML
/// is malformed. Unknown elements and dangling `hlink`s are tolerated (skipped), not errors.
pub fn parse(bytes: &[u8]) -> Result<Database, GrampsError> {
    let root = read_tree(bytes)?;
    let mut db = Database::default();
    for container in &root.children {
        match container.name.as_str() {
            "people" => db.people = container.children_named("person").map(person).collect(),
            "families" => db.families = container.children_named("family").map(family).collect(),
            "events" => db.events = container.children_named("event").map(event).collect(),
            "places" => db.places = container.children_named("placeobj").map(place).collect(),
            "sources" => db.sources = container.children_named("source").map(source).collect(),
            "citations" => db.citations = container.children_named("citation").map(citation).collect(),
            "repositories" => db.repositories = container.children_named("repository").map(repository).collect(),
            "objects" => db.objects = container.children_named("object").map(media).collect(),
            "notes" => db.notes = container.children_named("note").map(note).collect(),
            "tags" => db.tags = container.children_named("tag").map(tag).collect(),
            _ => {}
        }
    }
    Ok(db)
}

/// The `handle` attribute, or empty when absent (a malformed record still parses).
fn handle(element: &Element) -> String {
    element.attr("handle").unwrap_or_default().to_owned()
}

/// The optional `id` (`gramps_id`) attribute.
fn gramps_id(element: &Element) -> Option<String> {
    element.attr("id").map(ToOwned::to_owned)
}

/// The Gramps `priv` flag (`priv="1"`), defaulting to `false` when absent or any other value.
fn private(element: &Element) -> bool {
    element.attr("priv") == Some("1")
}

/// Collects the `hlink` attribute of every child named `tag`.
fn hlinks(element: &Element, tag: &str) -> Vec<String> {
    element
        .children_named(tag)
        .filter_map(|c| c.attr("hlink").map(ToOwned::to_owned))
        .collect()
}

fn person(element: &Element) -> Person {
    Person {
        handle: handle(element),
        gramps_id: gramps_id(element),
        name: element.child("name").map(name),
        gender: element.child("gender").map(|g| gender(&g.text)),
        event_refs: hlinks(element, "eventref"),
        citation_refs: hlinks(element, "citationref"),
        note_refs: hlinks(element, "noteref"),
        media_refs: hlinks(element, "objref"),
        person_refs: element
            .children_named("personref")
            .filter_map(|p| {
                p.attr("hlink").map(|hlink| PersonRef {
                    hlink: hlink.to_owned(),
                    rel: p.attr("rel").map(association_kind),
                })
            })
            .collect(),
        private: private(element),
    }
}

fn family(element: &Element) -> Family {
    Family {
        handle: handle(element),
        gramps_id: gramps_id(element),
        father: element
            .child("father")
            .and_then(|f| f.attr("hlink"))
            .map(ToOwned::to_owned),
        mother: element
            .child("mother")
            .and_then(|m| m.attr("hlink"))
            .map(ToOwned::to_owned),
        child_refs: hlinks(element, "childref"),
        event_refs: hlinks(element, "eventref"),
        private: private(element),
    }
}

fn event(element: &Element) -> Event {
    Event {
        handle: handle(element),
        gramps_id: gramps_id(element),
        kind: element.child("type").map_or(EventKind::Birth, |t| event_kind(&t.text)),
        date: parse_date(element),
        place_ref: element
            .child("place")
            .and_then(|p| p.attr("hlink"))
            .map(ToOwned::to_owned),
        description: element
            .child("description")
            .map(|d| d.text.clone())
            .filter(|s| !s.is_empty()),
    }
}

fn place(element: &Element) -> Place {
    Place {
        handle: handle(element),
        gramps_id: gramps_id(element),
        name: element
            .child("pname")
            .and_then(|p| p.attr("value"))
            .map(ToOwned::to_owned),
        place_type: element.attr("type").map(ToOwned::to_owned),
        enclosed_by: hlinks(element, "placeref"),
    }
}

fn source(element: &Element) -> Source {
    Source {
        handle: handle(element),
        gramps_id: gramps_id(element),
        title: child_text(element, "stitle"),
        author: child_text(element, "sauthor"),
        pub_info: child_text(element, "spubinfo"),
        repository_refs: hlinks(element, "reporef"),
    }
}

fn citation(element: &Element) -> Citation {
    Citation {
        handle: handle(element),
        gramps_id: gramps_id(element),
        source_ref: element
            .child("sourceref")
            .and_then(|s| s.attr("hlink"))
            .map(ToOwned::to_owned),
        page: child_text(element, "page"),
        confidence: element.child("confidence").and_then(|c| c.text.parse().ok()),
    }
}

fn repository(element: &Element) -> Repository {
    Repository {
        handle: handle(element),
        gramps_id: gramps_id(element),
        name: child_text(element, "rname"),
    }
}

fn media(element: &Element) -> MediaObject {
    MediaObject {
        handle: handle(element),
        gramps_id: gramps_id(element),
        file: element.child("file").and_then(|f| f.attr("src")).map(ToOwned::to_owned),
        mime: element
            .child("file")
            .and_then(|f| f.attr("mime"))
            .map(ToOwned::to_owned),
    }
}

fn note(element: &Element) -> Note {
    Note {
        handle: handle(element),
        gramps_id: gramps_id(element),
        text: child_text(element, "text"),
    }
}

fn tag(element: &Element) -> Tag {
    Tag {
        handle: handle(element),
        name: element.attr("name").map(ToOwned::to_owned),
    }
}

/// The trimmed text of the first child named `tag`, if non-empty.
fn child_text(element: &Element, tag: &str) -> Option<String> {
    element.child(tag).map(|c| c.text.clone()).filter(|s| !s.is_empty())
}

/// Interprets a Gramps `<name>` into the interchange [`Name`].
fn name(element: &Element) -> Name {
    Name {
        name_type: element.attr("type").map(name_kind),
        given: child_text(element, "first"),
        surname_prefix: element
            .child("surname")
            .and_then(|s| s.attr("prefix"))
            .map(ToOwned::to_owned),
        surname: child_text(element, "surname"),
        nickname: child_text(element, "nick"),
        prefix: child_text(element, "title"),
        suffix: child_text(element, "suffix"),
    }
}

/// Maps a Gramps `<gender>` value onto a [`Gender`].
fn gender(value: &str) -> Gender {
    match value.trim() {
        "M" => Gender::Male,
        "F" => Gender::Female,
        "X" => Gender::Intersex,
        _ => Gender::Unknown,
    }
}

/// Interprets the date child of an event element into a [`Date`].
fn parse_date(element: &Element) -> Option<Date> {
    if let Some(dateval) = element.child("dateval") {
        let point = date_point(dateval.attr("val").unwrap_or_default());
        let modifier = match dateval.attr("type") {
            Some("before") => DateModifier::Before(point),
            Some("after") => DateModifier::After(point),
            Some("about") => DateModifier::About(point),
            Some("from") => DateModifier::From(point),
            Some("to") => DateModifier::To(point),
            _ => DateModifier::Exact(point),
        };
        return Some(build_date(modifier, dateval.attr("quality")));
    }
    if let Some(range) = element.child("daterange") {
        let modifier = DateModifier::Range {
            start: date_point(range.attr("start").unwrap_or_default()),
            end: date_point(range.attr("stop").unwrap_or_default()),
        };
        return Some(build_date(modifier, range.attr("quality")));
    }
    if let Some(span) = element.child("datespan") {
        let modifier = DateModifier::Span {
            start: date_point(span.attr("start").unwrap_or_default()),
            end: date_point(span.attr("stop").unwrap_or_default()),
        };
        return Some(build_date(modifier, span.attr("quality")));
    }
    element.child("datestr").map(|s| {
        build_date(
            DateModifier::TextOnly(s.attr("val").unwrap_or_default().to_owned()),
            None,
        )
    })
}

/// Wraps a modifier in a [`Date`], reading the Gramps `quality` attribute.
fn build_date(modifier: DateModifier, quality: Option<&str>) -> Date {
    Date {
        calendar: Calendar::Gregorian,
        quality: match quality {
            Some("estimated") => DateQuality::Estimated,
            Some("calculated") => DateQuality::Calculated,
            _ => DateQuality::Normal,
        },
        modifier,
        new_year_begins: None,
        original: String::new(),
    }
}

/// Parses a Gramps date string (`YYYY`, `YYYY-MM`, `YYYY-MM-DD`) into a [`DatePoint`].
fn date_point(value: &str) -> DatePoint {
    let mut parts = value.split('-');
    DatePoint {
        year: parts.next().and_then(|y| y.parse().ok()),
        month: parts.next().and_then(|m| m.parse().ok()),
        day: parts.next().and_then(|d| d.parse().ok()),
    }
}

/// Maps a Gramps event-type label onto an [`EventKind`]; an unrecognized label falls back to `Birth`
/// (the interchange enum has no custom escape).
fn event_kind(label: &str) -> EventKind {
    match label.trim() {
        "Death" => EventKind::Death,
        "Marriage" => EventKind::Marriage,
        "Baptism" => EventKind::Baptism,
        "Christening" => EventKind::Christening,
        "Burial" => EventKind::Burial,
        "Cremation" => EventKind::Cremation,
        "Census" => EventKind::Census,
        "Residence" => EventKind::Residence,
        "Immigration" => EventKind::Immigration,
        "Emigration" => EventKind::Emigration,
        "Adoption" => EventKind::Adoption,
        "Confirmation" => EventKind::Confirmation,
        "Bar Mitzvah" => EventKind::BarMitzvah,
        "Bas Mitzvah" => EventKind::BasMitzvah,
        "First Communion" => EventKind::FirstCommunion,
        "Graduation" => EventKind::Graduation,
        "Naturalization" => EventKind::Naturalization,
        "Ordination" => EventKind::Ordination,
        "Probate" => EventKind::Probate,
        "Retirement" => EventKind::Retirement,
        "Will" => EventKind::Will,
        "Engagement" => EventKind::Engagement,
        "Annulment" => EventKind::Annulment,
        "Divorce" => EventKind::Divorce,
        "Divorce Filing" => EventKind::DivorceFiled,
        "Marriage Banns" => EventKind::MarriageBanns,
        "Marriage Contract" => EventKind::MarriageContract,
        "Marriage License" => EventKind::MarriageLicense,
        "Marriage Settlement" => EventKind::MarriageSettlement,
        _ => EventKind::Birth,
    }
}

/// Maps a Gramps name-type label onto a [`NameKind`](genealogy_interchange::NameKind).
fn name_kind(label: &str) -> genealogy_interchange::NameKind {
    use genealogy_interchange::NameKind;
    match label.trim() {
        "Birth Name" => NameKind::BirthName,
        "Married Name" => NameKind::MarriedName,
        "Maiden" => NameKind::Maiden,
        "Immigrant" => NameKind::Immigrant,
        "Professional" => NameKind::Professional,
        "Also Known As" => NameKind::AlsoKnownAs,
        "Religious Name" => NameKind::ReligiousName,
        other => NameKind::Other(other.to_owned()),
    }
}

/// Maps a Gramps `personref` relationship label onto an [`AssociationKind`].
fn association_kind(label: &str) -> AssociationKind {
    match label.trim() {
        "Clergy" => AssociationKind::Clergy,
        "Friend" => AssociationKind::Friend,
        "Godparent" => AssociationKind::Godparent,
        "Neighbour" => AssociationKind::Neighbour,
        "Officiator" => AssociationKind::Officiator,
        "Witness" => AssociationKind::Witness,
        "Child" => AssociationKind::Child,
        "Father" => AssociationKind::Father,
        "Mother" => AssociationKind::Mother,
        "Parent" => AssociationKind::Parent,
        "Husband" => AssociationKind::Husband,
        "Wife" => AssociationKind::Wife,
        "Spouse" => AssociationKind::Spouse,
        "Multiple" => AssociationKind::Multiple,
        other => AssociationKind::Other(other.to_owned()),
    }
}
