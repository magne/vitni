//! The bulk-import wizard's Plan and Review stages (ADR 0040 §3, §4; `import.html`).
//!
//! Once the file is read, the host plans it and the wizard shows the plan: what the import would write,
//! kind by kind, and under the counts each record it adds to with what it gains. With possible matches
//! the operator reviews them, or imports them all as new and decides later; without, the plan is
//! imported as it stands. After a review the plan is shown once more, as the answers leave it. The Review stage puts one pair at a time in the
//! shared compare view, as the assisted import's Match stage does, and adds the bulk answers: *Treat all
//! N probable matches of this kind as the same*, and *Decide the rest later*.

use vitni_app::{PlanStep, PlanSummary, PlannedChange, PlannedRecord, ReviewReply};
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

/// One record a plan adds to: its name, its id in the tree and what it gains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRecordLabels {
    /// The incoming record's label, or its id when it has none.
    pub name: String,
    /// The record's human id.
    pub human_id: String,
    /// "updates occupation" or "reused · adds author".
    pub change: String,
}

/// The records of one kind a plan adds to, under a heading: "Persons · 2 records change".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRecordGroupLabels {
    /// The heading.
    pub heading: String,
    /// The records, in plan order.
    pub records: Vec<PlanRecordLabels>,
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
    /// The records each kind adds to, for the kinds that have any.
    pub records: Vec<PlanRecordGroupLabels>,
    /// Whether the record lists are unfolded: they are once the plan is reviewed.
    pub unfolded: bool,
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
        records: record_groups(chrome, summary),
        unfolded: false,
        note,
        primary,
        defer,
        cancel: chrome.bulk_import_plan_cancel(),
    }
}

/// The labels of the plan as the review's answers leave it: its counts and records, unfolded, with
/// *Import* and *Cancel* only.
#[must_use]
pub fn bulk_confirm_labels(chrome: &Chrome, summary: &PlanSummary) -> BulkPlanLabels {
    BulkPlanLabels {
        heading: chrome.bulk_import_confirm_heading(),
        unfolded: true,
        note: None,
        primary: chrome.bulk_import_plan_import(),
        defer: None,
        ..bulk_plan_labels(chrome, summary)
    }
}

/// The records `summary` adds to, grouped by kind in the order its kinds are written.
fn record_groups(chrome: &Chrome, summary: &PlanSummary) -> Vec<PlanRecordGroupLabels> {
    let mut groups = Vec::new();
    for row in &summary.kinds {
        let records: Vec<PlanRecordLabels> = summary
            .records
            .iter()
            .filter(|record| record.kind == row.kind)
            .map(|record| record_labels(chrome, record))
            .collect();
        if records.is_empty() {
            continue;
        }
        groups.push(PlanRecordGroupLabels {
            heading: chrome.bulk_import_plan_records(row.kind.as_str(), records.len()),
            records,
        });
    }
    groups
}

fn record_labels(chrome: &Chrome, record: &PlannedRecord) -> PlanRecordLabels {
    let fields: Vec<String> = record
        .fields
        .iter()
        .map(|field| chrome.bulk_import_plan_field(field))
        .collect();
    let fields = fields.join(", ");
    PlanRecordLabels {
        name: if record.label.is_empty() {
            record.human_id.clone()
        } else {
            record.label.clone()
        },
        human_id: record.human_id.clone(),
        change: match record.change {
            PlannedChange::Updates => chrome.bulk_import_plan_updates(&fields),
            PlannedChange::Reuses => chrome.bulk_import_plan_reuses(&fields),
        },
    }
}

/// The Plan stage: the plan's counts by kind and the records it adds to, and what to do with it. The
/// primary action reviews the possible matches, or imports the plan when it has none; *Cancel* writes
/// nothing.
#[component]
pub fn BulkPlanStage(labels: BulkPlanLabels, onstep: EventHandler<PlanStep>) -> Element {
    rsx! {
        Card {
            PlanBody { labels: labels.clone() }
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

/// The plan as the review's answers leave it: *Import* commits it, *Cancel* writes nothing.
#[component]
pub fn BulkConfirmStage(labels: BulkPlanLabels, onconfirm: EventHandler<bool>) -> Element {
    rsx! {
        Card {
            PlanBody { labels: labels.clone() }
            div { class: "wrap", style: "gap:var(--sp-2);margin-top:var(--sp-3)",
                Button { label: labels.cancel.clone(), onclick: move |_| onconfirm.call(false) }
                div { class: "spacer" }
                Button {
                    label: labels.primary.clone(),
                    variant: ButtonVariant::Primary,
                    onclick: move |_| onconfirm.call(true),
                }
            }
        }
    }
}

/// A plan's heading, counts by kind, the records each kind adds to, and what it comes to.
#[component]
fn PlanBody(labels: BulkPlanLabels) -> Element {
    rsx! {
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
        for group in labels.records.iter() {
            details { class: "plan-records", open: labels.unfolded,
                summary { "{group.heading}" }
                div { class: "stack",
                    for record in group.records.iter() {
                        div { class: "fact-row",
                            span { class: "grow", "{record.name}" }
                            span { class: "badge", "{record.human_id}" }
                            span { class: "muted", "{record.change}" }
                        }
                    }
                }
            }
        }
        if let Some(note) = &labels.note {
            p { class: "muted", role: "status", "{note}" }
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
