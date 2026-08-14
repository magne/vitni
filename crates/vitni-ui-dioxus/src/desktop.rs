//! The desktop entry point (ADR 0035 §1): the window/theme setup the `vitni` launcher calls when it
//! is invoked with no arguments.
//!
//! Opening the window needs a system webview, so the real body is behind the `desktop` feature; a
//! build without it still exposes [`run_desktop`] (the library, components, and tests build with no
//! webview libs) and says so instead of opening a window. Run the GUI with `cargo run -p vitni`.

/// The window size used when a workspace has no saved geometry (logical pixels).
#[cfg(feature = "desktop")]
const DEFAULT_WINDOW_SIZE: (f64, f64) = (1280.0, 840.0);

/// Opens the desktop window and runs the app until it closes.
///
/// Must be called on the process main thread: `dioxus::desktop` drives `tao`, which requires it. The
/// launcher's `main` is sync for that reason (ADR 0035 §3).
#[cfg(feature = "desktop")]
pub fn run_desktop() {
    use dioxus::desktop::tao::dpi::LogicalSize;
    use dioxus::desktop::{Config, WindowBuilder};

    use crate::app::{App, resolve_startup_prefs, scripts_head, styles_head};

    tracing_subscriber::fmt::init();

    // Resolve the persisted theme + window geometry before the window opens so the first paint uses
    // the right palette and the window restores to its last size/state.
    let prefs = resolve_startup_prefs();

    // Restore the saved size (else the default), and maximized state.
    let (width, height) = prefs
        .geometry
        .map_or(DEFAULT_WINDOW_SIZE, |g| (f64::from(g.width), f64::from(g.height)));
    let mut window = WindowBuilder::new()
        .with_title("Vitni")
        .with_inner_size(LogicalSize::new(width, height));
    if prefs.geometry.is_some_and(|g| g.maximized) {
        window = window.with_maximized(true);
    }
    // Saved position is applied post-mount (`WindowGeometryManager`), where monitor layout is known
    // and an off-screen window can be recentred.

    // Inject the design-system CSS into the index `<head>` and seed the native background with the
    // resolved theme's `--bg`, so there is no white flash or flash-of-unstyled-content on launch.
    let config = Config::new()
        .with_custom_head(format!("{}{}", styles_head(), scripts_head()))
        .with_background_color(prefs.resolved_theme.background_rgba())
        .with_window(window);
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .with_context(prefs)
        .launch(App);
}

/// Reports that this build has no renderer, instead of opening a window.
#[cfg(not(feature = "desktop"))]
pub fn run_desktop() {
    eprintln!(
        "vitni was built with no GUI renderer; rebuild the launcher with its default `gui` \
         feature (needs a system webview, e.g. libwebkit2gtk) to open a window. Terminal \
         commands work in this build: try `vitni --help`."
    );
}
