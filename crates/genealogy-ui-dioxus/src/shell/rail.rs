//! The navigation rail: an "Entities" group and a "Tools" group, a single tab stop with roving
//! focus (↑/↓ move between items; Enter/Space activate the focused one).

use dioxus::prelude::*;
use genealogy_ui::{Destination, RailGroup, RailItem, rail_items};

use crate::shell::ChromeCtx;
use crate::shell::nav_state::NavState;

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
    rsx! {
        aside { class: "rail", role: "navigation", aria_label: "{chrome.0.aria_primary_nav()}",
            div { class: "brand",
                span { class: "logo", aria_hidden: "true", "G" }
                span { "{chrome.0.brand_title()}" }
            }
            nav {
                onkeydown: move |event| roving_keys(&event, focused, nodes, total),
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
    rsx! {
        a {
            role: "listitem",
            class: if is_active { "nav-item active" } else { "nav-item" },
            tabindex: if is_stop { "0" } else { "-1" },
            aria_current: if is_active { "page" } else { "false" },
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
                span { class: "count", aria_hidden: "true", "—" }
            }
        }
    }
}

/// ↑/↓ move the single tab stop and pull DOM focus to the newly focused item.
fn roving_keys(
    event: &KeyboardEvent,
    mut focused: Signal<usize>,
    nodes: Signal<Vec<Option<MountedEvent>>>,
    total: usize,
) {
    let current = focused.peek().min(total.saturating_sub(1));
    let next = match event.key() {
        Key::ArrowDown => (current + 1).min(total.saturating_sub(1)),
        Key::ArrowUp => current.saturating_sub(1),
        _ => return,
    };
    event.prevent_default();
    focused.set(next);
    if let Some(node) = nodes.peek().get(next).and_then(Clone::clone) {
        spawn(async move {
            let _ = node.set_focus(true).await;
        });
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
