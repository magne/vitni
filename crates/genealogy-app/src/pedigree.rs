//! Pedigree traversal use-cases (Phase 5 PR 18): ancestor/descendant charts and a kinship
//! calculator, over the Family-mediated Person↔Family graph.
//!
//! Person projections carry no parent/child links — every kinship fact lives on the `Family`
//! aggregate (a partner assertion, a child assertion). The traversal therefore joins
//! [`list_persons`]/[`list_families`] once into in-memory lookups (mirroring the `Lookups` pattern
//! in `person.rs`/`family.rs`) and walks those, never issuing a per-person query.
//!
//! Every recursive walk is bounded by [`MAX_GENERATION_DEPTH`] and guards against a cycle (a person
//! who is their own ancestor, however that got asserted) by tracking the current path: revisiting a
//! path member stops the walk there rather than looping forever.

use std::collections::{BTreeSet, HashMap};

use genealogy_core::enums::Restriction;
use genealogy_core::ids::PersonId;
use genealogy_core::person::PersonView;
use genealogy_core::provenance::Confidence;
use genealogy_db::Store;

use crate::dto::lifespan;
use crate::error::AppError;
use crate::family::list_families;
use crate::person::list_persons;
use crate::use_case;
use crate::workspace::Workspace;

/// The hard cap on how many generations any traversal recurses, regardless of what a caller
/// requests — event-sourced data can assert a cycle, so this is also the cycle-safety backstop.
const MAX_GENERATION_DEPTH: u32 = 10;

/// A person referenced from a pedigree traversal: display name + lifespan for the chart, the stable
/// ids for navigation, and the privacy restrictions (never hidden by the app layer — the frontend
/// decides how to render a restricted record, matching every other `*Summary`/`*Ref` DTO).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRef {
    /// The person's user-facing identifier (e.g. `I0001`).
    pub human_id: String,
    /// The person's stable `PersonId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The person's display name, if resolved.
    pub name: Option<String>,
    /// A "born – died" lifespan summary, if birth/death years are known.
    pub vitals: Option<String>,
    /// The person's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// One slot in the ancestor chart: a known ancestor (possibly with further known ancestors of their
/// own), or an unresearched slot — either no parent is recorded, or [`MAX_GENERATION_DEPTH`]/the
/// caller's requested depth was reached. Both render as the mockup's dashed placeholder — "a
/// research to-do, not a dead end" — so they are not distinguished here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AncestorSlot {
    /// A known ancestor.
    Known(Box<AncestorNode>),
    /// No further ancestor is shown at this slot.
    Unknown,
}

/// One known ancestor in the chart: their person ref, the surety that they are a parent of the
/// descendant immediately below them (the family's child assertion), and their own two parent slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AncestorNode {
    /// The ancestor themselves.
    pub person: PersonRef,
    /// The surety that `person` is a parent of the descendant below this node.
    pub confidence: Option<Confidence>,
    /// How many citations back that child assertion.
    pub source_count: usize,
    /// This ancestor's father slot (assertion order's first partner in their birth family).
    pub father: AncestorSlot,
    /// This ancestor's mother slot (assertion order's second partner).
    pub mother: AncestorSlot,
}

/// The ancestor chart rooted at the focus person (the Pedigree view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedigreeChart {
    /// The person the chart is centered on.
    pub focus: PersonRef,
    /// The focus person's father slot.
    pub father: AncestorSlot,
    /// The focus person's mother slot.
    pub mother: AncestorSlot,
}

/// One descendant in the chart: their person ref, the surety that they are a child of the parent
/// immediately above them, and their own children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescendantNode {
    /// The descendant themselves.
    pub person: PersonRef,
    /// The surety that `person` is a child of the parent above this node.
    pub confidence: Option<Confidence>,
    /// How many citations back that child assertion.
    pub source_count: usize,
    /// This descendant's own children, in `human_id` order.
    pub children: Vec<DescendantNode>,
}

/// The descendant chart rooted at the focus person (the Descendants view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescendantChart {
    /// The person the chart is centered on.
    pub focus: PersonRef,
    /// The focus person's children, in `human_id` order.
    pub children: Vec<DescendantNode>,
}

/// The kinship found between two people (the Relationships view's result). Carries the raw
/// generation counts rather than a pre-composed term (e.g. "second cousin once removed") so the
/// frontend localizes the display label (ADR 0003) — `genealogy-app` stays string-free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kinship {
    /// The two `human_id`s resolve to the same person.
    Same,
    /// `person_a` is an ancestor of `person_b`, `generations` generations up (1 = parent).
    Ancestor {
        /// How many generations up from `person_b` to `person_a`.
        generations: u32,
    },
    /// `person_a` is a descendant of `person_b`, `generations` generations down (1 = child).
    Descendant {
        /// How many generations down from `person_b` to `person_a`.
        generations: u32,
    },
    /// The two share at least one parent.
    Sibling {
        /// Whether both parents are shared (a full sibling) or only one (a half sibling).
        full: bool,
    },
    /// The two share a nearest common ancestor who is neither of them.
    CommonAncestor {
        /// The shared ancestor.
        common_ancestor: PersonRef,
        /// Generations from `person_a` up to the common ancestor.
        up_a: u32,
        /// Generations from `person_b` up to the common ancestor.
        up_b: u32,
    },
}

/// The result of the kinship calculator (the Relationships view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipResult {
    /// The first person.
    pub person_a: PersonRef,
    /// The second person.
    pub person_b: PersonRef,
    /// The kinship found within [`MAX_GENERATION_DEPTH`] of both people, or `None` if they share no
    /// ancestor that close (not necessarily unrelated — just not found within the search depth).
    pub kinship: Option<Kinship>,
}

/// Builds the ancestor chart for `human_id`, showing up to `depth` generations of parents above the
/// focus person (clamped to at least 1 and at most [`MAX_GENERATION_DEPTH`]).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a store/read-model error.
pub async fn ancestors(workspace: &Workspace, human_id: &str, depth: u32) -> Result<PedigreeChart, AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let lookups = PedigreeLookups::load(workspace).await?;
    let info = lookups.person(person_id, human_id)?;
    let depth = depth.clamp(1, MAX_GENERATION_DEPTH);

    let mut path = vec![person_id];
    let edges = lookups.parent_edges(person_id);
    let father = ancestor_slot(&lookups, edges.first(), &mut path, depth);
    let mother = ancestor_slot(&lookups, edges.get(1), &mut path, depth);
    Ok(PedigreeChart {
        focus: person_ref(info, person_id),
        father,
        mother,
    })
}

/// Builds the descendant chart for `human_id`, showing up to `depth` generations of children below
/// the focus person (clamped to at least 1 and at most [`MAX_GENERATION_DEPTH`]).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a store/read-model error.
pub async fn descendants(workspace: &Workspace, human_id: &str, depth: u32) -> Result<DescendantChart, AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let lookups = PedigreeLookups::load(workspace).await?;
    let info = lookups.person(person_id, human_id)?;
    let depth = depth.clamp(1, MAX_GENERATION_DEPTH);

    let mut path = vec![person_id];
    let children = descendant_nodes(&lookups, person_id, &mut path, depth);
    Ok(DescendantChart {
        focus: person_ref(info, person_id),
        children,
    })
}

/// Computes the kinship between two people: same record, a direct ancestor/descendant line, a
/// (half-)sibling relationship, or the nearest common ancestor's generation distance from each.
/// Searches up to [`MAX_GENERATION_DEPTH`] generations on both sides.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if either `human_id` does not exist, or a store/read-model error.
pub async fn relationship(
    workspace: &Workspace,
    human_id_a: &str,
    human_id_b: &str,
) -> Result<RelationshipResult, AppError> {
    let store = workspace.store();
    let person_a_id = resolve_person_id(store, human_id_a).await?;
    let person_b_id = resolve_person_id(store, human_id_b).await?;
    let lookups = PedigreeLookups::load(workspace).await?;
    let info_a = lookups.person(person_a_id, human_id_a)?;
    let info_b = lookups.person(person_b_id, human_id_b)?;
    let person_a = person_ref(info_a, person_a_id);
    let person_b = person_ref(info_b, person_b_id);

    let kinship = find_kinship(&lookups, person_a_id, person_b_id);
    Ok(RelationshipResult {
        person_a,
        person_b,
        kinship,
    })
}

/// Finds the [`Kinship`] between two distinct-or-not people, or `None` if no shared ancestor turns
/// up within [`MAX_GENERATION_DEPTH`] of both.
fn find_kinship(lookups: &PedigreeLookups, person_a_id: PersonId, person_b_id: PersonId) -> Option<Kinship> {
    if person_a_id == person_b_id {
        return Some(Kinship::Same);
    }
    let dist_a = ancestor_distances(lookups, person_a_id);
    let dist_b = ancestor_distances(lookups, person_b_id);
    if let Some(&generations) = dist_b.get(&person_a_id) {
        return Some(Kinship::Ancestor { generations });
    }
    if let Some(&generations) = dist_a.get(&person_b_id) {
        return Some(Kinship::Descendant { generations });
    }

    let mut nearest: Option<(PersonId, u32, u32)> = None;
    for (&candidate, &up_a) in &dist_a {
        let Some(&up_b) = dist_b.get(&candidate) else { continue };
        if nearest.is_none_or(|(_, best_a, best_b)| up_a + up_b < best_a + best_b) {
            nearest = Some((candidate, up_a, up_b));
        }
    }
    let (common_ancestor_id, up_a, up_b) = nearest?;
    if up_a == 1 && up_b == 1 {
        let shared_parents = dist_a
            .iter()
            .filter(|&(pid, &up)| up == 1 && dist_b.get(pid) == Some(&1))
            .count();
        return Some(Kinship::Sibling {
            full: shared_parents >= 2,
        });
    }
    let info = lookups.persons.get(&common_ancestor_id)?;
    Some(Kinship::CommonAncestor {
        common_ancestor: person_ref(info, common_ancestor_id),
        up_a,
        up_b,
    })
}

/// Breadth-first generation distances from `start` up through its ancestors (`start` itself at
/// distance 0), capped at [`MAX_GENERATION_DEPTH`]. Cycle-safe: a `PersonId` already recorded keeps
/// its (nearer) distance and is not re-queued.
fn ancestor_distances(lookups: &PedigreeLookups, start: PersonId) -> HashMap<PersonId, u32> {
    let mut distances = HashMap::from([(start, 0)]);
    let mut frontier = vec![start];
    for generation in 1..=MAX_GENERATION_DEPTH {
        let mut next = Vec::new();
        for &person_id in &frontier {
            for edge in lookups.parent_edges(person_id) {
                if distances.contains_key(&edge.person_id) {
                    continue;
                }
                distances.insert(edge.person_id, generation);
                next.push(edge.person_id);
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    distances
}

/// Recursively builds one ancestor slot: `Unknown` if there is no edge, the depth budget is spent,
/// or `edge.person_id` is already on the current path (a cycle); otherwise a known node with its own
/// father/mother slots built from the remaining budget.
fn ancestor_slot(
    lookups: &PedigreeLookups,
    edge: Option<&Edge>,
    path: &mut Vec<PersonId>,
    remaining: u32,
) -> AncestorSlot {
    let Some(edge) = edge.filter(|_| remaining > 0) else {
        return AncestorSlot::Unknown;
    };
    if path.contains(&edge.person_id) {
        return AncestorSlot::Unknown;
    }
    let Some(info) = lookups.persons.get(&edge.person_id) else {
        return AncestorSlot::Unknown;
    };

    path.push(edge.person_id);
    let edges = lookups.parent_edges(edge.person_id);
    let father = ancestor_slot(lookups, edges.first(), path, remaining - 1);
    let mother = ancestor_slot(lookups, edges.get(1), path, remaining - 1);
    path.pop();

    AncestorSlot::Known(Box::new(AncestorNode {
        person: person_ref(info, edge.person_id),
        confidence: edge.confidence,
        source_count: edge.source_count,
        father,
        mother,
    }))
}

/// Recursively builds a person's descendant nodes, in `human_id` order, guarding against a cycle the
/// same way [`ancestor_slot`] does (skipping a child already on the current path).
fn descendant_nodes(
    lookups: &PedigreeLookups,
    person_id: PersonId,
    path: &mut Vec<PersonId>,
    remaining: u32,
) -> Vec<DescendantNode> {
    if remaining == 0 {
        return Vec::new();
    }
    let mut nodes = Vec::new();
    for edge in lookups.children_of(person_id) {
        if path.contains(&edge.person_id) {
            continue;
        }
        let Some(info) = lookups.persons.get(&edge.person_id) else {
            continue;
        };
        path.push(edge.person_id);
        let children = descendant_nodes(lookups, edge.person_id, path, remaining - 1);
        path.pop();
        nodes.push(DescendantNode {
            person: person_ref(info, edge.person_id),
            confidence: edge.confidence,
            source_count: edge.source_count,
            children,
        });
    }
    nodes.sort_by(|a, b| a.person.human_id.cmp(&b.person.human_id));
    nodes
}

/// Builds a [`PersonRef`] from a resolved [`PersonInfo`] and the id it was looked up by.
fn person_ref(info: &PersonInfo, person_id: PersonId) -> PersonRef {
    PersonRef {
        human_id: info.human_id.clone(),
        id: person_id.to_string(),
        name: info.name.clone(),
        vitals: info.vitals.clone(),
        restrictions: info.restrictions.clone(),
    }
}

/// A parent-child edge: the surety that the two are related, joined from the family's child
/// assertion, and the other end's `PersonId`.
#[derive(Debug, Clone, Copy)]
struct Edge {
    person_id: PersonId,
    confidence: Option<Confidence>,
    source_count: usize,
}

/// A person joined from the Person projection: display name, lifespan, and restrictions.
struct PersonInfo {
    human_id: String,
    name: Option<String>,
    vitals: Option<String>,
    restrictions: BTreeSet<Restriction>,
}

/// The lookups the traversal needs to walk the Person↔Family graph without a per-person query: a
/// person's join info, and — per person — the parent edges (father-slot-first, from the family where
/// they are a child) and the child edges (from every family where they are a partner).
struct PedigreeLookups {
    persons: HashMap<PersonId, PersonInfo>,
    parents_of: HashMap<PersonId, Vec<Edge>>,
    children_of: HashMap<PersonId, Vec<Edge>>,
}

impl PedigreeLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        let person_ids: HashMap<String, PersonId> = store
            .list_persons()
            .await?
            .iter()
            .filter_map(|p| Some((p.human_id()?.to_string(), p.person_id()?)))
            .collect();

        let mut persons = HashMap::with_capacity(person_ids.len());
        for summary in list_persons(workspace).await? {
            let Some(&person_id) = person_ids.get(&summary.human_id) else {
                continue;
            };
            persons.insert(
                person_id,
                PersonInfo {
                    human_id: summary.human_id.clone(),
                    name: summary.display_name.clone(),
                    vitals: lifespan(summary.birth_year(), summary.death_year()),
                    restrictions: summary.restrictions.clone(),
                },
            );
        }

        let mut parents_of: HashMap<PersonId, Vec<Edge>> = HashMap::new();
        let mut children_of: HashMap<PersonId, Vec<Edge>> = HashMap::new();
        for family in list_families(workspace).await? {
            let partner_ids: Vec<PersonId> = family
                .partners
                .iter()
                .filter_map(|partner| person_ids.get(&partner.human_id).copied())
                .collect();
            for child in &family.children {
                let Some(&child_id) = person_ids.get(&child.human_id) else {
                    continue;
                };
                for &parent_id in &partner_ids {
                    let edge = Edge {
                        person_id: parent_id,
                        confidence: child.confidence,
                        source_count: child.source_count,
                    };
                    parents_of.entry(child_id).or_default().push(edge);
                    children_of.entry(parent_id).or_default().push(Edge {
                        person_id: child_id,
                        confidence: child.confidence,
                        source_count: child.source_count,
                    });
                }
            }
        }

        Ok(Self {
            persons,
            parents_of,
            children_of,
        })
    }

    /// Looks up a joined person by id, or [`AppError::PersonNotFound`] naming `human_id` (the id the
    /// caller looked it up by) when the projection has no row for it.
    fn person(&self, person_id: PersonId, human_id: &str) -> Result<&PersonInfo, AppError> {
        self.persons
            .get(&person_id)
            .ok_or_else(|| AppError::PersonNotFound(human_id.to_owned()))
    }

    /// This person's parent edges: the father slot first, the mother slot second, from the (first)
    /// family where they are recorded as a child, in partner-assertion order.
    fn parent_edges(&self, person_id: PersonId) -> &[Edge] {
        self.parents_of.get(&person_id).map_or(&[], Vec::as_slice)
    }

    /// This person's child edges, from every family where they are a partner.
    fn children_of(&self, person_id: PersonId) -> &[Edge] {
        self.children_of.get(&person_id).map_or(&[], Vec::as_slice)
    }
}

/// Resolves a person `human_id` to its aggregate [`PersonId`], or [`AppError::PersonNotFound`].
async fn resolve_person_id(store: &Store, human_id: &str) -> Result<PersonId, AppError> {
    use_case::resolve_id(store.find_person(human_id).await?, PersonView::person_id, || {
        AppError::PersonNotFound(human_id.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::{AncestorSlot, Kinship, ancestors, descendants, relationship};
    use crate::config::{AppDefaults, IdFormats, OperatorConfig, WorkspaceDefaults};
    use crate::family::{add_child, add_partner, create_family};
    use crate::person::{NewPerson, PersonNameParts, create_person, set_restrictions};
    use crate::session::Session;
    use crate::use_case::{MutationMeta, Provenance};
    use crate::workspace::Workspace;
    use genealogy_core::enums::{ChildParentRelationship, EvidenceLevel, Restriction};
    use genealogy_core::ids::AgentId;
    use genealogy_core::provenance::{Agent, AgentKind};
    use std::collections::BTreeSet;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn operator() -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
            email: None,
        }
    }

    fn session() -> Session {
        Session::new(Agent {
            kind: AgentKind::Human,
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Ada".to_owned()),
        })
    }

    async fn setup() -> (Workspace, Session, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let ws = dir.path().join("ws");
        Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
        let workspace = Workspace::open(
            &ws,
            &operator(),
            &WorkspaceDefaults {
                id_formats: IdFormats {
                    person: "I%04d".to_owned(),
                    family: "F%04d".to_owned(),
                    ..IdFormats::default()
                },
                ..Default::default()
            },
        )
        .await
        .expect("open");
        (workspace, session(), dir)
    }

    /// Creates a person with a given/surname (so test assertions can read a display name).
    async fn person(workspace: &Workspace, session: &Session, given: &str, surname: &str) -> String {
        create_person(
            workspace,
            session,
            NewPerson {
                human_id: None,
                name: Some(PersonNameParts::simple(
                    Some(given.to_owned()),
                    Some(surname.to_owned()),
                )),
                evidence_level: EvidenceLevel::Conclusion,
            },
            Provenance::default(),
            &[],
        )
        .await
        .expect("create person")
    }

    /// Creates a family linking `partners` to `children`, each child biologically related to every
    /// partner (the common case the fixtures need).
    async fn family(workspace: &Workspace, session: &Session, partners: &[&str], children: &[&str]) -> String {
        let family_id = create_family(workspace, session, Provenance::default(), &[])
            .await
            .expect("create family");
        for partner in partners {
            add_partner(workspace, session, &family_id, partner, MutationMeta::default())
                .await
                .expect("add partner");
        }
        let relationships: Vec<(String, ChildParentRelationship)> = partners
            .iter()
            .map(|p| ((*p).to_owned(), ChildParentRelationship::Birth))
            .collect();
        for child in children {
            add_child(
                workspace,
                session,
                &family_id,
                child,
                relationships.clone(),
                MutationMeta::default(),
            )
            .await
            .expect("add child");
        }
        family_id
    }

    /// Counts consecutive `Known` father slots starting from `slot` (0 if already `Unknown`).
    fn father_chain_depth(slot: &AncestorSlot) -> u32 {
        match slot {
            AncestorSlot::Unknown => 0,
            AncestorSlot::Known(node) => 1 + father_chain_depth(&node.father),
        }
    }

    #[tokio::test]
    async fn ancestors_spans_multiple_generations_with_father_before_mother() {
        let (workspace, session, _dir) = setup().await;
        let ggf = person(&workspace, &session, "William", "Smith").await;
        let ggm = person(&workspace, &session, "Sarah", "Hill").await;
        let gf = person(&workspace, &session, "Thomas", "Smith").await;
        let gm = person(&workspace, &session, "Anna", "Berg").await;
        let father = person(&workspace, &session, "John", "Smith").await;
        let mother = person(&workspace, &session, "Mary", "Doe").await;
        let focus = person(&workspace, &session, "Alice", "Smith").await;
        family(&workspace, &session, &[&ggf, &ggm], &[&gf]).await;
        family(&workspace, &session, &[&gf, &gm], &[&father]).await;
        family(&workspace, &session, &[&father, &mother], &[&focus]).await;

        let chart = ancestors(&workspace, &focus, 4).await.expect("ancestors");
        assert_eq!(chart.focus.human_id, focus);
        let AncestorSlot::Known(dad) = &chart.father else {
            panic!("father should be known")
        };
        assert_eq!(dad.person.human_id, father);
        assert_eq!(dad.person.name.as_deref(), Some("John Smith"));
        let AncestorSlot::Known(mom) = &chart.mother else {
            panic!("mother should be known")
        };
        assert_eq!(mom.person.human_id, mother);
        let AncestorSlot::Known(paternal_gf) = &dad.father else {
            panic!("paternal grandfather known")
        };
        assert_eq!(paternal_gf.person.human_id, gf);
        let AncestorSlot::Known(paternal_ggf) = &paternal_gf.father else {
            panic!("great-grandfather known")
        };
        assert_eq!(paternal_ggf.person.human_id, ggf);
        // Depth 4 = focus + 3 ancestor generations: the great-grandparents' own parents are unknown.
        assert!(matches!(paternal_ggf.father, AncestorSlot::Unknown));
    }

    #[tokio::test]
    async fn ancestors_of_a_person_with_no_recorded_parents_is_empty() {
        let (workspace, session, _dir) = setup().await;
        let solo = person(&workspace, &session, "Lone", "Wolf").await;

        let chart = ancestors(&workspace, &solo, 4).await.expect("ancestors");
        assert!(matches!(chart.father, AncestorSlot::Unknown));
        assert!(matches!(chart.mother, AncestorSlot::Unknown));
    }

    #[tokio::test]
    async fn ancestors_respects_the_generation_cap_even_when_more_ancestors_exist() {
        let (workspace, session, _dir) = setup().await;
        // A chain of 13 people (P0..P12), each the single-partner child of the next: P0's real
        // ancestor line is 12 generations deep, well past the hard cap.
        let mut chain = Vec::with_capacity(13);
        for index in 0..13 {
            chain.push(person(&workspace, &session, "Chain", &index.to_string()).await);
        }
        for generation in 1..13 {
            family(&workspace, &session, &[&chain[generation]], &[&chain[generation - 1]]).await;
        }

        let chart = ancestors(&workspace, &chain[0], 50).await.expect("ancestors");
        assert_eq!(
            father_chain_depth(&chart.father),
            10,
            "the hard cap stops the walk at 10 known ancestor generations"
        );
    }

    #[tokio::test]
    async fn descendants_of_a_childless_person_is_empty() {
        let (workspace, session, _dir) = setup().await;
        let solo = person(&workspace, &session, "Only", "Child").await;

        let chart = descendants(&workspace, &solo, 4).await.expect("descendants");
        assert!(chart.children.is_empty());
    }

    #[tokio::test]
    async fn descendants_spans_multiple_generations() {
        let (workspace, session, _dir) = setup().await;
        let grandparent = person(&workspace, &session, "Old", "Root").await;
        let partner = person(&workspace, &session, "Old", "Partner").await;
        let parent = person(&workspace, &session, "Mid", "Branch").await;
        let sibling = person(&workspace, &session, "Other", "Branch").await;
        let child = person(&workspace, &session, "Young", "Leaf").await;
        family(&workspace, &session, &[&grandparent, &partner], &[&parent, &sibling]).await;
        family(&workspace, &session, &[&parent], &[&child]).await;

        let chart = descendants(&workspace, &grandparent, 4).await.expect("descendants");
        assert_eq!(chart.children.len(), 2, "both children of the grandparent");
        let branch = chart
            .children
            .iter()
            .find(|node| node.person.human_id == parent)
            .expect("parent branch");
        assert_eq!(branch.children.len(), 1);
        assert_eq!(branch.children[0].person.human_id, child);
        let leaf = chart
            .children
            .iter()
            .find(|node| node.person.human_id == sibling)
            .expect("sibling branch");
        assert!(leaf.children.is_empty());
    }

    #[tokio::test]
    async fn ancestor_cycle_does_not_infinite_loop() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session, "A", "Loop").await;
        let b = person(&workspace, &session, "B", "Loop").await;
        // A is B's parent, and (an asserted data error) B is also A's parent: a genuine cycle.
        family(&workspace, &session, &[&a], &[&b]).await;
        family(&workspace, &session, &[&b], &[&a]).await;

        let chart = ancestors(&workspace, &a, 8).await.expect("ancestors terminates");
        let AncestorSlot::Known(dad) = &chart.father else {
            panic!("b should be known")
        };
        assert_eq!(dad.person.human_id, b);
        assert!(
            matches!(dad.father, AncestorSlot::Unknown),
            "the walk stops rather than looping back to a"
        );
    }

    #[tokio::test]
    async fn relationship_finds_full_siblings() {
        let (workspace, session, _dir) = setup().await;
        let father = person(&workspace, &session, "Dad", "Sibling").await;
        let mother = person(&workspace, &session, "Mom", "Sibling").await;
        let a = person(&workspace, &session, "Kid", "One").await;
        let b = person(&workspace, &session, "Kid", "Two").await;
        family(&workspace, &session, &[&father, &mother], &[&a, &b]).await;

        let result = relationship(&workspace, &a, &b).await.expect("relationship");
        assert_eq!(result.kinship, Some(Kinship::Sibling { full: true }));
    }

    #[tokio::test]
    async fn relationship_finds_half_siblings() {
        let (workspace, session, _dir) = setup().await;
        let father = person(&workspace, &session, "Dad", "Half").await;
        let mother_a = person(&workspace, &session, "Mom", "A").await;
        let mother_b = person(&workspace, &session, "Mom", "B").await;
        let a = person(&workspace, &session, "Kid", "Alpha").await;
        let b = person(&workspace, &session, "Kid", "Beta").await;
        family(&workspace, &session, &[&father, &mother_a], &[&a]).await;
        family(&workspace, &session, &[&father, &mother_b], &[&b]).await;

        let result = relationship(&workspace, &a, &b).await.expect("relationship");
        assert_eq!(result.kinship, Some(Kinship::Sibling { full: false }));
    }

    #[tokio::test]
    async fn relationship_finds_grandparent_ancestor_and_its_reverse_as_descendant() {
        let (workspace, session, _dir) = setup().await;
        let grandparent = person(&workspace, &session, "Grand", "Parent").await;
        let grandparent_partner = person(&workspace, &session, "Grand", "Partner").await;
        let parent = person(&workspace, &session, "Mid", "Parent").await;
        let parent_partner = person(&workspace, &session, "Mid", "Partner").await;
        let grandchild = person(&workspace, &session, "The", "Grandchild").await;
        family(&workspace, &session, &[&grandparent, &grandparent_partner], &[&parent]).await;
        family(&workspace, &session, &[&parent, &parent_partner], &[&grandchild]).await;

        let ancestor = relationship(&workspace, &grandparent, &grandchild)
            .await
            .expect("relationship");
        assert_eq!(ancestor.kinship, Some(Kinship::Ancestor { generations: 2 }));

        let descendant = relationship(&workspace, &grandchild, &grandparent)
            .await
            .expect("relationship");
        assert_eq!(descendant.kinship, Some(Kinship::Descendant { generations: 2 }));
    }

    #[tokio::test]
    async fn relationship_of_the_same_person_is_same() {
        let (workspace, session, _dir) = setup().await;
        let solo = person(&workspace, &session, "Just", "Me").await;

        let result = relationship(&workspace, &solo, &solo).await.expect("relationship");
        assert_eq!(result.kinship, Some(Kinship::Same));
    }

    #[tokio::test]
    async fn relationship_returns_none_for_unrelated_people() {
        let (workspace, session, _dir) = setup().await;
        let a = person(&workspace, &session, "Alone", "One").await;
        let b = person(&workspace, &session, "Alone", "Two").await;

        let result = relationship(&workspace, &a, &b).await.expect("relationship");
        assert_eq!(result.kinship, None);
    }

    #[tokio::test]
    async fn ancestors_carry_confidence_source_count_and_restrictions() {
        let (workspace, session, _dir) = setup().await;
        let father = person(&workspace, &session, "Secret", "Parent").await;
        let focus = person(&workspace, &session, "Restricted", "Child").await;
        family(&workspace, &session, &[&father], &[&focus]).await;
        set_restrictions(
            &workspace,
            &session,
            &father,
            BTreeSet::from([Restriction::Confidential]),
            MutationMeta::default(),
        )
        .await
        .expect("set restrictions");

        let chart = ancestors(&workspace, &focus, 2).await.expect("ancestors");
        let AncestorSlot::Known(dad) = &chart.father else {
            panic!("father known")
        };
        assert_eq!(dad.source_count, 0, "no citations backed the child assertion");
        assert!(
            dad.person.restrictions.contains(&Restriction::Confidential),
            "the restriction is carried through, not filtered out"
        );
    }
}
