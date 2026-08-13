//! Transient feedback: toast notifications.

use dioxus::prelude::*;

/// Whether a toast is a routine confirmation (auto-dismissing) or an error (sticky until dismissed).
/// Defined here, beside [`Toast`] itself, and reused by
/// [`Notice`](crate::shell::nav_state::Notice) so the shell's notice model and the presentational
/// surface share one vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastKind {
    /// A routine confirmation (e.g. "Saved") — auto-dismisses after a set time.
    #[default]
    Info,
    /// An error — stays until the operator dismisses it.
    Error,
}

/// A toast notification. Controlled: renders nothing unless `visible`. An info toast is announced
/// politely so it does not steal focus; an error toast is announced assertively (`role="alert"`), the
/// same colour-not-alone convention as the rest of the design system — the class and role carry the
/// distinction, never colour alone. An optional action (e.g. Dismiss) is forwarded via `onaction`.
#[component]
pub fn Toast(
    /// Whether the toast is shown.
    visible: bool,
    /// The already-localized message.
    message: String,
    /// Info (auto-dismissing) or error (sticky).
    #[props(default)]
    kind: ToastKind,
    /// An optional action label (e.g. "Dismiss").
    #[props(default)]
    action_label: Option<String>,
    /// Fired when the action is activated.
    #[props(default)]
    onaction: Option<EventHandler<MouseEvent>>,
) -> Element {
    if !visible {
        return rsx! {};
    }
    let (class, role, live) = match kind {
        ToastKind::Info => ("toast", "status", "polite"),
        ToastKind::Error => ("toast error", "alert", "assertive"),
    };
    rsx! {
        div { class, role, aria_live: live,
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
