//! Gramps XML export plugin (ADR 0013, ADR 0018): read persons, families, events, and the owned
//! records (places, sources, citations, media, notes, repositories) through the host `query`
//! capability, serialize them to Gramps XML with `genealogy-gramps-xml`, and write the document to
//! the host-resolved export sink.
//!
//! Human ids become Gramps handles and `gramps_id`s. Events are distributed onto the record they
//! belong under the way the importer placed them — a family-event kind whose participant set matches
//! a family's partners is referenced by that family, every other event by each participant person —
//! mirroring `gramps-import`, so an import → export → import cycle is stable.

wit_bindgen::generate!({
    world: "bulk-export",
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.16.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.16.0": genealogy_plugin_api::log,
        "genealogy:host-api/query@0.16.0": genealogy_plugin_api::query,
        "genealogy:host-api/progress@0.16.0": genealogy_plugin_api::progress,
        "genealogy:host-api/export-sink@0.16.0": genealogy_plugin_api::export_sink,
    },
});

use std::collections::{BTreeSet, HashMap};

use genealogy_gramps_xml::{
    ChildRef, Citation, Database, Event, EventRef, EventRefAttribute, Family, Gender, MediaObject, Note, Person,
    PersonRef, Place, Repository, Source,
};
use genealogy_interchange::{EventKind, Name, age_value};
use genealogy_plugin_api::{convert, query, types};

struct Exporter;

/// A participant in an event, as the exporter reconstructs it from a person's participation: the
/// participant's human id (a Gramps handle), their role, age, attributes, and note human-ids.
struct ParticipantInfo {
    person: String,
    role: types::ParticipantRole,
    age: Option<types::Age>,
    attributes: Vec<types::Attribute>,
    notes: Vec<String>,
}

impl ParticipantInfo {
    /// Whether this participation carries anything beyond a bare primary link (so it needs a
    /// person-side `<eventref>` with a role attribute or `<attribute>`/`<noteref>` children).
    fn has_payload(&self) -> bool {
        self.role != types::ParticipantRole::Primary
            || self.age.is_some()
            || !self.attributes.is_empty()
            || !self.notes.is_empty()
    }

    /// Builds the person-side `<eventref>` for this participation (bare when it carries no payload).
    fn event_ref(&self, event_id: &str) -> EventRef {
        let mut attributes = Vec::new();
        if let Some(age) = &self.age {
            attributes.push(EventRefAttribute {
                attribute_type: "Age".to_owned(),
                value: age_value(&convert::age_from_wit(age)),
            });
        }
        for attribute in &self.attributes {
            attributes.push(EventRefAttribute {
                attribute_type: attribute.attribute_type.clone(),
                value: attribute.value.clone(),
            });
        }
        EventRef {
            hlink: event_id.to_owned(),
            // Gramps defaults an unstated role to Primary, so a primary participant omits it.
            role: (self.role != types::ParticipantRole::Primary)
                .then(|| convert::participant_role_to_gramps_role(self.role)),
            attributes,
            note_refs: self.notes.clone(),
            // Participation citations are import-only (they ride the assertion envelope, ADR 0020).
            citation_refs: Vec::new(),
        }
    }
}

impl Guest for Exporter {
    fn run_export() -> Result<u32, String> {
        let persons = query::list_persons().map_err(|e| format!("list-persons failed: {e:?}"))?;
        let families = query::list_families().map_err(|e| format!("list-families failed: {e:?}"))?;
        let events = query::list_events().map_err(|e| format!("list-events failed: {e:?}"))?;
        let sources = query::list_sources().map_err(|e| format!("list-sources failed: {e:?}"))?;
        let citations = query::list_citations().map_err(|e| format!("list-citations failed: {e:?}"))?;
        let media = query::list_media().map_err(|e| format!("list-media failed: {e:?}"))?;
        let notes = query::list_notes().map_err(|e| format!("list-notes failed: {e:?}"))?;
        let repositories = query::list_repositories().map_err(|e| format!("list-repositories failed: {e:?}"))?;
        let places = query::list_places().map_err(|e| format!("list-places failed: {e:?}"))?;
        let total = (persons.len() + families.len()) as u32;
        genealogy_plugin_api::log_info(&format!("exporting {} people and {} families", persons.len(), families.len()));

        // event human-id -> participants (role + age + attributes + notes), from each person's
        // participations.
        let mut event_participants: HashMap<String, Vec<ParticipantInfo>> = HashMap::new();
        for person in &persons {
            for participation in &person.participations {
                event_participants.entry(participation.event.clone()).or_default().push(ParticipantInfo {
                    person: person.human_id.clone(),
                    role: participation.role,
                    age: participation.age.clone(),
                    attributes: participation.attributes.clone(),
                    notes: participation.notes.clone(),
                });
            }
        }

        let mut people: Vec<Person> = persons.into_iter().map(person).collect();
        let person_index: HashMap<String, usize> =
            people.iter().enumerate().map(|(i, p)| (p.handle.clone(), i)).collect();
        // event handle -> family index, from each family's explicit event links.
        let family_event_links: HashMap<String, usize> = families
            .iter()
            .enumerate()
            .flat_map(|(index, family)| family.events.iter().map(move |event| (event.clone(), index)))
            .collect();
        let mut family_records: Vec<Family> = families.into_iter().map(family).collect();

        distribute_events(
            &events,
            &event_participants,
            &family_event_links,
            &mut people,
            &person_index,
            &mut family_records,
        );

        let db = Database {
            people,
            families: family_records,
            events: events.into_iter().map(event).collect(),
            places: places.into_iter().map(place).collect(),
            sources: sources.into_iter().map(source).collect(),
            citations: citations.into_iter().map(citation).collect(),
            repositories: repositories.into_iter().map(repository).collect(),
            objects: media.into_iter().map(media_object).collect(),
            notes: notes.into_iter().map(note).collect(),
            tags: Vec::new(),
        };

        if !genealogy_plugin_api::report("serialize", 0, Some(total))? {
            return Ok(0);
        }
        genealogy_plugin_api::write_export("export.gramps", &genealogy_gramps_xml::emit(&db))?;
        genealogy_plugin_api::report("written", total, Some(total))?;
        Ok(total)
    }
}

/// Routes each event's handle onto the record it belongs under: a family-event kind whose participant
/// set matches a family's partners is referenced by that family; every other event by each
/// participant person.
fn distribute_events(
    events: &[types::EventDto],
    event_participants: &HashMap<String, Vec<ParticipantInfo>>,
    family_event_links: &HashMap<String, usize>,
    people: &mut [Person],
    person_index: &HashMap<String, usize>,
    families: &mut [Family],
) {
    let family_partner_sets: Vec<BTreeSet<String>> = families
        .iter()
        .map(|f| f.father.iter().chain(f.mother.iter()).cloned().collect())
        .collect();
    let empty: Vec<ParticipantInfo> = Vec::new();
    for event_dto in events {
        let Some(kind) = event_dto.event_type.map(convert::event_type_from_wit) else {
            continue;
        };
        let participants = event_participants.get(&event_dto.human_id).unwrap_or(&empty);
        // An explicit family↔event link references the event from its family directly; otherwise fall
        // back to the participant-set heuristic. Either way, a partner carrying a payload (age, a
        // witness role, …) also gets a person-side `<eventref>` so the payload round-trips.
        if let Some(&index) = family_event_links.get(&event_dto.human_id) {
            families[index].event_refs.push(EventRef::bare(event_dto.human_id.as_str()));
            push_payload_event_refs(participants, &event_dto.human_id, people, person_index);
            continue;
        }
        if is_family_event(kind) {
            let set: BTreeSet<String> = participants.iter().map(|p| p.person.clone()).collect();
            if let Some(index) = family_partner_sets.iter().position(|partners| *partners == set) {
                families[index].event_refs.push(EventRef::bare(event_dto.human_id.as_str()));
                push_payload_event_refs(participants, &event_dto.human_id, people, person_index);
                continue;
            }
        }
        for participant in participants {
            if let Some(&index) = person_index.get(&participant.person) {
                people[index].event_refs.push(participant.event_ref(&event_dto.human_id));
            }
        }
    }
}

/// Adds a person-side `<eventref>` (with its payload) for every participant that carries payload —
/// used on the family-event path, where the family already references the event and the seen-set on
/// re-import keeps a partner single-asserted.
fn push_payload_event_refs(
    participants: &[ParticipantInfo],
    event_id: &str,
    people: &mut [Person],
    person_index: &HashMap<String, usize>,
) {
    for participant in participants {
        if participant.has_payload()
            && let Some(&index) = person_index.get(&participant.person)
        {
            people[index].event_refs.push(participant.event_ref(event_id));
        }
    }
}

/// Whether an event kind is a family-level event (mirrors `gedcom-export`).
fn is_family_event(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Marriage
            | EventKind::Divorce
            | EventKind::DivorceFiled
            | EventKind::Engagement
            | EventKind::Annulment
            | EventKind::MarriageBanns
            | EventKind::MarriageContract
            | EventKind::MarriageLicense
            | EventKind::MarriageSettlement
    )
}

fn person(dto: types::PersonDto) -> Person {
    let has_name = dto.given.is_some()
        || dto.surname.is_some()
        || dto.surname_prefix.is_some()
        || dto.nickname.is_some()
        || dto.name_prefix.is_some()
        || dto.name_suffix.is_some();
    let name = has_name.then(|| Name {
        name_type: dto.name_type.map(convert::name_type_from_wit),
        given: dto.given,
        surname_prefix: dto.surname_prefix,
        surname: dto.surname,
        nickname: dto.nickname,
        prefix: dto.name_prefix,
        suffix: dto.name_suffix,
    });
    Person {
        handle: dto.human_id,
        gramps_id: None,
        name,
        gender: dto.sex.map(|sex| gender_of(convert::sex_from_wit(sex))),
        // Filled by `distribute_events`.
        event_refs: Vec::new(),
        citation_refs: dto.citations,
        note_refs: dto.notes,
        media_refs: dto.media,
        person_refs: dto
            .associations
            .into_iter()
            .map(|a| PersonRef {
                hlink: a.other,
                rel: Some(convert::association_role_from_wit(a.role)),
            })
            .collect(),
        private: convert::private_from_wit(&dto.restrictions),
    }
}

fn family(dto: types::FamilyDto) -> Family {
    let father = dto.partners.first().cloned();
    let mother = dto.partners.get(1).cloned();
    let child_refs = dto
        .children
        .iter()
        .map(|child| {
            let rel_for = |partner: &Option<String>| {
                partner.as_ref().and_then(|target| {
                    child
                        .relationships
                        .iter()
                        .find(|rel| &rel.partner == target)
                        .map(|rel| convert::child_relationship_from_wit(&rel.relationship))
                })
            };
            ChildRef {
                hlink: child.human_id.clone(),
                mother_relationship: rel_for(&mother),
                father_relationship: rel_for(&father),
            }
        })
        .collect();
    Family {
        handle: dto.human_id,
        gramps_id: None,
        father,
        mother,
        child_refs,
        event_refs: Vec::new(),
        private: convert::private_from_wit(&dto.restrictions),
    }
}

fn event(dto: types::EventDto) -> Event {
    Event {
        handle: dto.human_id,
        gramps_id: None,
        kind: dto.event_type.map_or(EventKind::Birth, convert::event_type_from_wit),
        date: dto.date.as_ref().map(convert::date_from_wit),
        place_ref: dto.place,
        description: dto.description,
    }
}

fn place(dto: types::PlaceDto) -> Place {
    Place {
        handle: dto.human_id,
        gramps_id: None,
        name: dto.name,
        place_type: dto.place_type.map(place_type_label),
        enclosed_by: dto.enclosed_by,
    }
}

fn source(dto: types::SourceDto) -> Source {
    Source {
        handle: dto.human_id,
        gramps_id: None,
        title: dto.title,
        author: dto.author,
        pub_info: dto.pub_info,
        repository_refs: dto.repositories,
    }
}

fn citation(dto: types::CitationDto) -> Citation {
    Citation {
        handle: dto.human_id,
        gramps_id: None,
        source_ref: dto.source,
        page: dto.page,
        confidence: dto.confidence.map(confidence_value),
    }
}

fn repository(dto: types::RepositoryDto) -> Repository {
    Repository {
        handle: dto.human_id,
        gramps_id: None,
        name: dto.name,
    }
}

fn media_object(dto: types::MediaDto) -> MediaObject {
    MediaObject {
        handle: dto.human_id,
        gramps_id: None,
        file: dto.path,
        mime: dto.mime,
    }
}

fn note(dto: types::NoteDto) -> Note {
    Note {
        handle: dto.human_id,
        gramps_id: None,
        text: dto.text,
    }
}

/// Maps the host `sex` (already converted to interchange) onto a Gramps gender.
fn gender_of(sex: genealogy_interchange::Sex) -> Gender {
    match sex {
        genealogy_interchange::Sex::Male => Gender::Male,
        genealogy_interchange::Sex::Female => Gender::Female,
        genealogy_interchange::Sex::Intersex => Gender::Intersex,
        genealogy_interchange::Sex::Unknown => Gender::Unknown,
    }
}

/// Maps the host `confidence` onto a Gramps confidence integer (0–4).
fn confidence_value(confidence: types::Confidence) -> u8 {
    match confidence {
        types::Confidence::VeryLow => 0,
        types::Confidence::Low => 1,
        types::Confidence::Normal => 2,
        types::Confidence::High => 3,
        types::Confidence::VeryHigh => 4,
    }
}

/// Maps the host `place-type` onto a Gramps place-type label.
fn place_type_label(place_type: types::PlaceType) -> String {
    match place_type {
        types::PlaceType::Country => "Country".to_owned(),
        types::PlaceType::County => "County".to_owned(),
        types::PlaceType::Municipality => "Municipality".to_owned(),
        types::PlaceType::Parish => "Parish".to_owned(),
        types::PlaceType::City => "City".to_owned(),
        types::PlaceType::Town => "Town".to_owned(),
        types::PlaceType::Village => "Village".to_owned(),
        types::PlaceType::Farm => "Farm".to_owned(),
        types::PlaceType::Building => "Building".to_owned(),
        types::PlaceType::Custom(value) => value,
    }
}

export!(Exporter);
