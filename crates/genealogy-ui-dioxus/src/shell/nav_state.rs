//! Shell-wide navigation and UI state, provided as context.
//!
//! The rail, top bar, record tabstrip, status bar, keyboard dispatcher, and overlays all read and
//! write one [`NavState`] (a `Copy` bundle of signals) rather than threading props through six
//! components. The active [`Destination`] is the framework-neutral navigation key from
//! `genealogy-ui` (ADR 0008); the renderer merely interprets it.

use dioxus::prelude::*;
use genealogy_app::{RecentItem, ThemeMode, push_recent};
use genealogy_ui::{Category, Destination, NavHistory, NavLocation, RecordRef};

/// The entity category a destination shows a list + editor for, or `None` when the destination is a
/// full-width screen with no Explorer/editor (a tool, the workspace Dashboard, or Help). This is the
/// single source of truth for the two shell shapes: `Some` ⇒ `rail | Explorer | editor`, `None` ⇒
/// `rail | screen`.
#[must_use]
pub fn entity_category(destination: Destination) -> Option<Category> {
    match destination {
        Destination::Category(Category::Dashboard) | Destination::Tool(_) | Destination::Help { .. } => None,
        Destination::Category(category) => Some(category),
    }
}

/// Which overlay, if any, is layered over the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// No overlay.
    None,
    /// The command palette (`⌘K`).
    Palette,
    /// The keyboard-shortcuts help sheet (`?`).
    Help,
}

/// The active colour theme, mirrored onto `[data-theme]` at the shell root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    /// The dark palette (default).
    Dark,
    /// The light palette.
    Light,
}

impl Theme {
    /// The `[data-theme]` attribute value this theme renders with.
    #[must_use]
    pub fn attr(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    /// The window background `(r, g, b, a)` used to seed the native window before the first paint,
    /// matching this theme's `--bg` token (`tokens.css`: dark `#0f1419`, light `#f6f8fa`). Keep in
    /// sync with those tokens so there is no colour flash before the stylesheet applies.
    #[must_use]
    pub fn background_rgba(self) -> (u8, u8, u8, u8) {
        match self {
            Self::Dark => (15, 20, 25, 255),
            Self::Light => (246, 248, 250, 255),
        }
    }
}

/// Resolves a persisted [`ThemeMode`] to a concrete [`Theme`] for `[data-theme]` and the window
/// background. `System` queries the OS appearance on the desktop build; everywhere else (and on any
/// detection failure) it falls back to the historical dark default.
#[must_use]
pub fn resolve_theme(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::Light => Theme::Light,
        ThemeMode::Dark => Theme::Dark,
        ThemeMode::System => detect_os_theme(),
    }
}

/// The next mode in the System → Light → Dark → System cycle (the top-bar control).
#[must_use]
pub fn next_theme_mode(mode: ThemeMode) -> ThemeMode {
    match mode {
        ThemeMode::System => ThemeMode::Light,
        ThemeMode::Light => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::System,
    }
}

#[cfg(feature = "desktop")]
fn detect_os_theme() -> Theme {
    match dark_light::detect() {
        Ok(dark_light::Mode::Light) => Theme::Light,
        _ => Theme::Dark,
    }
}

#[cfg(not(feature = "desktop"))]
fn detect_os_theme() -> Theme {
    Theme::Dark
}

/// One open tab in the record strip: a saved record or an unsaved draft (a create form).
///
/// "Create is a tab": [`NavState::open_create`] appends a [`Self::Draft`]; committing it
/// ([`NavState::commit_draft`]) replaces it in place with the saved [`Self::Saved`], and cancelling
/// ([`NavState::cancel_draft`]) closes it. A draft has no `human_id` yet, so it never docks and is
/// never recorded in the "Jump back in" list; at most one draft per category is open at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTab {
    /// A saved record backed by a stored aggregate.
    Saved(RecordRef),
    /// An unsaved draft create form for a category (nothing is stored until it commits).
    Draft(Category),
}

impl OpenTab {
    /// The aggregate category this tab belongs to.
    #[must_use]
    pub fn category(&self) -> Category {
        match self {
            Self::Saved(record) => record.category,
            Self::Draft(category) => *category,
        }
    }

    /// The tab's stable record id, or `None` for a draft (which has no stored aggregate yet).
    #[must_use]
    pub fn human_id(&self) -> Option<&str> {
        match self {
            Self::Saved(record) => Some(&record.human_id),
            Self::Draft(_) => None,
        }
    }

    /// The saved record this tab holds, or `None` when it is still a draft.
    #[must_use]
    pub fn as_saved(&self) -> Option<&RecordRef> {
        match self {
            Self::Saved(record) => Some(record),
            Self::Draft(_) => None,
        }
    }

    /// Whether this tab is an unsaved draft.
    #[must_use]
    pub fn is_draft(&self) -> bool {
        matches!(self, Self::Draft(_))
    }

    /// Whether this tab is the saved record keyed by `(category, human_id)`.
    #[must_use]
    fn is_saved_key(&self, category: Category, human_id: &str) -> bool {
        self.category() == category && self.human_id() == Some(human_id)
    }
}

/// Shell-wide navigation/UI state, provided as context so every shell region shares one source of
/// truth. All fields are signals, so reads subscribe the reading component and writes from the
/// keyboard dispatcher re-render only the subscribers.
#[derive(Clone, Copy)]
pub struct NavState {
    /// The destination the work area is showing (the rail's category/tool selection).
    pub active: Signal<Destination>,
    /// The open record tabs, in strip order (the in-app tabstrip; independent of [`Self::active`]).
    pub records: Signal<Vec<OpenTab>>,
    /// The index into [`Self::records`] of the active record tab, or `None` when none are open.
    pub active_record: Signal<Option<usize>>,
    /// The record docked side-by-side with the active record (`.master-detail.split-2`), stored by
    /// its `(category, human_id)` key so it stays stable across tab closes/reorders rather than by a
    /// fragile index. `None` when no split is open. Resolve to a live [`RecordRef`] with
    /// [`Self::docked_record_ref`].
    pub docked_record: Signal<Option<(Category, String)>>,
    /// The record tab currently being dragged (its `(category, human_id)` key), set on
    /// `dragstart` and cleared on `dragend`/drop. Drives the tabstrip's `dragging` affordance and is
    /// consumed by [`Self::complete_tab_drag`] on drop.
    pub dragging_tab: Signal<Option<(Category, String)>>,
    /// The back/forward navigation history over [`NavLocation`]s (destination + focused record).
    pub history: Signal<NavHistory>,
    /// A monotonically-increasing "workspace data changed" ticket — bumped after any mutation
    /// (create, edit, undo) so shell-wide views derived from the data (the rail count badges)
    /// refetch.
    pub data_version: Signal<u32>,
    /// Which overlay is open, if any.
    pub overlay: Signal<Overlay>,
    /// The persisted colour-theme mode (System / Light / Dark) the user selected.
    pub theme_mode: Signal<ThemeMode>,
    /// The resolved colour theme mirrored onto `[data-theme]` (System resolved to a concrete palette).
    pub theme: Signal<Theme>,
    /// The recently-opened records (newest first, capped), driving the dashboard "Jump back in" list.
    /// Seeded from the workspace manifest at startup and persisted on change.
    pub recent: Signal<Vec<RecentItem>>,
    /// A monotonically-increasing "undo the active record" ticket — bumped by `⌘Z` when a record is
    /// open on its own screen; the active detail pane observes the bump and retracts the newest
    /// undoable assertion of its already-loaded change log.
    pub pending_undo: Signal<u32>,
    /// A pending prev/next-record step (`[` = `-1`, `]` = `+1`), set by the keyboard dispatcher and
    /// consumed by the active master-detail screen to open the neighbouring record.
    pub pending_step: Signal<Option<i8>>,
    /// The query the command palette opens pre-seeded with (the top-bar search's Enter, `⌘F`); the
    /// palette copies it into its input on open and then clears it.
    pub palette_seed: Signal<String>,
    /// A transient shell notice (a toast), e.g. "Nothing to undo" or the redo-unavailable
    /// explanation. `None` hides it.
    pub notice: Signal<Option<String>>,
}

impl Default for NavState {
    fn default() -> Self {
        Self::new()
    }
}

impl NavState {
    /// Creates the shell state on the Dashboard with no record open, following the OS theme.
    #[must_use]
    pub fn new() -> Self {
        Self::with_prefs(ThemeMode::System, resolve_theme(ThemeMode::System), Vec::new())
    }

    /// Creates the shell state seeded with the resolved startup theme and persisted recent list:
    /// `mode` is the persisted preference, `resolved` the concrete palette it resolves to (already
    /// computed by the caller so the shell and the pre-launch window background agree), `recent` the
    /// "Jump back in" list read from the workspace manifest.
    #[must_use]
    pub fn with_prefs(mode: ThemeMode, resolved: Theme, recent: Vec<RecentItem>) -> Self {
        let mut history = NavHistory::default();
        history.push(NavLocation {
            destination: Destination::Category(Category::Dashboard),
            record: None,
        });
        Self {
            active: Signal::new(Destination::Category(Category::Dashboard)),
            records: Signal::new(Vec::new()),
            active_record: Signal::new(None),
            docked_record: Signal::new(None),
            dragging_tab: Signal::new(None),
            history: Signal::new(history),
            data_version: Signal::new(0),
            overlay: Signal::new(Overlay::None),
            theme_mode: Signal::new(mode),
            theme: Signal::new(resolved),
            recent: Signal::new(recent),
            pending_undo: Signal::new(0),
            pending_step: Signal::new(None),
            palette_seed: Signal::new(String::new()),
            notice: Signal::new(None),
        }
    }

    /// Requests an undo of the active record (`⌘Z`) by bumping [`Self::pending_undo`]; the active
    /// detail pane observes the bump and retracts the newest undoable assertion.
    pub fn request_undo(&mut self) {
        let next = self.pending_undo.peek().wrapping_add(1);
        self.pending_undo.set(next);
    }

    /// Requests a prev/next-record step (`[`/`]`) on the active master-detail screen.
    pub fn step_record(&mut self, delta: i8) {
        self.pending_step.set(Some(delta));
    }

    /// Shows a transient shell notice (a toast).
    pub fn notify(&mut self, message: String) {
        self.notice.set(Some(message));
    }

    /// Dismisses the shell notice.
    pub fn dismiss_notice(&mut self) {
        self.notice.set(None);
    }

    /// Opens the command palette pre-seeded with `query` (the top-bar search's Enter).
    pub fn open_palette_seeded(&mut self, query: String) {
        self.palette_seed.set(query);
        self.overlay.set(Overlay::Palette);
    }

    /// Advances the theme mode (System → Light → Dark → System) and re-resolves the rendered theme.
    /// Returns the new mode so the caller can persist it.
    pub fn cycle_theme(&mut self) -> ThemeMode {
        let next = next_theme_mode(*self.theme_mode.peek());
        self.theme_mode.set(next);
        self.theme.set(resolve_theme(next));
        next
    }

    /// Requests context-aware creation of a new record on the active screen (the top-bar `New` and
    /// `⌘N`) by opening a draft tab ([`Self::open_create`]). A no-op unless the active destination is
    /// an aggregate category (not the Dashboard, and not a tool).
    pub fn request_new(&mut self) {
        let Destination::Category(category) = *self.active.peek() else {
            return;
        };
        if category == Category::Dashboard {
            return;
        }
        self.open_create(category);
    }

    /// Reveals `category` and opens a draft tab there (the tabstrip's new-record menu, the command
    /// palette) — unlike [`Self::request_new`], this works from any destination.
    pub fn request_new_for(&mut self, category: Category) {
        self.go_to(Destination::Category(category));
        self.open_create(category);
    }

    /// Opens a create-form draft tab for `category` and makes it active. At most one draft per
    /// category is open at a time: an existing draft is re-focused rather than duplicated. Nothing is
    /// stored until the draft commits ([`Self::commit_draft`]).
    pub fn open_create(&mut self, category: Category) {
        let existing = self
            .records
            .read()
            .iter()
            .position(|tab| tab.is_draft() && tab.category() == category);
        if let Some(index) = existing {
            self.active_record.set(Some(index));
            return;
        }
        self.records.write().push(OpenTab::Draft(category));
        let last = self.records.read().len().saturating_sub(1);
        self.active_record.set(Some(last));
    }

    /// Commits a draft in place: replaces the open draft tab for `record.category` with the saved
    /// `record`, keeping its position in the strip and making it active, and records it in the "Jump
    /// back in" list. Falls back to opening `record` as a fresh tab if no draft is open (e.g. the
    /// draft was closed mid-commit).
    pub fn commit_draft(&mut self, record: RecordRef) {
        if let Some(kind) = record.category.aggregate_kind() {
            push_recent(
                &mut self.recent.write(),
                RecentItem::Record {
                    kind: kind.to_owned(),
                    human_id: record.human_id.clone(),
                    label: record.label.clone(),
                },
            );
        }
        let draft = self
            .records
            .read()
            .iter()
            .position(|tab| tab.is_draft() && tab.category() == record.category);
        let Some(index) = draft else {
            self.open_record(record);
            return;
        };
        self.records.write()[index] = OpenTab::Saved(record);
        self.active_record.set(Some(index));
        let location = self.current_location();
        self.history.write().push(location);
    }

    /// Cancels the open draft tab for `category`, closing it (Cancel on a create form).
    pub fn cancel_draft(&mut self, category: Category) {
        let draft = self
            .records
            .read()
            .iter()
            .position(|tab| tab.is_draft() && tab.category() == category);
        if let Some(index) = draft {
            self.close_record(index);
        }
    }

    /// Marks the workspace data as changed so shell-wide derived views (the rail count badges)
    /// refetch. Called after a create/edit/undo succeeds.
    pub fn mark_changed(&mut self) {
        let next = self.data_version.peek().wrapping_add(1);
        self.data_version.set(next);
    }

    /// Navigates the work area to `destination` (the rail's category/tool selection). This does not
    /// touch the open record tabs — opening a record is [`Self::open_record`]. The "Jump back in"
    /// list remembers records only, so this never pushes to it — see [`Self::open_record`].
    ///
    /// Skips the history push when the resulting location is a bare category list with no record
    /// focused ([`NavLocation::is_recordless_list`]) — otherwise back/forward would step through
    /// empty list views. [`Self::open_record`] and [`Self::activate_record`] always focus a record,
    /// so their pushes are never skipped.
    pub fn go_to(&mut self, destination: Destination) {
        self.active.set(destination);
        let location = self.current_location();
        if !location.is_recordless_list() {
            self.history.write().push(location);
        }
    }

    /// Opens `record` as a tab — focusing the existing tab with the same `(category, human_id)` or
    /// appending a new one — makes it the active record, and records it in the "Jump back in" list.
    pub fn open_record(&mut self, record: RecordRef) {
        if let Some(kind) = record.category.aggregate_kind() {
            push_recent(
                &mut self.recent.write(),
                RecentItem::Record {
                    kind: kind.to_owned(),
                    human_id: record.human_id.clone(),
                    label: record.label.clone(),
                },
            );
        }
        let existing = self
            .records
            .read()
            .iter()
            .position(|open| open.is_saved_key(record.category, &record.human_id));
        if let Some(index) = existing {
            self.active_record.set(Some(index));
        } else {
            self.records.write().push(OpenTab::Saved(record));
            let last = self.records.read().len().saturating_sub(1);
            self.active_record.set(Some(last));
        }
        let location = self.current_location();
        self.history.write().push(location);
    }

    /// Opens `record` as a tab and, only when the editor is currently hidden (the active destination
    /// is not an entity category — a tool, the Dashboard, or Help), reveals the record's category so
    /// the editor becomes visible. From within an entity category the rail/Explorer list is left as
    /// is, so following a link opens the record beside the current list without switching it (the
    /// VS Code "open from search" behaviour). Used by the shared `RecordLink`; list-row selection
    /// calls [`Self::open_record`] directly (already on that category).
    pub fn reveal_record(&mut self, record: RecordRef) {
        if entity_category(*self.active.peek()).is_none() {
            self.go_to(Destination::Category(record.category));
        }
        self.open_record(record);
    }

    /// Activates the open record tab at the 0-based `index`, if it exists.
    pub fn activate_record(&mut self, index: usize) {
        if index >= self.records.read().len() {
            return;
        }
        self.active_record.set(Some(index));
        let location = self.current_location();
        self.history.write().push(location);
    }

    /// Switches to the 1-based record tab `n` (`⌘1…9`), if it exists.
    pub fn switch_record(&mut self, n: u8) {
        self.activate_record(usize::from(n).saturating_sub(1));
    }

    /// Closes the open record tab at `index`, falling back to a neighbouring tab and clearing the
    /// active record when none remain. Does not change the rail's [`Self::active`] destination.
    pub fn close_record(&mut self, index: usize) {
        if index >= self.records.read().len() {
            return;
        }
        self.records.write().remove(index);
        let remaining = self.records.read().len();
        if remaining == 0 {
            self.active_record.set(None);
        } else {
            // Closing a tab left of the active one shifts it left; keep the same record focused.
            let active = self.active_record.read().unwrap_or(0);
            let active = if index < active { active - 1 } else { active };
            self.active_record.set(Some(active.min(remaining - 1)));
        }
        // Drop the dock if its record is no longer open (the docked tab itself was closed).
        let docked_gone = self.docked_record.peek().as_ref().is_some_and(|(category, human_id)| {
            !self
                .records
                .peek()
                .iter()
                .any(|open| open.is_saved_key(*category, human_id))
        });
        if docked_gone {
            self.docked_record.set(None);
        }
    }

    /// The active open tab (saved record or draft), if any.
    #[must_use]
    pub fn active_tab(&self) -> Option<OpenTab> {
        self.active_record
            .read()
            .and_then(|index| self.records.read().get(index).cloned())
    }

    /// The active *saved* record, if any (for the tabstrip, breadcrumb, status bar, and detail pane).
    /// Returns `None` when the active tab is an unsaved draft.
    #[must_use]
    pub fn active_record_ref(&self) -> Option<RecordRef> {
        self.active_tab().and_then(|tab| tab.as_saved().cloned())
    }

    /// Docks the record tab keyed by `(category, human_id)` side-by-side with the active record. A
    /// no-op unless the key names an open tab that is not itself the active record (a record cannot
    /// dock beside itself); docking the record already docked toggles the split off; otherwise it
    /// replaces the dock. Stored by key so it survives tab closes/reorders ([`Self::docked_record`]).
    pub fn dock_record(&mut self, category: Category, human_id: &str) {
        let is_open = self
            .records
            .peek()
            .iter()
            .any(|open| open.is_saved_key(category, human_id));
        if !is_open || self.is_active_key(category, human_id) {
            return;
        }
        let already_docked = self
            .docked_record
            .peek()
            .as_ref()
            .is_some_and(|(docked_category, docked_id)| *docked_category == category && docked_id == human_id);
        if already_docked {
            self.docked_record.set(None);
        } else {
            self.docked_record.set(Some((category, human_id.to_owned())));
        }
    }

    /// Docks the 1-based record tab `n` (`⌘⇧1…9`), if it exists — mirrors [`Self::switch_record`],
    /// resolving the index to its key before delegating to [`Self::dock_record`].
    pub fn dock_record_tab(&mut self, n: u8) {
        let index = usize::from(n).saturating_sub(1);
        let key = self
            .records
            .peek()
            .get(index)
            .and_then(|tab| tab.as_saved().map(|record| (record.category, record.human_id.clone())));
        if let Some((category, human_id)) = key {
            self.dock_record(category, &human_id);
        }
    }

    /// Closes the docked split (the undock `✕`), leaving the tabs untouched.
    pub fn undock_record(&mut self) {
        self.docked_record.set(None);
    }

    /// The docked record as a live [`RecordRef`] (fresh label resolved against the open tabs), or
    /// `None` when nothing is docked, the docked tab has since closed, or the docked tab is itself
    /// the active record (the split collapses while it is active, but the dock state survives so it
    /// returns when another record becomes active).
    #[must_use]
    pub fn docked_record_ref(&self) -> Option<RecordRef> {
        let (category, human_id) = self.docked_record.read().clone()?;
        // `read()` (not `peek()`) so the reader re-renders when the active record changes and the
        // split needs to collapse or return.
        let active = self
            .active_record
            .read()
            .and_then(|index| self.records.read().get(index).cloned());
        if active.is_some_and(|tab| tab.is_saved_key(category, &human_id)) {
            return None;
        }
        self.records
            .read()
            .iter()
            .find(|open| open.is_saved_key(category, &human_id))
            .and_then(|tab| tab.as_saved().cloned())
    }

    /// Begins a tab drag from the tab keyed by `(category, human_id)` (`dragstart`).
    pub fn begin_tab_drag(&mut self, category: Category, human_id: &str) {
        self.dragging_tab.set(Some((category, human_id.to_owned())));
    }

    /// Ends a tab drag without a drop (`dragend`).
    pub fn end_tab_drag(&mut self) {
        self.dragging_tab.set(None);
    }

    /// Completes a tab drag by docking the dragged tab (`drop`): clears the drag and, if one was
    /// live, docks its record. The whole drop is one transition so it is testable end-to-end.
    pub fn complete_tab_drag(&mut self) {
        let dragged = self.dragging_tab.peek().clone();
        self.dragging_tab.set(None);
        if let Some((category, human_id)) = dragged {
            self.dock_record(category, &human_id);
        }
    }

    /// Whether `(category, human_id)` is the key of the currently active record tab.
    fn is_active_key(&self, category: Category, human_id: &str) -> bool {
        self.active_record
            .peek()
            .and_then(|index| self.records.peek().get(index).cloned())
            .is_some_and(|active| active.is_saved_key(category, human_id))
    }

    /// Closes any open overlay (`Esc`).
    pub fn close_overlay(&mut self) {
        self.overlay.set(Overlay::None);
    }

    /// Renames the label of the open record tab identified by `(category, human_id)`, if it is still
    /// open. A no-op when `label` is empty (a record is never renamed to a blank tab) or the record
    /// has since been closed.
    pub fn set_record_label(&mut self, category: Category, human_id: &str, label: String) {
        if label.is_empty() {
            return;
        }
        let mut records = self.records.write();
        let Some(OpenTab::Saved(record)) = records.iter_mut().find(|open| open.is_saved_key(category, human_id)) else {
            return;
        };
        record.label = label;
    }

    /// Re-keys the open record tab identified by `(category, old_human_id)` to `new_human_id` after a
    /// rename, so the tab (and the keyed detail pane it drives) follow the record to its new id. A
    /// no-op when the id is unchanged or the record has since been closed.
    pub fn rename_record(&mut self, category: Category, old_human_id: &str, new_human_id: String) {
        if old_human_id == new_human_id {
            return;
        }
        // Re-key the dock so the split follows the record to its new id.
        if let Some((docked_category, docked_id)) = self.docked_record.write().as_mut()
            && *docked_category == category
            && docked_id == old_human_id
        {
            docked_id.clone_from(&new_human_id);
        }
        let mut records = self.records.write();
        let Some(OpenTab::Saved(record)) = records
            .iter_mut()
            .find(|open| open.is_saved_key(category, old_human_id))
        else {
            return;
        };
        record.human_id = new_human_id;
    }

    /// Moves the navigation history one step back and applies the resulting location, if any (`⌘←`
    /// browser-style navigation). A no-op at the start of history.
    pub fn history_back(&mut self) {
        let Some(location) = self.history.write().back() else {
            return;
        };
        self.apply(location);
    }

    /// Moves the navigation history one step forward and applies the resulting location, if any
    /// (`⌘→` browser-style navigation). A no-op at the end of history.
    pub fn history_forward(&mut self) {
        let Some(location) = self.history.write().forward() else {
            return;
        };
        self.apply(location);
    }

    /// Whether [`Self::history_back`] would move the history (there is an earlier entry).
    #[must_use]
    pub fn can_back(&self) -> bool {
        self.history.read().can_back()
    }

    /// Whether [`Self::history_forward`] would move the history (there is a later entry).
    #[must_use]
    pub fn can_forward(&self) -> bool {
        self.history.read().can_forward()
    }

    /// The current navigation location: the active destination plus the active record's
    /// `(category, human_id)`, if any.
    fn current_location(&self) -> NavLocation {
        NavLocation {
            destination: *self.active.peek(),
            record: self
                .active_record_ref()
                .map(|record| (record.category, record.human_id)),
        }
    }

    /// Applies a [`NavLocation`] pulled from history: sets the active destination and, if the
    /// location names a record still open, re-focuses it — without pushing a new history entry.
    fn apply(&mut self, location: NavLocation) {
        self.active.set(location.destination);
        let index = location.record.and_then(|(category, human_id)| {
            self.records
                .read()
                .iter()
                .position(|open| open.is_saved_key(category, &human_id))
        });
        self.active_record.set(index);
    }
}
