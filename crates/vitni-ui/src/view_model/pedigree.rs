use super::{ConfidenceLevel, Localizer, RestrictionKind};

/// One person referenced from a pedigree chart: display name + lifespan, evidence cues, and stable
/// id for navigation. `confidence`/`source_count` describe the parent-child assertion linking this
/// node to the adjacent one; the focus person carries none (it is not itself an assertion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedigreeNodeVm {
    /// The person's user-facing id (e.g. `I0001`).
    pub human_id: String,
    /// The person's display name (falls back to the `human_id`).
    pub name: String,
    /// The "born – died" lifespan, if known.
    pub vitals: Option<String>,
    /// The surety of the parent-child link to the adjacent node, absent for the focus person.
    pub confidence: Option<ConfidenceLevel>,
    /// The localized confidence label (colour is never the only signal).
    pub confidence_label: Option<String>,
    /// How many citations back that link.
    pub source_count: usize,
    /// The person's privacy restrictions, as presentation kinds.
    pub restrictions: Vec<RestrictionKind>,
    /// Whether this node has at least one further generation beyond the flattened chart (the
    /// `aria-expanded` cue on its `role="treeitem"` — the chart itself never collapses/expands, so
    /// this only says whether the fan would continue). Unused (`false`) on the focus person and the
    /// relationship calculator's two people, which are not chart nodes.
    pub has_more: bool,
}

/// One slot in an ancestor-chart generation: a known ancestor, or a placeholder naming which parent
/// (of whom) is still unresearched — never rendered as a dead end (the evidence-first differentiator
/// carried into the pedigree chart).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PedigreeSlotVm {
    /// A known ancestor.
    Known(PedigreeNodeVm),
    /// No ancestor is shown at this slot; `hint` names whose parent it is, or a generic
    /// "unresearched" hint once the branch above is itself unknown.
    Unknown {
        /// The already-localized placeholder hint.
        hint: String,
    },
}

/// The Pedigree tool's view-model (PR 18): the focus person, the ancestor chart's generations (each
/// a complete row of `2^generation` slots, padded with [`PedigreeSlotVm::Unknown`] so the fan stays
/// rectangular), and the descendant chart's generations (variable width — a childless branch simply
/// ends, it is not a research gap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedigreeVm {
    /// The person both charts are centered on.
    pub focus: PedigreeNodeVm,
    /// Ancestor generations, nearest first (index 0 = parents).
    pub ancestor_generations: Vec<Vec<PedigreeSlotVm>>,
    /// Descendant generations, nearest first (index 0 = children).
    pub descendant_generations: Vec<Vec<PedigreeNodeVm>>,
}

impl PedigreeVm {
    /// Builds the view-model from the app's ancestor and descendant charts, localizing confidence
    /// labels and the unresearched-slot hints. `depth` is the number of generations shown on each
    /// side (as requested of the app use-cases) and bounds how many rows are flattened.
    #[must_use]
    pub fn build(
        ancestors: &vitni_app::PedigreeChart,
        descendants: &vitni_app::DescendantChart,
        depth: usize,
        loc: &Localizer,
    ) -> Self {
        Self {
            focus: pedigree_node_vm(&ancestors.focus, None, 0, false, loc),
            ancestor_generations: flatten_ancestors(ancestors, depth, loc),
            descendant_generations: flatten_descendants(descendants, depth, loc),
        }
    }
}

/// Builds a [`PedigreeNodeVm`] from an app [`PedigreePersonRef`](vitni_app::PedigreePersonRef).
pub(crate) fn pedigree_node_vm(
    person: &vitni_app::PedigreePersonRef,
    confidence: Option<vitni_app::Confidence>,
    source_count: usize,
    has_more: bool,
    loc: &Localizer,
) -> PedigreeNodeVm {
    let confidence = confidence.map(ConfidenceLevel::from);
    PedigreeNodeVm {
        human_id: person.human_id.clone(),
        name: person.name.clone().unwrap_or_else(|| person.human_id.clone()),
        vitals: person.vitals.clone(),
        confidence,
        confidence_label: confidence.map(|level| loc.confidence_label(level)),
        source_count,
        restrictions: person.restrictions.iter().map(|&r| RestrictionKind::from(r)).collect(),
        has_more,
    }
}

/// Which parent slot a placeholder/ancestor is — carried through the flatten walk to build the
/// "father of {name}" / "mother of {name}" hint (or the generic "unresearched" form).
#[derive(Debug, Clone, Copy)]
enum ParentSlot {
    Father,
    Mother,
}

/// The context a slot in the flatten walk carries: which parent role it is, and the known
/// descendant's name it is a parent of (absent once the branch itself has gone unresearched).
struct SlotContext {
    of_name: Option<String>,
    role: ParentSlot,
}

/// Flattens the ancestor tree into complete generation rows (padding unresearched branches so every
/// row has exactly `2^generation` slots), up to `depth` generations.
fn flatten_ancestors(chart: &vitni_app::PedigreeChart, depth: usize, loc: &Localizer) -> Vec<Vec<PedigreeSlotVm>> {
    let focus_name = chart.focus.name.clone().unwrap_or_else(|| chart.focus.human_id.clone());
    let mut frontier: Vec<(Option<&vitni_app::AncestorSlot>, SlotContext)> = vec![
        (
            Some(&chart.father),
            SlotContext {
                of_name: Some(focus_name.clone()),
                role: ParentSlot::Father,
            },
        ),
        (
            Some(&chart.mother),
            SlotContext {
                of_name: Some(focus_name),
                role: ParentSlot::Mother,
            },
        ),
    ];
    let mut generations = Vec::with_capacity(depth);
    for _ in 0..depth {
        if frontier.is_empty() {
            break;
        }
        let mut row = Vec::with_capacity(frontier.len());
        let mut next = Vec::with_capacity(frontier.len() * 2);
        for (slot, context) in frontier {
            match slot {
                Some(vitni_app::AncestorSlot::Known(node)) => {
                    let name = node.person.name.clone().unwrap_or_else(|| node.person.human_id.clone());
                    let has_more = matches!(node.father, vitni_app::AncestorSlot::Known(_))
                        || matches!(node.mother, vitni_app::AncestorSlot::Known(_));
                    row.push(PedigreeSlotVm::Known(pedigree_node_vm(
                        &node.person,
                        node.confidence,
                        node.source_count,
                        has_more,
                        loc,
                    )));
                    next.push((
                        Some(&node.father),
                        SlotContext {
                            of_name: Some(name.clone()),
                            role: ParentSlot::Father,
                        },
                    ));
                    next.push((
                        Some(&node.mother),
                        SlotContext {
                            of_name: Some(name),
                            role: ParentSlot::Mother,
                        },
                    ));
                }
                None | Some(vitni_app::AncestorSlot::Unknown) => {
                    row.push(PedigreeSlotVm::Unknown {
                        hint: unknown_hint(loc, &context),
                    });
                    next.push((
                        None,
                        SlotContext {
                            of_name: None,
                            role: ParentSlot::Father,
                        },
                    ));
                    next.push((
                        None,
                        SlotContext {
                            of_name: None,
                            role: ParentSlot::Mother,
                        },
                    ));
                }
            }
        }
        generations.push(row);
        frontier = next;
    }
    generations
}

/// The localized hint for an unresearched ancestor slot: "father/mother of {name}" when the known
/// descendant's name is still in context, else the generic "line unresearched" form.
fn unknown_hint(loc: &Localizer, context: &SlotContext) -> String {
    match (&context.of_name, context.role) {
        (Some(name), ParentSlot::Father) => loc.pedigree_unknown_father_of(name),
        (Some(name), ParentSlot::Mother) => loc.pedigree_unknown_mother_of(name),
        (None, ParentSlot::Father) => loc.pedigree_father_unresearched(),
        (None, ParentSlot::Mother) => loc.pedigree_mother_unresearched(),
    }
}

/// Flattens the descendant tree into generation rows, up to `depth` generations. Unlike the ancestor
/// chart, rows are not padded — an empty branch means no known children, not a research gap.
fn flatten_descendants(tree: &vitni_app::DescendantChart, depth: usize, loc: &Localizer) -> Vec<Vec<PedigreeNodeVm>> {
    let mut frontier: Vec<&vitni_app::DescendantNode> = tree.children.iter().collect();
    let mut generations = Vec::with_capacity(depth);
    for _ in 0..depth {
        if frontier.is_empty() {
            break;
        }
        let mut row = Vec::with_capacity(frontier.len());
        let mut next = Vec::new();
        for node in frontier {
            row.push(pedigree_node_vm(
                &node.person,
                node.confidence,
                node.source_count,
                !node.children.is_empty(),
                loc,
            ));
            next.extend(node.children.iter());
        }
        generations.push(row);
        frontier = next;
    }
    generations
}
