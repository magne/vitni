//! The candidate pass of planning (ADR 0040 §2): the profile of each new staged person, place, source
//! and repository, built from its graph and the plan, matched against the workspace.
//!
//! A person's profile gathers its names, sex, occupations and external ids, the vital events of the plan
//! it is the primary participant in (with each event's place), and its relatives through the plan's
//! family links. A relative that resolved onto a stored record is that record as the workspace knows it,
//! so a confirmed father brings his recorded birth and names to his child's match.

use std::collections::HashSet;

use uuid::Uuid;
use vitni_core::enums::{FactType, ParticipantRole};
use vitni_core::ids::{ImportRunId, PersonId, PlaceId};
use vitni_core::matching::profile::{
    PersonProfile, PlaceProfile, Relative, RepositoryProfile, SourceProfile, VitalEvent, VitalKind,
};
use vitni_core::matching::{DateBasis, MatchableKind};
use vitni_core::origin::{DatasetId, RecordOrigin};
use vitni_core::place_name::PlaceName;

use crate::error::AppError;
use crate::person::build_name;
use crate::profile::{Profile, vital_kind};
use crate::similar::{Matcher, SimilarRecord};
use crate::staging::graph::{EntityFields, LinkKind, StagedPerson};
use crate::staging::plan::{CANDIDATE_BAND, Disposition, Endpoint, ImportPlan, WriteScope};
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
pub(crate) async fn find(
    workspace: &Workspace,
    plan: &ImportPlan,
    origin: Option<(&DatasetId, ImportRunId)>,
) -> Result<Vec<(usize, Vec<SimilarRecord>)>, AppError> {
    let mut kinds: Vec<MatchableKind> = Vec::new();
    for entity in &plan.entities {
        if is_wanted(plan, entity.graph, entity.local_id) && !kinds.contains(&entity.kind) {
            kinds.push(entity.kind);
        }
    }
    let matcher = Matcher::load(workspace, &kinds).await?;
    let builder = Builder {
        plan,
        matcher: &matcher,
        origin,
        families: families(plan),
    };
    let mut found = Vec::new();
    for (index, entity) in plan.entities.iter().enumerate() {
        if !is_wanted(plan, entity.graph, entity.local_id) {
            continue;
        }
        let Some(profile) = builder.profile(index) else {
            continue;
        };
        let excluded = claimed_by_siblings(plan, entity.graph);
        let similar = matcher
            .similar(workspace, entity.kind, &profile, &excluded, CANDIDATE_BAND, LIMIT)
            .await?;
        found.push((index, similar));
    }
    Ok(found)
}

/// Whether the entity is new, written in full and of a matched kind.
fn is_wanted(plan: &ImportPlan, graph: usize, local_id: u32) -> bool {
    plan.entity(graph, local_id).is_some_and(|entity| {
        entity.scope == WriteScope::Full && entity.disposition == Disposition::New && MATCHED.contains(&entity.kind)
    })
}

/// The stored records the entities of `graph` resolved onto, which none of its other entities may
/// match (ADR 0038 §4).
fn claimed_by_siblings(plan: &ImportPlan, graph: usize) -> HashSet<String> {
    plan.entities
        .iter()
        .filter(|entity| entity.graph == graph)
        .filter_map(|entity| entity.disposition.target().map(|target| target.id.clone()))
        .collect()
}

/// A family of the plan: its partners and its children, as endpoints.
#[derive(Default)]
struct Family {
    partners: Vec<Endpoint>,
    children: Vec<Endpoint>,
}

/// Every family the plan's partner and child links make, by the family's endpoint.
fn families(plan: &ImportPlan) -> Vec<(Endpoint, Family)> {
    let mut families: Vec<(Endpoint, Family)> = Vec::new();
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
        let position = if let Some(position) = families.iter().position(|(key, _)| *key == family) {
            position
        } else {
            families.push((family, Family::default()));
            families.len() - 1
        };
        let slot = &mut families[position].1;
        if is_child {
            slot.children.push(member);
        } else {
            slot.partners.push(member);
        }
    }
    families
}

/// Builds the profiles of staged entities.
struct Builder<'a> {
    plan: &'a ImportPlan,
    matcher: &'a Matcher<'a>,
    origin: Option<(&'a DatasetId, ImportRunId)>,
    families: Vec<(Endpoint, Family)>,
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
        for (_, family) in &self.families {
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
        for planned in &self.plan.links {
            let link = &self.plan.graphs[planned.graph].links[planned.index].link;
            let LinkKind::Participation {
                person, event, role, ..
            } = link
            else {
                continue;
            };
            if *role != ParticipantRole::Primary
                || *self.plan.endpoint(planned.graph, person) != Endpoint::Planned(index)
            {
                continue;
            }
            let Endpoint::Planned(event_index) = self.plan.endpoint(planned.graph, event) else {
                continue;
            };
            let Some((_, staged)) = self.plan.staged(*event_index) else {
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
                place: self.place_of(*event_index),
            });
        }
        vitals
    }

    /// The place of the planned event `event`, through its place link.
    fn place_of(&self, event: usize) -> Option<PlaceProfile> {
        for planned in &self.plan.links {
            let link = &self.plan.graphs[planned.graph].links[planned.index].link;
            let LinkKind::EventPlace { event: of, place } = link else {
                continue;
            };
            if *self.plan.endpoint(planned.graph, of) != Endpoint::Planned(event) {
                continue;
            }
            return match self.plan.endpoint(planned.graph, place) {
                Endpoint::Planned(place) => self.planned_place(*place),
                Endpoint::Stored { aggregate_id, .. } => {
                    let id = Uuid::parse_str(aggregate_id.as_deref()?).ok()?;
                    self.matcher.profiles.place(PlaceId::from_uuid(id))
                }
                Endpoint::Dangling => None,
            };
        }
        None
    }

    fn planned_place(&self, index: usize) -> Option<PlaceProfile> {
        if let Some(target) = self.plan.entities.get(index)?.disposition.target() {
            let id = Uuid::parse_str(&target.id).ok()?;
            return self.matcher.profiles.place(PlaceId::from_uuid(id));
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
            self.matcher.profiles.relative(PersonId::from_uuid(id))
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
            Endpoint::Stored { aggregate_id, .. } => stored(aggregate_id.as_deref()?),
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

/// A place name as the record gives it: no language, no date.
fn place_name(text: &str) -> PlaceName {
    PlaceName {
        text: text.to_owned(),
        language: None,
        date: None,
    }
}
