//! The candidate pass of planning (ADR 0040 §2): the profile of each new staged person, place, source
//! and repository, built from its graph and the plan, matched against the workspace.
//!
//! A person's profile gathers its names, sex, occupations and external ids, the vital events of the plan
//! it is the primary participant in (with each event's place), and its relatives through the plan's
//! family links. A relative that resolved onto a stored record is that record as the workspace knows it,
//! so a confirmed father brings his recorded birth and names to his child's match.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;
use vitni_core::enums::{FactType, ParticipantRole};
use vitni_core::ids::{ImportRunId, PersonId, PlaceId};
use vitni_core::matching::profile::{
    PersonProfile, PlaceProfile, Relative, RepositoryProfile, SourceProfile, VitalEvent, VitalKind,
};
use vitni_core::matching::{DateBasis, MatchableKind};
use vitni_core::origin::{DatasetId, RecordOrigin};

use crate::error::AppError;
use crate::person::build_name;
use crate::profile::{Profile, Profiles, place_name, vital_kind};
use crate::similar::{Matcher, SimilarRecord, WHOLE_KIND};
use crate::staging::graph::{EntityFields, LinkKind, StagedPerson};
use crate::staging::plan::{CANDIDATE_BAND, Disposition, Endpoint, ImportPlan, PlannedEntity, WriteScope};
use crate::workspace::Workspace;

/// The kinds matched against the workspace. The rest belong to their record — an event, citation, note
/// or media object is new, or the one its own origin names.
pub(crate) const MATCHED: [MatchableKind; 4] = [
    MatchableKind::Person,
    MatchableKind::Place,
    MatchableKind::Source,
    MatchableKind::Repository,
];

/// The most candidates proposed for one entity.
const LIMIT: usize = 5;

/// The candidates of every new entity of a matched kind, by its index in the plan.
///
/// The workspace is read by record: the stored relatives and places the profiles carry, then each
/// entity's candidates. A kind with [`WHOLE_KIND`] or more wanted entities is read whole instead.
pub(crate) async fn find(
    workspace: &Workspace,
    plan: &ImportPlan,
    origin: Option<(&DatasetId, ImportRunId)>,
) -> Result<Vec<(usize, Vec<SimilarRecord>)>, AppError> {
    let wanted: Vec<usize> = (0..plan.entities.len())
        .filter(|index| is_wanted(&plan.entities[*index]))
        .collect();
    let mut matcher = Matcher::load(workspace, &crowded_kinds(plan, &wanted)).await?;
    let links = Links::of(plan);
    let (persons, places) = links.stored(plan, &wanted);
    let store = workspace.store();
    matcher.profiles.include(store, MatchableKind::Person, &persons).await?;
    matcher.profiles.include(store, MatchableKind::Place, &places).await?;
    let builder = Builder {
        plan,
        links: &links,
        profiles: &matcher.profiles,
        origin,
    };
    let profiles: Vec<(usize, Profile)> = wanted
        .iter()
        .filter_map(|index| Some((*index, builder.profile(*index)?)))
        .collect();
    let claimed = claimed_by_graph(plan);
    let unclaimed = HashSet::new();
    let mut found = Vec::new();
    for (index, profile) in profiles {
        let entity = &plan.entities[index];
        let excluded = claimed.get(&entity.graph).unwrap_or(&unclaimed);
        let similar = matcher
            .similar(workspace, entity.kind, &profile, excluded, CANDIDATE_BAND, LIMIT)
            .await?;
        found.push((index, similar));
    }
    Ok(found)
}

/// The kinds with at least [`WHOLE_KIND`] of the `wanted` entities, which are read whole.
fn crowded_kinds(plan: &ImportPlan, wanted: &[usize]) -> Vec<MatchableKind> {
    let mut counts: HashMap<MatchableKind, usize> = HashMap::new();
    for index in wanted {
        *counts.entry(plan.entities[*index].kind).or_default() += 1;
    }
    let mut crowded: Vec<MatchableKind> = counts
        .into_iter()
        .filter(|(_, count)| *count >= WHOLE_KIND)
        .map(|(kind, _)| kind)
        .collect();
    crowded.sort();
    crowded
}

/// Whether the entity is new, written in full and of a matched kind.
fn is_wanted(entity: &PlannedEntity) -> bool {
    entity.scope == WriteScope::Full && entity.disposition == Disposition::New && MATCHED.contains(&entity.kind)
}

/// The stored records each graph's entities resolved onto, which none of its other entities may match
/// (ADR 0038 §4).
fn claimed_by_graph(plan: &ImportPlan) -> HashMap<usize, HashSet<String>> {
    let mut claimed: HashMap<usize, HashSet<String>> = HashMap::new();
    for entity in &plan.entities {
        if let Some(target) = entity.disposition.target() {
            claimed.entry(entity.graph).or_default().insert(target.id.clone());
        }
    }
    claimed
}

/// A family of the plan: its partners and its children, as endpoints.
#[derive(Default)]
struct Family {
    partners: Vec<Endpoint>,
    children: Vec<Endpoint>,
}

/// Every family the plan's partner and child links make.
fn families(plan: &ImportPlan) -> Vec<Family> {
    let mut families: Vec<Family> = Vec::new();
    let mut positions: HashMap<Endpoint, usize> = HashMap::new();
    for planned in &plan.links {
        let link = &plan.graphs[planned.graph].links[planned.index].link;
        let (family, member, is_child) = match link {
            LinkKind::Partner { family, person } => (family, person, false),
            LinkKind::Child { family, child, .. } => (family, child, true),
            LinkKind::Participation { .. }
            | LinkKind::FamilyEvent { .. }
            | LinkKind::EventPlace { .. }
            | LinkKind::Enclosure { .. }
            | LinkKind::CitationOf { .. }
            | LinkKind::MediaOf { .. }
            | LinkKind::NoteOf { .. }
            | LinkKind::TagOf { .. }
            | LinkKind::SourceRepository { .. }
            | LinkKind::Association { .. } => continue,
        };
        let family = plan.endpoint(planned.graph, family).clone();
        let member = plan.endpoint(planned.graph, member).clone();
        let position = *positions.entry(family).or_insert_with(|| {
            families.push(Family::default());
            families.len() - 1
        });
        let slot = &mut families[position];
        if is_child {
            slot.children.push(member);
        } else {
            slot.partners.push(member);
        }
    }
    families
}

/// The plan's links a profile follows, indexed once.
struct Links {
    families: Vec<Family>,
    /// The families each person endpoint is a partner or child of, by index in `families`.
    memberships: HashMap<Endpoint, Vec<usize>>,
    /// The planned events each planned person is the primary participant in, in link order.
    primary_events: HashMap<usize, Vec<usize>>,
    /// The place each planned event's first place link names.
    event_places: HashMap<usize, Endpoint>,
}

impl Links {
    /// The family, participation and place links of `plan`.
    fn of(plan: &ImportPlan) -> Self {
        let families = families(plan);
        let mut memberships: HashMap<Endpoint, Vec<usize>> = HashMap::new();
        for (position, family) in families.iter().enumerate() {
            for member in family.partners.iter().chain(&family.children) {
                let of = memberships.entry(member.clone()).or_default();
                if !of.contains(&position) {
                    of.push(position);
                }
            }
        }
        let mut primary_events: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut event_places: HashMap<usize, Endpoint> = HashMap::new();
        for planned in &plan.links {
            match &plan.graphs[planned.graph].links[planned.index].link {
                LinkKind::Participation {
                    person, event, role, ..
                } if *role == ParticipantRole::Primary => {
                    if let (Endpoint::Planned(person), Endpoint::Planned(event)) = (
                        plan.endpoint(planned.graph, person),
                        plan.endpoint(planned.graph, event),
                    ) {
                        primary_events.entry(*person).or_default().push(*event);
                    }
                }
                LinkKind::EventPlace { event, place } => {
                    if let Endpoint::Planned(event) = plan.endpoint(planned.graph, event) {
                        let place = plan.endpoint(planned.graph, place).clone();
                        event_places.entry(*event).or_insert(place);
                    }
                }
                LinkKind::Participation { .. }
                | LinkKind::Partner { .. }
                | LinkKind::Child { .. }
                | LinkKind::FamilyEvent { .. }
                | LinkKind::Enclosure { .. }
                | LinkKind::CitationOf { .. }
                | LinkKind::MediaOf { .. }
                | LinkKind::NoteOf { .. }
                | LinkKind::TagOf { .. }
                | LinkKind::SourceRepository { .. }
                | LinkKind::Association { .. } => {}
            }
        }
        Self {
            families,
            memberships,
            primary_events,
            event_places,
        }
    }

    /// The stored persons and places (aggregate ids) the profiles of the `wanted` entities read: their
    /// relatives that are, or resolved onto, stored persons, and the places of their own vital events. A
    /// relative is compared without a place, so a staged relative's places are not read.
    fn stored(&self, plan: &ImportPlan, wanted: &[usize]) -> (Vec<String>, Vec<String>) {
        let target = |endpoint: &Endpoint| match endpoint {
            Endpoint::Planned(index) => plan.entities.get(*index)?.disposition.target().map(|t| t.id.clone()),
            Endpoint::Stored { aggregate_id, .. } => Some(aggregate_id.clone()),
            Endpoint::Dangling => None,
        };
        let (mut persons, mut places) = (Vec::new(), Vec::new());
        for &index in wanted {
            if plan.entities[index].kind != MatchableKind::Person {
                continue;
            }
            persons.extend(self.relatives(&Endpoint::Planned(index)).filter_map(target));
            for event in self.primary_events.get(&index).map(Vec::as_slice).unwrap_or_default() {
                places.extend(self.event_places.get(event).and_then(target));
            }
        }
        (persons, places)
    }

    /// Every member of every family `me` is a partner or child of, `me` among them.
    fn relatives<'s>(&'s self, me: &'s Endpoint) -> impl Iterator<Item = &'s Endpoint> {
        let memberships = self.memberships.get(me).map(Vec::as_slice).unwrap_or_default();
        memberships
            .iter()
            .filter_map(|position| self.families.get(*position))
            .flat_map(|family| family.partners.iter().chain(&family.children))
    }
}

/// Builds the profiles of staged entities, with the stored records they read.
struct Builder<'a> {
    plan: &'a ImportPlan,
    links: &'a Links,
    profiles: &'a Profiles,
    origin: Option<(&'a DatasetId, ImportRunId)>,
}

impl Builder<'_> {
    /// The profile of the entity `index`, if it is of a matched kind.
    fn profile(&self, index: usize) -> Option<Profile> {
        let (graph, entity) = self.plan.staged(index)?;
        let origins: Vec<RecordOrigin> = self
            .origin
            .map(|(dataset, run)| RecordOrigin {
                dataset: dataset.clone(),
                record: graph.record.clone(),
                item: entity.item.clone(),
                digest: None,
                run,
            })
            .into_iter()
            .collect();
        Some(match &entity.fields {
            EntityFields::Person(person) => Profile::Person(self.person(index, person, origins)),
            EntityFields::Place(place) => Profile::Place(PlaceProfile {
                names: vec![place_name(&place.name)],
                place_type: place.place_type.clone(),
                origins,
                ..PlaceProfile::default()
            }),
            EntityFields::Source(source) => Profile::Source(SourceProfile {
                title: source.title.clone(),
                author: source.author.clone(),
                publication: source.pub_info.clone(),
                origins,
                ..SourceProfile::default()
            }),
            EntityFields::Repository(repository) => Profile::Repository(RepositoryProfile {
                name: Some(repository.name.clone()),
                origins,
                ..RepositoryProfile::default()
            }),
            EntityFields::Family(_)
            | EntityFields::Event(_)
            | EntityFields::Citation(_)
            | EntityFields::Media(_)
            | EntityFields::Note(_)
            | EntityFields::Tag(_) => return None,
        })
    }

    fn person(&self, index: usize, person: &StagedPerson, origins: Vec<RecordOrigin>) -> PersonProfile {
        let me = Endpoint::Planned(index);
        let mut profile = PersonProfile {
            names: person.names.iter().cloned().map(build_name).collect(),
            sex: person.sex.clone(),
            vitals: self.vitals(index),
            occupations: occupations(person),
            origins,
            external_ids: person.external_ids.clone(),
            ..PersonProfile::default()
        };
        let memberships = self.links.memberships.get(&me).map(Vec::as_slice).unwrap_or_default();
        for family in memberships
            .iter()
            .filter_map(|position| self.links.families.get(*position))
        {
            let is_partner = family.partners.contains(&me);
            if family.children.contains(&me) {
                profile
                    .parents
                    .extend(family.partners.iter().filter_map(|parent| self.relative(parent)));
            }
            if is_partner {
                let others = family.partners.iter().filter(|partner| **partner != me);
                profile
                    .partners
                    .extend(others.filter_map(|partner| self.relative(partner)));
                profile
                    .children
                    .extend(family.children.iter().filter_map(|child| self.relative(child)));
            }
        }
        profile
    }

    /// The vital events of the plan the person `index` is the primary participant in.
    fn vitals(&self, index: usize) -> Vec<VitalEvent> {
        let mut vitals = Vec::new();
        for &event_index in self
            .links
            .primary_events
            .get(&index)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let Some((_, staged)) = self.plan.staged(event_index) else {
                continue;
            };
            let EntityFields::Event(fields) = &staged.fields else {
                continue;
            };
            let Some(kind) = vital_kind(&fields.event_type) else {
                continue;
            };
            vitals.push(VitalEvent {
                kind,
                date: fields.date.clone(),
                basis: DateBasis::Recorded,
                place: self.place_of(event_index),
            });
        }
        vitals
    }

    /// The place of the planned event `event`, through its first place link.
    fn place_of(&self, event: usize) -> Option<PlaceProfile> {
        match self.links.event_places.get(&event)? {
            Endpoint::Planned(place) => self.planned_place(*place),
            Endpoint::Stored { aggregate_id, .. } => {
                let id = Uuid::parse_str(aggregate_id).ok()?;
                self.profiles.place(PlaceId::from_uuid(id))
            }
            Endpoint::Dangling => None,
        }
    }

    fn planned_place(&self, index: usize) -> Option<PlaceProfile> {
        if let Some(target) = self.plan.entities.get(index)?.disposition.target() {
            let id = Uuid::parse_str(&target.id).ok()?;
            return self.profiles.place(PlaceId::from_uuid(id));
        }
        let (_, staged) = self.plan.staged(index)?;
        let EntityFields::Place(place) = &staged.fields else {
            return None;
        };
        Some(PlaceProfile {
            names: vec![place_name(&place.name)],
            place_type: place.place_type.clone(),
            ..PlaceProfile::default()
        })
    }

    /// A relative as the matcher sees one: the stored record a resolved one stands for, or the staged
    /// person's names, sex and birth.
    fn relative(&self, endpoint: &Endpoint) -> Option<Relative> {
        let stored = |id: &str| {
            let id = Uuid::parse_str(id).ok()?;
            self.profiles.relative(PersonId::from_uuid(id))
        };
        match endpoint {
            Endpoint::Planned(index) => {
                if let Some(target) = self.plan.entities.get(*index)?.disposition.target() {
                    return stored(&target.id);
                }
                let (_, staged) = self.plan.staged(*index)?;
                let EntityFields::Person(person) = &staged.fields else {
                    return None;
                };
                let vitals = self.vitals(*index);
                let birth = [VitalKind::Birth, VitalKind::Baptism]
                    .into_iter()
                    .find_map(|kind| vitals.iter().find(|vital| vital.kind == kind && vital.date.is_some()))
                    .cloned();
                Some(Relative {
                    names: person.names.iter().cloned().map(build_name).collect(),
                    sex: person.sex.clone(),
                    birth,
                })
            }
            Endpoint::Stored { aggregate_id, .. } => stored(aggregate_id),
            Endpoint::Dangling => None,
        }
    }
}

/// The occupations a person's facts record.
fn occupations(person: &StagedPerson) -> Vec<String> {
    person
        .facts
        .iter()
        .filter(|fact| fact.fact_type == FactType::Occupation)
        .filter_map(|fact| fact.value.clone())
        .collect()
}
