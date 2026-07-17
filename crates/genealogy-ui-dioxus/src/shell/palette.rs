//! The command palette (`⌘K`/`⌘F`, and the top-bar search's Enter).
//!
//! Blends records (the loaded entity lists, filtered by the query), commands (create a record, find
//! duplicates, open a tool/help), and the recently-opened records into one keyboard-navigable
//! listbox. The decision logic is the framework-free [`genealogy_ui::palette`] module; this component
//! is the thin Dioxus binding: it loads the rows once per open, filters them per keystroke with
//! [`palette_groups`], and renders the ARIA combobox/listbox from `search-palette.html`.

use dioxus::prelude::*;
use genealogy_app::RecentItem;
use genealogy_ui::{
    Category, Destination, PaletteAction, PaletteCommand, PaletteCommandVm, PaletteEntry, PaletteGroupKind, RecordRef,
    Tool, activate, move_active, palette_commands, palette_groups,
};

use crate::app::AppCtx;
use crate::components::TextInput;
use crate::services::load_palette_rows;
use crate::shell::ChromeCtx;
use crate::shell::focus_trap::trap_tab;
use crate::shell::nav_state::{NavState, Overlay};

/// One rendered palette option: its flat index (for `aria-activedescendant`), decorative icon,
/// already-localized label + kind badge, and the entry to activate.
struct OptionView {
    index: usize,
    icon: String,
    label: String,
    kind: String,
    entry: PaletteEntry,
}

/// One rendered palette group: its already-localized heading and its options.
struct GroupView {
    heading: String,
    options: Vec<OptionView>,
}

/// The command palette overlay, rendered only while [`Overlay::Palette`] is open.
#[component]
pub fn CommandPalette() -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    let services = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        _ => None,
    };
    let mut query = use_signal(String::new);
    let mut active = use_signal(|| 0_usize);
    // Seed the query from the top-bar search (or clear it for a bare ⌘K) each time the palette opens.
    use_effect(move || {
        if *nav.overlay.read() == Overlay::Palette {
            let seed = nav.palette_seed.peek().clone();
            query.set(seed);
            nav.palette_seed.set(String::new());
            active.set(0);
        }
    });
    // Load every pickable category's rows once per open (empty while closed / host-free).
    let rows = use_resource(move || {
        let services = services.clone();
        let open = *nav.overlay.read() == Overlay::Palette;
        async move {
            match (open, services) {
                (true, Some(services)) => load_palette_rows(services).await,
                _ => Vec::new(),
            }
        }
    });

    if *nav.overlay.read() != Overlay::Palette {
        return rsx! {};
    }

    let records = rows.read_unchecked().clone().unwrap_or_default();
    let commands = command_vms(&chrome);
    let recent = recent_entries(&nav);
    let query_text = query();
    let groups = palette_groups(&records, &commands, &recent, &query_text);
    let views = group_views(&chrome, &groups);
    let total: usize = views.iter().map(|group| group.options.len()).sum();
    let active_index = if total == 0 { 0 } else { active().min(total - 1) };
    let flat: Vec<PaletteEntry> = views
        .iter()
        .flat_map(|group| group.options.iter().map(|option| option.entry.clone()))
        .collect();

    let combobox_label = chrome.0.palette_combobox_label();
    let results_label = chrome.0.palette_results_label();
    let placeholder = chrome.0.palette_placeholder();
    let title = chrome.0.palette_title();
    let hint_navigate = chrome.0.palette_hint_navigate();
    let hint_open = chrome.0.palette_hint_open();
    let hint_anywhere = chrome.0.palette_hint_anywhere();

    rsx! {
        div { class: "overlay", onclick: move |_| nav.close_overlay(),
            div {
                class: "palette",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{title}",
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event| trap_tab(&event),
                h1 { class: "sr-only", "{title}" }
                div { class: "p-input",
                    TextInput {
                        class: "",
                        autofocus: true,
                        role: "combobox",
                        aria_label: "{combobox_label}",
                        aria_autocomplete: "list",
                        aria_expanded: "true",
                        aria_controls: "palette-listbox",
                        aria_activedescendant: "palette-opt-{active_index}",
                        placeholder: "{placeholder}",
                        value: "{query_text}",
                        oninput: move |event: FormEvent| query.set(event.value()),
                        onkeydown_extra: move |event: KeyboardEvent| {
                            match event.key() {
                                Key::ArrowDown => {
                                    event.prevent_default();
                                    active.set(move_active(active_index, total, 1));
                                }
                                Key::ArrowUp => {
                                    event.prevent_default();
                                    active.set(move_active(active_index, total, -1));
                                }
                                Key::Enter => {
                                    event.prevent_default();
                                    if let Some(entry) = flat.get(active_index) {
                                        run_action(&mut nav, activate(entry));
                                    }
                                }
                                _ => {}
                            }
                        },
                    }
                }
                div { id: "palette-listbox", role: "listbox", aria_label: "{results_label}",
                    for group in views.iter() {
                        div { class: "nav-group-label", role: "presentation", style: "padding:var(--sp-2) var(--sp-4) 2px", "{group.heading}" }
                        for option in group.options.iter() {
                            {
                                let selected = option.index == active_index;
                                let entry = option.entry.clone();
                                rsx! {
                                    div {
                                        class: if selected { "p-row sel" } else { "p-row" },
                                        role: "option",
                                        id: "palette-opt-{option.index}",
                                        aria_selected: if selected { "true" } else { "false" },
                                        onclick: move |_| run_action(&mut nav, activate(&entry)),
                                        span { style: "width:24px;text-align:center", aria_hidden: "true", "{option.icon}" }
                                        span { class: "grow", "{option.label}" }
                                        span { class: "p-kind", "{option.kind}" }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "p-input p-foot", style: "border-top:1px solid var(--line);border-bottom:none;padding:var(--sp-2) var(--sp-4);font-size:var(--fs-xs);color:var(--faint);display:flex;gap:var(--sp-3);align-items:center",
                    span { kbd { "↑" } kbd { "↓" } " {hint_navigate}" }
                    span { kbd { "↵" } " {hint_open}" }
                    span { style: "margin-left:auto", "{hint_anywhere}" }
                }
            }
        }
    }
}

/// Builds the localized command list (a `PaletteCommandVm` per default [`palette_commands`] entry).
fn command_vms(chrome: &ChromeCtx) -> Vec<PaletteCommandVm> {
    palette_commands()
        .into_iter()
        .map(|command| {
            let label = match &command {
                PaletteCommand::Create(category) => {
                    chrome.0.palette_cmd_create(&chrome.0.rail_label(category.label_id()))
                }
                PaletteCommand::FindDuplicates => chrome.0.palette_cmd_find_duplicates(),
                PaletteCommand::OpenTool(tool) => chrome.0.palette_cmd_open(&chrome.0.rail_label(tool.label_id())),
                PaletteCommand::OpenHelp => chrome.0.palette_cmd_open(&chrome.0.rail_label("nav-help")),
            };
            PaletteCommandVm { command, label }
        })
        .collect()
}

/// The recently-opened records as palette entries (newest first, top five). Tools are reached
/// through the Commands group.
fn recent_entries(nav: &NavState) -> Vec<PaletteEntry> {
    nav.recent
        .read()
        .iter()
        .filter_map(|item| {
            let RecentItem::Record { kind, human_id, label } = item;
            Category::from_aggregate_kind(kind).map(|category| PaletteEntry::Recent {
                category,
                human_id: human_id.clone(),
                label: label.clone(),
            })
        })
        .take(5)
        .collect()
}

/// Turns the pure [`genealogy_ui::PaletteGroup`]s into rendered views with localized headings, flat
/// option indices (for `aria-activedescendant`), icons, and kind badges.
fn group_views(chrome: &ChromeCtx, groups: &[genealogy_ui::PaletteGroup]) -> Vec<GroupView> {
    let mut views = Vec::new();
    let mut index = 0;
    for group in groups {
        let heading = match group.kind {
            PaletteGroupKind::Category(category) => chrome.0.rail_label(category.label_id()),
            PaletteGroupKind::Commands => chrome.0.palette_group_commands(),
            PaletteGroupKind::Recent => chrome.0.palette_group_recent(),
        };
        let mut options = Vec::new();
        for entry in &group.entries {
            let (icon, label, kind) = option_parts(chrome, entry);
            options.push(OptionView {
                index,
                icon,
                label,
                kind,
                entry: entry.clone(),
            });
            index += 1;
        }
        views.push(GroupView { heading, options });
    }
    views
}

/// The icon, label, and kind badge for one palette entry.
fn option_parts(chrome: &ChromeCtx, entry: &PaletteEntry) -> (String, String, String) {
    match entry {
        PaletteEntry::Record { category, row } => (
            category.icon().to_owned(),
            row.title.clone(),
            format!("{} · {}", chrome.0.rail_label(category.label_id()), row.display_id()),
        ),
        PaletteEntry::Recent { category, label, .. } => (
            "🕑".to_owned(),
            label.clone(),
            format!(
                "{} · {}",
                chrome.0.palette_kind_recent(),
                chrome.0.rail_label(category.label_id())
            ),
        ),
        PaletteEntry::Command(vm) => (
            command_icon(&vm.command),
            vm.label.clone(),
            chrome.0.palette_kind_command(),
        ),
    }
}

/// The decorative icon for a command option.
fn command_icon(command: &PaletteCommand) -> String {
    match command {
        PaletteCommand::Create(_) => "＋".to_owned(),
        PaletteCommand::FindDuplicates => Tool::Merge.icon().to_owned(),
        PaletteCommand::OpenTool(tool) => tool.icon().to_owned(),
        PaletteCommand::OpenHelp => "❔".to_owned(),
    }
}

/// Applies a palette action to the shell and closes the palette.
fn run_action(nav: &mut NavState, action: PaletteAction) {
    match action {
        PaletteAction::Open(reference) => open_record(nav, reference),
        PaletteAction::Run(command) => match command {
            PaletteCommand::Create(category) => nav.request_new_for(category),
            PaletteCommand::FindDuplicates => nav.go_to(Destination::Tool(Tool::Merge)),
            PaletteCommand::OpenTool(tool) => nav.go_to(Destination::Tool(tool)),
            PaletteCommand::OpenHelp => nav.go_to(Destination::Help { topic: None }),
        },
    }
    nav.close_overlay();
}

/// Navigates to a record's category and opens it as a tab.
fn open_record(nav: &mut NavState, reference: RecordRef) {
    nav.go_to(Destination::Category(reference.category));
    nav.open_record(reference);
}
