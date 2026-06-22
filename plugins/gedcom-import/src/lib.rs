//! GEDCOM import plugin (ADR 0013): read the document from the host-opened import source, parse it
//! with `genealogy-gedcom`, then create persons and families through the host `commands` capability,
//! reporting progress as it goes. The format-neutral plumbing (streaming, progress, logging) lives
//! in `genealogy-plugin-api`; this crate only bridges the GEDCOM [`Tree`](genealogy_gedcom::Tree) to
//! the host capabilities.

wit_bindgen::generate!({
    world: "bulk-import",
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.5.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.5.0": genealogy_plugin_api::log,
        "genealogy:host-api/commands@0.5.0": genealogy_plugin_api::commands,
        "genealogy:host-api/progress@0.5.0": genealogy_plugin_api::progress,
        "genealogy:host-api/import-source@0.5.0": genealogy_plugin_api::import_source,
    },
});

use std::collections::HashMap;

use genealogy_gedcom::{Event, EventKind, Sex};
use genealogy_plugin_api::commands;
use genealogy_plugin_api::types::{EventType, ExternalId, ParticipantRole, Sex as WitSex};

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
        // Source xref -> title, so a citation can title the source it creates.
        let source_titles: HashMap<&str, Option<&str>> = tree
            .sources
            .iter()
            .map(|source| (source.xref.as_str(), source.title.as_deref()))
            .collect();
        // Source xref -> created source human id, so a shared source is created once.
        let mut sources: HashMap<String, String> = HashMap::new();
        let mut imported: u32 = 0;

        for (index, individual) in tree.individuals.iter().enumerate() {
            let person = commands::create_person(
                individual.given.as_deref(),
                individual.surname.as_deref(),
                Some(&external_id(individual.uid.as_deref(), &individual.xref)),
            )
            .map_err(|error| format!("create-person failed: {error:?}"))?;
            // Owned attributes/events are written only on first creation, so re-import is idempotent.
            if person.created {
                if let Some(sex) = individual.sex {
                    commands::assert_sex(&person.human_id, wit_sex(sex))
                        .map_err(|error| format!("assert-sex failed: {error:?}"))?;
                }
                for event in &individual.events {
                    import_event(event, std::slice::from_ref(&person.human_id), &mut places)?;
                }
                for citation in &individual.citations {
                    let source_id = source_human_id(&citation.source_xref, &source_titles, &mut sources)?;
                    commands::create_citation(&source_id, citation.page.as_deref())
                        .map_err(|error| format!("create-citation failed: {error:?}"))?;
                }
            }
            xref_to_human.insert(individual.xref.clone(), person.human_id);
            imported += 1;
            if !genealogy_plugin_api::report("persons", index as u32 + 1, Some(individuals))? {
                return Ok(imported);
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
                if let Some(human_id) = xref_to_human.get(child) {
                    commands::add_child(&family_record.human_id, human_id)
                        .map_err(|error| format!("add-child failed: {error:?}"))?;
                }
            }
            if family_record.created {
                for event in &family.events {
                    import_event(event, &partner_ids, &mut places)?;
                }
            }
            imported += 1;
            if !genealogy_plugin_api::report("families", index as u32 + 1, Some(families))? {
                return Ok(imported);
            }
        }

        Ok(imported)
    }
}

/// Creates an event, sets its date and place, and links each participant as the primary. The place
/// is deduped by name through `places` so a shared place is created once.
fn import_event(event: &Event, participants: &[String], places: &mut HashMap<String, String>) -> Result<(), String> {
    let event_id =
        commands::create_event(wit_event_type(event.kind)).map_err(|error| format!("create-event failed: {error:?}"))?;
    if let Some(date) = event.date {
        commands::set_event_date(&event_id, date.year, date.month, date.day)
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
    for person in participants {
        commands::add_event_participant(person, &event_id, ParticipantRole::Primary)
            .map_err(|error| format!("add-participant failed: {error:?}"))?;
    }
    Ok(())
}

/// Returns the human id of the source for `source_xref`, creating it (titled from the parsed
/// top-level `SOUR` record) the first time and caching it in `sources` so it is created once.
fn source_human_id(
    source_xref: &str,
    titles: &HashMap<&str, Option<&str>>,
    sources: &mut HashMap<String, String>,
) -> Result<String, String> {
    if let Some(human_id) = sources.get(source_xref) {
        return Ok(human_id.clone());
    }
    let title = titles.get(source_xref).copied().flatten();
    let human_id = commands::create_source(title).map_err(|error| format!("create-source failed: {error:?}"))?;
    sources.insert(source_xref.to_owned(), human_id.clone());
    Ok(human_id)
}

/// Maps the parsed GEDCOM sex onto the host capability's `sex` enum.
fn wit_sex(sex: Sex) -> WitSex {
    match sex {
        Sex::Male => WitSex::Male,
        Sex::Female => WitSex::Female,
        Sex::Unknown => WitSex::Unknown,
    }
}

/// Maps the parsed GEDCOM event kind onto the host capability's `event-type` enum.
fn wit_event_type(kind: EventKind) -> EventType {
    match kind {
        EventKind::Birth => EventType::Birth,
        EventKind::Death => EventType::Death,
        EventKind::Marriage => EventType::Marriage,
        EventKind::Baptism => EventType::Baptism,
        EventKind::Burial => EventType::Burial,
        EventKind::Census => EventType::Census,
        EventKind::Residence => EventType::Residence,
        EventKind::Immigration => EventType::Immigration,
        EventKind::Emigration => EventType::Emigration,
    }
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
