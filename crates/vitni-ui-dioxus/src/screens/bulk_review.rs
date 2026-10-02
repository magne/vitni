//! The bulk-import wizard's Plan and Review stages (ADR 0040 §3, §4; `import.html`).
//!
//! Once the file is read, the host plans it and the wizard shows the plan: what the import would write,
//! kind by kind. With possible matches the operator reviews them, or imports them all as new and decides
//! later; without, the plan is imported as it stands. The Review stage puts one pair at a time in the
//! shared compare view, as the assisted import's Match stage does, and adds the bulk answers: *Treat all
//! N probable matches of this kind as the same*, and *Decide the rest later*.

use vitni_app::{PlanStep, PlanSummary, ReviewReply};
use vitni_ui::{CompareDecision, MatchStageVm, PairJudgment, plan_writes_nothing};

use super::prelude::*;
use crate::i18n::Chrome;
use crate::screens::import_match::kind_key;
use crate::screens::{DecisionDraft, MatchCompare, decision_foot};

/// One kind's row of the Plan table: its name and its six counts, in column order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRowLabels {
    /// The kind's name: "Persons".
    pub kind: String,
    /// New, unchanged, updated, already in the tree, possible matches, kept as recorded.
    pub counts: [u32; 6],
}

/// The Plan stage's labels, already localized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkPlanLabels {
    /// The stage heading.
    pub heading: String,
    /// The table's first column heading.
    pub kind_heading: String,
    /// The six count columns' headings.
    pub columns: [String; 6],
    /// One row per kind the file holds.
    pub rows: Vec<PlanRowLabels>,
    /// What the plan comes to: nothing to write, an empty file, or how many records may match.
    pub note: Option<String>,
    /// *Import*, or *Review N possible matches* when the plan has some.
    pub primary: String,
    /// *Decide all later and import*, offered only with possible matches.
    pub defer: Option<String>,
    /// *Cancel*.
    pub cancel: String,
}

/// The Plan stage's labels for `summary`.
#[must_use]
pub fn bulk_plan_labels(chrome: &Chrome, summary: &PlanSummary) -> BulkPlanLabels {
    let rows = summary
        .kinds
        .iter()
        .map(|row| {
            let counts = row.counts;
            PlanRowLabels {
                kind: chrome.bulk_import_plan_kind(row.kind.as_str()),
                counts: [
                    counts.new,
                    counts.unchanged,
                    counts.updated,
                    counts.linked,
                    counts.candidates,
                    counts.withheld,
                ],
            }
        })
        .collect();
    let note = if summary.kinds.is_empty() {
        Some(chrome.bulk_import_plan_empty())
    } else if plan_writes_nothing(summary) {
        Some(chrome.bulk_import_plan_nothing())
    } else if summary.candidates > 0 {
        Some(chrome.bulk_import_plan_matches(summary.candidates))
    } else {
        None
    };
    let (primary, defer) = if summary.candidates > 0 {
        (
            chrome.bulk_import_plan_review(summary.candidates),
            Some(chrome.bulk_import_plan_defer()),
        )
    } else {
        (chrome.bulk_import_plan_import(), None)
    };
    BulkPlanLabels {
        heading: chrome.bulk_import_plan_heading(),
        kind_heading: chrome.bulk_import_plan_kind_heading(),
        columns: chrome.bulk_import_plan_columns(),
        rows,
        note,
        primary,
        defer,
        cancel: chrome.bulk_import_plan_cancel(),
    }
}

/// The Plan stage: the plan's counts by kind, and what to do with it. The primary action reviews the
/// possible matches, or imports the plan when it has none; *Cancel* writes nothing.
#[component]
pub fn BulkPlanStage(labels: BulkPlanLabels, onstep: EventHandler<PlanStep>) -> Element {
    rsx! {
        Card {
            h3 { "{labels.heading}" }
            div { style: "overflow-x:auto",
                table { class: "tbl",
                    thead {
                        tr {
                            th { scope: "col", "{labels.kind_heading}" }
                            for column in labels.columns.iter() {
                                th { scope: "col", class: "num", "{column}" }
                            }
                        }
                    }
                    tbody {
                        for row in labels.rows.iter() {
                            tr {
                                td { "{row.kind}" }
                                for count in row.counts {
                                    td { class: if count == 0 { "num faint" } else { "num" }, "{count}" }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(note) = &labels.note {
                p { class: "muted", role: "status", "{note}" }
            }
            div { class: "wrap", style: "gap:var(--sp-2);margin-top:var(--sp-3)",
                Button { label: labels.cancel.clone(), onclick: move |_| onstep.call(PlanStep::Discard) }
                div { class: "spacer" }
                if let Some(defer) = &labels.defer {
                    Button { label: defer.clone(), onclick: move |_| onstep.call(PlanStep::DeferMatches) }
                }
                Button {
                    label: labels.primary.clone(),
                    variant: ButtonVariant::Primary,
                    onclick: move |_| onstep.call(PlanStep::Review),
                }
            }
        }
    }
}

/// The Review stage's labels, already localized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkReviewLabels {
    /// Which possible match this is: "Possible match 3 of 40".
    pub position: String,
    /// The question, per kind: "Is this person already in your tree?".
    pub heading: String,
    /// What a *Same* does to the stored record.
    pub left_caption: String,
    /// Where the incoming record comes from.
    pub right_caption: String,
    /// The bulk *Same* over the pair's probable group, when it has one.
    pub group: Option<String>,
    /// *Decide the rest later*.
    pub rest: String,
    /// *Cancel import*.
    pub cancel: String,
}

/// The Review stage's labels for `stage`.
#[must_use]
pub fn bulk_review_labels(chrome: &Chrome, stage: &MatchStageVm) -> BulkReviewLabels {
    let kind = kind_key(stage.kind);
    BulkReviewLabels {
        position: chrome.import_match_position(stage.position, stage.total),
        heading: chrome.import_match_heading(kind),
        left_caption: chrome.import_match_stored_caption(),
        right_caption: chrome.bulk_import_review_incoming_caption(),
        group: stage
            .group
            .map(|group| chrome.bulk_import_review_group(group.remaining, kind)),
        rest: chrome.bulk_import_review_rest(),
        cancel: chrome.bulk_import_review_cancel(),
    }
}

/// The Review stage over one pair. The wizard keys it by the pair, so each pair starts with an empty
/// reason and accepts one answer.
#[component]
pub fn BulkReviewStage(
    /// The pair.
    stage: MatchStageVm,
    /// The stage's labels.
    labels: BulkReviewLabels,
    /// The localized confidence choices for the decision foot.
    confidence_options: Vec<SelectChoice>,
    /// Fired once with the operator's reply.
    onanswer: EventHandler<ReviewReply>,
) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let draft = use_signal(DecisionDraft::default);
    let mut answered = use_signal(|| false);
    let answer = use_callback(move |reply: ReviewReply| {
        if answered() {
            return;
        }
        answered.set(true);
        onanswer.call(reply);
    });
    let judgment = move || {
        let DecisionDraft { reason, confidence } = draft();
        PairJudgment {
            rationale: Some(reason),
            confidence,
            assessment: None,
        }
    };
    let decided = stage.clone();
    let decide = use_callback(move |decision: CompareDecision| {
        answer.call(decided.review_reply(decision, judgment()));
    });
    let grouped = stage.clone();
    rsx! {
        p { class: "muted", style: "margin:0", "{labels.position}" }
        h3 { style: "margin:0", "{labels.heading}" }
        MatchCompare {
            vm: stage.compare.clone(),
            left_caption: labels.left_caption.clone(),
            right_caption: labels.right_caption.clone(),
            ondecide: move |decision| decide.call(decision),
            div { style: "display:flex;flex-direction:column;gap:var(--sp-4);margin-top:var(--sp-4)",
                {decision_foot(&chrome, confidence_options.clone(), draft, decide)}
                div { class: "wrap", style: "gap:var(--sp-2)",
                    Button {
                        label: labels.cancel.clone(),
                        onclick: move |_| answer.call(ReviewReply::Cancel),
                    }
                    div { class: "spacer" }
                    Button {
                        label: labels.rest.clone(),
                        onclick: move |_| answer.call(ReviewReply::DeferRest),
                    }
                    if let Some(group) = &labels.group {
                        Button {
                            label: group.clone(),
                            onclick: move |_| answer.call(grouped.group_reply(judgment())),
                        }
                    }
                }
            }
        }
    }
}
