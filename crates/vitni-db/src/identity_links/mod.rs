//! The identity cluster index (ADR 0039 §4).
//!
//! A merge decision (`PersonsMerged`) is recorded once, on the survivor's own stream, so the merged
//! record's projection never learns it was merged, and a chain of merges (C into B, then B into A) is
//! spread over several streams. Asking "which cluster is this record in, and what is its root?" needs
//! a cross-aggregate read no single projection can answer — the derived, rebuildable index this module
//! maintains (ADR 0010).
//!
//! `identity_edges` holds one row per live merge edge `(surviving, member)`, mirrored per survivor
//! from its projection. `identity_links` holds the transitive closure: one row per merged record,
//! naming its cluster's root — the survivor that is not itself merged. Both are keyed by the
//! [`MatchableKind`] so every matchable kind shares the tables.
//!
//! The app layer refuses cycles and double membership before it writes (ADR 0039 §4), but the index
//! stays well-defined without that guarantee: [`closure`] picks the lowest survivor id when a record is
//! merged into two clusters, and the lowest id on a cycle as the cycle's root.

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use std::collections::{BTreeMap, BTreeSet};

use vitni_core::person::event::{PersonEvent, PersonEventBody};

/// The live merge edges: one row per `(kind, surviving, member)`.
const IDENTITY_EDGES_TABLE: &str = "identity_edges";
/// The transitive closure: one row per `(kind, member)`, naming the member's root.
const IDENTITY_LINKS_TABLE: &str = "identity_links";

/// Whether a person event can change the survivor's live merge edges: a merge adds one, and a
/// retraction or supersession may remove one.
fn changes_edges(event: &PersonEvent) -> bool {
    match &event.body {
        PersonEventBody::PersonsMerged { .. }
        | PersonEventBody::AssertionRetracted { .. }
        | PersonEventBody::AssertionSuperseded { .. } => true,
        PersonEventBody::PersonCreated { .. }
        | PersonEventBody::NameAsserted { .. }
        | PersonEventBody::SexAsserted { .. }
        | PersonEventBody::FactAsserted { .. }
        | PersonEventBody::ParticipationAsserted { .. }
        | PersonEventBody::AssociationAsserted { .. }
        | PersonEventBody::MediaAttached { .. }
        | PersonEventBody::NoteAttached { .. }
        | PersonEventBody::CitationAdded { .. }
        | PersonEventBody::ExternalIdAdded { .. }
        | PersonEventBody::Tagged { .. }
        | PersonEventBody::Untagged { .. }
        | PersonEventBody::RestrictionsChanged { .. }
        | PersonEventBody::HumanIdChanged { .. }
        | PersonEventBody::PersonsDistinguished { .. } => false,
    }
}

/// The transitive closure of `edges` (`(surviving, member)` pairs): every member paired with its
/// cluster's root, ordered by member.
///
/// A member with several survivors follows the lowest survivor id. A cycle has no record that is not
/// merged, so the lowest id on it stands in as the root and gets no row of its own.
fn closure(edges: &[(String, String)]) -> Vec<(String, String)> {
    let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
    for (surviving, member) in edges {
        if surviving == member {
            continue;
        }
        let entry = parent.entry(member.as_str()).or_insert(surviving.as_str());
        if surviving.as_str() < *entry {
            *entry = surviving.as_str();
        }
    }
    let mut links = Vec::with_capacity(parent.len());
    for member in parent.keys() {
        let root = root_of(&parent, member);
        if root != *member {
            links.push(((*member).to_owned(), root.to_owned()));
        }
    }
    links
}

/// Follows `parent` from `start` to the first record with no parent, or, on a cycle, to the lowest id
/// on that cycle.
fn root_of<'a>(parent: &BTreeMap<&'a str, &'a str>, start: &'a str) -> &'a str {
    let mut path: Vec<&str> = vec![start];
    let mut seen: BTreeSet<&str> = BTreeSet::from([start]);
    let mut current = start;
    while let Some(next) = parent.get(current).copied() {
        if !seen.insert(next) {
            let cycle_start = path.iter().position(|id| *id == next).unwrap_or(0);
            return path.iter().skip(cycle_start).copied().min().unwrap_or(next);
        }
        path.push(next);
        current = next;
    }
    current
}

#[cfg(test)]
mod tests {
    use super::closure;

    fn edges(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(s, m)| ((*s).to_owned(), (*m).to_owned())).collect()
    }

    fn links(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        edges(pairs)
    }

    #[test]
    fn no_edges_mean_no_links() {
        assert!(closure(&[]).is_empty());
    }

    #[test]
    fn one_merge_links_the_member_to_the_survivor() {
        assert_eq!(closure(&edges(&[("a", "b")])), links(&[("b", "a")]));
    }

    #[test]
    fn a_chain_links_every_member_to_the_last_survivor() {
        assert_eq!(
            closure(&edges(&[("b", "c"), ("a", "b")])),
            links(&[("b", "a"), ("c", "a")])
        );
    }

    #[test]
    fn a_member_of_two_clusters_follows_the_lowest_survivor() {
        assert_eq!(closure(&edges(&[("z", "m"), ("a", "m")])), links(&[("m", "a")]));
    }

    #[test]
    fn a_cycle_is_rooted_at_its_lowest_id() {
        assert_eq!(
            closure(&edges(&[("a", "b"), ("b", "c"), ("c", "a"), ("c", "d")])),
            links(&[("b", "a"), ("c", "a"), ("d", "a")])
        );
    }

    #[test]
    fn a_tail_into_a_cycle_shares_the_cycle_root() {
        assert_eq!(
            closure(&edges(&[("c", "b"), ("b", "c"), ("b", "x")])),
            links(&[("c", "b"), ("x", "b")])
        );
    }

    #[test]
    fn a_self_edge_is_ignored() {
        assert!(closure(&edges(&[("a", "a")])).is_empty());
    }
}
