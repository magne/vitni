//! The navigation rail: an "Entities" group and a "Tools" group, a single tab stop with roving
//! focus (↑/↓ move between items; Enter/Space activate the focused one).

use dioxus::prelude::*;
use vitni_app::WorkspaceCounts;
use vitni_ui::{Category, Destination, RailGroup, RailItem, rail_items};

use crate::components::or_dash;
use crate::shell::nav_state::NavState;
use crate::shell::roving::roving_vertical;
use crate::shell::{ChromeCtx, CountsCtx};

/// The projected record count for a category, read from the workspace counts.
fn count_for(counts: &WorkspaceCounts, category: Category) -> Option<u64> {
    match category {
        Category::Dashboard => None,
        Category::People => Some(counts.person),
        Category::Families => Some(counts.family),
        Category::Events => Some(counts.event),
        Category::Places => Some(counts.place),
        Category::Sources => Some(counts.source),
        Category::Citations => Some(counts.citation),
        Category::Repositories => Some(counts.repository),
        Category::Media => Some(counts.media),
        Category::Notes => Some(counts.note),
        Category::ResearchNotes => Some(counts.research_note),
        Category::Tags => Some(counts.tag),
        Category::DnaTests => Some(counts.dna_test),
        Category::DnaMatches => Some(counts.dna_match),
    }
}

/// The primary navigation rail.
#[component]
pub fn Rail() -> Element {
    let chrome = use_context::<ChromeCtx>();
    let items = rail_items();
    let total = items.len();
    let focused = use_signal(|| 0_usize);
    let nodes = use_signal(|| vec![None::<MountedEvent>; total]);
    let entities: Vec<(usize, RailItem)> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.group == RailGroup::Entities)
        .map(|(index, item)| (index, *item))
        .collect();
    let tools: Vec<(usize, RailItem)> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.group == RailGroup::Tools)
        .map(|(index, item)| (index, *item))
        .collect();
    // Behind an open `SidePanel` the rail is inert, like every other region around the panel (#312).
    let behind_panel = use_context::<NavState>().panel_inert();
    rsx! {
        aside {
            class: "rail",
            role: "navigation",
            aria_label: "{chrome.0.aria_primary_nav()}",
            inert: behind_panel,
            aria_hidden: behind_panel,
            div { class: "brand",
                span { class: "logo", aria_hidden: "true", "V" }
                span { "{chrome.0.brand_title()}" }
            }
            nav {
                onkeydown: move |event| roving_vertical(&event, focused, nodes, total),
                RailGroupView {
                    label_id: "rail-group-entities",
                    heading: chrome.0.nav_group_entities(),
                    items: entities,
                    focused,
                    nodes,
                }
                div { class: "nav-sep", aria_hidden: "true" }
                RailGroupView {
                    label_id: "rail-group-tools",
                    heading: chrome.0.nav_group_tools(),
                    items: tools,
                    focused,
                    nodes,
                }
            }
        }
    }
}

/// One labelled group of rail items (`role="list"` with a heading it is `aria-labelledby`).
#[component]
fn RailGroupView(
    label_id: String,
    heading: String,
    items: Vec<(usize, RailItem)>,
    focused: Signal<usize>,
    nodes: Signal<Vec<Option<MountedEvent>>>,
) -> Element {
    rsx! {
        div { class: "nav-group-label", id: "{label_id}", "{heading}" }
        div { role: "list", aria_labelledby: "{label_id}",
            for (index , item) in items {
                RailItemView { index, item, focused, nodes }
            }
        }
    }
}

/// One rail item: a roving-tabindex nav entry that navigates on click or Enter/Space.
#[component]
fn RailItemView(
    index: usize,
    item: RailItem,
    focused: Signal<usize>,
    nodes: Signal<Vec<Option<MountedEvent>>>,
) -> Element {
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();
    let destination = item.destination;
    let is_active = *nav.active.read() == destination;
    let is_stop = focused() == index;
    let label = chrome.0.rail_label(item.label_id);
    // The badge count, when this is an entity item and the counts have loaded.
    let count = if item.has_count {
        let counts = try_consume_context::<CountsCtx>()
            .and_then(|ctx| *ctx.0.read())
            .flatten();
        match (counts, destination) {
            (Some(counts), Destination::Category(category)) => count_for(&counts, category),
            _ => None,
        }
    } else {
        None
    };
    // Fold the count into the accessible name so it is announced; the badge itself is decorative.
    let aria_label = count.map(|n| chrome.0.rail_item_count(&label, n));
    let count_text = or_dash(count.map(|n| n.to_string()));
    rsx! {
        a {
            role: "listitem",
            class: if is_active { "nav-item active" } else { "nav-item" },
            tabindex: if is_stop { "0" } else { "-1" },
            aria_current: if is_active { "page" } else { "false" },
            aria_label,
            onmounted: move |event| {
                if let Some(slot) = nodes.write().get_mut(index) {
                    *slot = Some(event);
                }
            },
            onclick: move |_| {
                focused.set(index);
                nav.go_to(destination);
            },
            onkeydown: move |event| activate_keys(&event, &mut nav, destination),
            span { class: "ico", aria_hidden: "true", "{item.icon}" }
            span { "{label}" }
            if item.has_count {
                span { class: "count", aria_hidden: "true", "{count_text}" }
            }
        }
    }
}

/// Enter/Space on a focused rail item navigates to its destination.
fn activate_keys(event: &KeyboardEvent, nav: &mut NavState, destination: Destination) {
    let Key::Character(character) = event.key() else {
        if event.key() == Key::Enter {
            event.prevent_default();
            nav.go_to(destination);
        }
        return;
    };
    if character == " " {
        event.prevent_default();
        nav.go_to(destination);
    }
}

#[cfg(test)]
mod tests {
    use super::count_for;
    use vitni_app::WorkspaceCounts;
    use vitni_ui::Category;

    #[test]
    fn maps_each_category_to_its_count_and_skips_the_dashboard() {
        let counts = WorkspaceCounts {
            person: 5,
            family: 3,
            event: 9,
            tag: 2,
            ..WorkspaceCounts::default()
        };
        assert_eq!(count_for(&counts, Category::People), Some(5));
        assert_eq!(count_for(&counts, Category::Families), Some(3));
        assert_eq!(count_for(&counts, Category::Events), Some(9));
        assert_eq!(count_for(&counts, Category::Tags), Some(2));
        assert_eq!(count_for(&counts, Category::Sources), Some(0));
        // The dashboard is an overview, not a counted entity.
        assert_eq!(count_for(&counts, Category::Dashboard), None);
    }
}
