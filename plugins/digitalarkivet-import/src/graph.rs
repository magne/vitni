//! What a record says beyond its person (ADR 0040, #410): its birth, its census or church-book event,
//! their places and the household's family, as entities and links of the record's graph and the
//! shared graphs it references.
//!
//! Every record of a residence references the same census event, residence, municipality and
//! household graphs by origin, and every participant of a church-book event the same event graph, so
//! each is written once and joined by the next record. A record references them only through links of
//! its own graph, so a record the host withholds withholds what only it reaches.

use vitni_digitalarkivet::{
    BirthValue, ChurchbookEventKind, ChurchbookRole, HouseholdPosition, PersonRecord, birth_value, churchbook_event,
    churchbook_role, family_position, household_number, municipality, residence,
};
use vitni_plugin_api::staging::{
    ChildLink, EntityFields, EntityKind, EntityRef, LinkKind, MemberLink, PairLink, ParticipationLink, StagedEvent,
    StagedFamily, StagedPlace,
};
use vitni_plugin_api::types::{
    Age, Attribute, DateCalendar, DateModifier, DatePoint, DateQuality, EventType, GenealogicalDate, ParticipantRole,
    PlaceType,
};
use vitni_plugin_api::{Graph, log_warn, origin_ref};

/// The item of a record's birth event, its place link and its participation.
const BIRTH: &str = "event:BIRT";

/// The graphs a record references by origin, each once.
#[derive(Default)]
pub struct References {
    records: Vec<String>,
    graphs: Vec<Graph>,
}

impl References {
    /// Adds the graph of `record` that `build` makes, unless it was added already.
    pub fn add(&mut self, record: &str, build: impl FnOnce(&str) -> Graph) {
        if !self.records.iter().any(|added| added == record) {
            self.records.push(record.to_owned());
            self.graphs.push(build(record));
        }
    }

    /// The referenced graphs, in the order they were added.
    pub fn into_graphs(self) -> Vec<Graph> {
        self.graphs
    }
}

/// Adds what `record` says beyond its person to `graph`: the birth, then the census (with the
/// household) or the church-book event.
pub fn add_record_content(graph: &mut Graph, references: &mut References, record: &PersonRecord, person: &EntityRef) {
    add_birth(graph, references, record, person);
    if let Some(residence) = residence(record) {
        add_census(graph, references, record, person, &residence);
    } else if churchbook_event(record).is_some() {
        add_churchbook_event(graph, references, record, person);
    }
}

/// The person's birth: dated by the birth value (estimated from an age at the record's date), at the
/// birthplace.
fn add_birth(graph: &mut Graph, references: &mut References, record: &PersonRecord, person: &EntityRef) {
    let date = record
        .birth
        .as_deref()
        .and_then(birth_value)
        .and_then(|value| birth_date(&value, record_year(record)));
    let place = record
        .birthplace
        .as_deref()
        .map(|name| birthplace(references, record, name));
    if date.is_none() && place.is_none() {
        return;
    }
    let event = graph.entity(Some(BIRTH), event_fields(EventType::Birth, date));
    graph.link(
        Some(BIRTH),
        participation(person, &event, ParticipantRole::Primary, None, Vec::new()),
    );
    if let Some(place) = place {
        graph.link(
            Some(BIRTH),
            LinkKind::EventPlace(PairLink {
                owner: event,
                target: place,
            }),
        );
    }
}

/// The birthplace: the census municipality when it bears the birthplace's name, else a place of that
/// name. A name is one place only within its census municipality, or its church book: Norway has many
/// a Nes and a Vik, which the matching engine, not the key, may later find to be the same.
fn birthplace(references: &mut References, record: &PersonRecord, name: &str) -> EntityRef {
    let municipality = record.source.title.as_deref().and_then(municipality);
    let key = match municipality {
        Some(municipality) if municipality.name == name => {
            return add_municipality(references, &municipality.code, &municipality.name);
        }
        Some(municipality) => format!("birthplace:municipality:{}:{name}", municipality.code),
        None => format!(
            "birthplace:source:{}:{name}",
            record.source.title.as_deref().unwrap_or(&record.record_url)
        ),
    };
    references.add(&key, |key| place_graph(key, name, None));
    origin_ref(EntityKind::Place, &key, None)
}

/// The census: the person's participation in the residence's census event, and its place in the
/// household's family.
fn add_census(
    graph: &mut Graph,
    references: &mut References,
    record: &PersonRecord,
    person: &EntityRef,
    residence: &vitni_digitalarkivet::Residence,
) {
    let municipality = record
        .source
        .title
        .as_deref()
        .and_then(municipality)
        .map(|municipality| add_municipality(references, &municipality.code, &municipality.name));
    let residence_key = format!("residence:{}", residence.id);
    references.add(&residence_key, |key| {
        let place_type = residence.rural.then_some(PlaceType::Farm);
        let mut graph = place_graph(key, &residence.name, place_type);
        if let Some(municipality) = municipality {
            graph.link(
                None,
                LinkKind::Enclosure(PairLink {
                    owner: EntityRef::Local(0),
                    target: municipality,
                }),
            );
        }
        graph
    });
    let census_key = format!("census:{}", residence.id);
    references.add(&census_key, |key| {
        let mut graph = Graph::new(key);
        let date = record
            .source
            .year
            .as_deref()
            .and_then(|year| year.parse().ok())
            .map(year_date);
        let event = graph.entity(None, event_fields(EventType::Census, date));
        graph.link(
            None,
            LinkKind::EventPlace(PairLink {
                owner: event,
                target: origin_ref(EntityKind::Place, &residence_key, None),
            }),
        );
        graph
    });
    let attributes = record
        .role
        .iter()
        .map(|role| Attribute {
            attribute_type: "Familiestilling".to_owned(),
            value: role.clone(),
        })
        .collect();
    let census = origin_ref(EntityKind::Event, &census_key, None);
    let age = record
        .birth
        .as_deref()
        .and_then(birth_value)
        .and_then(|value| age(&value));
    graph.link(
        Some("census"),
        participation(person, &census, ParticipantRole::Primary, age, attributes),
    );
    if let Some(position) = family_position(record) {
        let household = match household_number(record) {
            Some(number) => format!("household:{}:{number}", residence.id),
            None => format!("household:{}", residence.id),
        };
        add_household(graph, references, &household, person, position);
    }
}

/// The person's place in the household's family: a partner (the head, the spouse) or a child.
fn add_household(
    graph: &mut Graph,
    references: &mut References,
    key: &str,
    person: &EntityRef,
    position: HouseholdPosition,
) {
    references.add(key, |key| {
        let mut graph = Graph::new(key);
        graph.entity(
            Some("family"),
            EntityFields::Family(StagedFamily {
                external_ids: Vec::new(),
                restrictions: Vec::new(),
            }),
        );
        graph
    });
    let family = origin_ref(EntityKind::Family, key, Some("family"));
    let link = match position {
        HouseholdPosition::Head | HouseholdPosition::Spouse => LinkKind::Partner(MemberLink {
            family,
            person: person.clone(),
        }),
        HouseholdPosition::Child => LinkKind::Child(ChildLink {
            family,
            child: person.clone(),
            relationships: Vec::new(),
        }),
    };
    graph.link(Some("household"), link);
}

/// The church-book event the record belongs to, and the person's participation in it by role.
fn add_churchbook_event(graph: &mut Graph, references: &mut References, record: &PersonRecord, person: &EntityRef) {
    let Some(event) = churchbook_event(record) else {
        return;
    };
    let key = format!("churchbook-event:{}", event.id);
    references.add(&key, |key| {
        let mut graph = Graph::new(key);
        graph.entity(
            None,
            event_fields(event_type(event.kind), Some(event_date(&event.date))),
        );
        graph
    });
    let Some(role) = record.role.as_deref().and_then(churchbook_role) else {
        log_warn(&format!(
            "no participant role reads {:?}; importing {} without a part in the event",
            record.role, record.external_id.value
        ));
        return;
    };
    let event = origin_ref(EntityKind::Event, &key, None);
    graph.link(
        Some("event"),
        participation(person, &event, participant_role(role), None, Vec::new()),
    );
}

/// The municipality's graph, added once, and a reference to it.
fn add_municipality(references: &mut References, code: &str, name: &str) -> EntityRef {
    let key = format!("municipality:{code}");
    references.add(&key, |key| place_graph(key, name, Some(PlaceType::Municipality)));
    origin_ref(EntityKind::Place, &key, None)
}

/// A graph of one place, its own entity.
fn place_graph(key: &str, name: &str, place_type: Option<PlaceType>) -> Graph {
    let mut graph = Graph::new(key);
    graph.entity(
        None,
        EntityFields::Place(StagedPlace {
            name: name.to_owned(),
            place_type,
            coordinates: None,
            restrictions: Vec::new(),
        }),
    );
    graph
}

fn event_fields(event_type: EventType, date: Option<GenealogicalDate>) -> EntityFields {
    EntityFields::Event(StagedEvent {
        event_type,
        date,
        addresses: Vec::new(),
        restrictions: Vec::new(),
    })
}

fn participation(
    person: &EntityRef,
    event: &EntityRef,
    role: ParticipantRole,
    age: Option<Age>,
    attributes: Vec<Attribute>,
) -> LinkKind {
    LinkKind::Participation(ParticipationLink {
        person: person.clone(),
        event: event.clone(),
        role,
        age,
        attributes,
        notes: Vec::new(),
        citations: Vec::new(),
    })
}

/// The year an age on `record` is given at: the census year, or the church-book event's year — never
/// a church book's title year, which is the first year the book covers.
fn record_year(record: &PersonRecord) -> Option<i32> {
    if residence(record).is_some() {
        return record.source.year.as_deref()?.parse().ok();
    }
    match birth_value(&churchbook_event(record)?.date)? {
        BirthValue::Date { year, .. } | BirthValue::Year(year) => Some(year),
        BirthValue::Age { .. } | BirthValue::Text(_) => None,
    }
}

/// The birth date a birth value gives: the date or year itself, or `record_year` less the age
/// (calculated, about), or the text verbatim.
fn birth_date(value: &BirthValue, record_year: Option<i32>) -> Option<GenealogicalDate> {
    match value {
        BirthValue::Date { year, month, day } => Some(date(
            DateQuality::Normal,
            DateModifier::Exact(DatePoint {
                year: Some(*year),
                month: Some(*month),
                day: Some(*day),
            }),
        )),
        BirthValue::Year(year) => Some(year_date(*year)),
        BirthValue::Age { years, .. } => {
            let year = record_year? - i32::from(years.unwrap_or(0));
            Some(date(
                DateQuality::Calculated,
                DateModifier::About(DatePoint {
                    year: Some(year),
                    month: None,
                    day: None,
                }),
            ))
        }
        BirthValue::Text(text) => Some(date(DateQuality::Normal, DateModifier::TextOnly(text.clone()))),
    }
}

/// The age a birth value gives, when it is one.
fn age(value: &BirthValue) -> Option<Age> {
    match value {
        BirthValue::Age { years, months } => Some(Age {
            bound: None,
            years: *years,
            months: *months,
            days: None,
            phrase: None,
        }),
        BirthValue::Date { .. } | BirthValue::Year(_) | BirthValue::Text(_) => None,
    }
}

/// A church-book event's date: the heading's date or year, else its text verbatim.
fn event_date(text: &str) -> GenealogicalDate {
    match birth_value(text) {
        Some(value @ (BirthValue::Date { .. } | BirthValue::Year(_))) => {
            birth_date(&value, None).unwrap_or_else(|| text_date(text))
        }
        Some(BirthValue::Age { .. } | BirthValue::Text(_)) | None => text_date(text),
    }
}

fn text_date(text: &str) -> GenealogicalDate {
    date(DateQuality::Normal, DateModifier::TextOnly(text.to_owned()))
}

fn year_date(year: i32) -> GenealogicalDate {
    date(
        DateQuality::Normal,
        DateModifier::Exact(DatePoint {
            year: Some(year),
            month: None,
            day: None,
        }),
    )
}

fn date(quality: DateQuality, modifier: DateModifier) -> GenealogicalDate {
    GenealogicalDate {
        calendar: DateCalendar::Gregorian,
        quality,
        modifier,
        new_year_begins: None,
        original_text: None,
    }
}

fn event_type(kind: ChurchbookEventKind) -> EventType {
    match kind {
        ChurchbookEventKind::Baptism => EventType::Baptism,
        ChurchbookEventKind::Confirmation => EventType::Confirmation,
        ChurchbookEventKind::Marriage => EventType::Marriage,
        ChurchbookEventKind::Burial => EventType::Burial,
        ChurchbookEventKind::Immigration => EventType::Immigration,
        ChurchbookEventKind::Emigration => EventType::Emigration,
    }
}

fn participant_role(role: ChurchbookRole) -> ParticipantRole {
    match role {
        ChurchbookRole::Primary => ParticipantRole::Primary,
        ChurchbookRole::Father => ParticipantRole::Father,
        ChurchbookRole::Mother => ParticipantRole::Mother,
        ChurchbookRole::Godparent => ParticipantRole::Godparent,
        ChurchbookRole::Bride => ParticipantRole::Bride,
        ChurchbookRole::Groom => ParticipantRole::Groom,
        ChurchbookRole::Clergy => ParticipantRole::Clergy,
        ChurchbookRole::Witness => ParticipantRole::Witness,
    }
}
