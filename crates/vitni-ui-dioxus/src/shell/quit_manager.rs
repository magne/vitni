//! Application quit (`⌘Q`, PR1 §1.4): closes the native window once [`NavState::quit_requested`]
//! bumps (the operator confirmed, or there was nothing unsaved to confirm), and intercepts the close
//! the app did *not* start so it cannot discard unsaved work either (issue #281).
//!
//! `⌘Q` and `⌘W` reach [`NavState`] as webview→Rust IPC messages and route through
//! [`NavState::request_quit`]. A close the **window manager** starts — the titlebar `✕`, a session
//! logout, `wmctrl -c` — never touches that path: it arrives as tao's `WindowEvent::CloseRequested`,
//! which `dioxus-desktop` answers by destroying the webview (and exiting on the last window) in
//! `App::handle_close_requested` (`app.rs:194`). So the handler below has to be what stands in front of
//! it.
//!
//! Three facts of `dioxus-desktop` 0.7.9 make that possible:
//!
//! - **Every `use_wry_event_handler` closure runs first.** `launch.rs:21` calls `app.tick(&window_event)`
//!   — which fans out to the registered handlers (`app.rs:116` → `event_handlers.rs:50`) — *before*
//!   `launch.rs:33` dispatches `handle_close_requested`. Whatever close behaviour this module sets while
//!   handling `CloseRequested` is therefore the behaviour dioxus then reads.
//! - **Hiding is the only lever.** `handle_close_requested` destroys the webview unless the behaviour is
//!   [`WindowCloseBehaviour::WindowHides`], in which case it only calls `set_visible(false)`
//!   (`app.rs:203`). tao has already inhibited GTK's `delete-event`, and no handler of ours runs after
//!   the dispatch, so there is nothing else left to veto with.
//! - **`window.close()` re-enters the same dispatch.** It posts `UserWindowEvent::CloseWindow`, handled
//!   by `handle_close_requested` again (`launch.rs:41`), so the quit effect has to put the behaviour back
//!   to [`WindowCloseBehaviour::WindowCloses`] first — otherwise **Discard all** after a blocked close
//!   would hide the window instead of exiting.
//!
//! The re-show that undoes the hide sits on `Event::MainEventsCleared` rather than in a `use_effect`:
//! both arrive in the *same* event-loop iteration as the `CloseRequested` that hid the window, so the
//! window is back before a frame is presented, where an effect would wait on the next vdom poll.
//!
//! Mounted inside the [`Shell`](super::root::Shell) so the desktop window hook has context, mirroring
//! [`WindowGeometryManager`](super::window_geometry::WindowGeometryManager). The non-desktop build
//! (the SSR interpreter test) compiles an inert no-op — `keyboard.rs`'s dispatcher never links
//! against `dioxus::desktop`, and neither does this module's SSR half.

use dioxus::prelude::*;

/// Watches [`NavState::quit_requested`](crate::shell::nav_state::NavState::quit_requested) and closes
/// the native window on the next bump, and turns a window-manager close into the same confirm the
/// app's own `⌘Q` raises. Renders nothing.
#[cfg(feature = "desktop")]
#[component]
pub fn QuitManager() -> Element {
    use dioxus::desktop::tao::event::{Event, WindowEvent};
    use dioxus::desktop::{WindowCloseBehaviour, use_window, use_wry_event_handler};

    use crate::shell::nav_state::NavState;

    let mut nav = use_context::<NavState>();
    let window = use_window();
    {
        let window = window.clone();
        use_effect(move || {
            // `quit_requested` starts at 0 and is only ever bumped upward, so a positive value always
            // means a quit was requested (never fires from the initial effect run at mount).
            if *nav.quit_requested.read() > 0 {
                // A blocked window-manager close left the behaviour on `WindowHides`; `close()` runs
                // through the same dispatch, so reset it or this quit only hides the window.
                window.set_close_behavior(WindowCloseBehaviour::WindowCloses);
                window.close();
            }
        });
    }

    // Whether the last `CloseRequested` was blocked, and so hid a window that has to come back.
    let mut restore = use_signal(|| false);
    use_wry_event_handler(move |event, _| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => {
            // Set on *every* close request, not only the blocked ones: a cancelled confirm followed by
            // saving everything must close normally rather than hide again.
            if nav.request_window_close() {
                window.set_close_behavior(WindowCloseBehaviour::WindowHides);
                restore.set(true);
            } else {
                window.set_close_behavior(WindowCloseBehaviour::WindowCloses);
            }
        }
        Event::MainEventsCleared if *restore.peek() => {
            restore.set(false);
            window.set_visible(true);
            window.set_focus();
        }
        _ => {}
    });

    rsx! {}
}

/// The non-desktop no-op: the SSR shell test renders the shell without a window to close.
#[cfg(not(feature = "desktop"))]
#[component]
pub fn QuitManager() -> Element {
    rsx! {}
}
