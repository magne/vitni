//! GEDCOM export plugin (ADR 0013): read persons, families, events, and sources through the host
//! `query` capability, serialize them to GEDCOM with `genealogy-gedcom`, and write the document to
//! the host-resolved export sink, reporting progress. Human ids become GEDCOM xrefs. The
//! format-neutral plumbing and the WIT→interchange conversions live in `genealogy-plugin-api`; this
//! crate only bridges the DTOs to the GEDCOM [`Tree`](genealogy_gedcom::Tree).
//!
//! Events are distributed back to the records they belong under the way the importer placed them: a
//! family-event kind (marriage, divorce, …) whose participant set matches a family's partners nests
//! under that `FAM`; every other event nests under each participant `INDI`. This mirrors
//! `gedcom-import`'s `import_event`, so an import → export → import cycle is stable.

wit_bindgen::generate!({
    world: "bulk-export",
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.14.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.14.0": genealogy_plugin_api::log,
        "genealogy:host-api/query@0.14.0": genealogy_plugin_api::query,
        "genealogy:host-api/progress@0.14.0": genealogy_plugin_api::progress,
        "genealogy:host-api/export-sink@0.14.0": genealogy_plugin_api::export_sink,
    },
});

use std::collections::{BTreeSet, HashMap};

use genealogy_gedcom::{Association, Event, EventAssociation, EventKind, Fact, Name};
use genealogy_plugin_api::{convert, query, types};

/// A participant in an event, as the exporter reconstructs it from a person's participation: the
/// participant's human id (a GEDCOM xref), their role, age at the event, and note human-ids.
struct ParticipantInfo {
    person: String,
    role: types::ParticipantRole,
    age: Option<types::Age>,
    notes: Vec<String>,
}

struct Exporter;

impl Guest for Exporter {
    fn run_export() -> Result<u32, String> {
        let persons = query::list_persons().map_err(|error| format!("list-persons failed: {error:?}"))?;
        let families = query::list_families().map_err(|error| format!("list-families failed: {error:?}"))?;
        let events = query::list_events().map_err(|error| format!("list-events failed: {error:?}"))?;
        let sources = query::list_sources().map_err(|error| format!("list-sources failed: {error:?}"))?;
        let citations = query::list_citations().map_err(|error| format!("list-citations failed: {error:?}"))?;
        let media = query::list_media().map_err(|error| format!("list-media failed: {error:?}"))?;
        let notes = query::list_notes().map_err(|error| format!("list-notes failed: {error:?}"))?;
        let person_count = persons.len() as u32;
        let family_count = families.len() as u32;
        let total = person_count + family_count;
        genealogy_plugin_api::log_info(&format!(
            "exporting {person_count} individuals and {family_count} families"
        ));

        // Participation is recorded on the person (not the event), so build the event -> participant
        // map (role + age + notes per participant) from every person's participations before
        // consuming the person DTOs.
        let mut event_participants: HashMap<String, Vec<ParticipantInfo>> = HashMap::new();
        for person in &persons {
            for participation in &person.participations {
                event_participants
                    .entry(participation.event.clone())
                    .or_default()
                    .push(ParticipantInfo {
                        person: person.human_id.clone(),
                        role: participation.role,
                        age: participation.age.clone(),
                        notes: participation.notes.clone(),
                    });
            }
        }

        // Owned-record content keyed by human id, so each person's attached citations/media/notes
        // reconstruct their INDI.SOUR/OBJE/NOTE content.
        let citation_content: HashMap<String, genealogy_gedcom::Citation> = citations
            .into_iter()
            .map(|c| {
                (
                    c.human_id,
                    genealogy_gedcom::Citation {
                        source_xref: c.source.unwrap_or_default(),
                        page: c.page,
                    },
                )
            })
            .collect();
        let media_content: HashMap<String, genealogy_gedcom::MediaObject> = media
            .into_iter()
            .map(|m| {
                (
                    m.human_id,
                    genealogy_gedcom::MediaObject {
                        file: m.path,
                        title: None,
                        mime: m.mime,
                    },
                )
            })
            .collect();
        let note_content: HashMap<String, String> =
            notes.into_iter().filter_map(|n| n.text.map(|text| (n.human_id, text))).collect();

        let mut individuals: Vec<genealogy_gedcom::Individual> = persons
            .into_iter()
            .map(|person| individual(person, &citation_content, &media_content, &note_content))
            .collect();
        let individual_index: HashMap<String, usize> = individuals
            .iter()
            .enumerate()
            .map(|(index, individual)| (individual.xref.clone(), index))
            .collect();
        // event human-id -> family index, from each family's explicit event links.
        let family_event_links: HashMap<String, usize> = families
            .iter()
            .enumerate()
            .flat_map(|(index, family)| family.events.iter().map(move |event| (event.clone(), index)))
            .collect();
        let mut families: Vec<genealogy_gedcom::Family> = families
            .into_iter()
            .map(|family| {
                let children = family
                    .children
                    .iter()
                    .map(|child| child_ref(child, &family.partners))
                    .collect();
                genealogy_gedcom::Family {
                    xref: family.human_id,
                    uid: None,
                    partners: family.partners,
                    children,
                    events: Vec::new(),
                    restrictions: convert::restrictions_from_wit(&family.restrictions),
                }
            })
            .collect();

        distribute_events(
            events,
            &event_participants,
            &family_event_links,
            &mut individuals,
            &individual_index,
            &mut families,
            &note_content,
        );

        let tree = genealogy_gedcom::Tree {
            individuals,
            families,
            sources: sources
                .into_iter()
                .map(|source| genealogy_gedcom::Source {
                    xref: source.human_id,
                    title: source.title,
                    author: source.author,
                    pub_info: source.pub_info,
                })
                .collect(),
        };

        if !genealogy_plugin_api::report("serialize", 0, Some(total))? {
            return Ok(0);
        }
        let document = genealogy_gedcom::emit(&tree).into_bytes();
        genealogy_plugin_api::write_export("export.ged", &document)?;
        genealogy_plugin_api::report("written", total, Some(total))?;

        Ok(total)
    }
}

/// Routes each event onto the record it belongs under: a family-event kind whose participant set
/// matches a family's partners nests under that family; every other event nests under each
/// participant individual. `event_participants` maps an event's human id to its participants'. An
/// event whose type does not map to a GEDCOM tag is skipped (it cannot be represented), as is a
/// participant the export does not know.
fn distribute_events(
    events: Vec<types::EventDto>,
    event_participants: &HashMap<String, Vec<ParticipantInfo>>,
    family_event_links: &HashMap<String, usize>,
    individuals: &mut [genealogy_gedcom::Individual],
    individual_index: &HashMap<String, usize>,
    families: &mut [genealogy_gedcom::Family],
    note_content: &HashMap<String, String>,
) {
    let family_partner_sets: Vec<BTreeSet<String>> = families
        .iter()
        .map(|family| family.partners.iter().cloned().collect())
        .collect();
    for event_dto in events {
        let Some(kind) = event_dto.event_type.map(convert::event_type_from_wit) else {
            continue;
        };
        let empty: Vec<ParticipantInfo> = Vec::new();
        let participants = event_participants.get(&event_dto.human_id).unwrap_or(&empty);
        let base = Event {
            kind,
            date: event_dto.date.as_ref().map(convert::date_from_wit),
            place: event_dto.place.clone(),
            address: event_dto.addresses.first().map(convert::address_from_wit),
            age: None,
            husband_age: None,
            wife_age: None,
            associations: Vec::new(),
        };
        // An explicit family↔event link nests the event under its family directly (robust even when
        // the event has no participants); otherwise fall back to the participant-set heuristic.
        if let Some(&index) = family_event_links.get(&event_dto.human_id) {
            families[index].events.push(family_event(base, participants, note_content));
            continue;
        }
        if is_family_event(kind) {
            let set: BTreeSet<String> = participants.iter().map(|p| p.person.clone()).collect();
            if let Some(index) = family_partner_sets.iter().position(|partners| *partners == set) {
                let partners = families[index].partners.clone();
                families[index].events.push(family_event_for(base, participants, &partners, note_content));
                continue;
            }
        }
        nest_individual_event(&base, participants, individuals, individual_index, note_content);
    }
}

/// Builds a family event with the partners taken from the family the event links to.
fn family_event(base: Event, participants: &[ParticipantInfo], note_content: &HashMap<String, String>) -> Event {
    // The explicit-link path has no partner order to hand; recover it from the primary participants.
    let partners: Vec<String> = participants
        .iter()
        .filter(|p| p.role == types::ParticipantRole::Primary)
        .map(|p| p.person.clone())
        .collect();
    family_event_for(base, participants, &partners, note_content)
}

/// Fills a family event's `HUSB`/`WIFE` ages (from the partners, positionally) and its `ASSO`
/// witnesses (every non-partner, non-primary participant).
fn family_event_for(
    mut event: Event,
    participants: &[ParticipantInfo],
    partners: &[String],
    note_content: &HashMap<String, String>,
) -> Event {
    for participant in participants {
        if partners.first() == Some(&participant.person) {
            event.husband_age = participant.age.as_ref().map(convert::age_from_wit);
        } else if partners.get(1) == Some(&participant.person) {
            event.wife_age = participant.age.as_ref().map(convert::age_from_wit);
        } else if participant.role != types::ParticipantRole::Primary {
            event.associations.push(witness_association(participant, note_content));
        }
    }
    event
}

/// Nests an individual event under each primary participant (with their age), attaching every
/// non-primary participant as an `ASSO` witness. An event with no primary participant falls back to
/// nesting under every participant so nothing is dropped.
fn nest_individual_event(
    base: &Event,
    participants: &[ParticipantInfo],
    individuals: &mut [genealogy_gedcom::Individual],
    individual_index: &HashMap<String, usize>,
    note_content: &HashMap<String, String>,
) {
    let primaries: Vec<&ParticipantInfo> = participants
        .iter()
        .filter(|p| p.role == types::ParticipantRole::Primary)
        .collect();
    // With a primary present, non-primary participants ride as `ASSO` witnesses under it. With no
    // primary, every participant nests as its own event copy (nothing dropped) and carries no `ASSO`.
    let (hosts, associations): (Vec<&ParticipantInfo>, Vec<EventAssociation>) = if primaries.is_empty() {
        (participants.iter().collect(), Vec::new())
    } else {
        let associations = participants
            .iter()
            .filter(|p| p.role != types::ParticipantRole::Primary)
            .map(|p| witness_association(p, note_content))
            .collect();
        (primaries, associations)
    };
    for participant in hosts {
        let Some(&index) = individual_index.get(&participant.person) else {
            continue;
        };
        let mut event = base.clone();
        event.age = participant.age.as_ref().map(convert::age_from_wit);
        event.associations = associations.clone();
        individuals[index].events.push(event);
    }
}

/// Builds a GEDCOM event-level `ASSO` witness from a non-primary participant: the role and the
/// participant's notes (resolved to their text). Citations are left empty — participation citations
/// are import-only (they ride the assertion envelope, ADR 0020, and are not re-emitted).
fn witness_association(participant: &ParticipantInfo, note_content: &HashMap<String, String>) -> EventAssociation {
    EventAssociation {
        other_xref: participant.person.clone(),
        role: convert::participant_role_to_association_kind(participant.role),
        citations: Vec::new(),
        notes: participant
            .notes
            .iter()
            .filter_map(|human_id| note_content.get(human_id).cloned())
            .collect(),
    }
}

/// Builds a GEDCOM [`ChildRef`](genealogy_gedcom::ChildRef) from a family child, mapping the child's
/// relationship to the first partner onto `_FREL` (father) and to the second onto `_MREL` (mother).
fn child_ref(child: &types::FamilyChild, partners: &[String]) -> genealogy_gedcom::ChildRef {
    let rel_for = |partner: Option<&String>| -> Option<String> {
        partner.and_then(|target| {
            child
                .relationships
                .iter()
                .find(|rel| &rel.partner == target)
                .map(|rel| convert::child_relationship_from_wit(&rel.relationship))
        })
    };
    genealogy_gedcom::ChildRef {
        xref: child.human_id.clone(),
        father_relationship: rel_for(partners.first()),
        mother_relationship: rel_for(partners.get(1)),
    }
}

/// Whether an event kind is a GEDCOM `FAM`-level event (one nested under a family, not an individual).
fn is_family_event(kind: EventKind) -> bool {
    match kind {
        EventKind::Marriage
        | EventKind::Divorce
        | EventKind::DivorceFiled
        | EventKind::Engagement
        | EventKind::Annulment
        | EventKind::MarriageBanns
        | EventKind::MarriageContract
        | EventKind::MarriageLicense
        | EventKind::MarriageSettlement => true,
        EventKind::Birth
        | EventKind::Death
        | EventKind::Baptism
        | EventKind::Christening
        | EventKind::Burial
        | EventKind::Cremation
        | EventKind::Census
        | EventKind::Residence
        | EventKind::Immigration
        | EventKind::Emigration
        | EventKind::Adoption
        | EventKind::Confirmation
        | EventKind::BarMitzvah
        | EventKind::BasMitzvah
        | EventKind::FirstCommunion
        | EventKind::Graduation
        | EventKind::Naturalization
        | EventKind::Ordination
        | EventKind::Probate
        | EventKind::Retirement
        | EventKind::Will => false,
    }
}

/// Maps a person DTO onto a GEDCOM individual, reconstructing the structured `NAME`, sex, INDI-
/// attribute facts, and `ASSO` associations from its parts. Events are filled in by
/// [`distribute_events`].
fn individual(
    person: types::PersonDto,
    citation_content: &HashMap<String, genealogy_gedcom::Citation>,
    media_content: &HashMap<String, genealogy_gedcom::MediaObject>,
    note_content: &HashMap<String, String>,
) -> genealogy_gedcom::Individual {
    let has_name = person.given.is_some()
        || person.surname.is_some()
        || person.surname_prefix.is_some()
        || person.nickname.is_some()
        || person.name_prefix.is_some()
        || person.name_suffix.is_some();
    let name = has_name.then(|| Name {
        name_type: person.name_type.map(convert::name_type_from_wit),
        given: person.given,
        surname_prefix: person.surname_prefix,
        surname: person.surname,
        nickname: person.nickname,
        prefix: person.name_prefix,
        suffix: person.name_suffix,
    });
    let facts = person.facts.into_iter().filter_map(gedcom_fact).collect();
    let associations = person
        .associations
        .into_iter()
        .map(|association| Association {
            other_xref: association.other,
            role: Some(convert::association_role_from_wit(association.role)),
        })
        .collect();
    let citations = person
        .citations
        .iter()
        .filter_map(|human_id| citation_content.get(human_id).cloned())
        .collect();
    let media = person
        .media
        .iter()
        .filter_map(|human_id| media_content.get(human_id).cloned())
        .collect();
    let notes = person
        .notes
        .iter()
        .filter_map(|human_id| note_content.get(human_id).cloned())
        .collect();
    let restrictions = convert::restrictions_from_wit(&person.restrictions);
    genealogy_gedcom::Individual {
        xref: person.human_id,
        uid: None,
        name,
        sex: person.sex.map(convert::sex_from_wit),
        events: Vec::new(),
        facts,
        associations,
        citations,
        media,
        notes,
        restrictions,
    }
}

/// Maps a host `fact` read record onto a GEDCOM INDI-attribute fact. A fact whose type is event-like
/// (birth, death, …) or custom has no GEDCOM INDI-attribute tag and is dropped.
fn gedcom_fact(fact: types::Fact) -> Option<Fact> {
    Some(Fact {
        kind: convert::fact_type_from_wit(fact.fact_type)?,
        value: fact.value,
        date: fact.date.as_ref().map(convert::date_from_wit),
    })
}

export!(Exporter);
