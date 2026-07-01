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

/// Moves the single tab stop on a 2D grid (the pedigree chart's generation columns) and pulls DOM
/// focus to the newly focused cell. ↑/↓ move within the current column (index 0); ←/→ move across
/// columns (index 1), clamping the row into the target column's length. `shape[column]` is that
/// column's row count; other keys are left untouched so they can bubble.
pub fn roving_grid(
    event: &KeyboardEvent,
    shape: &[usize],
    mut focused: Signal<(usize, usize)>,
    nodes: Signal<Vec<Vec<Option<MountedEvent>>>>,
) {
    let (column, row) = *focused.peek();
    let column_len = shape.get(column).copied().unwrap_or(1).max(1);
    let next = match event.key() {
        Key::ArrowDown => (column, (row + 1).min(column_len - 1)),
        Key::ArrowUp => (column, row.saturating_sub(1)),
        Key::ArrowRight => {
            let next_column = (column + 1).min(shape.len().saturating_sub(1));
            let next_len = shape.get(next_column).copied().unwrap_or(1).max(1);
            (next_column, row.min(next_len - 1))
        }
        Key::ArrowLeft => {
            let next_column = column.saturating_sub(1);
            let next_len = shape.get(next_column).copied().unwrap_or(1).max(1);
            (next_column, row.min(next_len - 1))
        }
        _ => return,
    };
    if next == (column, row) {
        return;
    }
    event.prevent_default();
    focused.set(next);
    if let Some(node) = nodes
        .peek()
        .get(next.0)
        .and_then(|r| r.get(next.1))
        .and_then(Clone::clone)
    {
        spawn(async move {
            let _ = node.set_focus(true).await;
        });
    }
}
