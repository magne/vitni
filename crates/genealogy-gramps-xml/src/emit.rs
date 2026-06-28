//! Emits a [`Database`] as a Gramps XML document (plain, uncompressed).

use genealogy_interchange::{AssociationKind, Date, DateModifier, DatePoint, DateQuality, EventKind, Name, NameKind};

use crate::model::{
    Citation, Database, Event, Family, Gender, MediaObject, Note, Person, Place, Repository, Source, Tag,
};

/// Emits `db` as a Gramps XML document (plain XML — Gramps reads it without gzip).
#[must_use]
pub fn emit(db: &Database) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<database xmlns=\"http://gramps-project.org/xml/1.7.1/\">\n");
    section(&mut out, "people", &db.people, emit_person);
    section(&mut out, "families", &db.families, emit_family);
    section(&mut out, "events", &db.events, emit_event);
    section(&mut out, "places", &db.places, emit_place);
    section(&mut out, "sources", &db.sources, emit_source);
    section(&mut out, "citations", &db.citations, emit_citation);
    section(&mut out, "repositories", &db.repositories, emit_repository);
    section(&mut out, "objects", &db.objects, emit_media);
    section(&mut out, "notes", &db.notes, emit_note);
    section(&mut out, "tags", &db.tags, emit_tag);
    out.push_str("</database>\n");
    out.into_bytes()
}

/// Writes a `<name>…</name>` container wrapping each record, omitting an empty section.
fn section<T>(out: &mut String, name: &str, records: &[T], mut emit_one: impl FnMut(&mut String, &T)) {
    if records.is_empty() {
        return;
    }
    out.push('<');
    out.push_str(name);
    out.push_str(">\n");
    for record in records {
        emit_one(out, record);
    }
    out.push_str("</");
    out.push_str(name);
    out.push_str(">\n");
}

fn emit_person(out: &mut String, person: &Person) {
    out.push_str(&open(
        "person",
        &priv_attrs(&person.handle, person.gramps_id.as_deref(), person.private),
    ));
    if let Some(gender) = person.gender {
        out.push_str(&text_element("gender", gender_label(gender)));
    }
    if let Some(name) = &person.name {
        emit_name(out, name);
    }
    for hlink in &person.event_refs {
        out.push_str(&empty("eventref", &[("hlink", hlink)]));
    }
    for hlink in &person.citation_refs {
        out.push_str(&empty("citationref", &[("hlink", hlink)]));
    }
    for hlink in &person.note_refs {
        out.push_str(&empty("noteref", &[("hlink", hlink)]));
    }
    for hlink in &person.media_refs {
        out.push_str(&empty("objref", &[("hlink", hlink)]));
    }
    for person_ref in &person.person_refs {
        match &person_ref.rel {
            Some(rel) => out.push_str(&empty(
                "personref",
                &[("hlink", &person_ref.hlink), ("rel", association_label(rel))],
            )),
            None => out.push_str(&empty("personref", &[("hlink", &person_ref.hlink)])),
        }
    }
    out.push_str(&close("person"));
}

fn emit_family(out: &mut String, family: &Family) {
    out.push_str(&open(
        "family",
        &priv_attrs(&family.handle, family.gramps_id.as_deref(), family.private),
    ));
    if let Some(father) = &family.father {
        out.push_str(&empty("father", &[("hlink", father)]));
    }
    if let Some(mother) = &family.mother {
        out.push_str(&empty("mother", &[("hlink", mother)]));
    }
    for child in &family.child_refs {
        let mut attrs: Vec<(&str, &str)> = vec![("hlink", &child.hlink)];
        if let Some(mrel) = &child.mother_relationship {
            attrs.push(("mrel", mrel));
        }
        if let Some(frel) = &child.father_relationship {
            attrs.push(("frel", frel));
        }
        out.push_str(&empty("childref", &attrs));
    }
    for hlink in &family.event_refs {
        out.push_str(&empty("eventref", &[("hlink", hlink)]));
    }
    out.push_str(&close("family"));
}

fn emit_event(out: &mut String, event: &Event) {
    out.push_str(&open("event", &id_attrs(&event.handle, event.gramps_id.as_deref())));
    out.push_str(&text_element("type", event_label(event.kind)));
    if let Some(date) = &event.date {
        emit_date(out, date);
    }
    if let Some(place) = &event.place_ref {
        out.push_str(&empty("place", &[("hlink", place)]));
    }
    if let Some(description) = &event.description {
        out.push_str(&text_element("description", description));
    }
    out.push_str(&close("event"));
}

fn emit_place(out: &mut String, place: &Place) {
    let mut attrs = id_attrs(&place.handle, place.gramps_id.as_deref());
    if let Some(place_type) = &place.place_type {
        attrs.push(("type".to_owned(), place_type.clone()));
    }
    out.push_str(&open_owned("placeobj", &attrs));
    if let Some(name) = &place.name {
        out.push_str(&empty("pname", &[("value", name)]));
    }
    for hlink in &place.enclosed_by {
        out.push_str(&empty("placeref", &[("hlink", hlink)]));
    }
    out.push_str(&close("placeobj"));
}

fn emit_source(out: &mut String, source: &Source) {
    out.push_str(&open("source", &id_attrs(&source.handle, source.gramps_id.as_deref())));
    if let Some(title) = &source.title {
        out.push_str(&text_element("stitle", title));
    }
    if let Some(author) = &source.author {
        out.push_str(&text_element("sauthor", author));
    }
    if let Some(pub_info) = &source.pub_info {
        out.push_str(&text_element("spubinfo", pub_info));
    }
    for hlink in &source.repository_refs {
        out.push_str(&empty("reporef", &[("hlink", hlink)]));
    }
    out.push_str(&close("source"));
}

fn emit_citation(out: &mut String, citation: &Citation) {
    out.push_str(&open(
        "citation",
        &id_attrs(&citation.handle, citation.gramps_id.as_deref()),
    ));
    if let Some(page) = &citation.page {
        out.push_str(&text_element("page", page));
    }
    if let Some(confidence) = citation.confidence {
        out.push_str(&text_element("confidence", &confidence.to_string()));
    }
    if let Some(source) = &citation.source_ref {
        out.push_str(&empty("sourceref", &[("hlink", source)]));
    }
    out.push_str(&close("citation"));
}

fn emit_repository(out: &mut String, repository: &Repository) {
    out.push_str(&open(
        "repository",
        &id_attrs(&repository.handle, repository.gramps_id.as_deref()),
    ));
    if let Some(name) = &repository.name {
        out.push_str(&text_element("rname", name));
    }
    out.push_str(&close("repository"));
}

fn emit_media(out: &mut String, media: &MediaObject) {
    out.push_str(&open("object", &id_attrs(&media.handle, media.gramps_id.as_deref())));
    if let Some(file) = &media.file {
        let mut attrs: Vec<(&str, &str)> = vec![("src", file)];
        if let Some(mime) = &media.mime {
            attrs.push(("mime", mime));
        }
        out.push_str(&empty("file", &attrs));
    }
    out.push_str(&close("object"));
}

fn emit_note(out: &mut String, note: &Note) {
    out.push_str(&open("note", &id_attrs(&note.handle, note.gramps_id.as_deref())));
    if let Some(text) = &note.text {
        out.push_str(&text_element("text", text));
    }
    out.push_str(&close("note"));
}

fn emit_tag(out: &mut String, tag: &Tag) {
    match &tag.name {
        Some(name) => out.push_str(&empty("tag", &[("handle", &tag.handle), ("name", name)])),
        None => out.push_str(&empty("tag", &[("handle", &tag.handle)])),
    }
}

fn emit_name(out: &mut String, name: &Name) {
    match name.name_type.as_ref() {
        Some(kind) => out.push_str(&open_owned("name", &[("type".to_owned(), name_label(kind).to_owned())])),
        None => out.push_str("<name>\n"),
    }
    if let Some(given) = &name.given {
        out.push_str(&text_element("first", given));
    }
    if let Some(surname) = &name.surname {
        match &name.surname_prefix {
            Some(prefix) => {
                out.push_str("<surname prefix=\"");
                out.push_str(&escape(prefix));
                out.push_str("\">");
                out.push_str(&escape(surname));
                out.push_str("</surname>\n");
            }
            None => out.push_str(&text_element("surname", surname)),
        }
    }
    if let Some(nick) = &name.nickname {
        out.push_str(&text_element("nick", nick));
    }
    if let Some(title) = &name.prefix {
        out.push_str(&text_element("title", title));
    }
    if let Some(suffix) = &name.suffix {
        out.push_str(&text_element("suffix", suffix));
    }
    out.push_str(&close("name"));
}

fn emit_date(out: &mut String, date: &Date) {
    let quality = match date.quality {
        DateQuality::Normal => None,
        DateQuality::Estimated => Some("estimated"),
        DateQuality::Calculated => Some("calculated"),
    };
    match &date.modifier {
        DateModifier::Exact(point) => dateval(out, point, None, quality),
        DateModifier::Before(point) => dateval(out, point, Some("before"), quality),
        DateModifier::After(point) => dateval(out, point, Some("after"), quality),
        DateModifier::About(point) => dateval(out, point, Some("about"), quality),
        DateModifier::From(point) => dateval(out, point, Some("from"), quality),
        DateModifier::To(point) => dateval(out, point, Some("to"), quality),
        DateModifier::Range { start, end } => date_pair(out, "daterange", start, end, quality),
        DateModifier::Span { start, end } => date_pair(out, "datespan", start, end, quality),
        // Gramps has no structured interpreted date; both fall back to a free-text date string.
        DateModifier::Interpreted { phrase, .. } => out.push_str(&empty("datestr", &[("val", phrase)])),
        DateModifier::TextOnly(text) => out.push_str(&empty("datestr", &[("val", text)])),
    }
}

/// Writes a `<dateval>` with an optional modifier `type` and `quality`.
fn dateval(out: &mut String, point: &DatePoint, modifier: Option<&str>, quality: Option<&str>) {
    let mut attrs = vec![("val".to_owned(), format_point(point))];
    if let Some(modifier) = modifier {
        attrs.push(("type".to_owned(), modifier.to_owned()));
    }
    if let Some(quality) = quality {
        attrs.push(("quality".to_owned(), quality.to_owned()));
    }
    out.push_str(&empty_owned("dateval", &attrs));
}

/// Writes a `<daterange>` / `<datespan>` with `start`/`stop` bounds and an optional `quality`.
fn date_pair(out: &mut String, tag: &str, start: &DatePoint, end: &DatePoint, quality: Option<&str>) {
    let mut attrs = vec![
        ("start".to_owned(), format_point(start)),
        ("stop".to_owned(), format_point(end)),
    ];
    if let Some(quality) = quality {
        attrs.push(("quality".to_owned(), quality.to_owned()));
    }
    out.push_str(&empty_owned(tag, &attrs));
}

/// Formats a [`DatePoint`] as `YYYY`, `YYYY-MM`, or `YYYY-MM-DD` (Gramps `val` form).
fn format_point(point: &DatePoint) -> String {
    let Some(year) = point.year else {
        return String::new();
    };
    match (point.month, point.day) {
        (Some(month), Some(day)) => format!("{year:04}-{month:02}-{day:02}"),
        (Some(month), None) => format!("{year:04}-{month:02}"),
        _ => format!("{year}"),
    }
}

/// The `handle` plus optional `id` attributes every primary record carries.
fn id_attrs(handle: &str, gramps_id: Option<&str>) -> Vec<(String, String)> {
    let mut attrs = vec![("handle".to_owned(), handle.to_owned())];
    if let Some(id) = gramps_id {
        attrs.push(("id".to_owned(), id.to_owned()));
    }
    attrs
}

/// [`id_attrs`] plus the Gramps `priv="1"` attribute when the record is private (data-model §16).
fn priv_attrs(handle: &str, gramps_id: Option<&str>, private: bool) -> Vec<(String, String)> {
    let mut attrs = id_attrs(handle, gramps_id);
    if private {
        attrs.push(("priv".to_owned(), "1".to_owned()));
    }
    attrs
}

/// `<name attrs>\n` for borrowed attribute values.
fn open(name: &str, attrs: &[(String, String)]) -> String {
    open_owned(name, attrs)
}

fn open_owned(name: &str, attrs: &[(String, String)]) -> String {
    format!("<{}{}>\n", name, render_attrs(attrs))
}

fn close(name: &str) -> String {
    format!("</{name}>\n")
}

/// A self-closing `<name attrs/>\n` for borrowed `&str` attribute values.
fn empty(name: &str, attrs: &[(&str, &str)]) -> String {
    let owned: Vec<(String, String)> = attrs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    empty_owned(name, &owned)
}

fn empty_owned(name: &str, attrs: &[(String, String)]) -> String {
    format!("<{}{}/>\n", name, render_attrs(attrs))
}

/// `<name>escaped-text</name>\n`.
fn text_element(name: &str, text: &str) -> String {
    format!("<{name}>{}</{name}>\n", escape(text))
}

/// Renders attributes as ` k="escaped-v"` segments.
fn render_attrs(attrs: &[(String, String)]) -> String {
    let mut rendered = String::new();
    for (key, value) in attrs {
        rendered.push(' ');
        rendered.push_str(key);
        rendered.push_str("=\"");
        rendered.push_str(&escape(value));
        rendered.push('"');
    }
    rendered
}

/// Escapes the five XML metacharacters in text and attribute values.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn gender_label(gender: Gender) -> &'static str {
    match gender {
        Gender::Male => "M",
        Gender::Female => "F",
        Gender::Unknown => "U",
        Gender::Intersex => "X",
    }
}

fn event_label(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Birth => "Birth",
        EventKind::Death => "Death",
        EventKind::Marriage => "Marriage",
        EventKind::Baptism => "Baptism",
        EventKind::Christening => "Christening",
        EventKind::Burial => "Burial",
        EventKind::Cremation => "Cremation",
        EventKind::Census => "Census",
        EventKind::Residence => "Residence",
        EventKind::Immigration => "Immigration",
        EventKind::Emigration => "Emigration",
        EventKind::Adoption => "Adoption",
        EventKind::Confirmation => "Confirmation",
        EventKind::BarMitzvah => "Bar Mitzvah",
        EventKind::BasMitzvah => "Bas Mitzvah",
        EventKind::FirstCommunion => "First Communion",
        EventKind::Graduation => "Graduation",
        EventKind::Naturalization => "Naturalization",
        EventKind::Ordination => "Ordination",
        EventKind::Probate => "Probate",
        EventKind::Retirement => "Retirement",
        EventKind::Will => "Will",
        EventKind::Engagement => "Engagement",
        EventKind::Annulment => "Annulment",
        EventKind::Divorce => "Divorce",
        EventKind::DivorceFiled => "Divorce Filing",
        EventKind::MarriageBanns => "Marriage Banns",
        EventKind::MarriageContract => "Marriage Contract",
        EventKind::MarriageLicense => "Marriage License",
        EventKind::MarriageSettlement => "Marriage Settlement",
    }
}

fn name_label(kind: &NameKind) -> &str {
    match kind {
        NameKind::BirthName => "Birth Name",
        NameKind::MarriedName => "Married Name",
        NameKind::Maiden => "Maiden",
        NameKind::Immigrant => "Immigrant",
        NameKind::Professional => "Professional",
        NameKind::AlsoKnownAs => "Also Known As",
        NameKind::ReligiousName => "Religious Name",
        NameKind::Other(value) => value,
    }
}

fn association_label(kind: &AssociationKind) -> &str {
    match kind {
        AssociationKind::Clergy => "Clergy",
        AssociationKind::Friend => "Friend",
        AssociationKind::Godparent => "Godparent",
        AssociationKind::Neighbour => "Neighbour",
        AssociationKind::Officiator => "Officiator",
        AssociationKind::Witness => "Witness",
        AssociationKind::Child => "Child",
        AssociationKind::Father => "Father",
        AssociationKind::Mother => "Mother",
        AssociationKind::Parent => "Parent",
        AssociationKind::Husband => "Husband",
        AssociationKind::Wife => "Wife",
        AssociationKind::Spouse => "Spouse",
        AssociationKind::Multiple => "Multiple",
        AssociationKind::Other(value) => value,
    }
}
