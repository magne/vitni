//! GEDCOM import plugin (ADR 0013, ADR 0040): read the document from the host-opened import source,
//! parse it with `vitni-gedcom`, and submit one record graph per record through the host `staging`
//! capability, reporting progress as it goes. The host resolves and writes the graphs; this crate only
//! walks the GEDCOM [`Tree`](vitni_gedcom::Tree) into them. The format-neutral plumbing (streaming,
//! progress, logging, the graph builder) and the interchange→WIT conversions live in
//! `vitni-plugin-api`.
//!
//! An `INDI` or `FAM` record is a graph of its own entity, its events (`event:BIRT:0`), citations
//! (`citation:0`) and notes (`note:0`). A `SOUR` or `REPO` record is a graph keyed by its xref. A place
//! and a media file have no record of their own: each is a graph keyed by its name (`plac:Mandal`) or
//! path (`file:scan.jpg`), submitted the first time a record refers to it.

wit_bindgen::generate!({
    world: "bulk-import",
    path: "../../crates/vitni-plugin-host/wit",
    with: {
        "vitni:host-api/types@0.27.0": vitni_plugin_api::types,
        "vitni:host-api/log@0.27.0": vitni_plugin_api::log,
        "vitni:host-api/staging@0.27.0": vitni_plugin_api::staging,
        "vitni:host-api/progress@0.27.0": vitni_plugin_api::progress,
        "vitni:host-api/import-source@0.27.0": vitni_plugin_api::import_source,
    },
});

use std::collections::{HashMap, HashSet};

use vitni_gedcom::{
    Age, Calendar, Citation, Date, DateModifier, Event, EventAssociation, Family, Header, Individual, MediaObject,
    Repository, Source, Tree,
};
use vitni_plugin_api::staging::{
    AssociationLink, ChildLink, EntityFields, EntityKind, EntityRef, LinkKind, MediaLink, MemberLink, PairLink,
    ParticipationLink, RepositoryLink, StagedChildRel, StagedCitation, StagedEvent, StagedFamily, StagedMedia,
    StagedNote, StagedPerson, StagedPlace, StagedRepository, StagedSource,
};
use vitni_plugin_api::types::{ExternalId, Fact, NoteType, ParticipantRole, SourceMediaType};
use vitni_plugin_api::{Graph, convert, origin_ref};

struct Importer;

impl Guest for Importer {
    fn run_import() -> Result<u32, String> {
        let text = vitni_plugin_api::read_source_to_string()?;
        let tree = vitni_gedcom::parse(&text).map_err(|error| error.to_string())?;
        let individuals = tree.individuals.len() as u32;
        let families = tree.families.len() as u32;
        vitni_plugin_api::log_info(&format!("importing {individuals} individuals and {families} families"));

        // Declare the header's fingerprint and the file's own export date once, before any record:
        // the host proposes the file's dataset by the one (ADR 0037 §3) and reconciles single-valued
        // fields by the other (ADR 0029 §2).
        let file_asserted_at = tree.header.date.as_ref().and_then(file_asserted_at_string);
        vitni_plugin_api::begin_run(dataset_hint(&tree.header).as_deref(), file_asserted_at.as_deref())?;

        let mut shared = Shared::new(&tree);
        let mut imported: u32 = 0;
        for (index, individual) in tree.individuals.iter().enumerate() {
            individual_graph(individual, &mut shared)?.submit()?;
            imported += 1;
            if !vitni_plugin_api::report("persons", index as u32 + 1, Some(individuals))? {
                return Ok(imported);
            }
        }
        for (index, family) in tree.families.iter().enumerate() {
            family_graph(family, &mut shared)?.submit()?;
            imported += 1;
            if !vitni_plugin_api::report("families", index as u32 + 1, Some(families))? {
                return Ok(imported);
            }
        }
        Ok(imported)
    }
}

/// The records several records refer to — sources, repositories, places and media files — each
/// submitted as a graph of its own the first time it is referred to.
struct Shared<'a> {
    persons: HashSet<&'a str>,
    sources: HashMap<&'a str, &'a Source>,
    repositories: HashMap<&'a str, &'a Repository>,
    /// The shared records submitted so far, by kind and record.
    submitted: HashSet<(&'static str, String)>,
}

impl<'a> Shared<'a> {
    fn new(tree: &'a Tree) -> Self {
        Self {
            persons: tree
                .individuals
                .iter()
                .map(|individual| individual.xref.as_str())
                .collect(),
            sources: tree
                .sources
                .iter()
                .map(|source| (source.xref.as_str(), source))
                .collect(),
            repositories: tree
                .repositories
                .iter()
                .map(|repo| (repo.xref.as_str(), repo))
                .collect(),
            submitted: HashSet::new(),
        }
    }

    /// Whether `record` of `kind` is yet to be submitted, marking it submitted.
    fn first(&mut self, kind: &'static str, record: &str) -> bool {
        self.submitted.insert((kind, record.to_owned()))
    }

    /// The person `xref` names, when the document holds it.
    fn person(&self, xref: &str) -> Option<EntityRef> {
        self.persons
            .contains(xref)
            .then(|| origin_ref(EntityKind::Person, xref, None))
    }

    /// The place named `name`; a GEDCOM place has no record of its own, so its name is its key.
    fn place(&mut self, name: &str) -> Result<EntityRef, String> {
        let record = format!("plac:{name}");
        if self.first("place", &record) {
            let mut graph = Graph::new(&record);
            graph.entity(
                None,
                EntityFields::Place(StagedPlace {
                    name: name.to_owned(),
                    place_type: None,
                    restrictions: Vec::new(),
                }),
            );
            graph.submit()?;
        }
        Ok(origin_ref(EntityKind::Place, &record, None))
    }

    /// The source `xref` names, titled from its `SOUR` record (untitled when the record is missing),
    /// with the repositories holding it.
    fn source(&mut self, xref: &str) -> Result<EntityRef, String> {
        if self.first("source", xref) {
            let source = self.sources.get(xref).copied();
            let mut graph = Graph::new(xref);
            let entity = graph.entity(
                None,
                EntityFields::Source(StagedSource {
                    title: source.and_then(|source| source.title.clone()),
                    author: source.and_then(|source| source.author.clone()),
                    pub_info: source.and_then(|source| source.pub_info.clone()),
                    abbrev: source.and_then(|source| source.abbrev.clone()),
                    restrictions: Vec::new(),
                }),
            );
            for held in source
                .map(|source| source.repository_refs.as_slice())
                .unwrap_or_default()
            {
                let repository = self.repository(&held.xref)?;
                let media_type = held.medium.as_ref().map_or(
                    SourceMediaType::Custom(String::new()),
                    convert::source_media_kind_to_wit,
                );
                graph.link(
                    None,
                    LinkKind::SourceRepository(RepositoryLink {
                        source: entity.clone(),
                        repository,
                        call_number: held.call_number.clone(),
                        media_type,
                    }),
                );
            }
            graph.submit()?;
        }
        Ok(origin_ref(EntityKind::Source, xref, None))
    }

    /// The repository `xref` names, named from its `REPO` record.
    fn repository(&mut self, xref: &str) -> Result<EntityRef, String> {
        if self.first("repository", xref) {
            let name = self
                .repositories
                .get(xref)
                .and_then(|repo| repo.name.clone())
                .unwrap_or_default();
            let mut graph = Graph::new(xref);
            graph.entity(
                None,
                EntityFields::Repository(StagedRepository {
                    name,
                    restrictions: Vec::new(),
                }),
            );
            graph.submit()?;
        }
        Ok(origin_ref(EntityKind::Repository, xref, None))
    }

    /// The media file `object` names, or `None` when it names no `FILE`; its path is its key.
    fn media(&mut self, object: &MediaObject) -> Result<Option<EntityRef>, String> {
        let Some(file) = &object.file else {
            return Ok(None);
        };
        let record = format!("file:{file}");
        if self.first("media", &record) {
            let mut graph = Graph::new(&record);
            graph.entity(
                None,
                EntityFields::Media(StagedMedia {
                    path: Some(file.clone()),
                    mime: object.mime.clone(),
                    restrictions: Vec::new(),
                }),
            );
            graph.submit()?;
        }
        Ok(Some(origin_ref(EntityKind::Media, &record, None)))
    }
}

/// The graph of one `INDI` record.
fn individual_graph(individual: &Individual, shared: &mut Shared<'_>) -> Result<Graph, String> {
    let mut graph = Graph::new(&individual.xref);
    let person = graph.entity(
        None,
        EntityFields::Person(StagedPerson {
            names: individual.names.iter().map(convert::name_to_wit).collect(),
            sex: individual.sex.map(convert::sex_to_wit),
            facts: individual
                .facts
                .iter()
                .map(|fact| Fact {
                    fact_type: convert::fact_type_to_wit(fact.kind),
                    value: fact.value.clone(),
                    date: fact.date.as_ref().map(convert::date_to_wit),
                })
                .collect(),
            external_ids: individual.uid.as_deref().map(uid_external_id).into_iter().collect(),
            restrictions: convert::restrictions_to_wit(&individual.restrictions),
        }),
    );
    let mut events = EventKeys::default();
    for event in &individual.events {
        let item = events.next(event);
        let participants = [(person.clone(), event.age.as_ref())];
        add_event(&mut graph, shared, &item, event, &participants)?;
    }
    let owner = Owner {
        entity: &person,
        citations: &individual.citations,
        media: &individual.media,
        notes: &individual.notes,
    };
    add_owned(&mut graph, shared, &owner)?;
    for association in &individual.associations {
        if let Some(other) = shared.person(&association.other_xref) {
            graph.link(
                None,
                LinkKind::Association(AssociationLink {
                    person: person.clone(),
                    other,
                    role: convert::association_role_to_wit(association.role.as_ref()),
                }),
            );
        }
    }
    Ok(graph)
}

/// The graph of one `FAM` record.
fn family_graph(family: &Family, shared: &mut Shared<'_>) -> Result<Graph, String> {
    let mut graph = Graph::new(&family.xref);
    let entity = graph.entity(
        None,
        EntityFields::Family(StagedFamily {
            external_ids: family.uid.as_deref().map(uid_external_id).into_iter().collect(),
            restrictions: convert::restrictions_to_wit(&family.restrictions),
        }),
    );
    let partners: Vec<EntityRef> = family
        .partners
        .iter()
        .filter_map(|partner| shared.person(partner))
        .collect();
    for partner in &partners {
        graph.link(
            None,
            LinkKind::Partner(MemberLink {
                family: entity.clone(),
                person: partner.clone(),
            }),
        );
    }
    for child in &family.children {
        let Some(child_ref) = shared.person(&child.xref) else {
            continue;
        };
        let mut relationships = Vec::new();
        if let (Some(father), Some(frel)) = (partners.first(), &child.father_relationship) {
            relationships.push(StagedChildRel {
                partner: father.clone(),
                relationship: convert::child_relationship_to_wit(frel),
            });
        }
        if let (Some(mother), Some(mrel)) = (partners.get(1), &child.mother_relationship) {
            relationships.push(StagedChildRel {
                partner: mother.clone(),
                relationship: convert::child_relationship_to_wit(mrel),
            });
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
    let mut events = EventKeys::default();
    for event in &family.events {
        let item = events.next(event);
        let ages = [event.husband_age.as_ref(), event.wife_age.as_ref()];
        let participants: Vec<(EntityRef, Option<&Age>)> = partners
            .iter()
            .enumerate()
            .map(|(index, partner)| (partner.clone(), ages.get(index).copied().flatten()))
            .collect();
        let event_ref = add_event(&mut graph, shared, &item, event, &participants)?;
        graph.link(
            None,
            LinkKind::FamilyEvent(PairLink {
                owner: entity.clone(),
                target: event_ref,
            }),
        );
    }
    let owner = Owner {
        entity: &entity,
        citations: &family.citations,
        media: &family.media,
        notes: &family.notes,
    };
    add_owned(&mut graph, shared, &owner)?;
    Ok(graph)
}

/// A record's own entity with the citations, media and notes attached to it.
struct Owner<'a> {
    entity: &'a EntityRef,
    citations: &'a [Citation],
    media: &'a [MediaObject],
    notes: &'a [String],
}

/// Adds the owner's citations (`citation:<n>`), media and notes (`note:<n>`), each attached to it.
fn add_owned(graph: &mut Graph, shared: &mut Shared<'_>, owner: &Owner<'_>) -> Result<(), String> {
    for (n, citation) in owner.citations.iter().enumerate() {
        let citation = add_citation(graph, shared, &format!("citation:{n}"), citation)?;
        graph.link(
            None,
            LinkKind::CitationOf(PairLink {
                owner: owner.entity.clone(),
                target: citation,
            }),
        );
    }
    for object in owner.media {
        if let Some(media) = shared.media(object)? {
            graph.link(
                None,
                LinkKind::MediaOf(MediaLink {
                    owner: owner.entity.clone(),
                    media,
                    crop: None,
                    caption: object.caption.clone(),
                }),
            );
        }
    }
    for (n, text) in owner.notes.iter().enumerate() {
        let note = add_note(graph, &format!("note:{n}"), text, None);
        graph.link(
            None,
            LinkKind::NoteOf(PairLink {
                owner: owner.entity.clone(),
                target: note,
            }),
        );
    }
    Ok(())
}

/// Adds an event under `item`, with its date, address and place, each participant as the primary with
/// their age, and its `ASSO` witnesses (`<item>:asso:<n>`) — returning a reference to it.
fn add_event(
    graph: &mut Graph,
    shared: &mut Shared<'_>,
    item: &str,
    event: &Event,
    participants: &[(EntityRef, Option<&Age>)],
) -> Result<EntityRef, String> {
    let entity = graph.entity(
        Some(item),
        EntityFields::Event(StagedEvent {
            event_type: convert::event_type_to_wit(event.kind),
            date: event.date.as_ref().map(convert::date_to_wit),
            addresses: event.address.iter().map(convert::address_to_wit).collect(),
            restrictions: Vec::new(),
        }),
    );
    if let Some(place) = &event.place {
        // The place's point/geometry (`PLAC.MAP`, ADR 0024) is not yet threaded through the plugin
        // boundary — a follow-up; only the name is linked.
        let place = shared.place(&place.name)?;
        graph.link(
            Some(item),
            LinkKind::EventPlace(PairLink {
                owner: entity.clone(),
                target: place,
            }),
        );
    }
    for (person, age) in participants {
        graph.link(
            Some(item),
            LinkKind::Participation(ParticipationLink {
                person: person.clone(),
                event: entity.clone(),
                role: ParticipantRole::Primary,
                age: age.map(convert::age_to_wit),
                attributes: Vec::new(),
                notes: Vec::new(),
                citations: Vec::new(),
            }),
        );
    }
    for (n, association) in event.associations.iter().enumerate() {
        add_witness(graph, shared, &format!("{item}:asso:{n}"), &entity, association)?;
    }
    Ok(entity)
}

/// Adds an event-level `ASSO` witness's participation under `item`, with its notes and citations,
/// when the witness is a person of the document.
fn add_witness(
    graph: &mut Graph,
    shared: &mut Shared<'_>,
    item: &str,
    event: &EntityRef,
    association: &EventAssociation,
) -> Result<(), String> {
    let Some(witness) = shared.person(&association.other_xref) else {
        return Ok(());
    };
    let mut notes = Vec::with_capacity(association.notes.len());
    for (n, text) in association.notes.iter().enumerate() {
        notes.push(add_note(graph, &format!("{item}:note:{n}"), text, None));
    }
    let mut citations = Vec::with_capacity(association.citations.len());
    for (n, citation) in association.citations.iter().enumerate() {
        citations.push(add_citation(graph, shared, &format!("{item}:citation:{n}"), citation)?);
    }
    graph.link(
        Some(item),
        LinkKind::Participation(ParticipationLink {
            person: witness,
            event: event.clone(),
            role: convert::association_kind_to_participant_role(association.role.as_ref()),
            age: None,
            attributes: Vec::new(),
            notes,
            citations,
        }),
    );
    Ok(())
}

/// Adds a citation under `item` of the source it cites, with each `DATA.TEXT` transcription as a
/// `Transcript` note attached to it (`<item>:transcript:<n>`, data-model §6).
fn add_citation(
    graph: &mut Graph,
    shared: &mut Shared<'_>,
    item: &str,
    citation: &Citation,
) -> Result<EntityRef, String> {
    let source = shared.source(&citation.source_xref)?;
    let entity = graph.entity(
        Some(item),
        EntityFields::Citation(StagedCitation {
            source,
            page: citation.page.clone(),
            confidence: None,
            restrictions: Vec::new(),
        }),
    );
    for (n, text) in citation.transcriptions.iter().enumerate() {
        let note = add_note(
            graph,
            &format!("{item}:transcript:{n}"),
            text,
            Some(NoteType::Transcript),
        );
        graph.link(
            Some(item),
            LinkKind::NoteOf(PairLink {
                owner: entity.clone(),
                target: note,
            }),
        );
    }
    Ok(entity)
}

fn add_note(graph: &mut Graph, item: &str, text: &str, note_type: Option<NoteType>) -> EntityRef {
    graph.entity(
        Some(item),
        EntityFields::Note(StagedNote {
            text: text.to_owned(),
            note_type,
            restrictions: Vec::new(),
        }),
    )
}

/// Numbers a record's events per tag, so an event's item key (`event:BIRT:0`, `event:BIRT:1`) stays
/// the same when events of another kind are added before it.
#[derive(Default)]
struct EventKeys {
    seen: HashMap<&'static str, usize>,
}

impl EventKeys {
    fn next(&mut self, event: &Event) -> String {
        let tag = vitni_gedcom::event_tag(event.kind);
        let n = self.seen.entry(tag).or_insert(0);
        let key = format!("event:{tag}:{n}");
        *n += 1;
        key
    }
}

/// The header's fingerprint: the writing product (`HEAD.SOUR`) and the file name it recorded
/// (`HEAD.FILE`), which stay the same across re-exports of one tree. `None` without a file name: a
/// product alone is shared by unrelated files, and so are their xrefs (`I1`), so such a file is
/// proposed no dataset (ADR 0037 §3).
fn dataset_hint(header: &Header) -> Option<String> {
    let file = header.file.as_deref()?;
    let source = header.source.as_deref().unwrap_or_default();
    Some(format!("{source}|{file}"))
}

/// Renders the `HEAD.1 DATE` value as an RFC 3339 timestamp for `staging::begin-run` (ADR 0029 §2), or
/// `None` unless it is a fully-specified Gregorian calendar day: `TextOnly`, a modifier other than an
/// exact point (a range/span/estimate — GEDCOM header dates do not carry these in practice, but a
/// hand-edited file could), a non-Gregorian calendar, or a partial date (bare year, or year+month with
/// no day) all degrade to `None` rather than guess a specific instant — the same "honest about
/// carrying no structure" stance as an unparseable date (ADR 0029 §3).
fn file_asserted_at_string(date: &Date) -> Option<String> {
    if date.calendar != Calendar::Gregorian {
        return None;
    }
    let DateModifier::Exact(point) = &date.modifier else {
        return None;
    };
    let (year, month, day) = (point.year?, point.month?, point.day?);
    Some(format!("{year:04}-{month:02}-{day:02}T00:00:00Z"))
}

/// Builds the external id a `_UID` names (authority `gedcom-uid`). A `_UID` means the same person or
/// family in every file that carries it, so another dataset's copy resolves onto it. The xref is
/// file-local and never becomes an external id: it is the record's origin only (ADR 0037 §3).
fn uid_external_id(uid: &str) -> ExternalId {
    ExternalId {
        authority: "gedcom-uid".to_owned(),
        value: uid.to_owned(),
        kind: None,
        url: None,
    }
}

export!(Importer);
