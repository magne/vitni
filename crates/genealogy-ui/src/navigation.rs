//! Navigation state and the data-loading intents the renderer dispatches.
//!
//! [`Screen`] is the framework-neutral navigation state a renderer holds. [`Intent`] is a request to
//! load the app data a screen needs; the renderer turns it into a use-case call via
//! [`dispatch`](crate::intent::dispatch). Running a plugin is **not** an intent here: the plugin host
//! sits above this crate (ADR 0008), so a renderer orchestrates it directly and hands the result to
//! [`vocabulary::parse`](crate::vocabulary::parse).

/// Which screen the GUI is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    /// The list of persons in the workspace.
    PersonList,
    /// One person's detail view.
    PersonDetail {
        /// The person's user-facing id (e.g. `I0001`).
        human_id: String,
    },
    /// A panel rendering a form a plugin supplied (ADR 0012).
    PluginPanel,
}

/// A request to load the app data a screen needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// Load the person list.
    ShowList,
    /// Load one person's detail.
    ShowPerson {
        /// The person's user-facing id (e.g. `I0001`).
        human_id: String,
    },
}
