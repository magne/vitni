//! Evidence & provenance cues — the project's differentiators, rendered colour-not-alone (every
//! signal carries a text label or icon, never colour by itself).

use dioxus::prelude::*;
use vitni_ui::{ConfidenceLevel, EvidenceAxis, RestrictionKind};

/// A surety badge: a colour dot plus a text label. A `None` level means no judgment was recorded
/// (ADR 0021 §5) — rendered as faint text with no dot and no `data-level` attribute.
#[component]
pub fn ConfidenceBadge(
    /// The surety level (drives the colour); `None` when no judgment was recorded.
    level: Option<ConfidenceLevel>,
    /// The already-localized level label (the text that makes the colour redundant).
    label: String,
) -> Element {
    match level {
        Some(level) => rsx! {
            span { class: "conf", "data-level": level.data_level(),
                span { class: "dot" }
                "{label}"
            }
        },
        None => rsx! {
            span { class: "conf conf-unset", "{label}" }
        },
    }
}

/// An Evidence Explained axis chip (e.g. "original", "primary", "direct"). The hue is the axis; the
/// text is the value.
#[component]
pub fn EvidenceAxisChip(
    /// The analysis axis (drives the hue).
    axis: EvidenceAxis,
    /// The already-localized axis value.
    label: String,
) -> Element {
    let class = format!("ev {}", axis.css_class());
    rsx! {
        span { class, "{label}" }
    }
}

/// A flag marking a value with no supporting source — icon plus text.
#[component]
pub fn NoSourceFlag(
    /// The already-localized "no source" text.
    label: String,
) -> Element {
    rsx! {
        span { class: "no-source", "⚠ {label}" }
    }
}

/// A clickable source-count link.
#[component]
pub fn SourceLink(
    /// The already-localized text (e.g. "2 sources").
    label: String,
    /// Fired on activation.
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: "src-link",
            r#type: "button",
            style: "border:0;background:none;font:inherit;cursor:pointer",
            onclick: move |event| onclick.call(event),
            "❝ {label}"
        }
    }
}

/// A "why we believe this" popover listing the assertions/citations behind a value.
#[component]
pub fn ProvenancePopover(
    /// The already-localized heading.
    title: String,
    /// The claim rows.
    children: Element,
) -> Element {
    rsx! {
        div { class: "prov",
            h4 { "{title}" }
            {children}
        }
    }
}

/// One choice in a [`RestrictionSet`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictionChoice {
    /// The restriction kind.
    pub kind: RestrictionKind,
    /// The already-localized label.
    pub label: String,
}

/// A multi-select set of privacy restrictions (GEDCOM v7 `RESN`). Controlled: the selected set is a
/// prop and toggles are forwarded via `ontoggle`. Built set-ready for PR4; not yet wired to a
/// record (the core `Restriction` change is a prerequisite).
#[component]
pub fn RestrictionSet(
    /// The available restrictions, in display order.
    choices: Vec<RestrictionChoice>,
    /// The currently selected kinds.
    selected: Vec<RestrictionKind>,
    /// Fired with the kind whose toggle was activated.
    ontoggle: EventHandler<RestrictionKind>,
) -> Element {
    rsx! {
        div { class: "resn-set",
            for choice in choices.iter() {
                {
                    let kind = choice.kind;
                    let pressed = selected.contains(&kind);
                    rsx! {
                        button {
                            class: "resn",
                            "data-kind": kind.data_kind(),
                            aria_pressed: if pressed { "true" } else { "false" },
                            onclick: move |_| ontoggle.call(kind),
                            "{choice.label}"
                        }
                    }
                }
            }
        }
    }
}
