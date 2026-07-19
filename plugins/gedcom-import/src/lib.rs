//! GEDCOM import plugin (ADR 0013): read the document from the host-opened import source, parse it
//! with `genealogy-gedcom`, then create persons and families through the host `commands` capability,
//! reporting progress as it goes. The format-neutral plumbing (streaming, progress, logging) and the
//! interchange→WIT conversions live in `genealogy-plugin-api`; this crate only walks the GEDCOM
//! [`Tree`](genealogy_gedcom::Tree) and drives the host capabilities.

wit_bindgen::generate!({
    world: "bulk-import",
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.18.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.18.0": genealogy_plugin_api::log,
        "genealogy:host-api/commands@0.18.0": genealogy_plugin_api::commands,
        "genealogy:host-api/progress@0.18.0": genealogy_plugin_api::progress,
        "genealogy:host-api/import-source@0.18.0": genealogy_plugin_api::import_source,
    },
});

use std::collections::HashMap;

use genealogy_gedcom::{Age, Association, Event, EventAssociation, Fact, Source};
use genealogy_plugin_api::commands;
use genealogy_plugin_api::convert;
use genealogy_plugin_api::types::{ChildParentRel, ExternalId, ParticipantRole, ParticipationInput};

struct Importer;

impl Guest for Importer {
    fn run_import() -> Result<u32, String> {
        let text = genealogy_plugin_api::read_source_to_string()?;
        let tree = genealogy_gedcom::parse(&text).map_err(|error| error.to_string())?;
        let individuals = tree.individuals.len() as u32;
        let families = tree.families.len() as u32;
        genealogy_plugin_api::log_info(&format!("importing {individuals} individuals and {families} families"));

        let mut xref_to_human: HashMap<String, String> = HashMap::new();
        // Place name -> human id, so a place referenced by several events is created once.
        let mut places: HashMap<String, String> = HashMap::new();
        // Source xref -> record, so a citation can title (and set author/pub-info on) the source it creates.
        let source_index: HashMap<&str, &Source> =
            tree.sources.iter().map(|source| (source.xref.as_str(), source)).collect();
        // Source xref -> created source human id, so a shared source is created once.
        let mut sources: HashMap<String, String> = HashMap::new();
        // Media file -> created media human id, so a shared media object is created once.
        let mut media: HashMap<String, String> = HashMap::new();
        // Associations resolved after every person exists (the other person may be a forward ref).
        let mut pending_associations: Vec<(String, Association)> = Vec::new();
        // Event-level ASSO witnesses reference another person by xref (a forward ref) and are
        // asserted on that witness person, so they are flushed once every person and event exists.
        let mut pending_event_witnesses: Vec<(String, EventAssociation)> = Vec::new();
        let mut imported: u32 = 0;

        for (index, individual) in tree.individuals.iter().enumerate() {
            let person = commands::create_person(
                individual.name.as_ref().map(convert::name_to_wit).as_ref(),
                Some(&external_id(individual.uid.as_deref(), &individual.xref)),
            )
            .map_err(|error| format!("create-person failed: {error:?}"))?;
            // Owned attributes/events are written only on first creation, so re-import is idempotent.
            if person.created {
                if let Some(sex) = individual.sex {
                    commands::assert_sex(&person.human_id, convert::sex_to_wit(sex))
                        .map_err(|error| format!("assert-sex failed: {error:?}"))?;
                }
                for event in &individual.events {
                    import_event(
                        event,
                        std::slice::from_ref(&person.human_id),
                        std::slice::from_ref(&event.age),
                        &mut places,
                        &mut pending_event_witnesses,
                    )?;
                }
                for fact in &individual.facts {
                    import_fact(&person.human_id, fact)?;
                }
                for citation in &individual.citations {
                    let source_id = source_human_id(&citation.source_xref, &source_index, &mut sources)?;
                    let citation_id = commands::create_citation(&source_id, citation.page.as_deref())
                        .map_err(|error| format!("create-citation failed: {error:?}"))?;
                    commands::attach_person_citation(&person.human_id, &citation_id)
                        .map_err(|error| format!("attach-person-citation failed: {error:?}"))?;
                }
                for object in &individual.media {
                    if let Some(file) = &object.file {
                        let media_id = match media.get(file) {
                            Some(media_id) => media_id.clone(),
                            None => {
                                let media_id = commands::create_media(Some(file))
                                    .map_err(|error| format!("create-media failed: {error:?}"))?;
                                if let Some(mime) = &object.mime {
                                    commands::set_media_mime(&media_id, mime)
                                        .map_err(|error| format!("set-media-mime failed: {error:?}"))?;
                                }
                                media.insert(file.clone(), media_id.clone());
                                media_id
                            }
                        };
                        commands::attach_person_media(&person.human_id, &media_id, None, None)
                            .map_err(|error| format!("attach-person-media failed: {error:?}"))?;
                    }
                }
                for note in &individual.notes {
                    let note_id =
                        commands::create_note(note).map_err(|error| format!("create-note failed: {error:?}"))?;
                    commands::attach_person_note(&person.human_id, &note_id)
                        .map_err(|error| format!("attach-person-note failed: {error:?}"))?;
                }
                for association in &individual.associations {
                    pending_associations.push((person.human_id.clone(), association.clone()));
                }
                if !individual.restrictions.is_empty() {
                    commands::set_person_restrictions(
                        &person.human_id,
                        &convert::restrictions_to_wit(&individual.restrictions),
                    )
                    .map_err(|error| format!("set-person-restrictions failed: {error:?}"))?;
                }
            }
            xref_to_human.insert(individual.xref.clone(), person.human_id);
            imported += 1;
            if !genealogy_plugin_api::report("persons", index as u32 + 1, Some(individuals))? {
                return Ok(imported);
            }
        }

        // Associations reference another person by xref; resolve now that every person exists.
        for (person, association) in &pending_associations {
            if let Some(other) = xref_to_human.get(&association.other_xref) {
                commands::assert_association(person, other, &convert::association_role_to_wit(association.role.as_ref()))
                    .map_err(|error| format!("assert-association failed: {error:?}"))?;
            }
        }

        for (index, family) in tree.families.iter().enumerate() {
            let family_record = commands::create_family(Some(&external_id(family.uid.as_deref(), &family.xref)))
                .map_err(|error| format!("create-family failed: {error:?}"))?;
            let mut partner_ids = Vec::new();
            for partner in &family.partners {
                if let Some(human_id) = xref_to_human.get(partner) {
                    commands::add_partner(&family_record.human_id, human_id)
                        .map_err(|error| format!("add-partner failed: {error:?}"))?;
                    partner_ids.push(human_id.clone());
                }
            }
            for child in &family.children {
                if let Some(human_id) = xref_to_human.get(&child.xref) {
                    let mut relationships = Vec::new();
                    if let (Some(father), Some(frel)) = (partner_ids.first(), &child.father_relationship) {
                        relationships.push(ChildParentRel {
                            partner: father.clone(),
                            relationship: convert::child_relationship_to_wit(frel),
                        });
                    }
                    if let (Some(mother), Some(mrel)) = (partner_ids.get(1), &child.mother_relationship) {
                        relationships.push(ChildParentRel {
                            partner: mother.clone(),
                            relationship: convert::child_relationship_to_wit(mrel),
                        });
                    }
                    commands::add_child(&family_record.human_id, human_id, &relationships)
                        .map_err(|error| format!("add-child failed: {error:?}"))?;
                }
            }
            if family_record.created {
                for event in &family.events {
                    let event_id = import_event(
                        event,
                        &partner_ids,
                        &[event.husband_age.clone(), event.wife_age.clone()],
                        &mut places,
                        &mut pending_event_witnesses,
                    )?;
                    commands::link_family_event(&family_record.human_id, &event_id)
                        .map_err(|error| format!("link-family-event failed: {error:?}"))?;
                }
                if !family.restrictions.is_empty() {
                    commands::set_family_restrictions(
                        &family_record.human_id,
                        &convert::restrictions_to_wit(&family.restrictions),
                    )
                    .map_err(|error| format!("set-family-restrictions failed: {error:?}"))?;
                }
            }
            imported += 1;
            if !genealogy_plugin_api::report("families", index as u32 + 1, Some(families))? {
                return Ok(imported);
            }
        }

        // Event-level ASSO witnesses: now every person and event exists, resolve each witness's
        // xref and assert their participation (role + notes + citations→envelope) on the witness.
        for (event_id, association) in &pending_event_witnesses {
            let Some(witness) = xref_to_human.get(&association.other_xref) else {
                continue;
            };
            let mut notes = Vec::with_capacity(association.notes.len());
            for note in &association.notes {
                notes.push(commands::create_note(note).map_err(|error| format!("create-note failed: {error:?}"))?);
            }
            let mut citations = Vec::with_capacity(association.citations.len());
            for citation in &association.citations {
                let source_id = source_human_id(&citation.source_xref, &source_index, &mut sources)?;
                citations.push(
                    commands::create_citation(&source_id, citation.page.as_deref())
                        .map_err(|error| format!("create-citation failed: {error:?}"))?,
                );
            }
            let input = ParticipationInput {
                role: convert::association_kind_to_participant_role(association.role.as_ref()),
                age: None,
                attributes: Vec::new(),
                notes,
                citations,
            };
            commands::add_event_participant(witness, event_id, &input)
                .map_err(|error| format!("add-participant (witness) failed: {error:?}"))?;
        }

        Ok(imported)
    }
}

/// Creates an event, sets its date, place, and address, and links each participant as the primary
/// with their age (`ages[i]` aligns with `participants[i]`; a missing entry is no age). The place is
/// deduped by name through `places`. Event-level `ASSO` witnesses are queued in `witnesses` for the
/// caller to flush once every person exists (the witness may be a forward xref).
fn import_event(
    event: &Event,
    participants: &[String],
    ages: &[Option<Age>],
    places: &mut HashMap<String, String>,
    witnesses: &mut Vec<(String, EventAssociation)>,
) -> Result<String, String> {
    let event_id = commands::create_event(convert::event_type_to_wit(event.kind))
        .map_err(|error| format!("create-event failed: {error:?}"))?;
    if let Some(date) = &event.date {
        commands::set_event_date(&event_id, &convert::date_to_wit(date))
            .map_err(|error| format!("set-event-date failed: {error:?}"))?;
    }
    if let Some(place_name) = &event.place {
        let place_id = match places.get(place_name) {
            Some(place_id) => place_id.clone(),
            None => {
                let place_id =
                    commands::create_place(place_name).map_err(|error| format!("create-place failed: {error:?}"))?;
                places.insert(place_name.clone(), place_id.clone());
                place_id
            }
        };
        commands::link_event_place(&event_id, &place_id).map_err(|error| format!("link-place failed: {error:?}"))?;
    }
    if let Some(address) = &event.address {
        commands::set_event_address(&event_id, &convert::address_to_wit(address))
            .map_err(|error| format!("set-event-address failed: {error:?}"))?;
    }
    for (index, person) in participants.iter().enumerate() {
        let age = ages.get(index).and_then(|age| age.as_ref());
        let input = ParticipationInput {
            role: ParticipantRole::Primary,
            age: age.map(convert::age_to_wit),
            attributes: Vec::new(),
            notes: Vec::new(),
            citations: Vec::new(),
        };
        commands::add_event_participant(person, &event_id, &input)
            .map_err(|error| format!("add-participant failed: {error:?}"))?;
    }
    for association in &event.associations {
        witnesses.push((event_id.clone(), association.clone()));
    }
    Ok(event_id)
}

/// Asserts one INDI-attribute fact on a person.
fn import_fact(person: &str, fact: &Fact) -> Result<(), String> {
    let date = fact.date.as_ref().map(convert::date_to_wit);
    commands::assert_fact(person, &convert::fact_type_to_wit(fact.kind), fact.value.as_deref(), date.as_ref())
        .map_err(|error| format!("assert-fact failed: {error:?}"))
}

/// Returns the human id of the source for `source_xref`, creating it (titled from the parsed
/// top-level `SOUR` record) the first time and caching it in `sources` so it is created once.
fn source_human_id(
    source_xref: &str,
    index: &HashMap<&str, &Source>,
    sources: &mut HashMap<String, String>,
) -> Result<String, String> {
    if let Some(human_id) = sources.get(source_xref) {
        return Ok(human_id.clone());
    }
    let source = index.get(source_xref).copied();
    let title = source.and_then(|source| source.title.as_deref());
    let human_id = commands::create_source(title).map_err(|error| format!("create-source failed: {error:?}"))?;
    if let Some(source) = source {
        if let Some(author) = &source.author {
            commands::set_source_author(&human_id, author)
                .map_err(|error| format!("set-source-author failed: {error:?}"))?;
        }
        if let Some(pub_info) = &source.pub_info {
            commands::set_source_pub_info(&human_id, pub_info)
                .map_err(|error| format!("set-source-pub-info failed: {error:?}"))?;
        }
    }
    sources.insert(source_xref.to_owned(), human_id.clone());
    Ok(human_id)
}

/// Builds the external id a record is resolved by on re-import: the stable `_UID` when present
/// (authority `gedcom-uid`), else the per-file cross-reference (authority `gedcom-xref`). Either is
/// stable across re-exports of the same document, so an unchanged record resolves to itself.
fn external_id(uid: Option<&str>, xref: &str) -> ExternalId {
    match uid {
        Some(uid) => ExternalId {
            authority: "gedcom-uid".to_owned(),
            value: uid.to_owned(),
            kind: None,
            url: None,
        },
        None => ExternalId {
            authority: "gedcom-xref".to_owned(),
            value: xref.to_owned(),
            kind: None,
            url: None,
        },
    }
}

export!(Importer);
