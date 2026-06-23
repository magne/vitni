//! Roving-`tabindex` keyboard helper, shared by composite widgets (the navigation rail, entity
//! lists). One element in the group is the tab stop (`tabindex=0`); ↑/↓ move that stop and pull DOM
//! focus to the newly focused element. The group's items store their mounted nodes so focus can be
//! driven programmatically.

use dioxus::prelude::*;

/// Moves the single tab stop on ↑/↓ and pulls DOM focus to the newly focused item.
///
/// `focused` is the index of the current stop; `nodes` holds each item's mounted node (indexed the
/// same way); `total` is the item count. Other keys are left untouched so they can bubble.
pub fn roving_vertical(
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
