//! Transient feedback: toast notifications.

use dioxus::prelude::*;

/// A toast notification. Controlled: renders nothing unless `visible`. Announced politely so it does
/// not steal focus. An optional action (e.g. Undo) is forwarded via `onaction`.
#[component]
pub fn Toast(
    /// Whether the toast is shown.
    visible: bool,
    /// The already-localized message.
    message: String,
    /// An optional action label (e.g. "Undo").
    #[props(default)]
    action_label: Option<String>,
    /// Fired when the action is activated.
    #[props(default)]
    onaction: Option<EventHandler<MouseEvent>>,
) -> Element {
    if !visible {
        return rsx! {};
    }
    rsx! {
        div { class: "toast", role: "status", aria_live: "polite",
            "{message}"
            if let Some(action_label) = action_label {
                button {
                    class: "src-link",
                    r#type: "button",
                    style: "border:0;background:none;font:inherit;cursor:pointer",
                    onclick: move |event| {
                        if let Some(onaction) = onaction {
                            onaction.call(event);
                        }
                    },
                    "{action_label}"
                }
            }
        }
    }
}
