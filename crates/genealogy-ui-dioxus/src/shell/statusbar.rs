//! The bottom status bar (`contentinfo`): the active record, the open workspace, and build/theme
//! meta on the right.

use dioxus::prelude::*;

use crate::app::AppCtx;
use crate::components::StatusLine;
use crate::shell::ChromeCtx;
use crate::shell::nav_state::NavState;

/// The crate version, shown as build meta.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The shell status bar.
#[component]
pub fn ShellStatusbar() -> Element {
    let chrome = use_context::<ChromeCtx>();
    let nav = use_context::<NavState>();
    let active_label = nav.active_record_ref().map_or_else(
        || chrome.0.rail_label(nav.active.read().label_id()),
        |record| record.label,
    );
    let theme = nav.theme.read().attr();
    let workspace = workspace_name();
    rsx! {
        StatusLine { active_record: Some(active_label),
            if let Some(workspace) = workspace {
                span { "{workspace}" }
            }
            span { "{theme}" }
            span { "v{VERSION}" }
        }
    }
}

/// The open workspace's directory name, if the application state is available (absent in host-free
/// SSR tests).
fn workspace_name() -> Option<String> {
    let AppCtx::Ready(state) = try_consume_context::<AppCtx>()? else {
        return None;
    };
    state
        .services()
        .dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}
