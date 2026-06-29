//! The application shell (ADR 0008 §5): the navigation rail, top bar, record tabstrip, work area,
//! status bar, and the keyboard layer (central dispatcher, `g`-prefix nav, `⌘K` palette, `?` help).
//!
//! Shell regions share one [`NavState`](nav_state::NavState) via context and read localized chrome
//! through [`ChromeCtx`] (provided by the root [`App`](crate::app::App), or by tests directly), so
//! the regions render host-free in SSR tests.

use std::rc::Rc;

use dioxus::prelude::Resource;
use genealogy_app::WorkspaceCounts;

use crate::i18n::Chrome;

pub mod focus_trap;
pub mod help_overlay;
pub mod keyboard;
pub mod nav_state;
pub mod palette;
pub mod rail;
pub mod root;
pub mod roving;
pub mod statusbar;
pub mod tabstrip;
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
