//! The assisted-import wizard's ready-to-import stage (ADR 0046; `import.html`): the host's own stage,
//! shown before a record that adds to a record already in the tree is written. It lists each such
//! record with what it gains, in the bulk wizard's rows, and answers the host with *Import*, or leaves
//! the record (*Skip record*) or the session (*Cancel import*). The plugin neither sees nor drives it.

use vitni_app::{PlanReply, PlanSummary};

use super::prelude::*;
use crate::i18n::Chrome;
use crate::screens::{PlanRecordGroupLabels, PlanRecords, plan_record_groups};

/// The ready-to-import stage's labels, already localized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlanLabels {
    /// The stage heading: "Ready to import".
    pub heading: String,
    /// The stored records the record adds to, by kind.
    pub records: Vec<PlanRecordGroupLabels>,
    /// *Import*.
    pub import: String,
    /// *Skip record*.
    pub skip: String,
    /// *Cancel import*.
    pub cancel: String,
}

/// The labels for `summary` from the chrome catalogue.
#[must_use]
pub fn import_plan_labels(chrome: &Chrome, summary: &PlanSummary) -> ImportPlanLabels {
    ImportPlanLabels {
        heading: chrome.bulk_import_confirm_heading(),
        records: plan_record_groups(chrome, summary),
        import: chrome.bulk_import_plan_import(),
        skip: chrome.import_match_skip(),
        cancel: chrome.import_match_cancel(),
    }
}

/// The ready-to-import stage. It accepts one answer: a second click before the next stage arrives does
/// nothing.
#[component]
pub fn ImportPlanStage(
    /// The stage's labels.
    labels: ImportPlanLabels,
    /// Fired once with the user's reply.
    onanswer: EventHandler<PlanReply>,
) -> Element {
    let mut answered = use_signal(|| false);
    let answer = use_callback(move |reply: PlanReply| {
        if answered() {
            return;
        }
        answered.set(true);
        onanswer.call(reply);
    });
    rsx! {
        Card {
            h3 { "{labels.heading}" }
            PlanRecords { groups: labels.records.clone(), unfolded: true }
            div { class: "wrap", style: "gap:var(--sp-2);margin-top:var(--sp-3)",
                Button { label: labels.cancel.clone(), onclick: move |_| answer.call(PlanReply::Cancel) }
                div { class: "spacer" }
                Button { label: labels.skip.clone(), onclick: move |_| answer.call(PlanReply::Skip) }
                Button {
                    label: labels.import.clone(),
                    variant: ButtonVariant::Primary,
                    onclick: move |_| answer.call(PlanReply::Import),
                }
            }
        }
    }
}
