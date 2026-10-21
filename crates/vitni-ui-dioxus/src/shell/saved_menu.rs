//! The Explorer's *Saved* menu and the state transitions a saved search drives (ADR 0052 §8-10, #680).
//!
//! The toolbar control is a 30px icon button, ☆ while no saved search is applied and ★ while one is. Its
//! menu hangs full width under the toolbar like *Columns* and sort ([`ListMenu`]): the kind's saved
//! searches, then *Save as new*, *Update*, *Rename* and *Delete*, the last three acting on the applied
//! search. *Save as new* and *Rename* ask for the name in a field inside the menu — the viewer has no
//! `prompt()`.
//!
//! [`SavedMenu`] only renders and reports a [`SavedAction`]; the Explorer performs it. What an apply does
//! to the list's signals is [`finish_saved_run`], and the other transitions are free functions over
//! [`NavState`] and [`ExplorerQuery`] so they can be tested without a live webview.

use std::rc::Rc;

use dioxus::prelude::*;
use vitni_app::{AppError, ListQuery, ListedKind, SavedSearchId, SavedSearchList};
use vitni_ui::{
    ActiveFilter, ListLayout, Localizer, NameProblem, NamingMode, RowsPage, SavedEntryVm, SavedFilters, SavedNaming,
    StaleClauseVm, name_problem, saved_layout, saved_menu,
};

use crate::components::TextInput;
use crate::i18n::Chrome;
use crate::services::{
    Services, delete_saved_search_by_id, load_list_page, rename_saved_search_to, save_new_saved_search,
    update_saved_search_query,
};
use crate::shell::ChromeCtx;
use crate::shell::list_columns::{ListMenu, Scrim, SortItem, close_on_escape};
use crate::shell::list_filters::FilterEditing;
use crate::shell::nav_state::NavState;

/// The signals that make up one Explorer list's query, shared with the code that applies a saved search
/// to them.
#[derive(Clone, Copy)]
pub struct ExplorerQuery {
    /// The search box.
    pub search: Signal<String>,
    /// The clauses that can run.
    pub filters: Signal<Vec<ActiveFilter>>,
    /// The clauses of an applied saved search that cannot run. While any exists the list does not load.
    pub stale: Signal<Vec<StaleClauseVm>>,
    /// The columns and sort.
    pub layout: Signal<ListLayout>,
    /// The layout the applied saved search brought, so it is not written to the list's own preferences.
    pub applied_layout: Signal<Option<ListLayout>>,
    /// The saved search applied to the list, with its name.
    pub applied: Signal<Option<(SavedSearchId, String)>>,
    /// The open filter editor, which targets a clause of the list it was opened on.
    pub editing: Signal<Option<FilterEditing>>,
}

/// A saved search read afresh for applying: its identity, its runnable query and its rebuilt chips.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSaved {
    /// The saved search's identity.
    pub id: SavedSearchId,
    /// Its name.
    pub name: String,
    /// The query to run: the clauses that could be read.
    pub query: ListQuery,
    /// The filter bar's chips, and the clauses to mark.
    pub filters: SavedFilters,
}

/// Puts a resolved saved search on the list: its search text, chips, marked clauses and layout. It also closes
/// the filter editor, whose target belonged to the old clauses, and remembers the saved search as the applied
/// one. A search that could not be read raises its message as a notice and changes nothing.
pub fn finish_saved_run(mut nav: NavState, mut state: ExplorerQuery, result: Result<ResolvedSaved, String>) {
    let saved = match result {
        Ok(saved) => saved,
        Err(message) => return nav.notify_error(message),
    };
    let layout = saved_layout(&saved.query);
    state.search.set(saved.query.search);
    state.filters.set(saved.filters.applied);
    state.stale.set(saved.filters.stale);
    state.layout.set(layout.clone());
    state.applied_layout.set(Some(layout));
    state.applied.set(Some((saved.id, saved.name)));
    state.editing.set(None);
}

/// Reads the first page of `query`, unless a clause of the applied saved search is marked: running
/// only the clauses that could be read would list more than the saved search names (ADR 0052 §10), so the
/// list shows `marked_notice` instead and the store is not read.
pub async fn first_page_unless_marked(
    services: Services,
    query: ListQuery,
    marked: bool,
    marked_notice: String,
) -> (ListQuery, Result<RowsPage, String>) {
    if marked {
        return (query, Err(marked_notice));
    }
    let page = load_list_page(services, query.clone(), 0).await;
    (query, page)
}

/// What the menu asks the Explorer to do to the saved searches (applying one is its own callback).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavedAction {
    /// Store the list's query as a new saved search with this name.
    SaveNew(String),
    /// Replace the applied saved search's query with the list's.
    Update,
    /// Give the applied saved search this name.
    Rename(String),
    /// Delete the applied saved search.
    Delete,
}

/// What it takes to change the stored saved searches for one Explorer list and report the outcome.
pub struct SavedChange<'a> {
    /// The shell, which shows the outcome as a notice.
    pub nav: NavState,
    /// The list's signals; the applied search follows the change.
    pub state: ExplorerQuery,
    /// The workspace directory and the operator.
    pub services: &'a Services,
    /// The list's query now, which *Save as new* and *Update* store.
    pub query: &'a ListQuery,
    /// The chrome and data localizers, for the notice.
    pub localizers: (Rc<Chrome>, Rc<Localizer>),
}

impl SavedChange<'_> {
    /// Performs `action`, raising a notice for what happened. Returns whether it succeeded, so the caller
    /// closes the menu. Storing a query while a marked clause is on the list would drop that clause, so
    /// *Save as new* and *Update* refuse until it is fixed or removed.
    #[must_use]
    pub fn perform(mut self, action: SavedAction) -> bool {
        let (chrome, loc) = &self.localizers;
        let applied = self.state.applied.peek().clone();
        let stores_query = match action {
            SavedAction::SaveNew(_) | SavedAction::Update => true,
            SavedAction::Rename(_) | SavedAction::Delete => false,
        };
        if stores_query && !self.state.stale.peek().is_empty() {
            self.nav.notify_error(chrome.saved_marked_blocks_save());
            return false;
        }
        let outcome: Result<String, AppError> = match (action, applied) {
            (SavedAction::SaveNew(name), _) => save_new_saved_search(self.services, &name, self.query).map(|id| {
                self.state.applied.set(Some((id, name.clone())));
                chrome.saved_saved(&name)
            }),
            (SavedAction::Update, Some((id, name))) => {
                update_saved_search_query(self.services, id, self.query).map(|()| chrome.saved_updated(&name))
            }
            (SavedAction::Rename(name), Some((id, _))) => rename_saved_search_to(self.services, id, &name).map(|()| {
                self.state.applied.set(Some((id, name.clone())));
                chrome.saved_renamed(&name)
            }),
            (SavedAction::Delete, Some((id, name))) => delete_saved_search_by_id(self.services, id).map(|()| {
                self.state.applied.set(None);
                chrome.saved_deleted(&name)
            }),
            (SavedAction::Update | SavedAction::Rename(_) | SavedAction::Delete, None) => return false,
        };
        match outcome {
            Ok(message) => {
                self.nav.notify(message);
                true
            }
            Err(error) => {
                self.nav.notify_error(loc.error(&error));
                false
            }
        }
    }
}

/// The search a name field may keep the name of: the applied one when renaming, none when saving a new one.
#[must_use]
pub fn renamed_search(mode: NamingMode, applied: Option<&(SavedSearchId, String)>) -> Option<SavedSearchId> {
    match mode {
        NamingMode::New => None,
        NamingMode::Rename => applied.map(|(id, _)| *id),
    }
}

/// The *Saved* icon button and, when open, its menu.
#[component]
pub fn SavedMenu(
    kind: ListedKind,
    list: Signal<SavedSearchList>,
    applied: Signal<Option<(SavedSearchId, String)>>,
    open: Signal<Option<ListMenu>>,
    naming: Signal<Option<SavedNaming>>,
    #[props(default)] onapply: Option<EventHandler<SavedSearchId>>,
    #[props(default)] onact: Option<EventHandler<SavedAction>>,
) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let shown = open() == Some(ListMenu::Saved);
    use_effect(move || {
        if open() != Some(ListMenu::Saved) && naming.peek().is_some() {
            naming.set(None);
        }
    });
    let applied_id = applied.read().as_ref().map(|(id, _)| *id);
    let menu = saved_menu(&list.read(), kind, applied_id);
    let label = chrome.saved_title();
    let mut unreadable = Vec::with_capacity(menu.unreadable.len());
    for entry in &menu.unreadable {
        let label = match &entry.name {
            Some(name) => chrome.saved_unreadable_named(name),
            None => chrome.saved_unreadable(entry.position),
        };
        unreadable.push((entry.position, label));
    }
    let act = move |action: SavedAction| {
        if let Some(onact) = onact {
            onact.call(action);
        }
    };
    let submit = use_callback(move |()| {
        let Some(current) = naming.peek().clone() else {
            return;
        };
        let own = renamed_search(current.mode, applied.peek().as_ref());
        if let Some(problem) = name_problem(&list.peek(), &current.name, own) {
            naming.set(Some(SavedNaming {
                problem: Some(problem),
                ..current
            }));
            return;
        }
        let name = current.name.trim().to_owned();
        act(match current.mode {
            NamingMode::New => SavedAction::SaveNew(name),
            NamingMode::Rename => SavedAction::Rename(name),
        });
    });
    rsx! {
        span { class: "menu-anchor", onkeydown: move |event| close_on_escape(&event, open),
            button {
                class: "icon-btn",
                r#type: "button",
                title: "{label}",
                aria_label: "{label}",
                aria_haspopup: "menu",
                aria_expanded: if shown { "true" } else { "false" },
                onclick: move |_| open.set(if shown { None } else { Some(ListMenu::Saved) }),
                if applied_id.is_some() {
                    "★"
                } else {
                    "☆"
                }
            }
            if shown {
                Scrim { open }
                div {
                    class: "list-menu",
                    role: "menu",
                    aria_label: "{label}",
                    "data-hook": "saved-menu",
                    SavedEntries { entries: menu.entries, unreadable, onapply }
                    div { class: "menu-sep", role: "separator" }
                    if let Some(current) = naming() {
                        NameField { naming, current, onsubmit: submit }
                    } else {
                        SavedActions { naming, applied, acts_on_applied: menu.acts_on_applied, onact }
                    }
                }
            }
        }
    }
}

/// The saved searches of the kind, then the ones this version cannot read, disabled.
#[component]
fn SavedEntries(
    entries: Vec<SavedEntryVm>,
    unreadable: Vec<(usize, String)>,
    onapply: Option<EventHandler<SavedSearchId>>,
) -> Element {
    rsx! {
        for entry in entries {
            SortItem {
                key: "{entry.id}",
                label: entry.name.clone(),
                checked: entry.applied,
                onpick: move |()| {
                    if let Some(onapply) = onapply {
                        onapply.call(entry.id);
                    }
                },
            }
        }
        for (position , name) in unreadable {
            MenuItem {
                key: "unreadable-{position}",
                label: name,
                disabled: true,
                onpick: move |()| {},
            }
        }
    }
}

/// *Save as new*, *Update*, *Rename* and *Delete*; the last three need an applied saved search.
#[component]
fn SavedActions(
    naming: Signal<Option<SavedNaming>>,
    applied: Signal<Option<(SavedSearchId, String)>>,
    acts_on_applied: bool,
    onact: Option<EventHandler<SavedAction>>,
) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let act = move |action: SavedAction| {
        if let Some(onact) = onact {
            onact.call(action);
        }
    };
    rsx! {
        MenuItem {
            label: chrome.saved_save_new(),
            onpick: move |()| {
                naming.set(Some(SavedNaming { mode: NamingMode::New, name: String::new(), problem: None }));
            },
        }
        MenuItem {
            label: chrome.saved_update(),
            disabled: !acts_on_applied,
            onpick: move |()| act(SavedAction::Update),
        }
        MenuItem {
            label: chrome.saved_rename(),
            disabled: !acts_on_applied,
            onpick: move |()| {
                let name = applied.peek().as_ref().map(|(_, name)| name.clone()).unwrap_or_default();
                naming.set(Some(SavedNaming { mode: NamingMode::Rename, name, problem: None }));
            },
        }
        MenuItem {
            label: chrome.saved_delete(),
            disabled: !acts_on_applied,
            onpick: move |()| act(SavedAction::Delete),
        }
    }
}

/// One action of the menu, disabled when it has nothing to act on.
#[component]
fn MenuItem(label: String, #[props(default)] disabled: bool, onpick: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: "menu-item",
            role: "menuitem",
            r#type: "button",
            aria_disabled: if disabled { "true" } else { "false" },
            onclick: move |_| {
                if !disabled {
                    onpick.call(());
                }
            },
            "{label}"
        }
    }
}

/// The inline name field with *Save* and *Cancel*; a refused name shows its message under the field,
/// linked to it.
#[component]
fn NameField(naming: Signal<Option<SavedNaming>>, current: SavedNaming, onsubmit: Callback<()>) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    use_effect(|| {
        document::eval("document.getElementById('saved-name')?.focus();");
    });
    let message = current.problem.map(|problem| match problem {
        NameProblem::Empty => chrome.saved_name_empty(),
        NameProblem::Duplicate => chrome.saved_name_duplicate(),
    });
    rsx! {
        div { class: "saved-naming", role: "group", aria_label: "{chrome.saved_name()}",
            label { r#for: "saved-name", "{chrome.saved_name()}" }
            TextInput {
                id: "saved-name",
                value: "{current.name}",
                invalid: current.problem.is_some(),
                aria_describedby: current.problem.map(|_| "saved-name-error"),
                oninput: move |event: FormEvent| {
                    if let Some(held) = naming.write().as_mut() {
                        held.name = event.value();
                        held.problem = None;
                    }
                },
                onkeydown_extra: move |event: KeyboardEvent| {
                    if event.key() == Key::Enter {
                        event.prevent_default();
                        onsubmit.call(());
                    }
                },
            }
            if let Some(message) = message {
                span { id: "saved-name-error", class: "field-error", role: "alert", "{message}" }
            }
            div { class: "filter-actions",
                button {
                    class: "btn sm primary",
                    r#type: "button",
                    onclick: move |_| onsubmit.call(()),
                    "{chrome.saved_save()}"
                }
                button {
                    class: "btn sm ghost",
                    r#type: "button",
                    onclick: move |_| naming.set(None),
                    "{chrome.saved_cancel()}"
                }
            }
        }
    }
}
