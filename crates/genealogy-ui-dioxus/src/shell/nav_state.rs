//! Shell-wide navigation and UI state, provided as context.
//!
//! The rail, top bar, record tabstrip, status bar, keyboard dispatcher, and overlays all read and
//! write one [`NavState`] (a `Copy` bundle of signals) rather than threading props through six
//! components. The active [`Destination`] is the framework-neutral navigation key from
//! `genealogy-ui` (ADR 0008); the renderer merely interprets it.

use dioxus::prelude::*;
use genealogy_app::{RecentItem, ThemeMode, push_recent};
use genealogy_ui::{Category, Destination, RecordRef};

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
    /// A monotonically-increasing "create a new record" ticket — bumped by the top-bar `New` action
    /// and `⌘N`, observed by the active screen to open its create form (context-aware creation).
    pub new_request: Signal<u32>,
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
        Self {
            active: Signal::new(Destination::Category(Category::Dashboard)),
            records: Signal::new(Vec::new()),
            active_record: Signal::new(None),
            new_request: Signal::new(0),
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
    /// `⌘N`). The active screen observes [`Self::new_request`] and opens its create form.
    pub fn request_new(&mut self) {
        let next = self.new_request.peek().wrapping_add(1);
        self.new_request.set(next);
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
    }

    /// Activates the open record tab at the 0-based `index`, if it exists.
    pub fn activate_record(&mut self, index: usize) {
        if index < self.records.read().len() {
            self.active_record.set(Some(index));
        }
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
}
