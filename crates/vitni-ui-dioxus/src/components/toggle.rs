//! Single-state selection primitives (Phase 5 PR35): a boolean [`Switch`] (`role="switch"`) and a
//! single-choice [`RadioGroup`] (`role="radiogroup"` over `role="radio"` options). Each exposes
//! exactly one ARIA state attribute — `aria-checked` — never the multi-select `aria-pressed` the
//! [`RestrictionSet`](super::evidence::RestrictionSet) toggle group carries. Controlled: the value is
//! a prop and edits are forwarded through an `EventHandler`, so the call site owns the state. All
//! visible text is passed in already-localized — these components never call the localizer.

use dioxus::prelude::*;

/// A boolean on/off switch (`role="switch"`). A real `<button>` whose visible `state_text`
/// ("On"/"Off") carries the state so colour is never the only signal, with its accessible name from
/// `label`. Activating it forwards the toggled value (`!checked`) through `ontoggle`.
#[component]
pub fn Switch(
    /// Whether the switch is on (drives `aria-checked` and the design-system colour).
    checked: bool,
    /// The switch's already-localized accessible name.
    label: String,
    /// The already-localized visible state text ("On"/"Off") — the colour-not-alone signal.
    state_text: String,
    /// Fired on activation with the toggled value (`!checked`).
    ontoggle: EventHandler<bool>,
) -> Element {
    rsx! {
        button {
            class: "switch",
            r#type: "button",
            role: "switch",
            aria_checked: if checked { "true" } else { "false" },
            aria_label: "{label}",
            onclick: move |_| ontoggle.call(!checked),
            "{state_text}"
        }
    }
}

/// One choice in a [`RadioGroup`]. String-keyed so the component stays non-generic — the call site
/// maps its own enum to and from these ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioChoice {
    /// The stable id emitted through `onselect` when this choice is picked.
    pub id: String,
    /// The already-localized visible label.
    pub label: String,
}

/// A single-choice radio group (`role="radiogroup"` over `role="radio"` options). Controlled: the
/// selected id is a prop and picks are forwarded through `onselect`. It implements the WAI-ARIA
/// roving-tabindex contract — the selected radio is the single tab stop and Arrow keys move the
/// selection (wrapping), pulling DOM focus to the newly selected radio. A single ARIA state
/// (`aria-checked`) distinguishes it from the multi-select [`RestrictionSet`](super::evidence::RestrictionSet).
#[component]
pub fn RadioGroup(
    /// The group's already-localized accessible name.
    group_label: String,
    /// The available choices, in display order.
    choices: Vec<RadioChoice>,
    /// The selected choice's id.
    selected: String,
    /// Fired with the id of a newly selected choice.
    onselect: EventHandler<String>,
) -> Element {
    let total = choices.len();
    let mut nodes = use_signal(|| vec![None::<MountedEvent>; total]);
    rsx! {
        div {
            class: "resn-set",
            role: "radiogroup",
            aria_label: "{group_label}",
            style: "gap:8px",
            onkeydown: move |event| radio_keys(&event, &choices, &selected, nodes, &onselect),
            for (index , choice) in choices.iter().enumerate() {
                {
                    let checked = choice.id == selected;
                    let id = choice.id.clone();
                    rsx! {
                        button {
                            class: "resn",
                            r#type: "button",
                            role: "radio",
                            aria_checked: if checked { "true" } else { "false" },
                            tabindex: if checked { "0" } else { "-1" },
                            onmounted: move |event| {
                                if let Some(slot) = nodes.write().get_mut(index) {
                                    *slot = Some(event);
                                }
                            },
                            onclick: move |_| onselect.call(id.clone()),
                            "{choice.label}"
                        }
                    }
                }
            }
        }
    }
}

/// Arrow keys move the selection by one, wrapping at the ends; focus follows the selection. Other
/// keys are left untouched so they can bubble.
fn radio_keys(
    event: &KeyboardEvent,
    choices: &[RadioChoice],
    selected: &str,
    nodes: Signal<Vec<Option<MountedEvent>>>,
    onselect: &EventHandler<String>,
) {
    let total = choices.len();
    if total == 0 {
        return;
    }
    let current = choices.iter().position(|choice| choice.id == selected).unwrap_or(0);
    let next = match event.key() {
        Key::ArrowRight | Key::ArrowDown => (current + 1) % total,
        Key::ArrowLeft | Key::ArrowUp => (current + total - 1) % total,
        _ => return,
    };
    let Some(target) = choices.get(next) else {
        return;
    };
    event.prevent_default();
    onselect.call(target.id.clone());
    if let Some(node) = nodes.peek().get(next).and_then(Clone::clone) {
        spawn(async move {
            let _ = node.set_focus(true).await;
        });
    }
}
