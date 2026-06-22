//! GEDCOM export plugin (ADR 0013): read persons, families, events, and sources through the host
//! `query` capability, serialize them to GEDCOM with `genealogy-gedcom`, and write the document to
//! the host-resolved export sink, reporting progress. Human ids become GEDCOM xrefs. The
//! format-neutral plumbing lives in `genealogy-plugin-api`; this crate only bridges the DTOs to the
//! GEDCOM [`Tree`](genealogy_gedcom::Tree).
//!
//! Events are distributed back to the records they belong under the way the importer placed them: a
//! family-event kind (marriage, divorce, …) whose participant set matches a family's partners nests
//! under that `FAM`; every other event nests under each participant `INDI`. This mirrors
//! `gedcom-import`'s `import_event`, so an import → export → import cycle is stable.

wit_bindgen::generate!({
    world: "bulk-export",
    path: "../../crates/genealogy-plugin-host/wit",
    with: {
        "genealogy:host-api/types@0.7.0": genealogy_plugin_api::types,
        "genealogy:host-api/log@0.7.0": genealogy_plugin_api::log,
        "genealogy:host-api/query@0.7.0": genealogy_plugin_api::query,
        "genealogy:host-api/progress@0.7.0": genealogy_plugin_api::progress,
        "genealogy:host-api/export-sink@0.7.0": genealogy_plugin_api::export_sink,
    },
});

use std::collections::{BTreeSet, HashMap};

use genealogy_gedcom::{
    Address, Association, AssociationKind, Calendar, Date, DateModifier, DatePoint, DateQuality, Event, EventKind, Fact,
    FactKind, Name, NameKind, Sex,
};
use genealogy_plugin_api::query;
use genealogy_plugin_api::types::{
    Address as WitAddress, AssociationRole, DateCalendar, DateModifier as WitDateModifier, DatePoint as WitDatePoint,
    DateQuality as WitDateQuality, EventDto, EventType, FactType, GenealogicalDate, NameType, PersonDto,
    Sex as WitSex,
};

struct Exporter;

impl Guest for Exporter {
    fn run_export() -> Result<u32, String> {
        let persons = query::list_persons().map_err(|error| format!("list-persons failed: {error:?}"))?;
        let families = query::list_families().map_err(|error| format!("list-families failed: {error:?}"))?;
        let events = query::list_events().map_err(|error| format!("list-events failed: {error:?}"))?;
        let sources = query::list_sources().map_err(|error| format!("list-sources failed: {error:?}"))?;
        let person_count = persons.len() as u32;
        let family_count = families.len() as u32;
        let total = person_count + family_count;
        genealogy_plugin_api::log_info(&format!(
            "exporting {person_count} individuals and {family_count} families"
        ));

        // Participation is recorded on the person (not the event), so build the event -> participant
        // human-ids map from every person's participations before consuming the person DTOs.
        let mut event_participants: HashMap<String, Vec<String>> = HashMap::new();
        for person in &persons {
            for participation in &person.participations {
                event_participants
                    .entry(participation.event.clone())
                    .or_default()
                    .push(person.human_id.clone());
            }
        }

        let mut individuals: Vec<genealogy_gedcom::Individual> = persons.into_iter().map(individual).collect();
        let individual_index: HashMap<String, usize> = individuals
            .iter()
            .enumerate()
            .map(|(index, individual)| (individual.xref.clone(), index))
            .collect();
        let mut families: Vec<genealogy_gedcom::Family> = families
            .into_iter()
            .map(|family| genealogy_gedcom::Family {
                xref: family.human_id,
                uid: None,
                partners: family.partners,
                children: family.children,
                events: Vec::new(),
            })
            .collect();

        distribute_events(
            events,
            event_participants,
            &mut individuals,
            &individual_index,
            &mut families,
        );

        let tree = genealogy_gedcom::Tree {
            individuals,
            families,
            sources: sources
                .into_iter()
                .map(|source| genealogy_gedcom::Source {
                    xref: source.human_id,
                    title: source.title,
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
    events: Vec<EventDto>,
    event_participants: HashMap<String, Vec<String>>,
    individuals: &mut [genealogy_gedcom::Individual],
    individual_index: &HashMap<String, usize>,
    families: &mut [genealogy_gedcom::Family],
) {
    let family_partner_sets: Vec<BTreeSet<String>> = families
        .iter()
        .map(|family| family.partners.iter().cloned().collect())
        .collect();
    for event_dto in events {
        let Some(kind) = event_dto.event_type.map(event_kind) else {
            continue;
        };
        let participants = event_participants.get(&event_dto.human_id).cloned().unwrap_or_default();
        let event = Event {
            kind,
            date: event_dto.date.as_ref().map(gedcom_date),
            place: event_dto.place.clone(),
            address: event_dto.addresses.first().map(gedcom_address),
        };
        if is_family_event(kind) {
            let set: BTreeSet<String> = participants.iter().cloned().collect();
            if let Some(index) = family_partner_sets.iter().position(|partners| *partners == set) {
                families[index].events.push(event);
                continue;
            }
        }
        for person in &participants {
            if let Some(&index) = individual_index.get(person) {
                individuals[index].events.push(event.clone());
            }
        }
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
fn individual(person: PersonDto) -> genealogy_gedcom::Individual {
    let has_name = person.given.is_some()
        || person.surname.is_some()
        || person.surname_prefix.is_some()
        || person.nickname.is_some()
        || person.name_prefix.is_some()
        || person.name_suffix.is_some();
    let name = has_name.then(|| Name {
        name_type: person.name_type.map(name_kind),
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
            role: Some(association_kind(association.role)),
        })
        .collect();
    genealogy_gedcom::Individual {
        xref: person.human_id,
        uid: None,
        name,
        sex: person.sex.map(gedcom_sex),
        events: Vec::new(),
        facts,
        associations,
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
    }
}

/// Maps the host capability's `name-type` onto a GEDCOM name kind.
fn name_kind(name_type: NameType) -> NameKind {
    match name_type {
        NameType::BirthName => NameKind::BirthName,
        NameType::MarriedName => NameKind::MarriedName,
        NameType::Maiden => NameKind::Maiden,
        NameType::Immigrant => NameKind::Immigrant,
        NameType::Professional => NameKind::Professional,
        NameType::AlsoKnownAs => NameKind::AlsoKnownAs,
        NameType::ReligiousName => NameKind::ReligiousName,
        NameType::Custom(value) => NameKind::Other(value),
    }
}

/// Maps the host capability's `sex` enum onto a GEDCOM sex.
fn gedcom_sex(sex: WitSex) -> Sex {
    match sex {
        WitSex::Male => Sex::Male,
        WitSex::Female => Sex::Female,
        WitSex::Intersex => Sex::Intersex,
        WitSex::Unknown => Sex::Unknown,
    }
}

/// Maps the host capability's `event-type` enum onto a GEDCOM event kind.
fn event_kind(event_type: EventType) -> EventKind {
    match event_type {
        EventType::Birth => EventKind::Birth,
        EventType::Death => EventKind::Death,
        EventType::Marriage => EventKind::Marriage,
        EventType::Baptism => EventKind::Baptism,
        EventType::Christening => EventKind::Christening,
        EventType::Burial => EventKind::Burial,
        EventType::Cremation => EventKind::Cremation,
        EventType::Census => EventKind::Census,
        EventType::Residence => EventKind::Residence,
        EventType::Immigration => EventKind::Immigration,
        EventType::Emigration => EventKind::Emigration,
        EventType::Adoption => EventKind::Adoption,
        EventType::Confirmation => EventKind::Confirmation,
        EventType::BarMitzvah => EventKind::BarMitzvah,
        EventType::BasMitzvah => EventKind::BasMitzvah,
        EventType::FirstCommunion => EventKind::FirstCommunion,
        EventType::Graduation => EventKind::Graduation,
        EventType::Naturalization => EventKind::Naturalization,
        EventType::Ordination => EventKind::Ordination,
        EventType::Probate => EventKind::Probate,
        EventType::Retirement => EventKind::Retirement,
        EventType::Will => EventKind::Will,
        EventType::Engagement => EventKind::Engagement,
        EventType::Annulment => EventKind::Annulment,
        EventType::Divorce => EventKind::Divorce,
        EventType::DivorceFiled => EventKind::DivorceFiled,
        EventType::MarriageBanns => EventKind::MarriageBanns,
        EventType::MarriageContract => EventKind::MarriageContract,
        EventType::MarriageLicense => EventKind::MarriageLicense,
        EventType::MarriageSettlement => EventKind::MarriageSettlement,
    }
}

/// Maps a host `fact` read record onto a GEDCOM INDI-attribute fact. A fact whose type is event-like
/// (birth, death, …) has no GEDCOM INDI-attribute tag and is dropped (it is a GEDCOM event, not an
/// attribute).
fn gedcom_fact(fact: genealogy_plugin_api::types::Fact) -> Option<Fact> {
    Some(Fact {
        kind: fact_kind(fact.fact_type)?,
        value: fact.value,
        date: fact.date.as_ref().map(gedcom_date),
    })
}

/// Maps the host capability's `fact-type` onto a GEDCOM fact kind; event-like and custom values have
/// no INDI-attribute tag and return `None`.
fn fact_kind(fact_type: FactType) -> Option<FactKind> {
    let kind = match fact_type {
        FactType::Occupation => FactKind::Occupation,
        FactType::Religion => FactKind::Religion,
        FactType::Education => FactKind::Education,
        FactType::Caste => FactKind::Caste,
        FactType::PhysicalDescription => FactKind::PhysicalDescription,
        FactType::Ethnicity => FactKind::Ethnicity,
        FactType::NationalId => FactKind::NationalId,
        FactType::Nationality => FactKind::Nationality,
        FactType::NumberOfChildren => FactKind::NumberOfChildren,
        FactType::NumberOfMarriages => FactKind::NumberOfMarriages,
        FactType::Property => FactKind::Property,
        FactType::SocialSecurityNumber => FactKind::SocialSecurityNumber,
        FactType::NobilityTitle => FactKind::NobilityTitle,
        FactType::Birth
        | FactType::Death
        | FactType::Baptism
        | FactType::Burial
        | FactType::Residence
        | FactType::Custom(_) => return None,
    };
    Some(kind)
}

/// Maps the host capability's `association-role` onto a GEDCOM association kind.
fn association_kind(role: AssociationRole) -> AssociationKind {
    match role {
        AssociationRole::Clergy => AssociationKind::Clergy,
        AssociationRole::Friend => AssociationKind::Friend,
        AssociationRole::Godparent => AssociationKind::Godparent,
        AssociationRole::Neighbour => AssociationKind::Neighbour,
        AssociationRole::Officiator => AssociationKind::Officiator,
        AssociationRole::Witness => AssociationKind::Witness,
        AssociationRole::Child => AssociationKind::Child,
        AssociationRole::Father => AssociationKind::Father,
        AssociationRole::Mother => AssociationKind::Mother,
        AssociationRole::Parent => AssociationKind::Parent,
        AssociationRole::Husband => AssociationKind::Husband,
        AssociationRole::Wife => AssociationKind::Wife,
        AssociationRole::Spouse => AssociationKind::Spouse,
        AssociationRole::Multiple => AssociationKind::Multiple,
        AssociationRole::Custom(value) => AssociationKind::Other(value),
    }
}

/// Maps a host `address` record onto a GEDCOM address.
fn gedcom_address(address: &WitAddress) -> Address {
    Address {
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

/// Maps a host `genealogical-date` record onto a GEDCOM date — the inverse of the import plugin's
/// `wit_date`.
fn gedcom_date(date: &GenealogicalDate) -> Date {
    Date {
        calendar: gedcom_calendar(date.calendar),
        quality: gedcom_quality(date.quality),
        modifier: gedcom_modifier(&date.modifier),
        new_year_begins: date.new_year_begins,
        original: date.original_text.clone().unwrap_or_default(),
    }
}

/// Maps a host `date-calendar` onto a GEDCOM calendar.
fn gedcom_calendar(calendar: DateCalendar) -> Calendar {
    match calendar {
        DateCalendar::Gregorian => Calendar::Gregorian,
        DateCalendar::Julian => Calendar::Julian,
        DateCalendar::Hebrew => Calendar::Hebrew,
        DateCalendar::FrenchRepublican => Calendar::FrenchRepublican,
        DateCalendar::Islamic => Calendar::Islamic,
        DateCalendar::Swedish => Calendar::Swedish,
    }
}

/// Maps a host `date-quality` onto a GEDCOM date quality.
fn gedcom_quality(quality: WitDateQuality) -> DateQuality {
    match quality {
        WitDateQuality::Normal => DateQuality::Normal,
        WitDateQuality::Estimated => DateQuality::Estimated,
        WitDateQuality::Calculated => DateQuality::Calculated,
    }
}

/// Maps a host `date-modifier` onto a GEDCOM date modifier.
fn gedcom_modifier(modifier: &WitDateModifier) -> DateModifier {
    match modifier {
        WitDateModifier::Exact(point) => DateModifier::Exact(gedcom_point(point)),
        WitDateModifier::Before(point) => DateModifier::Before(gedcom_point(point)),
        WitDateModifier::After(point) => DateModifier::After(gedcom_point(point)),
        WitDateModifier::About(point) => DateModifier::About(gedcom_point(point)),
        WitDateModifier::Range(range) => DateModifier::Range {
            start: gedcom_point(&range.start),
            end: gedcom_point(&range.end),
        },
        WitDateModifier::Span(range) => DateModifier::Span {
            start: gedcom_point(&range.start),
            end: gedcom_point(&range.end),
        },
        WitDateModifier::FromDate(point) => DateModifier::From(gedcom_point(point)),
        WitDateModifier::ToDate(point) => DateModifier::To(gedcom_point(point)),
        WitDateModifier::Interpreted(interpreted) => DateModifier::Interpreted {
            date: gedcom_point(&interpreted.date),
            phrase: interpreted.phrase.clone(),
        },
        WitDateModifier::TextOnly(text) => DateModifier::TextOnly(text.clone()),
    }
}

/// Maps a host `date-point` onto a GEDCOM date point.
fn gedcom_point(point: &WitDatePoint) -> DatePoint {
    DatePoint {
        year: point.year,
        month: point.month,
        day: point.day,
    }
}

export!(Exporter);
