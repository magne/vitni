//! GEDCOM import plugin (ADR 0013): read the document from the host-opened import source, parse it
//! with `genealogy-gedcom`, then create persons and families through the host `commands` capability,
//! reporting progress as it goes. The format-neutral plumbing (streaming, progress, logging) lives
//! in `genealogy-plugin-api`; this crate only bridges the GEDCOM [`Tree`](genealogy_gedcom::Tree) to
//! the host capabilities.

wit_bindgen::generate!({
    world: "bulk-import",
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.6.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.6.0": genealogy_plugin_api::log,
        "genealogy:host-api/commands@0.6.0": genealogy_plugin_api::commands,
        "genealogy:host-api/progress@0.6.0": genealogy_plugin_api::progress,
        "genealogy:host-api/import-source@0.6.0": genealogy_plugin_api::import_source,
    },
});

use std::collections::HashMap;

use genealogy_gedcom::{
    Association, AssociationKind, Calendar, Date, DateModifier, DatePoint, DateQuality, Event, EventKind, Fact,
    FactKind, Name, NameKind, Sex,
};
use genealogy_plugin_api::commands;
use genealogy_plugin_api::types::{
    Address as WitAddress, AssociationRole, DateCalendar, DateModifier as WitDateModifier, DatePoint as WitDatePoint,
    DateQuality as WitDateQuality, DateRange, EventType, ExternalId, FactType, GenealogicalDate, InterpretedDate,
    NameType, ParticipantRole, PersonName, Sex as WitSex,
};

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
        // Media file -> created media human id, so a shared media object is created once.
        let mut media: HashMap<String, String> = HashMap::new();
        // Associations resolved after every person exists (the other person may be a forward ref).
        let mut pending_associations: Vec<(String, Association)> = Vec::new();
        let mut imported: u32 = 0;

        for (index, individual) in tree.individuals.iter().enumerate() {
            let person = commands::create_person(
                individual.name.as_ref().map(wit_person_name).as_ref(),
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
                for fact in &individual.facts {
                    import_fact(&person.human_id, fact)?;
                }
                for citation in &individual.citations {
                    let source_id = source_human_id(&citation.source_xref, &source_titles, &mut sources)?;
                    commands::create_citation(&source_id, citation.page.as_deref())
                        .map_err(|error| format!("create-citation failed: {error:?}"))?;
                }
                for object in &individual.media {
                    if let Some(file) = &object.file
                        && !media.contains_key(file)
                    {
                        let media_id = commands::create_media(Some(file))
                            .map_err(|error| format!("create-media failed: {error:?}"))?;
                        media.insert(file.clone(), media_id);
                    }
                }
                for note in &individual.notes {
                    commands::create_note(note).map_err(|error| format!("create-note failed: {error:?}"))?;
                }
                for association in &individual.associations {
                    pending_associations.push((person.human_id.clone(), association.clone()));
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
                commands::assert_association(person, other, &wit_association_role(association.role.as_ref()))
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

/// Creates an event, sets its date, place, and address, and links each participant as the primary.
/// The place is deduped by name through `places` so a shared place is created once.
fn import_event(event: &Event, participants: &[String], places: &mut HashMap<String, String>) -> Result<(), String> {
    let event_id =
        commands::create_event(wit_event_type(event.kind)).map_err(|error| format!("create-event failed: {error:?}"))?;
    if let Some(date) = &event.date {
        commands::set_event_date(&event_id, &wit_date(date))
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
        commands::set_event_address(&event_id, &wit_address(address))
            .map_err(|error| format!("set-event-address failed: {error:?}"))?;
    }
    for person in participants {
        commands::add_event_participant(person, &event_id, ParticipantRole::Primary)
            .map_err(|error| format!("add-participant failed: {error:?}"))?;
    }
    Ok(())
}

/// Asserts one INDI-attribute fact on a person.
fn import_fact(person: &str, fact: &Fact) -> Result<(), String> {
    let date = fact.date.as_ref().map(wit_date);
    commands::assert_fact(person, &wit_fact_type(fact.kind), fact.value.as_deref(), date.as_ref())
        .map_err(|error| format!("assert-fact failed: {error:?}"))
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

/// Maps a parsed GEDCOM [`Name`] onto the host capability's `person-name` record.
fn wit_person_name(name: &Name) -> PersonName {
    PersonName {
        name_type: name.name_type.as_ref().map_or(NameType::BirthName, wit_name_type),
        given: name.given.clone(),
        surname_prefix: name.surname_prefix.clone(),
        surname: name.surname.clone(),
        nickname: name.nickname.clone(),
        prefix: name.prefix.clone(),
        suffix: name.suffix.clone(),
    }
}

/// Maps a parsed GEDCOM name kind onto the host capability's `name-type`.
fn wit_name_type(kind: &NameKind) -> NameType {
    match kind {
        NameKind::BirthName => NameType::BirthName,
        NameKind::MarriedName => NameType::MarriedName,
        NameKind::Maiden => NameType::Maiden,
        NameKind::Immigrant => NameType::Immigrant,
        NameKind::Professional => NameType::Professional,
        NameKind::AlsoKnownAs => NameType::AlsoKnownAs,
        NameKind::ReligiousName => NameType::ReligiousName,
        NameKind::Other(value) => NameType::Custom(value.clone()),
    }
}

/// Maps a parsed GEDCOM [`Date`] onto the host capability's `genealogical-date` record.
fn wit_date(date: &Date) -> GenealogicalDate {
    GenealogicalDate {
        calendar: wit_calendar(date.calendar),
        quality: wit_quality(date.quality),
        modifier: wit_modifier(&date.modifier),
        new_year_begins: date.new_year_begins,
        original_text: Some(date.original.clone()),
    }
}

/// Maps a parsed GEDCOM calendar onto the host capability's `date-calendar`.
fn wit_calendar(calendar: Calendar) -> DateCalendar {
    match calendar {
        Calendar::Gregorian => DateCalendar::Gregorian,
        Calendar::Julian => DateCalendar::Julian,
        Calendar::Hebrew => DateCalendar::Hebrew,
        Calendar::FrenchRepublican => DateCalendar::FrenchRepublican,
        Calendar::Islamic => DateCalendar::Islamic,
        Calendar::Swedish => DateCalendar::Swedish,
    }
}

/// Maps a parsed GEDCOM date quality onto the host capability's `date-quality`.
fn wit_quality(quality: DateQuality) -> WitDateQuality {
    match quality {
        DateQuality::Normal => WitDateQuality::Normal,
        DateQuality::Estimated => WitDateQuality::Estimated,
        DateQuality::Calculated => WitDateQuality::Calculated,
    }
}

/// Maps a parsed GEDCOM date modifier onto the host capability's `date-modifier`.
fn wit_modifier(modifier: &DateModifier) -> WitDateModifier {
    match modifier {
        DateModifier::Exact(point) => WitDateModifier::Exact(wit_point(point)),
        DateModifier::Before(point) => WitDateModifier::Before(wit_point(point)),
        DateModifier::After(point) => WitDateModifier::After(wit_point(point)),
        DateModifier::About(point) => WitDateModifier::About(wit_point(point)),
        DateModifier::Range { start, end } => WitDateModifier::Range(DateRange {
            start: wit_point(start),
            end: wit_point(end),
        }),
        DateModifier::Span { start, end } => WitDateModifier::Span(DateRange {
            start: wit_point(start),
            end: wit_point(end),
        }),
        DateModifier::From(point) => WitDateModifier::FromDate(wit_point(point)),
        DateModifier::To(point) => WitDateModifier::ToDate(wit_point(point)),
        DateModifier::Interpreted { date, phrase } => WitDateModifier::Interpreted(InterpretedDate {
            date: wit_point(date),
            phrase: phrase.clone(),
        }),
        DateModifier::TextOnly(text) => WitDateModifier::TextOnly(text.clone()),
    }
}

/// Maps a parsed GEDCOM date point onto the host capability's `date-point`.
fn wit_point(point: &DatePoint) -> WitDatePoint {
    WitDatePoint {
        year: point.year,
        month: point.month,
        day: point.day,
    }
}

/// Maps a parsed GEDCOM [`Address`](genealogy_gedcom::Address) onto the host capability's `address`.
fn wit_address(address: &genealogy_gedcom::Address) -> WitAddress {
    WitAddress {
        lines: address.lines.clone(),
        locality: address.locality.clone(),
        region: address.region.clone(),
        postal_code: address.postal_code.clone(),
        country: address.country.clone(),
        phone: address.phone.clone(),
        email: address.email.clone(),
        fax: address.fax.clone(),
        www: address.www.clone(),
        original_text: address.original_text.clone(),
    }
}

/// Maps the parsed GEDCOM sex onto the host capability's `sex` enum.
fn wit_sex(sex: Sex) -> WitSex {
    match sex {
        Sex::Male => WitSex::Male,
        Sex::Female => WitSex::Female,
        // The host `sex` enum has no intersex value yet; record it as unknown.
        Sex::Intersex | Sex::Unknown => WitSex::Unknown,
    }
}

/// Maps the parsed GEDCOM event kind onto the host capability's `event-type` enum.
fn wit_event_type(kind: EventKind) -> EventType {
    match kind {
        EventKind::Birth => EventType::Birth,
        EventKind::Death => EventType::Death,
        EventKind::Marriage => EventType::Marriage,
        EventKind::Baptism => EventType::Baptism,
        EventKind::Christening => EventType::Christening,
        EventKind::Burial => EventType::Burial,
        EventKind::Cremation => EventType::Cremation,
        EventKind::Census => EventType::Census,
        EventKind::Residence => EventType::Residence,
        EventKind::Immigration => EventType::Immigration,
        EventKind::Emigration => EventType::Emigration,
        EventKind::Adoption => EventType::Adoption,
        EventKind::Confirmation => EventType::Confirmation,
        EventKind::BarMitzvah => EventType::BarMitzvah,
        EventKind::BasMitzvah => EventType::BasMitzvah,
        EventKind::FirstCommunion => EventType::FirstCommunion,
        EventKind::Graduation => EventType::Graduation,
        EventKind::Naturalization => EventType::Naturalization,
        EventKind::Ordination => EventType::Ordination,
        EventKind::Probate => EventType::Probate,
        EventKind::Retirement => EventType::Retirement,
        EventKind::Will => EventType::Will,
        EventKind::Engagement => EventType::Engagement,
        EventKind::Annulment => EventType::Annulment,
        EventKind::Divorce => EventType::Divorce,
        EventKind::DivorceFiled => EventType::DivorceFiled,
        EventKind::MarriageBanns => EventType::MarriageBanns,
        EventKind::MarriageContract => EventType::MarriageContract,
        EventKind::MarriageLicense => EventType::MarriageLicense,
        EventKind::MarriageSettlement => EventType::MarriageSettlement,
    }
}

/// Maps the parsed GEDCOM fact kind onto the host capability's `fact-type` variant.
fn wit_fact_type(kind: FactKind) -> FactType {
    match kind {
        FactKind::Occupation => FactType::Occupation,
        FactKind::Religion => FactType::Religion,
        FactKind::Education => FactType::Education,
        FactKind::Caste => FactType::Caste,
        FactKind::PhysicalDescription => FactType::PhysicalDescription,
        FactKind::Ethnicity => FactType::Ethnicity,
        FactKind::NationalId => FactType::NationalId,
        FactKind::Nationality => FactType::Nationality,
        FactKind::NumberOfChildren => FactType::NumberOfChildren,
        FactKind::NumberOfMarriages => FactType::NumberOfMarriages,
        FactKind::Property => FactType::Property,
        FactKind::SocialSecurityNumber => FactType::SocialSecurityNumber,
        FactKind::NobilityTitle => FactType::NobilityTitle,
    }
}

/// Maps the parsed GEDCOM association role onto the host capability's `association-role` variant
/// (an absent role becomes a custom `associate`).
fn wit_association_role(role: Option<&AssociationKind>) -> AssociationRole {
    match role {
        Some(AssociationKind::Clergy) => AssociationRole::Clergy,
        Some(AssociationKind::Friend) => AssociationRole::Friend,
        Some(AssociationKind::Godparent) => AssociationRole::Godparent,
        Some(AssociationKind::Neighbour) => AssociationRole::Neighbour,
        Some(AssociationKind::Officiator) => AssociationRole::Officiator,
        Some(AssociationKind::Witness) => AssociationRole::Witness,
        Some(AssociationKind::Child) => AssociationRole::Child,
        Some(AssociationKind::Father) => AssociationRole::Father,
        Some(AssociationKind::Mother) => AssociationRole::Mother,
        Some(AssociationKind::Parent) => AssociationRole::Parent,
        Some(AssociationKind::Husband) => AssociationRole::Husband,
        Some(AssociationKind::Wife) => AssociationRole::Wife,
        Some(AssociationKind::Spouse) => AssociationRole::Spouse,
        Some(AssociationKind::Multiple) => AssociationRole::Multiple,
        Some(AssociationKind::Other(value)) => AssociationRole::Custom(value.clone()),
        None => AssociationRole::Custom("associate".to_owned()),
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
