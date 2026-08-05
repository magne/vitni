//! "Jump back in" recent-list persistence (#205): an eager `use_effect` writes the list on every
//! change, plus a `CloseRequested`/`Destroyed` backstop that reads the signal directly rather than
//! waiting on the effect queue — mirroring [`WindowGeometryManager`](super::window_geometry) and
//! [`QuitManager`](super::quit_manager), the other two places in this crate that install such a handler.
//!
//! `⌘Q` cannot lose the write: it reaches `NavState` as its own webview→Rust IPC message, so by the
//! time it is processed, the effect from whatever mutated `recent` earlier (opening a record is its own,
//! prior IPC message) has already run — the effect queue is flushed as part of handling one message
//! before the next is even looked at. The gap this backstop closes is an **OS/WM-initiated** close —
//! the titlebar ✕, a session logout, `wmctrl -c` — which reaches `CloseRequested` directly from the tao
//! event loop, never through that queue, and so can arrive with the effect still pending. `gui-pass`
//! *can* drive that path — its `wm-close` step sends the toplevel a `WM_DELETE_WINDOW` `ClientMessage`,
//! which GDK dispatches with no window manager on the display — but `recent-survives-quit.toml` still
//! drives `⌘Q`, because it is the recent list surviving an ordinary quit that it is there to prove;
//! `wm-close-confirm.toml` is what covers the `CloseRequested` path itself.
//!
//! Mounted inside the [`Shell`](super::root::Shell) so the desktop window hook has context. The
//! non-desktop build (the SSR interpreter test) compiles an inert no-op so the shell renders host-free.

use dioxus::prelude::*;

/// Persists the "Jump back in" list on every change, and again on window close so an OS/WM-initiated
/// close (outside the app's own IPC queue) can never lose a pending write. Renders nothing.
#[cfg(feature = "desktop")]
#[component]
pub fn RecentPersistenceManager() -> Element {
    use dioxus::desktop::tao::event::{Event, WindowEvent};
    use dioxus::desktop::use_wry_event_handler;
    use genealogy_app::{ConfigStore as _, RecentItem};

    use crate::app::AppCtx;
    use crate::shell::nav_state::NavState;

    let nav = use_context::<NavState>();
    let dir = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().dir.clone()),
        _ => None,
    };

    // The last value written, so the close-hook backstop below never repeats a write the effect
    // already made.
    let mut last_written = use_signal(|| None::<Vec<RecentItem>>);

    // The eager write: fires whenever `recent` changes. Kept as a plain effect, not debounced — the
    // list changes at human pace (opening a record), and a debounce would only widen the loss window
    // this manager exists to close.
    {
        let dir = dir.clone();
        use_effect(move || {
            let recent = nav.recent.read().clone();
            let Some(dir) = &dir else { return };
            if let Err(error) = genealogy_app::FileConfigStore::for_workspace(dir.clone()).store_recent(&recent) {
                tracing::warn!(%error, "could not persist the recent list");
            }
            last_written.set(Some(recent));
        });
    }

    // Only reachable when a close arrives outside the webview's own IPC queue (titlebar ✕, session
    // logout, `wmctrl -c`) with the effect above still pending — a `⌘Q` quit is itself an IPC message,
    // so the effect has always already run by the time this handler would see one (see the module doc).
    use_wry_event_handler(move |event, _| {
        let Event::WindowEvent { event, .. } = event else {
            return;
        };
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                let recent = nav.recent.peek().clone();
                if let Some(dir) = dir.as_ref()
                    && *last_written.peek() != Some(recent.clone())
                {
                    if let Err(error) = genealogy_app::FileConfigStore::for_workspace(dir.clone()).store_recent(&recent)
                    {
                        tracing::warn!(%error, "could not persist the recent list on close");
                    }
                    last_written.set(Some(recent));
                }
            }
            _ => {}
        }
    });

    rsx! {}
}

/// The non-desktop no-op: the SSR shell test renders the shell without a window.
#[cfg(not(feature = "desktop"))]
#[component]
pub fn RecentPersistenceManager() -> Element {
    rsx! {}
}
