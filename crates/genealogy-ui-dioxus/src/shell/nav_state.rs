//! Shell-wide navigation and UI state, provided as context.
//!
//! The rail, top bar, record tabstrip, status bar, keyboard dispatcher, and overlays all read and
//! write one [`NavState`] (a `Copy` bundle of signals) rather than threading props through six
//! components. The active [`Destination`] is the framework-neutral navigation key from
//! `genealogy-ui` (ADR 0008); the renderer merely interprets it.

use dioxus::prelude::*;
use genealogy_app::{RecentItem, ThemeMode, push_recent};
use genealogy_ui::{Category, Destination, NavHistory, NavLocation, RecordRef};

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

/// Shell-wide navigation/UI state, provided as context so every shell region shares one source of
/// truth. All fields are signals, so reads subscribe the reading component and writes from the
/// keyboard dispatcher re-render only the subscribers.
#[derive(Clone, Copy)]
pub struct NavState {
    /// The destination the work area is showing (the rail's category/tool selection).
    pub active: Signal<Destination>,
    /// The open record tabs, in strip order (the in-app tabstrip; independent of [`Self::active`]).
    pub records: Signal<Vec<RecordRef>>,
    /// The index into [`Self::records`] of the active record tab, or `None` when none are open.
    pub active_record: Signal<Option<usize>>,
    /// The back/forward navigation history over [`NavLocation`]s (destination + focused record).
    pub history: Signal<NavHistory>,
    /// The category a "create a new record" request targets, if one is pending — set by the top-bar
    /// `New` action, `⌘N`, and the tabstrip's new-record menu; observed by the active screen to open
    /// its create form (context-aware creation).
    pub pending_create: Signal<Option<Category>>,
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
    /// The recently-opened records/tools (newest first, capped), driving the dashboard "Jump back in"
    /// list. Seeded from the workspace manifest at startup and persisted on change.
    pub recent: Signal<Vec<RecentItem>>,
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
            history: Signal::new(history),
            pending_create: Signal::new(None),
            data_version: Signal::new(0),
            overlay: Signal::new(Overlay::None),
            theme_mode: Signal::new(mode),
            theme: Signal::new(resolved),
            recent: Signal::new(recent),
        }
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
    /// `⌘N`). A no-op on the Dashboard (not an aggregate). The active screen observes
    /// [`Self::pending_create`] and opens its create form.
    pub fn request_new(&mut self) {
        let Destination::Category(category) = *self.active.peek() else {
            return;
        };
        if category == Category::Dashboard {
            return;
        }
        self.pending_create.set(Some(category));
    }

    /// Navigates to `category` and requests creation of a new record there (the tabstrip's
    /// new-record menu) — unlike [`Self::request_new`], this works from any destination.
    pub fn request_new_for(&mut self, category: Category) {
        self.go_to(Destination::Category(category));
        self.pending_create.set(Some(category));
    }

    /// Marks the workspace data as changed so shell-wide derived views (the rail count badges)
    /// refetch. Called after a create/edit/undo succeeds.
    pub fn mark_changed(&mut self) {
        let next = self.data_version.peek().wrapping_add(1);
        self.data_version.set(next);
    }

    /// Navigates the work area to `destination` (the rail's category/tool selection). This does not
    /// touch the open record tabs — opening a record is [`Self::open_record`]. Visiting a tool records
    /// it in the "Jump back in" list.
    pub fn go_to(&mut self, destination: Destination) {
        if let Destination::Tool(tool) = destination {
            push_recent(
                &mut self.recent.write(),
                RecentItem::Tool {
                    tool: tool.id().to_owned(),
                },
            );
        }
        self.active.set(destination);
        let location = self.current_location();
        self.history.write().push(location);
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
            .position(|open| open.category == record.category && open.human_id == record.human_id);
        if let Some(index) = existing {
            self.active_record.set(Some(index));
        } else {
            self.records.write().push(record);
            let last = self.records.read().len().saturating_sub(1);
            self.active_record.set(Some(last));
        }
        let location = self.current_location();
        self.history.write().push(location);
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
    }

    /// The active record, if any (for the tabstrip, breadcrumb, status bar, and detail pane).
    #[must_use]
    pub fn active_record_ref(&self) -> Option<RecordRef> {
        self.active_record
            .read()
            .and_then(|index| self.records.read().get(index).cloned())
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
        let Some(record) = records
            .iter_mut()
            .find(|open| open.category == category && open.human_id == human_id)
        else {
            return;
        };
        record.label = label;
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
                .position(|open| open.category == category && open.human_id == human_id)
        });
        self.active_record.set(index);
    }
}
