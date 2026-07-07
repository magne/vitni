//! The provenance block shown above Save on every edit form (`record-editing.html` §5b): the "why"
//! (rationale), the backing citations, the Evidence Explained axes, and the confidence — captured
//! once per save and applied to every assertion the form emits (operator + timestamp come from the
//! session, never typed). Controlled: the whole block binds to a [`ProvenanceDraft`] signal owned by
//! the form, so the form reads `draft()` when it dispatches the save.

use dioxus::prelude::*;
use genealogy_ui::{
    ConfidenceLevel, EVIDENCE_KINDS, EvidenceAxis, INFORMATION_KINDS, ProvenanceDraft, SOURCE_QUALITIES,
};

use crate::components::SelectChoice;
use crate::shell::focus_trap::keep_typing_local;

/// One evidence-analysis axis select in the block: its accessible name and its options (the first of
/// which is the unset "—"), tagged with the axis it drives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceAxis {
    /// Which analysis axis this select drives.
    pub axis: EvidenceAxis,
    /// The already-localized accessible name (e.g. "Source quality").
    pub aria_label: String,
    /// The options, unset ("—", value "") first, then one per axis value (value = its index).
    pub options: Vec<SelectChoice>,
}

/// The provenance block: rationale · citations · evidence axes · confidence, all bound to `draft`.
///
/// Every visible string arrives already localized; the enum ↔ index mapping for the selects lives
/// here (the option values are indices into [`SOURCE_QUALITIES`] / [`INFORMATION_KINDS`] /
/// [`EVIDENCE_KINDS`] / [`ConfidenceLevel::all`]).
#[component]
pub fn ProvenanceBlock(
    /// The draft this block edits (owned by the form; read back at save time).
    draft: Signal<ProvenanceDraft>,
    /// The block's accessible group name ("Provenance").
    heading: String,
    /// The rationale field label ("Reason for this change").
    reason_label: String,
    /// The rationale field hint ("optional · shown in History").
    reason_hint: String,
    /// The citations-row label ("Citations").
    citations_label: String,
    /// The "Attach citation…" button label (also the id-input's accessible name).
    attach_label: String,
    /// The per-chip detach button's accessible name ("Detach citation").
    detach_label: String,
    /// The confidence select label / accessible name ("Confidence").
    confidence_label: String,
    /// The evidence-row label ("Evidence").
    evidence_label: String,
    /// The five confidence options, index-valued and aligned to [`ConfidenceLevel::all`].
    confidence_options: Vec<SelectChoice>,
    /// The three evidence-analysis axis selects, in display order.
    axes: Vec<ProvenanceAxis>,
) -> Element {
    let mut draft = draft;
    let mut pending = use_signal(String::new);
    let confidence_index = ConfidenceLevel::all()
        .iter()
        .position(|level| *level == draft().confidence)
        .unwrap_or(2)
        .to_string();
    let citations = draft().citations;
    // A `.card` per `record-editing.html` §5b — the block reads as one bounded unit wherever it
    // renders (tab body, create pane, side panel), not a bare run of fields.
    rsx! {
        div { class: "card", role: "group", aria_label: "{heading}",
            div { class: "stack",
            div { class: "field", style: "margin-bottom:0",
                label { r#for: "prov-reason",
                    "{reason_label} "
                    span { class: "faint", "({reason_hint})" }
                }
                input {
                    class: "in",
                    r#type: "text",
                    id: "prov-reason",
                    name: "prov-reason",
                    oninput: move |event| draft.write().rationale = event.value(),
                    onkeydown: move |event| keep_typing_local(&event),
                }
            }
            div { class: "fact-row",
                span { class: "field-label", style: "width:96px;margin:0", "{citations_label}" }
                span { class: "grow wrap",
                    for (index , cid) in citations.iter().enumerate() {
                        span { class: "chip",
                            "❝ {cid} "
                            button {
                                class: "btn sm ghost",
                                r#type: "button",
                                aria_label: "{detach_label}",
                                style: "padding:0 4px",
                                onclick: move |_| {
                                    draft.write().citations.remove(index);
                                },
                                "×"
                            }
                        }
                    }
                    input {
                        class: "in",
                        r#type: "text",
                        value: "{pending}",
                        aria_label: "{attach_label}",
                        style: "width:auto",
                        oninput: move |event| pending.set(event.value()),
                        onkeydown: move |event| {
                            keep_typing_local(&event);
                            if event.key() == Key::Enter {
                                let id = pending().trim().to_owned();
                                if !id.is_empty() {
                                    draft.write().citations.push(id);
                                    pending.set(String::new());
                                }
                            }
                        },
                    }
                    button {
                        class: "btn sm ghost",
                        r#type: "button",
                        onclick: move |_| {
                            let id = pending().trim().to_owned();
                            if !id.is_empty() {
                                draft.write().citations.push(id);
                                pending.set(String::new());
                            }
                        },
                        "❝ {attach_label}"
                    }
                }
            }
            div { class: "fact-row",
                span { class: "field-label", style: "width:96px;margin:0", "{evidence_label}" }
                span { class: "grow wrap",
                    for axis in axes.iter() {
                        {axis_select(draft, axis)}
                    }
                }
            }
            div { class: "fact-row",
                span { class: "field-label", style: "width:96px;margin:0", "{confidence_label}" }
                select {
                    class: "in",
                    style: "width:auto",
                    aria_label: "{confidence_label}",
                    onchange: move |event| {
                        let chosen = event.value().parse::<usize>().ok().and_then(|index| ConfidenceLevel::all().get(index).copied());
                        if let Some(level) = chosen {
                            draft.write().confidence = level;
                        }
                    },
                    for option in confidence_options.iter() {
                        option {
                            value: "{option.value}",
                            selected: option.value == confidence_index,
                            "{option.label}"
                        }
                    }
                }
            }
            }
        }
    }
}

/// Renders one evidence-analysis axis select, mapping the chosen option index back to the axis's
/// enum value (or `None` for the unset "—" option).
fn axis_select(mut draft: Signal<ProvenanceDraft>, axis: &ProvenanceAxis) -> Element {
    let current = match axis.axis {
        EvidenceAxis::Source => draft()
            .source
            .and_then(|value| SOURCE_QUALITIES.iter().position(|q| *q == value)),
        EvidenceAxis::Information => draft()
            .information
            .and_then(|value| INFORMATION_KINDS.iter().position(|k| *k == value)),
        EvidenceAxis::Evidence => draft()
            .evidence
            .and_then(|value| EVIDENCE_KINDS.iter().position(|k| *k == value)),
    };
    let current = current.map(|index| index.to_string()).unwrap_or_default();
    let which = axis.axis;
    let options = axis.options.clone();
    let aria_label = axis.aria_label.clone();
    rsx! {
        select {
            class: "in",
            style: "width:auto",
            aria_label: "{aria_label}",
            onchange: move |event| {
                let index = event.value().parse::<usize>().ok();
                match which {
                    EvidenceAxis::Source => {
                        draft.write().source = index.and_then(|i| SOURCE_QUALITIES.get(i).copied());
                    }
                    EvidenceAxis::Information => {
                        draft.write().information = index.and_then(|i| INFORMATION_KINDS.get(i).copied());
                    }
                    EvidenceAxis::Evidence => {
                        draft.write().evidence = index.and_then(|i| EVIDENCE_KINDS.get(i).copied());
                    }
                }
            },
            for option in options.iter() {
                option { value: "{option.value}", selected: option.value == current, "{option.label}" }
            }
        }
    }
}
