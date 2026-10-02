//! The shared match-compare view (#411; ADR 0038 §3, ADR 0039; `merge.html`): two records side by
//! side, one row per term the matching engine compared — both values, a non-colour outcome mark and
//! the term's weighted explanation — under a header naming each side with its origin chip and the scan
//! region it was read from. Every host decides with the same three keys (ADR 0030 §2): `y` *Same*,
//! `n` *Not the same*, `l` *Decide later*, read while focus is anywhere in the view but a text field.
//!
//! The view is kind-neutral: a host builds a [`MatchCompareVm`] for any matchable kind, passes the
//! captions that say what each decision does to its side, and places its own notices and
//! [`decision_foot`] inside, so the keys reach them too.

use super::prelude::*;
use crate::components::SelectInput;
use crate::i18n::Chrome;
use crate::shell::keyboard::shortcut_key;
use vitni_ui::{CompareDecision, CompareRowVm, CompareSideVm, RowOutcome};

/// What the operator records with a decision: their reason and confidence (ADR 0039 §1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DecisionDraft {
    /// The reason, as typed.
    pub reason: String,
    /// The confidence, or `None` when unset.
    pub confidence: Option<ConfidenceLevel>,
}

/// Focuses the mounted compare view without scrolling it to the top of the viewport, which would hide
/// the host's heading and Back button above it. `document::eval` is a no-op under SSR.
const FOCUS_COMPARE: &str = "document.getElementById('match-compare')?.focus({ preventScroll: true });";

/// The match-compare view over `vm`, wrapping the host's `children` (notices and the decision foot).
/// Takes focus when it mounts, so its keys work without a click; `ondecide` fires with the decision
/// a key makes.
#[component]
pub fn MatchCompare(
    /// The pair, its rows and the assessment.
    vm: MatchCompareVm,
    /// What a *Same* decision does to the left record (e.g. "survivor · keeps id").
    left_caption: String,
    /// What a *Same* decision does to the right record (e.g. "becomes a persona").
    right_caption: String,
    /// Fired with the decision a key makes.
    ondecide: EventHandler<CompareDecision>,
    /// The host's notices and decision foot.
    children: Element,
) -> Element {
    let chrome = use_context::<ChromeCtx>();
    rsx! {
        section {
            id: "match-compare",
            class: "match-compare",
            tabindex: "-1",
            aria_label: "{chrome.0.match_compare_label()}",
            onmounted: move |_| {
                spawn(async move {
                    let _ = document::eval(FOCUS_COMPARE).await;
                });
            },
            onkeydown: move |event| {
                if let Some(decision) = decision_for(&event) {
                    event.prevent_default();
                    event.stop_propagation();
                    ondecide.call(decision);
                }
            },
            p { class: "muted", style: "margin:0 0 var(--sp-3)", "{vm.assessment_line}" }
            div { class: "card", style: "padding:0",
                div { class: "merge-grid",
                    {side_header(&vm.left, &left_caption, true)}
                    div { class: "merge-pick", aria_hidden: "true", "⟷" }
                    {side_header(&vm.right, &right_caption, false)}
                    for row in &vm.rows {
                        {compare_row(&chrome.0, row)}
                    }
                }
            }
            {children}
        }
    }
}

/// The decision a key event makes: a bare `y`, `n` or `l` ([`vitni_ui::compare_decision`]).
fn decision_for(event: &KeyboardEvent) -> Option<CompareDecision> {
    let modifiers = event.modifiers();
    let modifier = vitni_ui::Modifier {
        command: modifiers.ctrl() || modifiers.meta(),
        shift: modifiers.shift(),
        alt: modifiers.alt(),
    };
    vitni_ui::compare_decision(shortcut_key(&event.key())?, modifier)
}

/// One side's header cell: label, id, caption, origin chip and evidence snippet.
fn side_header(side: &CompareSideVm, caption: &str, survivor: bool) -> Element {
    let class = if survivor { "merge-col survivor" } else { "merge-col" };
    rsx! {
        div { class,
            div { style: "font-weight:700;font-size:var(--fs-lg)", "{side.label}" }
            div { class: "muted", style: "font-size:var(--fs-xs)",
                span { class: "badge", "{side.human_id}" }
                " {caption}"
            }
            if let Some(origin) = &side.origin {
                div { style: "margin-top:var(--sp-2)",
                    span { class: "chip origin-chip", title: "{origin.title}", "{origin.label}" }
                }
            }
            if let Some(snippet) = &side.evidence {
                div { class: "crop-frame img-frame img-scan evidence-snippet",
                    img { src: "{snippet.src}", alt: "{snippet.caption}", loading: "lazy" }
                    div { class: "crop-outline", style: "{snippet.crop_css}" }
                    span { class: "img-cap", "{snippet.caption}" }
                }
            }
        }
    }
}

/// One compared term: the left value, the outcome mark, the right value with the explanation.
fn compare_row(chrome: &Chrome, row: &CompareRowVm) -> Element {
    let (outcome, glyph) = match row.outcome {
        RowOutcome::Agree => ("agree", "="),
        RowOutcome::Partial => ("partial", "≈"),
        RowOutcome::Disagree => ("disagree", "≠"),
        RowOutcome::Missing => ("missing", "–"),
        RowOutcome::Conflict => ("conflict", "✕"),
    };
    rsx! {
        div { class: "merge-col survivor",
            div { class: "field-label", "{row.feature}" }
            {value_cell(chrome, row.left.as_deref())}
        }
        div { class: "merge-pick",
            span { class: "match-mark", "data-outcome": outcome, title: "{row.outcome_label}",
                span { aria_hidden: "true", "{glyph}" }
                span { class: "sr-only", "{row.outcome_label}" }
            }
        }
        div { class: "merge-col",
            div { class: "field-label", "{row.feature}" }
            {value_cell(chrome, row.right.as_deref())}
            div { class: "match-reasons muted", "{row.explanation}" }
        }
    }
}

/// A side's value, or the muted "not recorded" when it has none.
fn value_cell(chrome: &Chrome, value: Option<&str>) -> Element {
    match value {
        Some(value) => rsx! { div { "{value}" } },
        None => rsx! { div { class: "muted", "{chrome.match_not_recorded()}" } },
    }
}

/// The decision foot (`merge.html`): the reason and confidence recorded with the decision, bound to
/// `draft`, then *Decide later*, *Not the same* and *Same (reversible)*, each showing its key. Pure over
/// its args (the confidence options arrive localized), so an SSR test renders it without an `AppCtx`.
/// A blank reason records no rationale ([`PairJudgment::decision`]).
pub fn decision_foot(
    chrome: &Chrome,
    confidence_options: Vec<SelectChoice>,
    draft: Signal<DecisionDraft>,
    ondecide: Callback<CompareDecision>,
) -> Element {
    let mut draft = draft;
    let DecisionDraft { reason, confidence } = draft();
    let confidence_index = confidence
        .and_then(|level| ConfidenceLevel::all().iter().position(|l| *l == level))
        .map(|index| index.to_string())
        .unwrap_or_default();
    let confidence_label = chrome.merge_confidence_label();
    rsx! {
        div {
            class: "card",
            style: "display:flex;align-items:flex-end;gap:var(--sp-4);flex-wrap:wrap",
            div { class: "field", style: "flex:1;min-width:260px;margin:0",
                label { r#for: "merge-reason",
                    "{chrome.merge_reason_label()} "
                    span { class: "faint", "{chrome.merge_reason_hint()}" }
                }
                TextInput {
                    id: "merge-reason",
                    name: "merge-reason",
                    value: "{reason}",
                    oninput: move |event: FormEvent| draft.write().reason = event.value(),
                }
            }
            div { class: "field", style: "margin:0",
                label { r#for: "merge-confidence", "{confidence_label}" }
                SelectInput {
                    id: "merge-confidence",
                    style: "width:auto",
                    aria_label: "{confidence_label}",
                    selected: confidence_index,
                    options: confidence_options,
                    onchange: move |event: FormEvent| {
                        let index = event.value().parse::<usize>().ok();
                        draft.write().confidence = index.and_then(|i| ConfidenceLevel::all().get(i).copied());
                    },
                }
            }
            div { class: "spacer" }
            Button {
                label: chrome.match_decide_later(),
                shortcut: "l".to_owned(),
                onclick: move |_| ondecide.call(CompareDecision::Later),
            }
            Button {
                label: chrome.match_decide_distinct(),
                shortcut: "n".to_owned(),
                onclick: move |_| ondecide.call(CompareDecision::Distinct),
            }
            Button {
                label: chrome.match_decide_same(),
                shortcut: "y".to_owned(),
                variant: ButtonVariant::Primary,
                onclick: move |_| ondecide.call(CompareDecision::Same),
            }
        }
    }
}
