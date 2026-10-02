//! Gramps XML import plugin (ADR 0013, ADR 0018, ADR 0040): read the document from the host-opened
//! import source, parse it with `vitni-gramps-xml`, and submit one record graph per Gramps object
//! through the host `staging` capability, turning Gramps's `hlink` references (events, places, sources,
//! citations, notes, media, repositories, tags) into references to those objects' graphs.
//!
//! Every object is a graph keyed by its Gramps `handle`, submitted the first time a person or family
//! refers to it, so the host writes each once; re-importing the same `.gramps` file resolves every
//! record onto what the first run made and writes nothing already on record (ADR 0037 §4).

wit_bindgen::generate!({
    world: "bulk-import",
    path: "../../crates/vitni-plugin-host/wit",
    with: {
        "vitni:host-api/types@0.25.0": vitni_plugin_api::types,
        "vitni:host-api/log@0.25.0": vitni_plugin_api::log,
        "vitni:host-api/staging@0.25.0": vitni_plugin_api::staging,
        "vitni:host-api/progress@0.25.0": vitni_plugin_api::progress,
        "vitni:host-api/import-source@0.25.0": vitni_plugin_api::import_source,
    },
});

use std::collections::{HashMap, HashSet};

use vitni_gramps_xml::{Citation, Database, Event, EventRef, Family, Gender, Note, Person, Place, Region, Source};
use vitni_interchange::{DatePoint, parse_age};
use vitni_plugin_api::staging::{
    AssociationLink, ChildLink, EntityFields, EntityKind, EntityRef, LinkKind, MediaLink, MemberLink, PairLink,
    ParticipationLink, RepositoryLink, StagedChildRel, StagedCitation, StagedEvent, StagedFamily, StagedMedia,
    StagedNote, StagedPerson, StagedPlace, StagedRepository, StagedSource, StagedTag,
};
use vitni_plugin_api::types::{Attribute, Confidence, MediaCrop, NoteType, ParticipantRole, PlaceType, Sex};
use vitni_plugin_api::{Graph, convert, origin_ref};

/// Maps a Gramps `<region>` crop (top-left origin + extent, percent) onto the host `media-crop`
/// record a media link carries (ADR 0017 §9).
fn region_to_crop(region: Region) -> MediaCrop {
    MediaCrop {
        left: region.left,
        top: region.top,
        width: region.width,
        height: region.height,
    }
}

struct Importer;

/// The parsed record indexes, and the objects whose graphs were submitted, so each referenced object
/// is submitted exactly once.
struct Resolver<'a> {
    persons: HashSet<&'a str>,
    events: HashMap<String, &'a Event>,
    places: HashMap<String, &'a Place>,
    sources: HashMap<String, &'a Source>,
    citations: HashMap<String, &'a Citation>,
    notes: HashMap<String, &'a Note>,
    media_file: HashMap<String, Option<String>>,
    media_mime: HashMap<String, Option<String>>,
    repository_name: HashMap<String, Option<String>>,
    tag_name: HashMap<String, Option<String>>,
    /// The objects submitted so far, by kind and handle.
    submitted: HashSet<(&'static str, String)>,
}

impl Guest for Importer {
    fn run_import() -> Result<u32, String> {
        let bytes = vitni_plugin_api::read_source_to_end()?;
        let db = vitni_gramps_xml::parse(&bytes).map_err(|error| error.to_string())?;
        let people = db.people.len() as u32;
        let families = db.families.len() as u32;
        vitni_plugin_api::log_info(&format!("importing {people} people and {families} families"));

        // Declare the researcher (the header's fingerprint) and the file's own export date once,
        // before any record: the host proposes the file's dataset by the one (ADR 0037 §3) and
        // reconciles single-valued fields by the other (ADR 0029 §2).
        let file_asserted_at = db.header.date.as_ref().and_then(file_asserted_at_string);
        vitni_plugin_api::begin_run(db.header.researcher.as_deref(), file_asserted_at.as_deref())?;

        let mut resolver = Resolver::new(&db);
        // (person, event) handle pairs already given a participation, so a partner whose person-side
        // eventref carries a payload is not given a second, bare one by the family.
        let mut participants: HashSet<(String, String)> = HashSet::new();
        let mut imported = 0u32;
        for (index, person) in db.people.iter().enumerate() {
            person_graph(person, &mut resolver, &mut participants)?.submit()?;
            imported += 1;
            if !vitni_plugin_api::report("people", index as u32 + 1, Some(people))? {
                return Ok(imported);
            }
        }
        for (index, family) in db.families.iter().enumerate() {
            family_graph(family, &mut resolver, &mut participants)?.submit()?;
            imported += 1;
            if !vitni_plugin_api::report("families", index as u32 + 1, Some(families))? {
                return Ok(imported);
            }
        }
        Ok(imported)
    }
}

/// Renders `<created date="…">` as an RFC 3339 timestamp for `staging::begin-run` (ADR 0029 §2), or
/// `None` unless it names a full day: a bare year or year and month carries no instant to reconcile
/// by (ADR 0029 §3).
fn file_asserted_at_string(date: &DatePoint) -> Option<String> {
    let (year, month, day) = (date.year?, date.month?, date.day?);
    Some(format!("{year:04}-{month:02}-{day:02}T00:00:00Z"))
}

/// The graph of one `<person>`.
fn person_graph(
    person: &Person,
    resolver: &mut Resolver<'_>,
    participants: &mut HashSet<(String, String)>,
) -> Result<Graph, String> {
    let mut graph = Graph::new(&person.handle);
    let entity = graph.entity(
        None,
        EntityFields::Person(StagedPerson {
            names: person.names.iter().map(convert::name_to_wit).collect(),
            sex: person.gender.map(gender_to_sex),
            facts: Vec::new(),
            external_ids: Vec::new(),
            restrictions: convert::private_to_wit(person.private),
        }),
    );
    for event_ref in &person.event_refs {
        let Some(event) = resolver.event(&event_ref.hlink)? else {
            continue;
        };
        if participants.insert((person.handle.clone(), event_ref.hlink.clone())) {
            let link = participation(resolver, entity.clone(), event, event_ref)?;
            graph.link(Some(&format!("eventref:{}", event_ref.hlink)), link);
        }
    }
    for handle in &person.citation_refs {
        if let Some(citation) = resolver.citation(handle)? {
            graph.link(
                None,
                LinkKind::CitationOf(PairLink {
                    owner: entity.clone(),
                    target: citation,
                }),
            );
        }
    }
    for handle in &person.note_refs {
        if let Some(note) = resolver.note(handle)? {
            graph.link(
                None,
                LinkKind::NoteOf(PairLink {
                    owner: entity.clone(),
                    target: note,
                }),
            );
        }
    }
    for media_ref in &person.media_refs {
        if let Some(media) = resolver.media(&media_ref.hlink)? {
            graph.link(
                None,
                LinkKind::MediaOf(MediaLink {
                    owner: entity.clone(),
                    media,
                    crop: media_ref.region.map(region_to_crop),
                    caption: None,
                }),
            );
        }
    }
    for person_ref in &person.person_refs {
        if let Some(other) = resolver.person(&person_ref.hlink) {
            graph.link(
                None,
                LinkKind::Association(AssociationLink {
                    person: entity.clone(),
                    other,
                    role: convert::association_role_to_wit(person_ref.rel.as_ref()),
                }),
            );
        }
    }
    for handle in &person.tag_refs {
        if let Some(tag) = resolver.tag(handle)? {
            graph.link(
                None,
                LinkKind::TagOf(PairLink {
                    owner: entity.clone(),
                    target: tag,
                }),
            );
        }
    }
    Ok(graph)
}

/// The graph of one `<family>`.
fn family_graph(
    family: &Family,
    resolver: &mut Resolver<'_>,
    participants: &mut HashSet<(String, String)>,
) -> Result<Graph, String> {
    let mut graph = Graph::new(&family.handle);
    let entity = graph.entity(
        None,
        EntityFields::Family(StagedFamily {
            external_ids: Vec::new(),
            restrictions: convert::private_to_wit(family.private),
        }),
    );
    let mut partners = Vec::new();
    for handle in family.father.iter().chain(family.mother.iter()) {
        if let Some(partner) = resolver.person(handle) {
            graph.link(
                None,
                LinkKind::Partner(MemberLink {
                    family: entity.clone(),
                    person: partner,
                }),
            );
            partners.push(handle.clone());
        }
    }
    for child in &family.child_refs {
        let Some(child_ref) = resolver.person(&child.hlink) else {
            continue;
        };
        let mut relationships = Vec::new();
        let parents = [
            (&child.father_relationship, &family.father),
            (&child.mother_relationship, &family.mother),
        ];
        for (relationship, parent) in parents {
            if let (Some(relationship), Some(parent)) = (relationship, parent.as_ref().and_then(|h| resolver.person(h)))
            {
                relationships.push(StagedChildRel {
                    partner: parent,
                    relationship: convert::child_relationship_to_wit(relationship),
                });
            }
        }
        graph.link(
            None,
            LinkKind::Child(ChildLink {
                family: entity.clone(),
                child: child_ref,
                relationships,
            }),
        );
    }
    for event_ref in &family.event_refs {
        let Some(event) = resolver.event(&event_ref.hlink)? else {
            continue;
        };
        graph.link(
            None,
            LinkKind::FamilyEvent(PairLink {
                owner: entity.clone(),
                target: event.clone(),
            }),
        );
        for partner in &partners {
            // A partner whose own eventref already took part (with a payload) is not given a bare
            // primary participation here.
            if !participants.insert((partner.clone(), event_ref.hlink.clone())) {
                continue;
            }
            let Some(person) = resolver.person(partner) else {
                continue;
            };
            graph.link(
                Some(&format!("eventref:{}", event_ref.hlink)),
                LinkKind::Participation(ParticipationLink {
                    person,
                    event: event.clone(),
                    role: ParticipantRole::Primary,
                    age: None,
                    attributes: Vec::new(),
                    notes: Vec::new(),
                    citations: Vec::new(),
                }),
            );
        }
    }
    for handle in &family.tag_refs {
        if let Some(tag) = resolver.tag(handle)? {
            graph.link(
                None,
                LinkKind::TagOf(PairLink {
                    owner: entity.clone(),
                    target: tag,
                }),
            );
        }
    }
    Ok(graph)
}

impl<'a> Resolver<'a> {
    fn new(db: &'a Database) -> Self {
        Self {
            persons: db.people.iter().map(|person| person.handle.as_str()).collect(),
            events: index(&db.events, |e| &e.handle),
            places: index(&db.places, |p| &p.handle),
            sources: index(&db.sources, |s| &s.handle),
            citations: index(&db.citations, |c| &c.handle),
            notes: index(&db.notes, |n| &n.handle),
            media_file: db.objects.iter().map(|o| (o.handle.clone(), o.file.clone())).collect(),
            media_mime: db.objects.iter().map(|o| (o.handle.clone(), o.mime.clone())).collect(),
            repository_name: db
                .repositories
                .iter()
                .map(|r| (r.handle.clone(), r.name.clone()))
                .collect(),
            tag_name: db.tags.iter().map(|t| (t.handle.clone(), t.name.clone())).collect(),
            submitted: HashSet::new(),
        }
    }

    /// Whether the object `handle` of `kind` is yet to be submitted, marking it submitted.
    fn first(&mut self, kind: &'static str, handle: &str) -> bool {
        self.submitted.insert((kind, handle.to_owned()))
    }

    /// The person `handle` names, when the document holds it.
    fn person(&self, handle: &str) -> Option<EntityRef> {
        self.persons
            .contains(handle)
            .then(|| origin_ref(EntityKind::Person, handle, None))
    }

    /// The event `handle` names, with its date and place. A dangling handle yields `None` (tolerated,
    /// not an error).
    fn event(&mut self, handle: &str) -> Result<Option<EntityRef>, String> {
        let Some(event) = self.events.get(handle).copied() else {
            return Ok(None);
        };
        if self.first("event", handle) {
            let mut graph = Graph::new(handle);
            let entity = graph.entity(
                None,
                EntityFields::Event(StagedEvent {
                    event_type: convert::event_type_to_wit(event.kind),
                    date: event.date.as_ref().map(convert::date_to_wit),
                    addresses: Vec::new(),
                    restrictions: Vec::new(),
                }),
            );
            if let Some(place_handle) = &event.place_ref
                && let Some(place) = self.place(place_handle)?
            {
                graph.link(
                    None,
                    LinkKind::EventPlace(PairLink {
                        owner: entity,
                        target: place,
                    }),
                );
            }
            graph.submit()?;
        }
        Ok(Some(origin_ref(EntityKind::Event, handle, None)))
    }

    /// The place `handle` names, with its type and the places enclosing it.
    fn place(&mut self, handle: &str) -> Result<Option<EntityRef>, String> {
        let Some(place) = self.places.get(handle).copied() else {
            return Ok(None);
        };
        if self.first("place", handle) {
            let mut graph = Graph::new(handle);
            let entity = graph.entity(
                None,
                EntityFields::Place(StagedPlace {
                    name: place.name.clone().unwrap_or_default(),
                    place_type: place.place_type.as_deref().map(place_type_of),
                    restrictions: Vec::new(),
                }),
            );
            for enclosing_handle in &place.enclosed_by {
                if let Some(enclosing) = self.place(enclosing_handle)? {
                    graph.link(
                        None,
                        LinkKind::Enclosure(PairLink {
                            owner: entity.clone(),
                            target: enclosing,
                        }),
                    );
                }
            }
            graph.submit()?;
        }
        Ok(Some(origin_ref(EntityKind::Place, handle, None)))
    }

    /// The source `handle` names, with its author, publication, abbreviation and repositories.
    fn source(&mut self, handle: &str) -> Result<Option<EntityRef>, String> {
        let Some(source) = self.sources.get(handle).copied() else {
            return Ok(None);
        };
        if self.first("source", handle) {
            let mut graph = Graph::new(handle);
            let entity = graph.entity(
                None,
                EntityFields::Source(StagedSource {
                    title: source.title.clone(),
                    author: source.author.clone(),
                    pub_info: source.pub_info.clone(),
                    abbrev: source.abbrev.clone(),
                    restrictions: Vec::new(),
                }),
            );
            for reporef in &source.repository_refs {
                if let Some(repository) = self.repository(&reporef.hlink)? {
                    let media_type = reporef.medium.as_ref().map_or(
                        vitni_plugin_api::types::SourceMediaType::Custom(String::new()),
                        convert::source_media_kind_to_wit,
                    );
                    graph.link(
                        None,
                        LinkKind::SourceRepository(RepositoryLink {
                            source: entity.clone(),
                            repository,
                            call_number: reporef.call_number.clone(),
                            media_type,
                        }),
                    );
                }
            }
            graph.submit()?;
        }
        Ok(Some(origin_ref(EntityKind::Source, handle, None)))
    }

    /// The citation `handle` names, with its source, page, confidence and notes — a transcription
    /// among them (data-model §6). A citation of no source the document holds yields `None`.
    fn citation(&mut self, handle: &str) -> Result<Option<EntityRef>, String> {
        let Some(citation) = self.citations.get(handle).copied() else {
            return Ok(None);
        };
        let source = match &citation.source_ref {
            Some(source_handle) => self.source(source_handle)?,
            None => None,
        };
        let Some(source) = source else {
            return Ok(None);
        };
        if self.first("citation", handle) {
            let mut graph = Graph::new(handle);
            let entity = graph.entity(
                None,
                EntityFields::Citation(StagedCitation {
                    source,
                    page: citation.page.clone(),
                    confidence: citation.confidence.map(confidence_of),
                    restrictions: Vec::new(),
                }),
            );
            for note_handle in &citation.note_refs {
                if let Some(note) = self.note(note_handle)? {
                    graph.link(
                        None,
                        LinkKind::NoteOf(PairLink {
                            owner: entity.clone(),
                            target: note,
                        }),
                    );
                }
            }
            graph.submit()?;
        }
        Ok(Some(origin_ref(EntityKind::Citation, handle, None)))
    }

    /// The note `handle` names, with its type.
    fn note(&mut self, handle: &str) -> Result<Option<EntityRef>, String> {
        let Some(note) = self.notes.get(handle).copied() else {
            return Ok(None);
        };
        if self.first("note", handle) {
            let mut graph = Graph::new(handle);
            graph.entity(
                None,
                EntityFields::Note(StagedNote {
                    text: note.text.clone().unwrap_or_default(),
                    note_type: note.note_type.as_deref().map(note_type_of),
                    restrictions: Vec::new(),
                }),
            );
            graph.submit()?;
        }
        Ok(Some(origin_ref(EntityKind::Note, handle, None)))
    }

    /// The media object `handle` names.
    fn media(&mut self, handle: &str) -> Result<Option<EntityRef>, String> {
        let Some(file) = self.media_file.get(handle).cloned() else {
            return Ok(None);
        };
        if self.first("media", handle) {
            let mut graph = Graph::new(handle);
            graph.entity(
                None,
                EntityFields::Media(StagedMedia {
                    path: file,
                    mime: self.media_mime.get(handle).cloned().flatten(),
                    restrictions: Vec::new(),
                }),
            );
            graph.submit()?;
        }
        Ok(Some(origin_ref(EntityKind::Media, handle, None)))
    }

    /// The repository `handle` names.
    fn repository(&mut self, handle: &str) -> Result<Option<EntityRef>, String> {
        let Some(name) = self.repository_name.get(handle).cloned() else {
            return Ok(None);
        };
        if self.first("repository", handle) {
            let mut graph = Graph::new(handle);
            graph.entity(
                None,
                EntityFields::Repository(StagedRepository {
                    name: name.unwrap_or_default(),
                    restrictions: Vec::new(),
                }),
            );
            graph.submit()?;
        }
        Ok(Some(origin_ref(EntityKind::Repository, handle, None)))
    }

    /// The tag `handle` names.
    fn tag(&mut self, handle: &str) -> Result<Option<EntityRef>, String> {
        let Some(name) = self.tag_name.get(handle).cloned() else {
            return Ok(None);
        };
        if self.first("tag", handle) {
            let mut graph = Graph::new(handle);
            graph.entity(
                None,
                EntityFields::Tag(StagedTag {
                    name: name.unwrap_or_default(),
                }),
            );
            graph.submit()?;
        }
        Ok(Some(origin_ref(EntityKind::Tag, handle, None)))
    }
}

/// The participation a person's `<eventref>` records: the role (default `primary`), the age (from the
/// `"Age"` attribute), the other attributes, and the note and citation refs (ADR 0019). The citations
/// ride the assertion envelope (ADR 0020).
fn participation(
    resolver: &mut Resolver<'_>,
    person: EntityRef,
    event: EntityRef,
    event_ref: &EventRef,
) -> Result<LinkKind, String> {
    let mut age = None;
    let mut attributes = Vec::new();
    for attribute in &event_ref.attributes {
        if attribute.attribute_type == "Age" {
            age = parse_age(&attribute.value).map(|parsed| convert::age_to_wit(&parsed));
        } else {
            attributes.push(Attribute {
                attribute_type: attribute.attribute_type.clone(),
                value: attribute.value.clone(),
            });
        }
    }
    let mut notes = Vec::new();
    for handle in &event_ref.note_refs {
        if let Some(note) = resolver.note(handle)? {
            notes.push(note);
        }
    }
    let mut citations = Vec::new();
    for handle in &event_ref.citation_refs {
        if let Some(citation) = resolver.citation(handle)? {
            citations.push(citation);
        }
    }
    Ok(LinkKind::Participation(ParticipationLink {
        person,
        event,
        role: event_ref
            .role
            .as_deref()
            .map_or(ParticipantRole::Primary, convert::gramps_role_to_participant_role),
        age,
        attributes,
        notes,
        citations,
    }))
}

/// Builds a `handle -> &record` index.
fn index<T>(records: &[T], handle: impl Fn(&T) -> &String) -> HashMap<String, &T> {
    records.iter().map(|record| (handle(record).clone(), record)).collect()
}

/// Maps a Gramps gender onto the host `sex` enum.
fn gender_to_sex(gender: Gender) -> Sex {
    match gender {
        Gender::Male => Sex::Male,
        Gender::Female => Sex::Female,
        Gender::Intersex => Sex::Intersex,
        Gender::Unknown => Sex::Unknown,
    }
}

/// Maps a Gramps confidence integer (0–4) onto the host `confidence` enum.
fn confidence_of(value: u8) -> Confidence {
    match value {
        0 => Confidence::VeryLow,
        1 => Confidence::Low,
        3 => Confidence::High,
        4 => Confidence::VeryHigh,
        _ => Confidence::Normal,
    }
}

/// Maps a Gramps note-type label onto the host `note-type` variant; a Gramps type with no vitni
/// counterpart is kept verbatim as a custom type.
fn note_type_of(label: &str) -> NoteType {
    match label {
        "General" => NoteType::General,
        "Research" => NoteType::Research,
        "Transcript" => NoteType::Transcript,
        "Citation" => NoteType::Citation,
        other => NoteType::Custom(other.to_owned()),
    }
}

/// Maps a Gramps place-type label onto the host `place-type` variant.
fn place_type_of(label: &str) -> PlaceType {
    match label {
        "Country" => PlaceType::Country,
        "County" | "State" | "Province" => PlaceType::County,
        "Municipality" => PlaceType::Municipality,
        "Parish" => PlaceType::Parish,
        "City" => PlaceType::City,
        "Town" => PlaceType::Town,
        "Village" => PlaceType::Village,
        "Farm" => PlaceType::Farm,
        "Building" => PlaceType::Building,
        other => PlaceType::Custom(other.to_owned()),
    }
}

export!(Importer);
