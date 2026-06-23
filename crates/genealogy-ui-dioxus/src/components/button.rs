//! Buttons: a labelled action button and an icon-only button.

use dioxus::prelude::*;

/// The visual variant of a [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// The primary, accent-filled action.
    Primary,
    /// A neutral default action.
    Default,
    /// A low-emphasis, borderless action.
    Ghost,
    /// A destructive action.
    Danger,
}

impl ButtonVariant {
    /// The class list for this variant (always includes the base `btn`).
    fn class(self) -> &'static str {
        match self {
            Self::Primary => "btn primary",
            Self::Default => "btn",
            Self::Ghost => "btn ghost",
            Self::Danger => "btn danger",
        }
    }
}

/// A labelled action button. The caller owns any state it mutates; the click is forwarded via
/// `onclick`.
#[component]
pub fn Button(
    /// The visible, already-localized label.
    label: String,
    /// The visual variant; defaults to [`ButtonVariant::Default`].
    #[props(default = ButtonVariant::Default)]
    variant: ButtonVariant,
    /// Whether to render the compact `sm` size.
    #[props(default)]
    small: bool,
    /// Whether the button is disabled.
    #[props(default)]
    disabled: bool,
    /// Fired on click.
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let class = if small {
        format!("{} sm", variant.class())
    } else {
        variant.class().to_owned()
    };
    rsx! {
        button { class, disabled, onclick: move |event| onclick.call(event), "{label}" }
    }
}

/// An icon-only button. Requires an accessible `label`, rendered as `aria-label`.
#[component]
pub fn IconButton(
    /// The glyph or symbol to show.
    icon: String,
    /// The accessible name (already localized), rendered as `aria-label`.
    label: String,
    /// Fired on click.
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: "icon-btn",
            aria_label: "{label}",
            onclick: move |event| onclick.call(event),
            "{icon}"
        }
    }
}
