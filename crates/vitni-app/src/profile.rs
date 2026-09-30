//! The person, family and event profiles the matching engine compares (ADR 0038 §2), built from the
//! workspace's views.
//!
//! A person profile gathers a person's names, sex, occupations and external ids, the vital events they are the
//! primary participant in (a christening counts as a baptism) with each event's place, and their
//! relatives — parents, partners and children, each with names, sex and a birth. With no dated birth or
//! baptism, an age recorded at a dated event (a census) stands in, as a birth computed from that age.
//! Its places carry every enclosing place and the country, which, with the places of the person's other
//! events and their parents' vital events, select the name cultures. Its origin is the one of the
//! assertion that created the person, never a participation's: a church record's father, mother and
//! child all take part in one baptism item.
//!
//! A family profile holds its partners' person profiles — without their partners and children, which
//! the family compares itself — its children, and its linked marriage as an event profile. An event
//! profile holds its type, date and place, and every person taking part, with their role, names, sex
//! and birth; participation is the person's, so the participants are read from the persons. Each
//! profile's origin is the one of the assertion that created its record.
//!
//! [`ProfileLookups`] reads every person, event, place and family once, so building many profiles costs
//! one load.

use std::collections::{HashMap, HashSet};

use vitni_core::date::{DateQuality, GenealogicalDate};
use vitni_core::enums::{ChildParentRelationship, EventType, FactType, ParticipantRole, PlaceType};
use vitni_core::event::EventView;
use vitni_core::family::{ChildEntry, FamilyView};
use vitni_core::ids::{EventId, PersonId, PlaceId};
use vitni_core::matching::DateBasis;
use vitni_core::matching::date::year;
use vitni_core::matching::profile::{
    EventProfile, FamilyProfile, Participant, PersonProfile, PlaceMention, PlaceProfile, Relative, VitalEvent,
    VitalKind,
};
use vitni_core::origin::RecordOrigin;
use vitni_core::person::PersonView;
use vitni_core::place::PlaceView;
use vitni_core::provenance::EventContext;
use vitni_db::{DbError, Store};

use crate::error::AppError;
use crate::event::{DateParts, gregorian_date};
use crate::person::resolve_person_id_public;
use crate::use_case;
use crate::workspace::Workspace;

/// Builds the matching profile of the person `human_id`.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or [`AppError`] on a store read failure.
pub async fn person_profile(workspace: &Workspace, human_id: &str) -> Result<PersonProfile, AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id_public(store, human_id).await?;
    let lookups = ProfileLookups::load(store).await?;
    let origins = creating_origins(store, "person", &person_id.to_string()).await?;
    lookups
        .profile(person_id, origins)
        .ok_or_else(|| AppError::PersonNotFound(human_id.to_owned()))
}

/// Builds the matching profile of the family `human_id`.
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if no such family exists, or [`AppError`] on a store read failure.
pub async fn family_profile(workspace: &Workspace, human_id: &str) -> Result<FamilyProfile, AppError> {
    let store = workspace.store();
    let found = store.find_family(human_id).await?;
    let Some((view, family_id)) = found.and_then(|view| view.family_id().map(|id| (view, id))) else {
        return Err(AppError::FamilyNotFound(human_id.to_owned()));
    };
    let lookups = ProfileLookups::load(store).await?;
    let mut partners = Vec::new();
    for partner in view.partners() {
        let origins = creating_origins(store, "person", &partner.to_string()).await?;
        if let Some(mut profile) = lookups.profile(partner, origins) {
            profile.partners.clear();
            profile.children.clear();
            partners.push(profile);
        }
    }
    let children = view
        .children()
        .iter()
        .filter_map(|child| lookups.relative(child.child_id))
        .collect();
    let marriage = view
        .linked_events()
        .into_iter()
        .find(|event| lookups.events.get(event).and_then(EventView::event_type) == Some(&EventType::Marriage));
    let marriage = match marriage {
        Some(event) => {
            let origins = creating_origins(store, "event", &event.to_string()).await?;
            lookups.event(event, origins)
        }
        None => None,
    };
    Ok(FamilyProfile {
        partners,
        children,
        marriage,
        origins: creating_origins(store, "family", &family_id.to_string()).await?,
        external_ids: view.external_ids().into_iter().cloned().collect(),
    })
}

/// Builds the matching profile of the event `human_id`.
///
/// # Errors
///
/// [`AppError::EventNotFound`] if no such event exists, or [`AppError`] on a store read failure.
pub async fn event_profile(workspace: &Workspace, human_id: &str) -> Result<EventProfile, AppError> {
    let store = workspace.store();
    let not_found = || AppError::EventNotFound(human_id.to_owned());
    let event_id = use_case::resolve_id(store.find_event(human_id).await?, EventView::event_id, not_found)?;
    let lookups = ProfileLookups::load(store).await?;
    let origins = creating_origins(store, "event", &event_id.to_string()).await?;
    lookups.event(event_id, origins).ok_or_else(not_found)
}

/// The origin of the assertion that created a record — the first event of its aggregate's stream — as
/// the profile's origins: none when it was not imported.
async fn creating_origins(
    store: &Store,
    aggregate_type: &str,
    aggregate_id: &str,
) -> Result<Vec<RecordOrigin>, AppError> {
    #[derive(serde::Deserialize)]
    struct Header {
        context: EventContext,
    }
    let events = store.read_aggregate_events(aggregate_type, aggregate_id).await?;
    let Some(created) = events.iter().min_by_key(|event| event.sequence) else {
        return Ok(Vec::new());
    };
    let header: Header = serde_json::from_str(&created.payload).map_err(|e| {
        AppError::Db(DbError::Backend(format!(
            "decoding the creating event of {aggregate_type} {aggregate_id}: {e}"
        )))
    })?;
    Ok(header.context.origin.map(|origin| *origin).into_iter().collect())
}

/// The workspace's persons, events, places and family links, read once.
struct ProfileLookups {
    persons: HashMap<PersonId, PersonView>,
    events: HashMap<EventId, EventView>,
    places: HashMap<PlaceId, PlaceView>,
    participants_of: HashMap<EventId, Vec<(PersonId, ParticipantRole)>>,
    parents_of: HashMap<PersonId, Vec<PersonId>>,
    partners_of: HashMap<PersonId, Vec<PersonId>>,
    children_of: HashMap<PersonId, Vec<PersonId>>,
}

/// Appends `value` to `key`'s list unless it is already there.
fn link(map: &mut HashMap<PersonId, Vec<PersonId>>, key: PersonId, value: PersonId) {
    let values = map.entry(key).or_default();
    if !values.contains(&value) {
        values.push(value);
    }
}

impl ProfileLookups {
    async fn load(store: &Store) -> Result<Self, AppError> {
        let mut persons = HashMap::new();
        let mut participants_of: HashMap<EventId, Vec<(PersonId, ParticipantRole)>> = HashMap::new();
        for view in store.list_persons().await? {
            if let Some(id) = view.person_id() {
                for participation in view.participations() {
                    participants_of
                        .entry(participation.event_id)
                        .or_default()
                        .push((id, participation.role.clone()));
                }
                persons.insert(id, view);
            }
        }
        let mut events = HashMap::new();
        for view in store.list_events().await? {
            if let Some(id) = view.event_id() {
                events.insert(id, view);
            }
        }
        let mut places = HashMap::new();
        for view in store.list_places().await? {
            if let Some(id) = view.place_id() {
                places.insert(id, view);
            }
        }
        let mut lookups = Self {
            persons,
            events,
            places,
            participants_of,
            parents_of: HashMap::new(),
            partners_of: HashMap::new(),
            children_of: HashMap::new(),
        };
        for family in store.list_families().await? {
            lookups.add_family(&family);
        }
        Ok(lookups)
    }

    /// Records the partner, parent and child links of one family. A child is linked only to the partners
    /// it is a birth (or unspecified) child of: an adoptive, foster or step parent is not the parent a
    /// record of the child's birth names.
    fn add_family(&mut self, family: &FamilyView) {
        let partners = family.partners();
        let children = family.children();
        for &partner in &partners {
            for &other in &partners {
                if other != partner {
                    link(&mut self.partners_of, partner, other);
                }
            }
            for child in &children {
                if is_birth_child(child, partner) {
                    link(&mut self.parents_of, child.child_id, partner);
                    link(&mut self.children_of, partner, child.child_id);
                }
            }
        }
    }

    /// The profile of `person_id`, with its creating `origins`, or `None` when it is not projected.
    fn profile(&self, person_id: PersonId, origins: Vec<RecordOrigin>) -> Option<PersonProfile> {
        let view = self.persons.get(&person_id)?;
        let related = |map: &HashMap<PersonId, Vec<PersonId>>| -> Vec<Relative> {
            let ids = map.get(&person_id).map_or(&[][..], Vec::as_slice);
            ids.iter().filter_map(|id| self.relative(*id)).collect()
        };
        let parents = related(&self.parents_of);
        let mut lineage = self.mentions(view);
        for parent in self.parents_of.get(&person_id).into_iter().flatten() {
            if let Some(parent) = self.persons.get(parent) {
                lineage.extend(self.vitals(parent).iter().filter_map(mention));
            }
        }
        let occupations = view
            .facts()
            .into_iter()
            .filter(|fact| fact.value.fact_type == FactType::Occupation)
            .filter_map(|fact| fact.value.value.clone())
            .collect();
        Some(PersonProfile {
            names: view.names().into_iter().cloned().collect(),
            sex: view.sex().cloned(),
            vitals: self.vitals(view),
            lineage,
            occupations,
            parents,
            partners: related(&self.partners_of),
            children: related(&self.children_of),
            origins,
            external_ids: view.external_ids().into_iter().cloned().collect(),
        })
    }

    /// The profile of `event_id`, with its creating `origins`, or `None` when it is not projected.
    fn event(&self, event_id: EventId, origins: Vec<RecordOrigin>) -> Option<EventProfile> {
        let view = self.events.get(&event_id)?;
        let taking_part = self.participants_of.get(&event_id).map_or(&[][..], Vec::as_slice);
        let mut participants = Vec::new();
        for (person_id, role) in taking_part {
            if let Some(person) = self.relative(*person_id) {
                participants.push(Participant {
                    role: role.clone(),
                    person,
                });
            }
        }
        Some(EventProfile {
            event_type: view.event_type().cloned(),
            date: view.date().cloned(),
            place: view.place_id().map(|place| self.place(place)),
            participants,
            origins,
        })
    }

    /// A relative's names, sex and birth (or the baptism standing in for it).
    fn relative(&self, person_id: PersonId) -> Option<Relative> {
        let view = self.persons.get(&person_id)?;
        let vitals = self.vitals(view);
        let birth = [VitalKind::Birth, VitalKind::Baptism]
            .into_iter()
            .find_map(|kind| vitals.iter().find(|vital| vital.kind == kind && is_dated(vital)))
            .cloned();
        Some(Relative {
            names: view.names().into_iter().cloned().collect(),
            sex: view.sex().cloned(),
            birth,
        })
    }

    /// The vital events `view` is the primary participant in, and, when neither a birth nor a baptism
    /// is dated, a birth computed from an age recorded at a dated event.
    fn vitals(&self, view: &PersonView) -> Vec<VitalEvent> {
        let mut vitals = Vec::new();
        for participation in view.participations() {
            if participation.role != ParticipantRole::Primary {
                continue;
            }
            let Some(event) = self.events.get(&participation.event_id) else {
                continue;
            };
            let Some(kind) = event.event_type().and_then(vital_kind) else {
                continue;
            };
            vitals.push(VitalEvent {
                kind,
                date: event.date().cloned(),
                basis: DateBasis::Recorded,
                place: event.place_id().map(|place| self.place(place)),
            });
        }
        let born = vitals.iter().any(|vital| {
            let birth = match vital.kind {
                VitalKind::Birth | VitalKind::Baptism => true,
                VitalKind::Death | VitalKind::Burial => false,
            };
            birth && is_dated(vital)
        });
        if !born {
            vitals.extend(self.birth_from_age(view));
        }
        vitals
    }

    /// A birth computed from the first age recorded at a dated event.
    fn birth_from_age(&self, view: &PersonView) -> Option<VitalEvent> {
        view.participations().into_iter().find_map(|participation| {
            let years = participation.age.as_ref()?.years?;
            let event = self.events.get(&participation.event_id)?;
            let date = birth_from_age(event.date()?, years)?;
            Some(VitalEvent {
                kind: VitalKind::Birth,
                date: Some(date),
                basis: DateBasis::FromAge,
                place: None,
            })
        })
    }

    /// The countries of the places of `view`'s events that are not vital events — residences, censuses
    /// — which select name cultures without being compared.
    fn mentions(&self, view: &PersonView) -> Vec<PlaceMention> {
        let mut mentions = Vec::new();
        for participation in view.participations() {
            let Some(event) = self.events.get(&participation.event_id) else {
                continue;
            };
            if event.event_type().and_then(vital_kind).is_some() {
                continue;
            }
            let Some(country) = event.place_id().and_then(|place| self.place(place).country) else {
                continue;
            };
            mentions.push(PlaceMention {
                country,
                date: event.date().cloned(),
            });
        }
        mentions
    }

    /// A place's names, every place enclosing it, its country and coordinates.
    fn place(&self, place_id: PlaceId) -> PlaceProfile {
        let Some(view) = self.places.get(&place_id) else {
            return PlaceProfile {
                id: Some(place_id),
                ..PlaceProfile::default()
            };
        };
        let enclosing = self.enclosing(place_id);
        let country = std::iter::once(&place_id)
            .chain(&enclosing)
            .filter_map(|id| self.places.get(id))
            .find(|place| place.place_type() == Some(&PlaceType::Country))
            .and_then(|country| country.names().first().map(|name| name.text.clone()));
        PlaceProfile {
            id: Some(place_id),
            names: view.names().into_iter().cloned().collect(),
            place_type: view.place_type().cloned(),
            enclosing,
            country,
            coordinates: view.coordinates().copied(),
            origins: Vec::new(),
        }
    }

    /// Every place enclosing `place_id`, nearest first, through every enclosure link; cycle-safe.
    fn enclosing(&self, place_id: PlaceId) -> Vec<PlaceId> {
        let mut seen = HashSet::from([place_id]);
        let mut found = Vec::new();
        let mut next = 0;
        let mut frontier = vec![place_id];
        while let Some(&current) = frontier.get(next) {
            next += 1;
            let Some(view) = self.places.get(&current) else {
                continue;
            };
            for parent in view.enclosed_by() {
                if seen.insert(parent.place_id) {
                    found.push(parent.place_id);
                    frontier.push(parent.place_id);
                }
            }
        }
        found
    }
}

/// The vital kind an event type is, if any. A christening is a baptism.
fn vital_kind(event_type: &EventType) -> Option<VitalKind> {
    match event_type {
        EventType::Birth => Some(VitalKind::Birth),
        EventType::Baptism | EventType::Christening => Some(VitalKind::Baptism),
        EventType::Death => Some(VitalKind::Death),
        EventType::Burial => Some(VitalKind::Burial),
        EventType::Marriage
        | EventType::Cremation
        | EventType::Census
        | EventType::Residence
        | EventType::Immigration
        | EventType::Emigration
        | EventType::Adoption
        | EventType::Confirmation
        | EventType::BarMitzvah
        | EventType::BasMitzvah
        | EventType::FirstCommunion
        | EventType::Graduation
        | EventType::Naturalization
        | EventType::Ordination
        | EventType::Probate
        | EventType::Retirement
        | EventType::Will
        | EventType::Engagement
        | EventType::Annulment
        | EventType::Divorce
        | EventType::DivorceFiled
        | EventType::MarriageBanns
        | EventType::MarriageContract
        | EventType::MarriageLicense
        | EventType::MarriageSettlement
        | EventType::Custom(_) => None,
    }
}

/// Whether `child` is a birth child of `partner`: the relationship is a birth, unknown, or unstated.
fn is_birth_child(child: &ChildEntry, partner: PersonId) -> bool {
    let Some((_, relationship)) = child.relationships.iter().find(|(parent, _)| *parent == partner) else {
        return true;
    };
    match relationship {
        ChildParentRelationship::Birth | ChildParentRelationship::Unknown => true,
        ChildParentRelationship::Adopted
        | ChildParentRelationship::Foster
        | ChildParentRelationship::Step
        | ChildParentRelationship::Sealed
        | ChildParentRelationship::Custom(_) => false,
    }
}

/// Whether a vital event carries a date with a year, the least the matcher can compare.
fn is_dated(vital: &VitalEvent) -> bool {
    vital.date.as_ref().and_then(year).is_some()
}

/// The country a vital event's place lies in, as a lineage mention.
fn mention(vital: &VitalEvent) -> Option<PlaceMention> {
    Some(PlaceMention {
        country: vital.place.as_ref()?.country.clone()?,
        date: vital.date.clone(),
    })
}

/// The birth year of someone `years` old at `date`, as a calculated date; `None` for an undated
/// (text-only or yearless) `date`.
fn birth_from_age(date: &GenealogicalDate, years: u16) -> Option<GenealogicalDate> {
    let year = year(date)?.checked_sub(i32::from(years))?;
    let mut birth = gregorian_date(DateParts {
        year,
        month: None,
        day: None,
    });
    birth.quality = DateQuality::Calculated;
    Some(birth)
}
