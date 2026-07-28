//! The navigation rail's descriptor list: framework-neutral data a renderer turns into the `<nav>`.
//!
//! Each [`RailItem`] names its icon, Fluent label id, `g`-key, destination, and whether it shows a
//! count badge. Labels are message *ids* (ADR 0003) — the renderer's chrome catalogue resolves them;
//! this crate ships no display text. Icons are emoji presented `aria-hidden` (the label is the name).
//!
//! The per-item icon/label/`g`-key are projected from the [`Category`]/[`Tool`] const methods, so the
//! rail and the `g`-prefix shortcut map share one source and cannot drift.

use crate::navigation::{Category, Destination, Tool};

/// Which rail group an item belongs to (entities vs. tools — never mixed, per the locked design).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailGroup {
    /// The "Entities" group (a list of things).
    Entities,
    /// The "Tools" group (an action/function).
    Tools,
}

/// One rail entry the renderer maps to a nav item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RailItem {
    /// Which group this belongs to.
    pub group: RailGroup,
    /// Where activating this item navigates.
    pub destination: Destination,
    /// The emoji icon, rendered `aria-hidden` (decorative; the label is the accessible name).
    pub icon: &'static str,
    /// The Fluent message id for the label (resolved by the renderer's chrome catalogue).
    pub label_id: &'static str,
    /// The `g`-prefix key, if this destination has one.
    pub nav_key: Option<char>,
    /// Whether this item shows a count badge (`aria-hidden`; the count comes from app data later).
    pub has_count: bool,
}

/// The complete rail, in display order: every entity category first, then every tool.
///
/// Count badges are flagged on every entity except [`Category::Dashboard`]; tools carry none. The
/// renderer wires the actual numbers from app aggregates in a later PR.
#[must_use]
pub fn rail_items() -> Vec<RailItem> {
    let mut items = Vec::with_capacity(Category::all().len() + Tool::all().len());
    for category in Category::all() {
        items.push(RailItem {
            group: RailGroup::Entities,
            destination: Destination::Category(category),
            icon: category.icon(),
            label_id: category.label_id(),
            nav_key: category.nav_key(),
            has_count: !matches_dashboard(category),
        });
    }
    for tool in Tool::all() {
        items.push(RailItem {
            group: RailGroup::Tools,
            destination: Destination::Tool(tool),
            icon: tool.icon(),
            label_id: tool.label_id(),
            nav_key: None,
            has_count: false,
        });
    }
    items.push(RailItem {
        group: RailGroup::Tools,
        destination: Destination::Help { topic: None },
        icon: "❔",
        label_id: "nav-help",
        nav_key: None,
        has_count: false,
    });
    items
}

/// Whether a category is the dashboard — the one entity without a count badge.
fn matches_dashboard(category: Category) -> bool {
    match category {
        Category::Dashboard => true,
        Category::People
        | Category::Families
        | Category::Events
        | Category::Places
        | Category::Sources
        | Category::Citations
        | Category::Repositories
        | Category::Media
        | Category::Notes
        | Category::ResearchNotes
        | Category::Tags
        | Category::DnaTests
        | Category::DnaMatches => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{RailGroup, rail_items};
    use crate::navigation::{Category, Destination, Tool};

    #[test]
    fn every_destination_has_exactly_one_rail_item() {
        let destinations: Vec<Destination> = rail_items().iter().map(|item| item.destination).collect();
        // Every category, every tool, plus the single Help entry.
        assert_eq!(destinations.len(), Category::all().len() + Tool::all().len() + 1);
        for category in Category::all() {
            let target = Destination::Category(category);
            assert_eq!(destinations.iter().filter(|item| **item == target).count(), 1);
        }
        for tool in Tool::all() {
            let target = Destination::Tool(tool);
            assert_eq!(destinations.iter().filter(|item| **item == target).count(), 1);
        }
        let help = Destination::Help { topic: None };
        assert_eq!(destinations.iter().filter(|item| **item == help).count(), 1);
    }

    #[test]
    fn entity_nav_keys_match_their_category() {
        for item in rail_items() {
            if let Destination::Category(category) = item.destination {
                assert_eq!(item.nav_key, category.nav_key());
            }
        }
    }

    #[test]
    fn entities_precede_tools() {
        let mut seen_tool = false;
        for item in rail_items() {
            match item.group {
                RailGroup::Tools => seen_tool = true,
                RailGroup::Entities => assert!(!seen_tool, "an entity item followed a tool item"),
            }
        }
    }

    #[test]
    fn only_non_dashboard_entities_carry_a_count() {
        for item in rail_items() {
            let is_dashboard = item.destination == Destination::Category(Category::Dashboard);
            let expected = item.group == RailGroup::Entities && !is_dashboard;
            assert_eq!(item.has_count, expected, "count flag wrong for {:?}", item.destination);
        }
    }

    #[test]
    fn label_ids_are_unique() {
        let labels: Vec<&str> = rail_items().iter().map(|item| item.label_id).collect();
        let unique: BTreeSet<&str> = labels.iter().copied().collect();
        assert_eq!(labels.len(), unique.len());
    }
}
