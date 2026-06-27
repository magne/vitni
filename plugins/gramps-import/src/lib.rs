//! Gramps XML import plugin (ADR 0013, ADR 0018): read the document from the host-opened import
//! source, parse it with `genealogy-gramps-xml`, then create persons and families through the host
//! `commands` capability, resolving Gramps's `hlink` references (events, places, sources, citations,
//! notes, media, repositories) into owned aggregates and attaching them to their owner.
//!
//! Owned records are created on first reference and cached by Gramps `handle`, and only while their
//! owner is newly created, so re-importing the same `.gramps` file is idempotent (the person/family
//! resolves by its `gramps-id` external id and its owned records are skipped).

wit_bindgen::generate!({
    world: "bulk-import",
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.12.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.12.0": genealogy_plugin_api::log,
        "genealogy:host-api/commands@0.12.0": genealogy_plugin_api::commands,
        "genealogy:host-api/progress@0.12.0": genealogy_plugin_api::progress,
        "genealogy:host-api/import-source@0.12.0": genealogy_plugin_api::import_source,
    },
});

use std::collections::HashMap;

use genealogy_gramps_xml::{Citation, Database, Event, Gender, Place, Source};
use genealogy_interchange::AssociationKind;
use genealogy_plugin_api::commands;
use genealogy_plugin_api::convert;
use genealogy_plugin_api::types::{ChildParentRel, Confidence, ExternalId, ParticipantRole, PlaceType, Sex};

struct Importer;

/// Create-once caches keyed by Gramps `handle`, plus the parsed record indexes, threaded through the
/// import so each referenced record is created exactly once.
struct Resolver<'a> {
    events: HashMap<String, &'a Event>,
    places: HashMap<String, &'a Place>,
    sources: HashMap<String, &'a Source>,
    citations: HashMap<String, &'a Citation>,
    note_text: HashMap<String, String>,
    media_file: HashMap<String, Option<String>>,
    media_mime: HashMap<String, Option<String>>,
    repository_name: HashMap<String, Option<String>>,
    // handle -> created human id
    created_events: HashMap<String, String>,
    created_places: HashMap<String, String>,
    created_sources: HashMap<String, String>,
    created_citations: HashMap<String, String>,
    created_notes: HashMap<String, String>,
    created_media: HashMap<String, String>,
    created_repositories: HashMap<String, String>,
}

impl Guest for Importer {
    fn run_import() -> Result<u32, String> {
        let bytes = genealogy_plugin_api::read_source_to_end()?;
        let db = genealogy_gramps_xml::parse(&bytes).map_err(|error| error.to_string())?;
        let people = db.people.len() as u32;
        let families = db.families.len() as u32;
        genealogy_plugin_api::log_info(&format!("importing {people} people and {families} families"));

        let mut resolver = Resolver::new(&db);
        // Gramps handle -> created person human id, for resolving family members and associations.
        let mut handle_to_human: HashMap<String, String> = HashMap::new();
        let mut pending_associations: Vec<(String, String, Option<AssociationKind>)> = Vec::new();
        let mut imported = 0u32;

        for (index, person) in db.people.iter().enumerate() {
            let record = commands::create_person(
                person.name.as_ref().map(convert::name_to_wit).as_ref(),
                Some(&external_id(person.gramps_id.as_deref(), &person.handle)),
            )
            .map_err(|error| format!("create-person failed: {error:?}"))?;
            if record.created {
                if let Some(gender) = person.gender {
                    commands::assert_sex(&record.human_id, gender_to_sex(gender))
                        .map_err(|error| format!("assert-sex failed: {error:?}"))?;
                }
                for handle in &person.event_refs {
                    let event = resolver.ensure_event(handle)?;
                    if let Some(event) = event {
                        commands::add_event_participant(&record.human_id, &event, ParticipantRole::Primary)
                            .map_err(|error| format!("add-participant failed: {error:?}"))?;
                    }
                }
                for handle in &person.citation_refs {
                    if let Some(citation) = resolver.ensure_citation(handle)? {
                        commands::attach_person_citation(&record.human_id, &citation)
                            .map_err(|error| format!("attach-person-citation failed: {error:?}"))?;
                    }
                }
                for handle in &person.note_refs {
                    if let Some(note) = resolver.ensure_note(handle)? {
                        commands::attach_person_note(&record.human_id, &note)
                            .map_err(|error| format!("attach-person-note failed: {error:?}"))?;
                    }
                }
                for handle in &person.media_refs {
                    if let Some(media) = resolver.ensure_media(handle)? {
                        commands::attach_person_media(&record.human_id, &media)
                            .map_err(|error| format!("attach-person-media failed: {error:?}"))?;
                    }
                }
                for person_ref in &person.person_refs {
                    pending_associations.push((record.human_id.clone(), person_ref.hlink.clone(), person_ref.rel.clone()));
                }
                if person.private {
                    commands::set_person_restrictions(&record.human_id, &convert::private_to_wit(person.private))
                        .map_err(|error| format!("set-person-restrictions failed: {error:?}"))?;
                }
            }
            handle_to_human.insert(person.handle.clone(), record.human_id);
            imported += 1;
            if !genealogy_plugin_api::report("people", index as u32 + 1, Some(people))? {
                return Ok(imported);
            }
        }

        for (person, other_handle, rel) in &pending_associations {
            if let Some(other) = handle_to_human.get(other_handle) {
                commands::assert_association(person, other, &convert::association_role_to_wit(rel.as_ref()))
                    .map_err(|error| format!("assert-association failed: {error:?}"))?;
            }
        }

        for (index, family) in db.families.iter().enumerate() {
            let record = commands::create_family(Some(&external_id(family.gramps_id.as_deref(), &family.handle)))
                .map_err(|error| format!("create-family failed: {error:?}"))?;
            let mut partner_ids = Vec::new();
            for handle in family.father.iter().chain(family.mother.iter()) {
                if let Some(human_id) = handle_to_human.get(handle) {
                    commands::add_partner(&record.human_id, human_id)
                        .map_err(|error| format!("add-partner failed: {error:?}"))?;
                    partner_ids.push(human_id.clone());
                }
            }
            for child in &family.child_refs {
                if let Some(human_id) = handle_to_human.get(&child.hlink) {
                    let mut relationships = Vec::new();
                    if let (Some(frel), Some(father)) = (
                        &child.father_relationship,
                        family.father.as_ref().and_then(|h| handle_to_human.get(h)),
                    ) {
                        relationships.push(ChildParentRel {
                            partner: father.clone(),
                            relationship: convert::child_relationship_to_wit(frel),
                        });
                    }
                    if let (Some(mrel), Some(mother)) = (
                        &child.mother_relationship,
                        family.mother.as_ref().and_then(|h| handle_to_human.get(h)),
                    ) {
                        relationships.push(ChildParentRel {
                            partner: mother.clone(),
                            relationship: convert::child_relationship_to_wit(mrel),
                        });
                    }
                    commands::add_child(&record.human_id, human_id, &relationships)
                        .map_err(|error| format!("add-child failed: {error:?}"))?;
                }
            }
            if record.created {
                for handle in &family.event_refs {
                    if let Some(event) = resolver.ensure_event(handle)? {
                        commands::link_family_event(&record.human_id, &event)
                            .map_err(|error| format!("link-family-event failed: {error:?}"))?;
                        for partner in &partner_ids {
                            commands::add_event_participant(partner, &event, ParticipantRole::Primary)
                                .map_err(|error| format!("add-participant failed: {error:?}"))?;
                        }
                    }
                }
                if family.private {
                    commands::set_family_restrictions(&record.human_id, &convert::private_to_wit(family.private))
                        .map_err(|error| format!("set-family-restrictions failed: {error:?}"))?;
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

impl<'a> Resolver<'a> {
    fn new(db: &'a Database) -> Self {
        Self {
            events: index(&db.events, |e| &e.handle),
            places: index(&db.places, |p| &p.handle),
            sources: index(&db.sources, |s| &s.handle),
            citations: index(&db.citations, |c| &c.handle),
            note_text: db.notes.iter().map(|n| (n.handle.clone(), n.text.clone().unwrap_or_default())).collect(),
            media_file: db.objects.iter().map(|o| (o.handle.clone(), o.file.clone())).collect(),
            media_mime: db.objects.iter().map(|o| (o.handle.clone(), o.mime.clone())).collect(),
            repository_name: db.repositories.iter().map(|r| (r.handle.clone(), r.name.clone())).collect(),
            created_events: HashMap::new(),
            created_places: HashMap::new(),
            created_sources: HashMap::new(),
            created_citations: HashMap::new(),
            created_notes: HashMap::new(),
            created_media: HashMap::new(),
            created_repositories: HashMap::new(),
        }
    }

    /// Creates the event for `handle` (once), setting its date and linked place, and returns its
    /// human id. A dangling handle yields `None` (tolerated, not an error).
    fn ensure_event(&mut self, handle: &str) -> Result<Option<String>, String> {
        if let Some(human_id) = self.created_events.get(handle) {
            return Ok(Some(human_id.clone()));
        }
        let Some(event) = self.events.get(handle).copied() else {
            return Ok(None);
        };
        let human_id = commands::create_event(convert::event_type_to_wit(event.kind))
            .map_err(|error| format!("create-event failed: {error:?}"))?;
        if let Some(date) = &event.date {
            commands::set_event_date(&human_id, &convert::date_to_wit(date))
                .map_err(|error| format!("set-event-date failed: {error:?}"))?;
        }
        if let Some(place_handle) = &event.place_ref
            && let Some(place) = self.ensure_place(place_handle)?
        {
            commands::link_event_place(&human_id, &place).map_err(|error| format!("link-place failed: {error:?}"))?;
        }
        self.created_events.insert(handle.to_owned(), human_id.clone());
        Ok(Some(human_id))
    }

    /// Creates the place for `handle` (once), its type, and its enclosing-place chain.
    fn ensure_place(&mut self, handle: &str) -> Result<Option<String>, String> {
        if let Some(human_id) = self.created_places.get(handle) {
            return Ok(Some(human_id.clone()));
        }
        let Some(place) = self.places.get(handle).copied() else {
            return Ok(None);
        };
        let human_id = commands::create_place(place.name.as_deref().unwrap_or_default())
            .map_err(|error| format!("create-place failed: {error:?}"))?;
        self.created_places.insert(handle.to_owned(), human_id.clone());
        if let Some(place_type) = &place.place_type {
            commands::set_place_type(&human_id, &place_type_of(place_type))
                .map_err(|error| format!("set-place-type failed: {error:?}"))?;
        }
        for enclosing_handle in &place.enclosed_by {
            if let Some(enclosing) = self.ensure_place(enclosing_handle)? {
                commands::set_place_enclosed_by(&human_id, &enclosing)
                    .map_err(|error| format!("set-place-enclosed-by failed: {error:?}"))?;
            }
        }
        Ok(Some(human_id))
    }

    /// Creates the source for `handle` (once), its author/pub-info, and its repository links.
    fn ensure_source(&mut self, handle: &str) -> Result<Option<String>, String> {
        if let Some(human_id) = self.created_sources.get(handle) {
            return Ok(Some(human_id.clone()));
        }
        let Some(source) = self.sources.get(handle).copied() else {
            return Ok(None);
        };
        let human_id =
            commands::create_source(source.title.as_deref()).map_err(|error| format!("create-source failed: {error:?}"))?;
        self.created_sources.insert(handle.to_owned(), human_id.clone());
        if let Some(author) = &source.author {
            commands::set_source_author(&human_id, author).map_err(|error| format!("set-source-author failed: {error:?}"))?;
        }
        if let Some(pub_info) = &source.pub_info {
            commands::set_source_pub_info(&human_id, pub_info)
                .map_err(|error| format!("set-source-pub-info failed: {error:?}"))?;
        }
        for repo_handle in &source.repository_refs {
            if let Some(repository) = self.ensure_repository(repo_handle)? {
                commands::link_source_repository(&human_id, &repository)
                    .map_err(|error| format!("link-source-repository failed: {error:?}"))?;
            }
        }
        Ok(Some(human_id))
    }

    /// Creates the citation for `handle` (once), its source, page, and confidence.
    fn ensure_citation(&mut self, handle: &str) -> Result<Option<String>, String> {
        if let Some(human_id) = self.created_citations.get(handle) {
            return Ok(Some(human_id.clone()));
        }
        let Some(citation) = self.citations.get(handle).copied() else {
            return Ok(None);
        };
        let source = match &citation.source_ref {
            Some(source_handle) => self.ensure_source(source_handle)?,
            None => None,
        };
        let Some(source) = source else {
            return Ok(None);
        };
        let human_id = commands::create_citation(&source, citation.page.as_deref())
            .map_err(|error| format!("create-citation failed: {error:?}"))?;
        self.created_citations.insert(handle.to_owned(), human_id.clone());
        if let Some(confidence) = citation.confidence {
            commands::set_citation_confidence(&human_id, confidence_of(confidence))
                .map_err(|error| format!("set-citation-confidence failed: {error:?}"))?;
        }
        Ok(Some(human_id))
    }

    /// Creates the note for `handle` (once).
    fn ensure_note(&mut self, handle: &str) -> Result<Option<String>, String> {
        if let Some(human_id) = self.created_notes.get(handle) {
            return Ok(Some(human_id.clone()));
        }
        let Some(text) = self.note_text.get(handle) else {
            return Ok(None);
        };
        let human_id = commands::create_note(text).map_err(|error| format!("create-note failed: {error:?}"))?;
        self.created_notes.insert(handle.to_owned(), human_id.clone());
        Ok(Some(human_id))
    }

    /// Creates the media object for `handle` (once).
    fn ensure_media(&mut self, handle: &str) -> Result<Option<String>, String> {
        if let Some(human_id) = self.created_media.get(handle) {
            return Ok(Some(human_id.clone()));
        }
        let Some(file) = self.media_file.get(handle) else {
            return Ok(None);
        };
        let human_id =
            commands::create_media(file.as_deref()).map_err(|error| format!("create-media failed: {error:?}"))?;
        if let Some(Some(mime)) = self.media_mime.get(handle) {
            commands::set_media_mime(&human_id, mime).map_err(|error| format!("set-media-mime failed: {error:?}"))?;
        }
        self.created_media.insert(handle.to_owned(), human_id.clone());
        Ok(Some(human_id))
    }

    /// Creates the repository for `handle` (once).
    fn ensure_repository(&mut self, handle: &str) -> Result<Option<String>, String> {
        if let Some(human_id) = self.created_repositories.get(handle) {
            return Ok(Some(human_id.clone()));
        }
        let Some(name) = self.repository_name.get(handle) else {
            return Ok(None);
        };
        let human_id = commands::create_repository(name.as_deref().unwrap_or_default())
            .map_err(|error| format!("create-repository failed: {error:?}"))?;
        self.created_repositories.insert(handle.to_owned(), human_id.clone());
        Ok(Some(human_id))
    }
}

/// Builds a `handle -> &record` index.
fn index<T>(records: &[T], handle: impl Fn(&T) -> &String) -> HashMap<String, &T> {
    records.iter().map(|record| (handle(record).clone(), record)).collect()
}

/// Builds the external id a record is resolved by on re-import: the stable `gramps-id` when present,
/// else the per-file `gramps-handle`.
fn external_id(gramps_id: Option<&str>, handle: &str) -> ExternalId {
    match gramps_id {
        Some(id) => ExternalId {
            authority: "gramps-id".to_owned(),
            value: id.to_owned(),
            kind: None,
            url: None,
        },
        None => ExternalId {
            authority: "gramps-handle".to_owned(),
            value: handle.to_owned(),
            kind: None,
            url: None,
        },
    }
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
