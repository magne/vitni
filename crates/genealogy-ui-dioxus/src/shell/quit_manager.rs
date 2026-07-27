//! Application quit (`⌘Q`, PR1 §1.4): closes the native window once [`NavState::quit_requested`]
//! bumps (the operator confirmed, or there was nothing unsaved to confirm).
//!
//! Mounted inside the [`Shell`](super::root::Shell) so the desktop window hook has context, mirroring
//! [`WindowGeometryManager`](super::window_geometry::WindowGeometryManager). The non-desktop build
//! (the SSR interpreter test) compiles an inert no-op — `keyboard.rs`'s dispatcher never links
//! against `dioxus::desktop`, and neither does this module's SSR half.

use dioxus::prelude::*;

/// Watches [`NavState::quit_requested`](crate::shell::nav_state::NavState::quit_requested) and closes
/// the native window on the next bump. Renders nothing.
#[cfg(feature = "desktop")]
#[component]
pub fn QuitManager() -> Element {
    use dioxus::desktop::use_window;

    use crate::shell::nav_state::NavState;

    let nav = use_context::<NavState>();
    let window = use_window();
    use_effect(move || {
        // `quit_requested` starts at 0 and is only ever bumped upward, so a positive value always
        // means a quit was requested (never fires from the initial effect run at mount).
        if *nav.quit_requested.read() > 0 {
            window.close();
        }
    });
    rsx! {}
}

/// The non-desktop no-op: the SSR shell test renders the shell without a window to close.
#[cfg(not(feature = "desktop"))]
#[component]
pub fn QuitManager() -> Element {
    rsx! {}
}
