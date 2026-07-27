//! The transitive, cycle-aware, date-aware Place hierarchy walk (ADR 0026 §1).
//!
//! The enclosed-by chain used to be read one level deep, so a place's breadcrumb showed only its
//! *direct* enclosers, never the walk up to a country. [`hierarchy_chain`] follows the primary
//! (first-asserted) or date-resolved `enclosed_by` link from a place up to a top-level place,
//! producing the full chain; [`generated_title`] turns that chain into a display title (e.g. "Saint
//! Petersburg, Russia"). A visited-id set plus [`MAX_HIERARCHY_DEPTH`] guard a malformed/circular
//! enclosure chain from looping the walk forever — real jurisdiction hierarchies are ~5 levels deep,
//! so the cap is purely defensive.
//!
//! The walk is generic over a caller-supplied `resolve` step (each place's own next-hop, if any)
//! rather than over [`genealogy_core::place::PlaceView`] directly, so it is unit-testable with a
//! trivial in-memory resolver — no store, no folded event history required.

use std::collections::HashSet;

use genealogy_core::ids::{AssertionId, PlaceId};
use genealogy_core::provenance::Confidence;

/// The maximum enclosure-chain depth the walk follows before giving up.
const MAX_HIERARCHY_DEPTH: usize = 32;

/// One resolved hop in the transitive enclosure walk: the enclosing place's id, the specific dated
/// link's date/surety, and the `AssertionId` a per-row correction targets (ADR 0026 §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HierarchyHop {
    /// The enclosing place this hop resolved to.
    pub place_id: PlaceId,
    /// The date the enclosing link held, if the link is dated.
    pub date: Option<genealogy_core::date::GenealogicalDate>,
    /// The operator's surety in the enclosing-by assertion.
    pub confidence: Option<Confidence>,
    /// The `AssertionId` that introduced this specific link.
    pub assertion_id: AssertionId,
}

/// Walks from `start` up the enclosure chain: `resolve(place_id)` returns the next hop for a given
/// place, or `None` at a top-level place (or one not in the lookup). Cycle-safe — a place id already
/// visited stops the walk rather than looping — and capped at [`MAX_HIERARCHY_DEPTH`].
#[must_use]
pub(crate) fn hierarchy_chain(start: PlaceId, resolve: impl Fn(PlaceId) -> Option<HierarchyHop>) -> Vec<HierarchyHop> {
    let mut chain = Vec::new();
    let mut visited = HashSet::from([start]);
    let mut current = start;
    for _ in 0..MAX_HIERARCHY_DEPTH {
        let Some(hop) = resolve(current) else { break };
        if !visited.insert(hop.place_id) {
            break;
        }
        current = hop.place_id;
        chain.push(hop);
    }
    chain
}

/// Builds the generated title (e.g. "Saint Petersburg, Russia"): the place's own name (or its
/// `human_id` when unnamed), followed by each ancestor's resolved name, nearest first, skipping any
/// ancestor with no resolved name (an unnamed link contributes nothing readable to the sentence).
#[must_use]
pub(crate) fn generated_title(own_name: Option<&str>, own_human_id: &str, ancestor_names: &[Option<String>]) -> String {
    let mut parts = vec![own_name.unwrap_or(own_human_id).to_owned()];
    parts.extend(ancestor_names.iter().flatten().cloned());
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::{HierarchyHop, generated_title, hierarchy_chain};
    use genealogy_core::ids::{AssertionId, PlaceId};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn place(n: u128) -> PlaceId {
        PlaceId::from_uuid(Uuid::from_u128(n))
    }

    fn hop(to: PlaceId) -> HierarchyHop {
        HierarchyHop {
            place_id: to,
            date: None,
            confidence: None,
            assertion_id: AssertionId::from_uuid(Uuid::from_u128(999)),
        }
    }

    #[test]
    fn walks_the_chain_to_a_top_level_place() {
        // farm(1) -> parish(2) -> county(3) -> country(4) -> (nothing further)
        let links = HashMap::from([(place(1), place(2)), (place(2), place(3)), (place(3), place(4))]);
        let chain = hierarchy_chain(place(1), |id| links.get(&id).map(|&next| hop(next)));
        let ids: Vec<_> = chain.iter().map(|h| h.place_id).collect();
        assert_eq!(ids, vec![place(2), place(3), place(4)]);
    }

    #[test]
    fn a_cycle_terminates_the_walk_rather_than_looping_forever() {
        // a(1) -> b(2) -> a(1) -> ...
        let links = HashMap::from([(place(1), place(2)), (place(2), place(1))]);
        let chain = hierarchy_chain(place(1), |id| links.get(&id).map(|&next| hop(next)));
        assert_eq!(chain.iter().map(|h| h.place_id).collect::<Vec<_>>(), vec![place(2)]);
    }

    #[test]
    fn a_self_enclosure_yields_an_empty_chain() {
        let links = HashMap::from([(place(1), place(1))]);
        let chain = hierarchy_chain(place(1), |id| links.get(&id).map(|&next| hop(next)));
        assert!(chain.is_empty());
    }

    #[test]
    fn depth_is_capped_against_a_pathologically_long_chain() {
        // A chain of 100 links, each enclosing the next — far past any real jurisdiction depth.
        let mut links = HashMap::new();
        for n in 0..100u128 {
            links.insert(place(n), place(n + 1));
        }
        let chain = hierarchy_chain(place(0), |id| links.get(&id).map(|&next| hop(next)));
        assert_eq!(chain.len(), super::MAX_HIERARCHY_DEPTH);
    }

    #[test]
    fn a_place_with_no_enclosing_link_yields_an_empty_chain() {
        let chain = hierarchy_chain(place(1), |_| None);
        assert!(chain.is_empty());
    }

    #[test]
    fn generated_title_joins_own_name_and_ancestor_names() {
        let title = generated_title(Some("Saint Petersburg"), "P0001", &[Some("Russia".to_owned())]);
        assert_eq!(title, "Saint Petersburg, Russia");
    }

    #[test]
    fn generated_title_falls_back_to_human_id_when_unnamed() {
        let title = generated_title(None, "P0002", &[]);
        assert_eq!(title, "P0002");
    }

    #[test]
    fn generated_title_skips_unnamed_ancestors() {
        let title = generated_title(Some("Vågå"), "P0001", &[None, Some("Norway".to_owned())]);
        assert_eq!(title, "Vågå, Norway");
    }
}
