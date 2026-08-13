//! The application shell (ADR 0008 §5): the navigation rail, top bar, record tabstrip, work area,
//! status bar, and the keyboard layer (central dispatcher, `g`-prefix nav, `⌘K` palette, `?` help).
//!
//! Shell regions share one [`NavState`](nav_state::NavState) via context and read localized chrome
//! through [`ChromeCtx`] (provided by the root [`App`](crate::app::App), or by tests directly), so
//! the regions render host-free in SSR tests.

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::{ReadableExt, Resource, Signal, try_consume_context};
use vitni_app::{ShortcutConfig, WorkspaceCounts};

use crate::i18n::Chrome;

pub mod close_confirm;
pub mod explorer;
pub mod focus_trap;
pub mod help_overlay;
pub mod keyboard;
pub mod nav_state;
pub mod palette;
pub mod quit_manager;
pub mod rail;
pub mod recent_persistence;
pub mod root;
pub mod roving;
pub mod statusbar;
pub mod tab_label;
pub mod tabstrip;
pub mod toast;
pub mod topbar;
pub mod window_geometry;

pub use root::Shell;

/// The chrome localizer, provided as context so every shell region resolves its labels without the
/// full application state (which an SSR test does not build).
#[derive(Clone)]
pub struct ChromeCtx(pub Rc<Chrome>);

/// The workspace per-aggregate counts, provided as context for the rail count badges. The resource
/// refetches when [`NavState::data_version`](nav_state::NavState::data_version) bumps; the outer
/// `None` is "still loading", the inner `None` is "could not load" (no badges shown).
#[derive(Clone, Copy)]
pub struct CountsCtx(pub Resource<Option<WorkspaceCounts>>);

/// The resolution state of a record link's current name.
#[derive(Clone, PartialEq, Eq)]
pub enum NameState {
    /// A resolution is in flight.
    Loading,
    /// Resolved: the record's current name, or `None` when it has none (the link shows the id).
    Ready(Option<String>),
}

/// A resolved (or in-flight) record name, tagged with the [`NavState::data_version`] it was resolved
/// under so a later mutation makes the entry stale (miss → re-resolve) without a global cache clear.
#[derive(Clone)]
pub struct CachedName {
    /// The `data_version` this entry was resolved under.
    pub version: u32,
    /// The resolution state.
    pub state: NameState,
}

/// A shell-wide memo cache of current record names, keyed by `(aggregate kind id, human id)` and
/// shared by every [`RecordLink`](crate::screens::RecordLink) so a name is resolved once per data
/// version rather than per link. Provided by the shell root; absent under bare SSR tests (links then
/// fall back to their supplied label).
#[derive(Clone, Copy)]
pub struct NameCache(pub Signal<HashMap<(String, String), CachedName>>);

/// The live client-scope `[shortcuts]` overrides (ADR 0030 §3), provided by the shell root so saving
/// the Preferences shortcuts card takes effect immediately — the keyboard dispatcher and the `?`
/// overlay both resolve against this signal, not a value frozen at startup.
#[derive(Clone, Copy)]
pub struct ShortcutsCtx(pub Signal<ShortcutConfig>);

/// Reads the live [`ShortcutsCtx`] (provided by the real shell; absent in a bare SSR test of a
/// sub-component) and resolves it against the default map (ADR 0030 §1). Absent context resolves to
/// the unmodified default map.
#[must_use]
pub fn resolved_shortcuts_from_context() -> Vec<vitni_ui::Shortcut> {
    match try_consume_context::<ShortcutsCtx>() {
        Some(ctx) => vitni_ui::resolved_shortcuts(&ctx.0.read().bindings).0,
        None => vitni_ui::shortcuts(),
    }
}
