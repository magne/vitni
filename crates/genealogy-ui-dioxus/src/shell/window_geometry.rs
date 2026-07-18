//! Native-window geometry persistence (ADR 0005): restore the saved size/position/maximized state
//! on startup, recentre an off-screen window when the monitor layout changed, and persist the live
//! geometry back to the workspace manifest (debounced, plus a flush on close).
//!
//! Mounted inside the [`Shell`](super::root::Shell) so the desktop window hooks have context. The
//! non-desktop build (the SSR interpreter test) compiles an inert no-op so the shell renders
//! host-free.

use dioxus::prelude::*;

/// Tracks and persists the native-window geometry. Renders nothing.
#[cfg(feature = "desktop")]
#[component]
pub fn WindowGeometryManager() -> Element {
    use std::time::Duration;

    use dioxus::desktop::tao::event::{Event, WindowEvent};
    use dioxus::desktop::{use_window, use_wry_event_handler};
    use genealogy_app::WindowGeometry;

    use crate::app::{AppCtx, StartupPrefs};

    let window = use_window();
    let dir = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().dir.clone()),
        _ => None,
    };
    let saved = try_consume_context::<StartupPrefs>().and_then(|prefs| prefs.geometry);

    // Restore position once monitors are known; recentre if the saved spot is off-screen now.
    {
        let window = window.clone();
        use_effect(move || {
            if let Some(geometry) = saved
                && !geometry.maximized
            {
                restore_position(&window, geometry);
            }
        });
    }

    // The latest observed geometry, captured on every resize/move; persisted on a debounce.
    let mut latest = use_signal(|| None::<WindowGeometry>);
    let mut unsaved = use_signal(|| false);

    {
        let window = window.clone();
        let dir = dir.clone();
        use_wry_event_handler(move |event, _| {
            let Event::WindowEvent { event, .. } = event else {
                return;
            };
            match event {
                WindowEvent::Resized(_) | WindowEvent::Moved(_) => {
                    if let Some(geometry) = current_geometry(&window) {
                        latest.set(Some(geometry));
                        unsaved.set(true);
                    }
                }
                WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                    if let (Some(dir), Some(geometry)) = (dir.as_ref(), *latest.peek()) {
                        persist(dir, geometry);
                        unsaved.set(false);
                    }
                }
                _ => {}
            }
        });
    }

    // Debounced write: flush at most a couple of times a second while the user drags/resizes,
    // instead of on every pixel of movement.
    use_future(move || {
        let dir = dir.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if *unsaved.peek()
                    && let (Some(dir), Some(geometry)) = (dir.as_ref(), *latest.peek())
                {
                    persist(dir, geometry);
                    unsaved.set(false);
                }
            }
        }
    });

    rsx! {}
}

/// Reads the window's current geometry in logical pixels (DPI-independent), or `None` if the
/// position is unavailable.
#[cfg(feature = "desktop")]
fn current_geometry(window: &dioxus::desktop::DesktopContext) -> Option<genealogy_app::WindowGeometry> {
    let scale = window.scale_factor();
    let position = window.outer_position().ok()?.to_logical::<i32>(scale);
    let size = window.inner_size().to_logical::<u32>(scale);
    Some(genealogy_app::WindowGeometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized: window.is_maximized(),
    })
}

/// Applies the saved position, recentring on a visible monitor if no monitor still contains the
/// window's title-bar region (a monitor was removed or rearranged since the geometry was saved).
#[cfg(feature = "desktop")]
fn restore_position(window: &dioxus::desktop::DesktopContext, geometry: genealogy_app::WindowGeometry) {
    use dioxus::desktop::tao::dpi::LogicalPosition;

    let (x, y) = (f64::from(geometry.x), f64::from(geometry.y));
    let on_screen = window.available_monitors().any(|monitor| {
        let scale = monitor.scale_factor();
        let origin = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);
        // A 100x40 logical-px strip around the title bar must overlap the monitor to count as visible.
        x + 100.0 > origin.x && x < origin.x + size.width && y + 40.0 > origin.y && y < origin.y + size.height
    });
    if on_screen {
        window.set_outer_position(LogicalPosition::new(x, y));
        return;
    }
    if let Some(monitor) = window.current_monitor().or_else(|| window.primary_monitor()) {
        let scale = monitor.scale_factor();
        let origin = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);
        let centred_x = origin.x + (size.width - f64::from(geometry.width)) / 2.0;
        let centred_y = origin.y + (size.height - f64::from(geometry.height)) / 2.0;
        window.set_outer_position(LogicalPosition::new(centred_x, centred_y));
    }
}

/// Persists geometry to the workspace manifest, best-effort (a write failure is logged, not surfaced).
#[cfg(feature = "desktop")]
fn persist(dir: &std::path::Path, geometry: genealogy_app::WindowGeometry) {
    use genealogy_app::ConfigStore as _;
    if let Err(error) = genealogy_app::FileConfigStore::for_workspace(dir.to_path_buf()).store_window(geometry) {
        tracing::warn!(%error, "could not persist the window geometry");
    }
}

/// The non-desktop no-op: the SSR shell test renders the shell without a window.
#[cfg(not(feature = "desktop"))]
#[component]
pub fn WindowGeometryManager() -> Element {
    rsx! {}
}
