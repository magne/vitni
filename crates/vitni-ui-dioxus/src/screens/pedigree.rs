//! The Pedigree tool (Phase 5 PR 18; `pedigree.html`): a view switcher over one focus person — an
//! ancestor chart, a descendant chart, and a kinship calculator between two people. Single-view like
//! [`super::DashboardScreen`], not the list/detail pair; "List" instead navigates away to the Person
//! category (matching the mockup's `<a>`-vs-`<button>` distinction between it and the three real
//! in-screen modes).
//!
//! The ancestor/descendant charts are `role="tree"` — [`AncestorTreeView`]/[`DescendantTreeView`] are
//! exported so an SSR test can render them directly over a hand-built [`PedigreeVm`], the same
//! pure-render-and-inspect pattern as [`super::dashboard_view`].

use super::prelude::*;
use crate::components::Tabs;
use crate::i18n::Chrome;
use crate::shell::roving::roving_grid;
use vitni_ui::{PedigreeNodeVm, PedigreeSlotVm, RelationshipVm};

/// How many ancestor/descendant generations the chart shows by default.
const DEFAULT_GENERATIONS: u32 = 4;
/// The generations-input's allowed range (the app layer's own hard cap is higher; this keeps the
/// chart legible).
const GENERATIONS_RANGE: (u32, u32) = (1, 10);

/// The outcome of a resource guarded by a picker input that starts empty: [`Self::Empty`] before the
/// user has supplied a value (focus person, or both relationship ids), [`Self::Data`] once a fetch
/// was attempted (itself possibly an error). Avoids nesting `Option`s on the resource.
#[derive(Debug, Clone, PartialEq)]
enum PickerFetch {
    /// No value has been submitted yet.
    Empty,
    /// A fetch was attempted for a submitted value.
    Data(ScreenData),
}

/// The Pedigree tool's three in-screen modes. "List" is not one of them — see the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PedigreeView {
    Pedigree,
    Descendants,
    Relationships,
}

impl PedigreeView {
    const ALL: [Self; 3] = [Self::Pedigree, Self::Descendants, Self::Relationships];

    const fn id(self) -> &'static str {
        match self {
            Self::Pedigree => "pedigree",
            Self::Descendants => "descendants",
            Self::Relationships => "relationships",
        }
    }
}

/// The Pedigree tool screen.
#[component]
pub fn PedigreeScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loading = state.chrome().loading();
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();

    let default_focus = nav
        .active_record_ref()
        .filter(|record| record.category == Category::People)
        .map(|record| record.human_id)
        .unwrap_or_default();
    let mut view = use_signal(|| PedigreeView::Pedigree);
    let mut focus_input = use_signal(|| default_focus.clone());
    let mut focus = use_signal(move || default_focus);
    let mut depth = use_signal(|| DEFAULT_GENERATIONS);
    let person_a_input = use_signal(String::new);
    let person_b_input = use_signal(String::new);
    let person_a = use_signal(String::new);
    let person_b = use_signal(String::new);

    let pedigree_services = state.services().clone();
    let pedigree_data = use_resource(move || {
        let services = pedigree_services.clone();
        let human_id = focus();
        let depth = depth();
        let _ = nav.data_version.read();
        async move {
            if human_id.trim().is_empty() {
                return PickerFetch::Empty;
            }
            PickerFetch::Data(load_screen(services, Intent::ShowPedigree { human_id, depth }).await)
        }
    });
    let relationship_services = state.services().clone();
    let relationship_data = use_resource(move || {
        let services = relationship_services.clone();
        let human_id_a = person_a();
        let human_id_b = person_b();
        let _ = nav.data_version.read();
        async move {
            if human_id_a.trim().is_empty() || human_id_b.trim().is_empty() {
                return PickerFetch::Empty;
            }
            PickerFetch::Data(load_screen(services, Intent::ComputeRelationship { human_id_a, human_id_b }).await)
        }
    });

    let tabs: Vec<TabItem> = PedigreeView::ALL
        .iter()
        .map(|mode| TabItem {
            id: mode.id().to_owned(),
            label: chrome.0.pedigree_view_label(mode.id()),
            count: None,
        })
        .collect();
    let active = PedigreeView::ALL.iter().position(|mode| *mode == view()).unwrap_or(0);

    rsx! {
        div { style: "display:flex;flex-direction:column;height:100%;min-height:0",
            h1 { class: "sr-only", "{chrome.0.rail_label(\"nav-pedigree\")}" }
            div {
                class: "wrap",
                style: "align-items:center;gap:var(--sp-3);padding:var(--sp-3) var(--sp-5);border-bottom:1px solid var(--line);display:flex",
                Button {
                    label: chrome.0.pedigree_view_label("list"),
                    small: true,
                    onclick: move |_| nav.go_to(Destination::Category(Category::People)),
                }
                Tabs {
                    tabs: tabs.clone(),
                    active,
                    aria_label: Some(chrome.0.pedigree_view_switcher_label()),
                    onselect: move |index: usize| view.set(PedigreeView::ALL[index]),
                    {rsx! {}}
                }
            }
            if matches!(view(), PedigreeView::Pedigree | PedigreeView::Descendants) {
                div {
                    class: "wrap",
                    style: "align-items:center;gap:var(--sp-3);padding:var(--sp-3) var(--sp-5) 0",
                    TextInput {
                        style: "max-width:180px",
                        aria_label: chrome.0.pedigree_focus_label(),
                        placeholder: chrome.0.pedigree_focus_label(),
                        value: "{focus_input}",
                        oninput: move |event: FormEvent| focus_input.set(event.value()),
                    }
                    TextInput {
                        style: "max-width:90px",
                        kind: TextInputKind::Number,
                        min: "{GENERATIONS_RANGE.0}",
                        max: "{GENERATIONS_RANGE.1}",
                        aria_label: chrome.0.pedigree_generations_label(),
                        value: "{depth}",
                        oninput: move |event: FormEvent| {
                            if let Ok(value) = event.value().parse::<u32>() {
                                depth.set(value.clamp(GENERATIONS_RANGE.0, GENERATIONS_RANGE.1));
                            }
                        },
                    }
                    Button {
                        label: chrome.0.pedigree_show(),
                        variant: ButtonVariant::Primary,
                        small: true,
                        onclick: move |_| focus.set(focus_input()),
                    }
                }
            }
            div { style: "flex:1;min-height:0;overflow:auto;padding:var(--sp-4) var(--sp-5)",
                match view() {
                    PedigreeView::Relationships => relationship_body(
                        &chrome.0,
                        &loading,
                        relationship_data.read_unchecked().as_ref(),
                        person_a_input,
                        person_b_input,
                        person_a,
                        person_b,
                    ),
                    other => pedigree_body(
                        &chrome.0,
                        state.data_loc(),
                        &loading,
                        other,
                        depth(),
                        pedigree_data.read_unchecked().as_ref(),
                    ),
                }
            }
        }
    }
}

/// Renders the Pedigree/Descendants views' body: the loading/empty/error states, or the active
/// chart over the loaded [`PedigreeVm`], captioned with the focus person and generation count.
fn pedigree_body(
    chrome: &Chrome,
    loc: &Localizer,
    loading: &str,
    view: PedigreeView,
    depth: u32,
    data: Option<&PickerFetch>,
) -> Element {
    match data {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(PickerFetch::Empty) => rsx! { p { class: "empty", "{chrome.pedigree_empty_focus()}" } },
        Some(PickerFetch::Data(ScreenData::Error(message))) => rsx! { p { class: "empty", "{message}" } },
        Some(PickerFetch::Data(ScreenData::Loaded(IntentOutcome::Pedigree(vm)))) => rsx! {
            p { class: "muted", "{loc.pedigree_focus(&vm.focus.name, usize::try_from(depth).unwrap_or(usize::MAX))}" }
            if view == PedigreeView::Descendants {
                DescendantTreeView { focus: vm.focus.clone(), generations: vm.descendant_generations.clone() }
            } else {
                AncestorTreeView { focus: vm.focus.clone(), generations: vm.ancestor_generations.clone() }
            }
        },
        Some(PickerFetch::Data(ScreenData::Loaded(_))) => rsx! {},
    }
}

/// Renders the Relationships view's body: the two-id form, and (once both are entered) the computed
/// kinship over the loaded [`RelationshipVm`].
fn relationship_body(
    chrome: &Chrome,
    loading: &str,
    data: Option<&PickerFetch>,
    mut person_a_input: Signal<String>,
    mut person_b_input: Signal<String>,
    mut person_a: Signal<String>,
    mut person_b: Signal<String>,
) -> Element {
    rsx! {
        div {
            class: "wrap",
            style: "align-items:center;gap:var(--sp-3);margin-bottom:var(--sp-4)",
            TextInput {
                aria_label: chrome.pedigree_person_a_label(),
                placeholder: chrome.pedigree_person_a_label(),
                value: "{person_a_input}",
                oninput: move |event: FormEvent| person_a_input.set(event.value()),
            }
            TextInput {
                aria_label: chrome.pedigree_person_b_label(),
                placeholder: chrome.pedigree_person_b_label(),
                value: "{person_b_input}",
                oninput: move |event: FormEvent| person_b_input.set(event.value()),
            }
            Button {
                label: chrome.pedigree_compute(),
                variant: ButtonVariant::Primary,
                small: true,
                onclick: move |_| {
                    person_a.set(person_a_input());
                    person_b.set(person_b_input());
                },
            }
        }
        match data {
            None => rsx! { p { class: "loading", "{loading}" } },
            Some(PickerFetch::Empty) => rsx! { p { class: "empty", "{chrome.pedigree_empty_relationship()}" } },
            Some(PickerFetch::Data(ScreenData::Error(message))) => rsx! { p { class: "empty", "{message}" } },
            Some(PickerFetch::Data(ScreenData::Loaded(IntentOutcome::Relationship(vm)))) => rsx! {
                RelationshipView { vm: (**vm).clone() }
            },
            Some(PickerFetch::Data(ScreenData::Loaded(_))) => rsx! {},
        }
    }
}

/// The ancestor chart: `role="tree"`, the focus person as the root `treeitem`, then one `.ped-col`
/// per requested generation. An unresearched slot renders as a dashed placeholder `treeitem` — never
/// a dead end. Arrow keys walk the fan: ↑/↓ within a generation, ←/→ across generations.
#[component]
pub fn AncestorTreeView(focus: PedigreeNodeVm, generations: Vec<Vec<PedigreeSlotVm>>) -> Element {
    let chrome = try_consume_context::<ChromeCtx>();
    let tree_label = chrome.map_or_else(String::new, |chrome| chrome.0.pedigree_ancestor_tree_label());
    let shape: Vec<usize> = std::iter::once(1).chain(generations.iter().map(Vec::len)).collect();
    let last_generation = generations.len();
    let nodes = use_signal(|| {
        shape
            .iter()
            .map(|&len| vec![None::<MountedEvent>; len])
            .collect::<Vec<_>>()
    });
    let focused = use_signal(|| (0_usize, 0_usize));

    rsx! {
        div {
            class: "ped",
            role: "tree",
            aria_label: tree_label,
            onkeydown: move |event| roving_grid(&event, &shape, focused, nodes),
            div { class: "ped-col",
                PedTreeItem {
                    node: focus,
                    column: 0,
                    row: 0,
                    is_root: true,
                    expanded: last_generation > 0,
                    focused,
                    nodes,
                }
            }
            for (index , slots) in generations.into_iter().enumerate() {
                div { class: "ped-col",
                    for (row , slot) in slots.into_iter().enumerate() {
                        {
                            let expanded = index + 1 < last_generation;
                            match slot {
                                PedigreeSlotVm::Known(node) => rsx! {
                                    PedTreeItem { node, column: index + 1, row, is_root: false, expanded, focused, nodes }
                                },
                                PedigreeSlotVm::Unknown { hint } => rsx! {
                                    UnknownTreeItem { hint, column: index + 1, row, expanded, focused, nodes }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The descendant chart: `role="tree"`, the focus person as the root `treeitem`, then one `.ped-col`
/// per generation actually recorded — a childless branch simply ends (not a research gap, so no
/// placeholder). Arrow keys walk it the same way as [`AncestorTreeView`].
#[component]
pub fn DescendantTreeView(focus: PedigreeNodeVm, generations: Vec<Vec<PedigreeNodeVm>>) -> Element {
    let chrome = try_consume_context::<ChromeCtx>();
    let tree_label = chrome.map_or_else(String::new, |chrome| chrome.0.pedigree_descendant_tree_label());
    let shape: Vec<usize> = std::iter::once(1).chain(generations.iter().map(Vec::len)).collect();
    let has_children = !generations.is_empty();
    let nodes = use_signal(|| {
        shape
            .iter()
            .map(|&len| vec![None::<MountedEvent>; len])
            .collect::<Vec<_>>()
    });
    let focused = use_signal(|| (0_usize, 0_usize));

    rsx! {
        div {
            class: "ped",
            role: "tree",
            aria_label: tree_label,
            onkeydown: move |event| roving_grid(&event, &shape, focused, nodes),
            div { class: "ped-col",
                PedTreeItem { node: focus, column: 0, row: 0, is_root: true, expanded: has_children, focused, nodes }
            }
            for (index , row) in generations.into_iter().enumerate() {
                div { class: "ped-col",
                    for (row_index , node) in row.into_iter().enumerate() {
                        {
                            let expanded = node.has_more;
                            rsx! {
                                PedTreeItem { node, column: index + 1, row: row_index, is_root: false, expanded, focused, nodes }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One known chart node: name, lifespan, and (except for the root) the confidence badge on the
/// parent-child assertion linking it to the adjacent generation. Clicking it opens the person's
/// record — the chart's `RecordLink`-equivalent, inlined because a `treeitem`'s roving `tabindex`
/// and `aria-expanded` do not fit `RecordLink`'s fixed props.
#[component]
fn PedTreeItem(
    node: PedigreeNodeVm,
    column: usize,
    row: usize,
    is_root: bool,
    expanded: bool,
    focused: Signal<(usize, usize)>,
    mut nodes: Signal<Vec<Vec<Option<MountedEvent>>>>,
) -> Element {
    let mut nav = use_context::<NavState>();
    let is_focused = *focused.read() == (column, row);
    let class = if is_root { "ped-node focus" } else { "ped-node" };
    let human_id = node.human_id.clone();
    let label = node.name.clone();
    rsx! {
        button {
            class,
            r#type: "button",
            role: "treeitem",
            aria_expanded: if expanded { "true" } else { "false" },
            aria_selected: if is_focused { "true" } else { "false" },
            tabindex: if is_focused { "0" } else { "-1" },
            onmounted: move |event| {
                if let Some(column_nodes) = nodes.write().get_mut(column)
                    && let Some(slot) = column_nodes.get_mut(row)
                {
                    *slot = Some(event);
                }
            },
            onclick: move |_| {
                nav.go_to(Destination::Category(Category::People));
                nav.open_record(RecordRef {
                    category: Category::People,
                    human_id: human_id.clone(),
                    label: label.clone(),
                });
            },
            div { class: "pn-name", "{node.name}" }
            if let Some(vitals) = &node.vitals {
                div { class: "pn-dates", "{vitals}" }
            }
            if let (Some(level), Some(confidence_label)) = (node.confidence, node.confidence_label.clone()) {
                ConfidenceBadge { level, label: confidence_label }
            }
        }
    }
}

/// An unresearched ancestor slot: a dashed, non-navigating `treeitem` naming which parent (of whom)
/// is still unknown — a research to-do, not a dead end.
#[component]
fn UnknownTreeItem(
    hint: String,
    column: usize,
    row: usize,
    expanded: bool,
    focused: Signal<(usize, usize)>,
    mut nodes: Signal<Vec<Vec<Option<MountedEvent>>>>,
) -> Element {
    let chrome = try_consume_context::<ChromeCtx>();
    let unknown_label = chrome.map_or_else(String::new, |chrome| chrome.0.pedigree_unknown_label());
    let is_focused = *focused.read() == (column, row);
    rsx! {
        button {
            class: "ped-node",
            style: "opacity:0.6;border-style:dashed",
            r#type: "button",
            role: "treeitem",
            aria_expanded: if expanded { "true" } else { "false" },
            aria_selected: if is_focused { "true" } else { "false" },
            tabindex: if is_focused { "0" } else { "-1" },
            onmounted: move |event| {
                if let Some(column_nodes) = nodes.write().get_mut(column)
                    && let Some(slot) = column_nodes.get_mut(row)
                {
                    *slot = Some(event);
                }
            },
            div { class: "pn-name faint", "{unknown_label}" }
            div { class: "pn-dates", "{hint}" }
        }
    }
}

/// The kinship calculator's result: the two people (as `RecordLink`s) and the localized summary.
#[component]
pub fn RelationshipView(vm: RelationshipVm) -> Element {
    rsx! {
        div { class: "grid-2", style: "gap:var(--sp-4)",
            div { class: "ped-node",
                RecordLink {
                    category: Category::People,
                    human_id: vm.person_a.human_id.clone(),
                    label: vm.person_a.name.clone(),
                }
                if let Some(vitals) = &vm.person_a.vitals {
                    div { class: "pn-dates", "{vitals}" }
                }
            }
            div { class: "ped-node",
                RecordLink {
                    category: Category::People,
                    human_id: vm.person_b.human_id.clone(),
                    label: vm.person_b.name.clone(),
                }
                if let Some(vitals) = &vm.person_b.vitals {
                    div { class: "pn-dates", "{vitals}" }
                }
            }
        }
        p { style: "margin-top:var(--sp-4);font-weight:600", "{vm.summary}" }
    }
}
