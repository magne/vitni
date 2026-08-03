//! "Jump back in" recent-list persistence (#205): an eager `use_effect` writes the list on every
//! change, plus a `CloseRequested`/`Destroyed` backstop that reads the signal directly rather than
//! waiting on the effect queue — mirroring [`WindowGeometryManager`](super::window_geometry), whose
//! own `CloseRequested` handler is why geometry never had this gap.
//!
//! `use_effect` runs on the post-render flush, not at mutation time. A quit that closes the window
//! before that flush lands would otherwise lose the write; the wry handler reads
//! [`NavState::recent`](super::nav_state::NavState::recent) with `.peek()` and persists it itself, so
//! the list reaches disk regardless of whether the queued effect got a turn first.
//!
//! Mounted inside the [`Shell`](super::root::Shell) so the desktop window hook has context. The
//! non-desktop build (the SSR interpreter test) compiles an inert no-op so the shell renders host-free.

use dioxus::prelude::*;

/// Persists the "Jump back in" list on every change, and again on window close so a quit can never
/// race the effect queue. Renders nothing.
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
