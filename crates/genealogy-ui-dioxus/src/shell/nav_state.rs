//! Shell-wide navigation and UI state, provided as context.
//!
//! The rail, top bar, record tabstrip, status bar, keyboard dispatcher, and overlays all read and
//! write one [`NavState`] (a `Copy` bundle of signals) rather than threading props through six
//! components. The active [`Destination`] is the framework-neutral navigation key from
//! `genealogy-ui` (ADR 0008); the renderer merely interprets it.

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination};

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

    /// The opposite theme (for the toggle control).
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

/// Shell-wide navigation/UI state, provided as context so every shell region shares one source of
/// truth. All fields are signals, so reads subscribe the reading component and writes from the
/// keyboard dispatcher re-render only the subscribers.
#[derive(Clone, Copy)]
pub struct NavState {
    /// The destination the work area is showing.
    pub active: Signal<Destination>,
    /// The open record tabs, in strip order.
    pub tabs: Signal<Vec<Destination>>,
    /// The index into [`Self::tabs`] of the active tab.
    pub active_tab: Signal<usize>,
    /// Which overlay is open, if any.
    pub overlay: Signal<Overlay>,
    /// The active colour theme.
    pub theme: Signal<Theme>,
}

impl Default for NavState {
    fn default() -> Self {
        Self::new()
    }
}

impl NavState {
    /// Creates the shell state with People active and a single open tab on a dark theme.
    #[must_use]
    pub fn new() -> Self {
        let people = Destination::Category(Category::People);
        Self {
            active: Signal::new(people),
            tabs: Signal::new(vec![people]),
            active_tab: Signal::new(0),
            overlay: Signal::new(Overlay::None),
            theme: Signal::new(Theme::Dark),
        }
    }

    /// Navigates to `destination`, opening its record tab (or focusing the existing one).
    pub fn go_to(&mut self, destination: Destination) {
        self.active.set(destination);
        let position = self.tabs.read().iter().position(|tab| *tab == destination);
        if let Some(index) = position {
            self.active_tab.set(index);
        } else {
            self.tabs.write().push(destination);
            let last = self.tabs.read().len().saturating_sub(1);
            self.active_tab.set(last);
        }
    }

    /// Activates the open record tab at the 0-based `index`, if it exists.
    pub fn activate_tab(&mut self, index: usize) {
        let Some(destination) = self.tabs.read().get(index).copied() else {
            return;
        };
        self.active_tab.set(index);
        self.active.set(destination);
    }

    /// Switches to the 1-based record tab `n` (`⌘1…9`), if it exists.
    pub fn switch_tab(&mut self, n: u8) {
        self.activate_tab(usize::from(n).saturating_sub(1));
    }

    /// Closes the open record tab at `index`, falling back to a neighbouring tab.
    pub fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.read().len() || self.tabs.read().len() == 1 {
            return;
        }
        self.tabs.write().remove(index);
        let last = self.tabs.read().len().saturating_sub(1);
        let active = self.active_tab.read().min(last);
        self.active_tab.set(active);
        if let Some(destination) = self.tabs.read().get(active).copied() {
            self.active.set(destination);
        }
    }

    /// Closes any open overlay (`Esc`).
    pub fn close_overlay(&mut self) {
        self.overlay.set(Overlay::None);
    }
}
