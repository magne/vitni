//! The record picker and nested draft card (`record-editing.html` §6b, `design-system.html`).
//!
//! A record link is a find-or-create control: [`record_picker`] draws a search input over the
//! already-loaded options ([`PickerOptions`]), a floating result list of reused [`ListRow`]s (a
//! `position:fixed` overlay measured from the input's on-screen box, so it escapes the `.detail`
//! scroll pane's `overflow:hidden` clip rather than pushing siblings down), a trailing "+ New …" row,
//! and a collapsed [`.picker-value`](picker_value) chip once a record is picked. Picking "+ New" flips
//! the call site's [`RecordLink`](vitni_ui::RecordLink) to `New(..)`, which renders a [`draft_card`]
//! whose body may hold another picker (a citation → source cascade); discarding the card resets the
//! link to `Empty`.
//!
//! Every fn here is pure over signals (no `use_*` hooks, no `AppCtx`) *except* [`PickerSearch`], which
//! owns the hooks needed to measure the input's on-screen position and close the list on pane scroll —
//! isolating that state in a real component scope keeps it safe under the conditional
//! search-view/collapsed-view branching above it (a plain fn's hooks would drift out of step with
//! that branching), and gives the scroll/resize listener a `use_drop` to tear itself down against: the
//! branching above unmounts [`PickerSearch`] on every pick and remounts it on every clear, so without
//! that teardown each cycle would leave one more inert JS listener on `window` (#204).
//! [`PickerSearch`]'s props are therefore fully owned, already-localized data (never
//! `&Localizer`/`&RecordPicker`, which a `#[component]`'s `Clone + PartialEq` props can't carry) —
//! [`picker_search`] resolves everything it needs while it still holds those by reference. A call site
//! can still render a picker conditionally and the SSR tests can exercise the markup without an app;
//! the measured pixel position is runtime/WebKitGTK-only and not SSR-testable (like the provenance
//! popover). The filtering lives in [`vitni_ui::picker_rows`]; this module is only the framework
//! binding.

use dioxus::prelude::*;
use vitni_ui::ActionLabel;
use vitni_ui::{ActiveMove, Localizer, PickerSelection, PickerState, RowVm, next_active, picker_rows};

use crate::components::{IconButton, ListRow, TextInput};

/// The already-localized configuration of one picker: its field label, the element-id base, the entity
/// noun (`person`/`place`/…) used in the placeholder and "+ New" row, and whether "+ New" is offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerConfig {
    /// The field's already-localized label.
    pub label: String,
    /// The field's machine name / element id.
    pub name: String,
    /// The already-localized entity noun the picker searches for (from `Localizer::picker_entity`).
    pub entity_label: String,
    /// Whether the picker offers a "+ New …" create row (a find-or-create picker vs existing-only).
    pub allow_new: bool,
}

/// The load state of a picker's options, loaded via `load_picker_rows` when the form opens and again
/// after every create/edit/undo (the call site's resource subscribes to
/// [`data_version_ticket`](crate::shell::nav_state::data_version_ticket) — #266). A refetch keeps the
/// last `Ready` rows on screen, so a reload is never a "Loading" flash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerOptions {
    /// The options are still loading.
    Loading,
    /// Loading failed with an already-localized message.
    Failed(String),
    /// The loaded, client-side-filterable rows.
    Ready(Vec<RowVm>),
}

/// The three callbacks a picker fires: a row picked, the selection cleared, and "+ New" chosen (with
/// the live query). Bundled so the picker fns stay within the argument budget.
#[derive(Clone, Copy)]
pub struct PickerCallbacks {
    /// Fired when a result row is picked, with its selection.
    pub onpick: Callback<PickerSelection>,
    /// Fired when the collapsed selection is cleared.
    pub onclear: Callback<()>,
    /// Fired when the "+ New …" row is chosen, with the live query text.
    pub onnew: Callback<String>,
}

/// Everything one picker needs to render: its config, the live picker state, the loaded options, the
/// ids to exclude from results (e.g. already-picked partners), and the callbacks it fires.
#[derive(Clone)]
pub struct RecordPicker {
    /// The picker's labels + behaviour flags.
    pub config: PickerConfig,
    /// The live query / open / selection state (owned by the call site so it reseeds cleanly).
    pub state: Signal<PickerState>,
    /// The options to search, refetched whenever the workspace data changes.
    pub options: PickerOptions,
    /// Record ids to hide from the results (e.g. already-picked ids).
    pub exclude: Vec<String>,
    /// The callbacks the picker fires on pick / clear / "+ New".
    pub callbacks: PickerCallbacks,
}

/// The read-first view state of a [`draft_picker_field`]: whether the record is being edited, the
/// current selection derived from the draft's `RecordLink` (so it reseeds without stale picker state),
/// and whether that link differs from the committed one (drives the modified tint + reset).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DraftPickerView {
    /// Whether the record is in edit mode (the picker) rather than view mode (a read box).
    pub editing: bool,
    /// The linked record, derived from the draft's `RecordLink::Existing` (or `None` when unset).
    pub selection: Option<PickerSelection>,
    /// Whether the link differs from its committed value (shows the reset control + tint).
    pub modified: bool,
}

/// Maps a picker resource's read (`use_resource(..).read_unchecked().as_ref()`) to [`PickerOptions`].
#[must_use]
pub fn picker_options(loaded: Option<&Result<Vec<RowVm>, String>>) -> PickerOptions {
    match loaded {
        None => PickerOptions::Loading,
        Some(Err(message)) => PickerOptions::Failed(message.clone()),
        Some(Ok(rows)) => PickerOptions::Ready(rows.clone()),
    }
}

/// A find-or-create record picker (`record-editing.html` §6b): a labelled field showing either a
/// collapsed [`.picker-value`](picker_value) chip once a record is picked, or a search input with a
/// floating result list. The list closes on pick / clear / Esc, on an outside click (a click-away
/// scrim, mirroring the provenance popover's), or on focus leaving the control.
pub fn record_picker(loc: &Localizer, picker: &RecordPicker) -> Element {
    let selection = picker.state.read().selection.clone();
    rsx! {
        div { class: "field",
            label { r#for: "{picker.config.name}", "{picker.config.label}" }
            if let Some(selection) = selection {
                {picker_value(loc, &selection, false, picker.state, picker.callbacks.onclear, None)}
            } else {
                {picker_search(loc, picker)}
            }
        }
    }
}

/// A read-first record link for the whole-record editor (`event.html` place): in view mode a `.field`
/// read box showing `Title (P0021)`; in edit mode the picker, with the collapsed selection derived
/// from the draft's `RecordLink` (not the picker state) so `use_record_edit` reseeds cleanly, plus a
/// modified tint + reset when the link differs from the committed one.
pub fn draft_picker_field(
    loc: &Localizer,
    picker: &RecordPicker,
    view: &DraftPickerView,
    onreset: Callback<()>,
) -> Element {
    if !view.editing {
        let display = view
            .selection
            .as_ref()
            .map_or_else(|| "—".to_owned(), PickerSelection::display);
        return rsx! {
            div { class: "field",
                label { r#for: "{picker.config.name}", "{picker.config.label}" }
                span { class: "val", "{display}" }
            }
        };
    }
    let reset = Some((onreset, loc.action_reset_field(&picker.config.label)));
    rsx! {
        div { class: "field",
            label { r#for: "{picker.config.name}", "{picker.config.label}" }
            if let Some(selection) = view.selection.clone() {
                {picker_value(loc, &selection, view.modified, picker.state, picker.callbacks.onclear, reset)}
            } else {
                {picker_search(loc, picker)}
            }
        }
    }
}

/// A nested draft card (`design-system.html`): a raised, bordered, offset sub-form for a record being
/// created inline. Its head carries the uppercase title, a "draft" badge, and a discard control (✕);
/// nesting alternates the surface via CSS. The `body` is the record's own fields (which may hold
/// another picker → another card).
pub fn draft_card(title: &str, badge: &str, discard_label: String, ondiscard: Callback<()>, body: Element) -> Element {
    rsx! {
        div { class: "draft-card",
            div { class: "draft-card-head",
                h4 { class: "draft-card-title", "{title}" }
                span { class: "badge draft", "{badge}" }
                IconButton {
                    icon: "✕".to_owned(),
                    label: discard_label.clone(),
                    title: discard_label,
                    onclick: move |_| ondiscard.call(()),
                }
            }
            {body}
        }
    }
}

/// The collapsed selection chip: the title, the mono id, an optional reset control, and the clear (✕)
/// control (labelled `Clear selection`). Clearing empties the picker state and fires `onclear`.
fn picker_value(
    loc: &Localizer,
    selection: &PickerSelection,
    modified: bool,
    mut state: Signal<PickerState>,
    onclear: Callback<()>,
    reset: Option<(Callback<()>, String)>,
) -> Element {
    let class = if modified {
        "picker-value modified"
    } else {
        "picker-value"
    };
    let clear_label = loc.picker_clear();
    rsx! {
        div { class,
            span { class: "val", "{selection.title}" }
            span { class: "row-id", "{selection.human_id}" }
            if let Some((onreset, reset_label)) = reset {
                IconButton {
                    icon: "↺".to_owned(),
                    label: reset_label.clone(),
                    title: reset_label,
                    onclick: move |_| onreset.call(()),
                }
            }
            IconButton {
                icon: "✕".to_owned(),
                label: clear_label.clone(),
                title: clear_label,
                onclick: move |_| {
                    state.write().clear();
                    onclear.call(());
                },
            }
        }
    }
}

/// The already-localized, already-filtered content [`picker_results`] renders — resolved by
/// [`picker_search`] (still holding `loc`/`picker` by reference) so [`PickerSearch`] never needs
/// either as a prop.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PickerResultsView {
    /// Options are still loading; the list renders nothing at all (not even the floating shell).
    Hidden,
    /// Loading failed with an already-localized message.
    Failed(String),
    /// The matched rows (already capped, already excluded) plus the empty/"+ New" labels — each
    /// `None` when that line does not show (rows found; creation disallowed).
    Ready {
        matched: Vec<RowVm>,
        empty_label: Option<String>,
        new_label: Option<String>,
    },
}

/// Resolves [`PickerOptions`] + the live query into the [`PickerResultsView`] [`PickerSearch`] and
/// [`picker_results`] render — the only place this module still calls [`picker_rows`] or `loc`.
fn picker_results_view(loc: &Localizer, picker: &RecordPicker, query: &str) -> PickerResultsView {
    let rows = match &picker.options {
        PickerOptions::Loading => return PickerResultsView::Hidden,
        PickerOptions::Failed(message) => return PickerResultsView::Failed(message.clone()),
        PickerOptions::Ready(rows) => rows,
    };
    let matched = picker_rows(rows, query, &picker.exclude);
    let empty_label = matched.is_empty().then(|| loc.picker_empty());
    let new_label = picker.config.allow_new.then(|| {
        if query.is_empty() {
            loc.picker_new(&picker.config.entity_label)
        } else {
            loc.picker_new_query(&picker.config.entity_label, query)
        }
    });
    PickerResultsView::Ready {
        matched,
        empty_label,
        new_label,
    }
}

/// The search field: resolves the already-localized labels and the [`PickerResultsView`] here (while
/// `loc`/`picker` are still plain references) and hands them to [`PickerSearch`], the hook-owning
/// component that renders the input, floats the result list, and wires the close behaviors.
fn picker_search(loc: &Localizer, picker: &RecordPicker) -> Element {
    let state = picker.state;
    let query = state.read().query.clone();
    let placeholder = loc.picker_placeholder(&picker.config.entity_label);
    let scrim_label = loc.action_label(ActionLabel::Dismiss);
    let results = picker_results_view(loc, picker, &query);
    rsx! {
        PickerSearch {
            name: picker.config.name.clone(),
            value: query,
            placeholder,
            scrim_label,
            state,
            results,
            onpick: picker.callbacks.onpick,
            onnew: picker.callbacks.onnew,
        }
    }
}

/// The search input plus its floating result-list overlay. The one component in this module that
/// owns hooks (see the module doc): it measures the anchor's on-screen box via `onmounted` +
/// `MountedData::get_client_rect`, and republishes the result as CSS custom properties
/// (`--pk-top`/`--pk-left`/`--pk-min-width`, `components.css`) on the `.picker-anchor` wrapper, which
/// the still-pure [`picker_results`] reads via `var()` for its `position:fixed` placement — so a
/// pure fn, not this component, still owns the `.picker-results` markup. Re-measures whenever `state`
/// changes while open (typing, focus); a capture-phase `scroll`/`resize` listener armed once at mount
/// (`document::eval` — window-level, since the scrolling `.tab-body`/`.detail` ancestor isn't reachable
/// from here) closes the list on pane scroll rather than tracking a live reposition. Closes on scrim
/// click, on `Esc` (via `TextInput`'s `onkeydown_extra`), and on focus leaving the anchor
/// (`onfocusout`, which bubbles). `WebKitGTK` focuses a `<button>` on pointer-down, which would blur the
/// input and close the list *before* a row's own click lands — every row, the "+ New" row, and the
/// scrim call `event.prevent_default()` on `onmousedown` to suppress that focus shift. Under SSR
/// `onmounted` never fires, so the measured style stays unset and `.picker-results` still renders
/// (unpositioned, like the provenance popover's SSR fallback).
///
/// `active` is the highlighted-option index — ephemeral, Dioxus-local `use_signal` state (not
/// `PickerState`, which stays a plain data struct with no rendering concerns): reset to `0` whenever
/// the list opens or the query changes, and read clamped into `[0, nav_len - 1]` (the matched rows
/// plus the trailing "+ New …" row, when shown) via [`next_active`]. ↑/↓/Home/End move it; `Enter`
/// commits the highlighted row (or, past the last matched row, fires "+ New …") — mirroring the
/// existing row/`+ New` click handlers exactly. Only `Enter` commits; `Space` is left untouched so it
/// keeps typing into the search box. Handled keys call both `prevent_default` and `stop_propagation`
/// so the global shortcut layer never sees them (`TextInput`'s typing guard already covers plain
/// characters).
#[component]
fn PickerSearch(
    /// The field's element id / name.
    name: String,
    /// The live query text.
    value: String,
    /// The already-localized search placeholder.
    placeholder: String,
    /// The already-localized accessible name for the click-away scrim.
    scrim_label: String,
    /// The live query / open / selection state (owned by the call site).
    mut state: Signal<PickerState>,
    /// The already-resolved result-list content.
    results: PickerResultsView,
    /// Fired when a result row is picked.
    onpick: Callback<PickerSelection>,
    /// Fired when "+ New …" is chosen, with the live query.
    onnew: Callback<String>,
) -> Element {
    let anchor_style = use_signal(String::new);
    let mut anchor = use_signal(|| None::<MountedEvent>);
    let mut active = use_signal(|| 0usize);
    // `name` is already this field's page-unique element-id base (it backs `id="{name}"` and
    // `"{name}-listbox"` below), so it doubles as the `window`-scoped key `watch_scroll_close`/
    // `unwatch_scroll_close` use to arm and later remove the exact same JS listener — see #204.
    let scroll_close_key = format!("__pickerScrollClose_{name}");
    use_drop({
        let key = scroll_close_key.clone();
        move || unwatch_scroll_close(&key)
    });
    let open = state.read().open;
    use_effect(move || {
        if state.read().open
            && let Some(node) = anchor.peek().clone()
        {
            measure_anchor(node, anchor_style);
        }
    });
    let query = value.clone();
    let (nav_matched, nav_len) = match &results {
        PickerResultsView::Ready { matched, new_label, .. } => {
            let len = matched.len() + usize::from(new_label.is_some());
            (matched.clone(), len)
        }
        PickerResultsView::Hidden | PickerResultsView::Failed(_) => (Vec::new(), 0),
    };
    let active_index = active().min(nav_len.saturating_sub(1));
    let listbox_id = format!("{name}-listbox");
    let active_id = (open && nav_len > 0).then(|| format!("{name}-opt-{active_index}"));
    let nav = PickerNav {
        name: &name,
        query: &query,
        active: active_index,
    };
    let new_query = query.clone();
    rsx! {
        div {
            class: "picker-anchor",
            style: if anchor_style.read().is_empty() { None } else { Some(anchor_style()) },
            onmounted: move |event| {
                anchor.set(Some(event.clone()));
                measure_anchor(event, anchor_style);
                watch_scroll_close(state, &scroll_close_key);
            },
            onfocusout: move |_| state.write().open = false,
            TextInput {
                id: "{name}",
                name: "{name}",
                value: "{value}",
                placeholder,
                role: "combobox",
                aria_expanded: if open { "true" } else { "false" },
                aria_controls: "{listbox_id}",
                aria_activedescendant: active_id,
                onfocus: move |_| {
                    state.write().open = true;
                    active.set(0);
                },
                oninput: move |event: FormEvent| {
                    let mut state = state.write();
                    state.query = event.value();
                    state.open = true;
                    active.set(0);
                },
                onkeydown_extra: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        event.stop_propagation();
                        state.write().open = false;
                        return;
                    }
                    if !state.read().open || nav_len == 0 {
                        return;
                    }
                    let mv = match event.key() {
                        Key::ArrowDown => ActiveMove::Down,
                        Key::ArrowUp => ActiveMove::Up,
                        Key::Home => ActiveMove::First,
                        Key::End => ActiveMove::Last,
                        Key::Enter => {
                            event.prevent_default();
                            event.stop_propagation();
                            let index = active().min(nav_len.saturating_sub(1));
                            if let Some(row) = nav_matched.get(index) {
                                state.write().pick(row);
                                onpick.call(PickerSelection::from_row(row));
                            } else {
                                onnew.call(new_query.clone());
                                state.write().open = false;
                            }
                            return;
                        }
                        _ => return,
                    };
                    event.prevent_default();
                    event.stop_propagation();
                    active.set(next_active(active(), mv, nav_len));
                },
            }
            if open {
                {picker_results(&results, &nav, onpick, onnew, state)}
                button {
                    class: "picker-scrim",
                    r#type: "button",
                    aria_label: scrim_label,
                    onmousedown: move |event: MouseEvent| event.prevent_default(),
                    onclick: move |_| state.write().open = false,
                }
            }
        }
    }
}

/// The already-derived id base + live query + clamped highlight index [`picker_results`] renders
/// against — bundled so the fn stays within the module's positional-parameter budget while still
/// taking both the matched-row/"+ New" ARIA ids (`"{name}-opt-{index}"`, the listbox `"{name}-listbox"`)
/// and the highlight [`PickerSearch`] computed from its local `active` signal.
struct PickerNav<'a> {
    /// The field's element id / name (the id base: `"{name}-listbox"`, `"{name}-opt-{index}"`).
    name: &'a str,
    /// The live query text (for the "+ New …" `onnew` callback).
    query: &'a str,
    /// The highlighted index, already clamped into `[0, nav_len - 1]`.
    active: usize,
}

/// Reads `node`'s on-screen box and republishes it as the anchor's `style` (a no-op under SSR, where
/// `get_client_rect` returns `MountedError::NotSupported`).
fn measure_anchor(node: MountedEvent, mut style: Signal<String>) {
    spawn(async move {
        if let Ok(rect) = node.get_client_rect().await {
            style.set(format!(
                "--pk-top:{}px;--pk-left:{}px;--pk-min-width:{}px;",
                rect.origin.y + rect.size.height,
                rect.origin.x,
                rect.size.width,
            ));
        }
    });
}

/// Arms a capture-phase `window` `scroll`/`resize` listener that closes the picker — the scrolling
/// ancestor (`.tab-body`/`.detail`) isn't reachable from this component, but capture-phase window
/// listeners still observe scroll events on any descendant scroller. Armed once per mount (not
/// re-armed per open/close); `key` stashes the listener closure on `window` under a page-unique name so
/// [`unwatch_scroll_close`] can remove this exact listener — via a separate `document::eval` call,
/// which shares no JS-side scope with this one — when [`PickerSearch`]'s `use_drop` fires on unmount
/// (#204: before this, the JS-side listener outlived the mount and leaked one per pick/clear cycle).
fn watch_scroll_close(mut state: Signal<PickerState>, key: &str) {
    let script = format!(
        r"
        const closePicker = () => dioxus.send(true);
        window['{key}'] = closePicker;
        window.addEventListener('scroll', closePicker, true);
        window.addEventListener('resize', closePicker, true);
        "
    );
    let mut listener = document::eval(&script);
    spawn(async move {
        while listener.recv::<bool>().await.is_ok() {
            state.write().open = false;
        }
    });
}

/// Removes the `scroll`/`resize` listener [`watch_scroll_close`] stashed on `window` under `key`, run
/// from [`PickerSearch`]'s `use_drop` on unmount. A no-op if the listener was never armed (`onmounted`
/// never fired, e.g. under SSR).
fn unwatch_scroll_close(key: &str) {
    let script = format!(
        r"
        const closePicker = window['{key}'];
        if (closePicker) {{
            window.removeEventListener('scroll', closePicker, true);
            window.removeEventListener('resize', closePicker, true);
            delete window['{key}'];
        }}
        "
    );
    document::eval(&script);
}

/// The floating result list: a reused [`ListRow`] per matched row (already capped upstream, the one
/// at `nav.active` highlighted), a `picker-empty` line when nothing matches, and a trailing "+ New …"
/// row (highlighted instead once `nav.active` reaches it) when the picker allows it. The listbox and
/// each option carry the ids [`PickerSearch`]'s `aria-activedescendant` wiring points at
/// (`"{nav.name}-listbox"`, `"{nav.name}-opt-{index}"`, the "+ New" row using `index == matched.len()`).
/// Positioned by `.picker-results`' own CSS (`components.css`), reading the `--pk-*` custom
/// properties [`PickerSearch`] sets on the ancestor `.picker-anchor` — this fn stays pure.
fn picker_results(
    view: &PickerResultsView,
    nav: &PickerNav<'_>,
    onpick: Callback<PickerSelection>,
    onnew: Callback<String>,
    mut state: Signal<PickerState>,
) -> Element {
    let (matched, empty_label, new_label) = match view {
        PickerResultsView::Hidden => return rsx! {},
        PickerResultsView::Failed(message) => {
            return rsx! {
                div { class: "picker-results", id: "{nav.name}-listbox", role: "listbox",
                    p { class: "picker-empty", "{message}" }
                }
            };
        }
        PickerResultsView::Ready {
            matched,
            empty_label,
            new_label,
        } => (matched, empty_label, new_label),
    };
    let query = nav.query.to_owned();
    let active = nav.active;
    rsx! {
        div { class: "picker-results", id: "{nav.name}-listbox", role: "listbox",
            for (index , row) in matched.iter().cloned().enumerate() {
                ListRow {
                    key: "{row.id}",
                    id: Some(format!("{}-opt-{index}", nav.name)),
                    title: row.title.clone(),
                    subtitle: row.subtitle.clone(),
                    id_label: Some(row.id.clone()),
                    avatar: row.avatar.clone(),
                    selected: index == active,
                    onmousedown: move |event: MouseEvent| event.prevent_default(),
                    onclick: {
                        let selection = PickerSelection::from_row(&row);
                        move |_| {
                            state.write().pick(&row);
                            onpick.call(selection.clone());
                        }
                    },
                }
            }
            if let Some(empty_label) = empty_label {
                p { class: "picker-empty", "{empty_label}" }
            }
            if let Some(new_label) = new_label {
                button {
                    class: if active == matched.len() { "picker-new sel" } else { "picker-new" },
                    id: "{nav.name}-opt-{matched.len()}",
                    r#type: "button",
                    onmousedown: move |event: MouseEvent| event.prevent_default(),
                    onclick: move |_| {
                        onnew.call(query.clone());
                        state.write().open = false;
                    },
                    "{new_label}"
                }
            }
        }
    }
}
