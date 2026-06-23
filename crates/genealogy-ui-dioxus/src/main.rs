//! The `genealogy-gui` binary (ADR 0008): the Dioxus desktop frontend, parallel to `genealogy`.
//!
//! Launching the desktop window needs a system webview, so it is behind the `desktop` feature; a
//! build without it still produces the binary (and the library, components, and tests build with no
//! webview libs). Run with the feature enabled: `cargo run -p genealogy-ui-dioxus --features desktop`.

#[cfg(feature = "desktop")]
fn main() {
    use dioxus::desktop::{Config, WindowBuilder};
    use genealogy_ui_dioxus::app::App;

    tracing_subscriber::fmt::init();

    // Seed the native window with the dark `--bg` (#0f1419) so there is no white flash before the
    // stylesheet paints — the GUI defaults to the dark theme.
    let config = Config::new()
        .with_background_color((15, 20, 25, 255))
        .with_window(WindowBuilder::new().with_title("Genealogy"));
    dioxus::LaunchBuilder::desktop().with_cfg(config).launch(App);
}

#[cfg(not(feature = "desktop"))]
fn main() {
    eprintln!(
        "genealogy-gui was built without the `desktop` feature; rebuild with \
         `--features desktop` (needs a system webview, e.g. libwebkit2gtk) to run the GUI."
    );
}
