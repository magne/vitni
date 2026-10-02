//! The assisted-import wizard's match stage (ADR 0040 §4; `import.html`): the host's own stage, shown
//! when a record being imported may already be in the tree. It puts one pair to the user in the shared
//! compare view — the stored record on the left, the side a *Same* keeps — and answers the host with
//! *Same*, *Not the same* or *Decide later*, or leaves the record (*Skip record*) or the session
//! (*Cancel import*). The plugin neither sees nor drives it.

use vitni_app::{MatchReply, MatchableKind};
use vitni_ui::{CompareDecision, MatchStageVm, PairJudgment};

use super::prelude::*;
use crate::i18n::Chrome;
use crate::screens::{DecisionDraft, MatchCompare, decision_foot};

/// The match stage's labels, already localized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchStageLabels {
    /// Which possible match this is: "Possible match 1 of 2".
    pub position: String,
    /// The question, per kind: "Is this person already in your tree?".
    pub heading: String,
    /// What a *Same* does to the stored record.
    pub left_caption: String,
    /// Where the incoming record comes from.
    pub right_caption: String,
    /// *Skip record*.
    pub skip: String,
    /// *Cancel import*.
    pub cancel: String,
}

/// The labels for `stage` from the chrome catalogue.
#[must_use]
pub fn match_stage_labels(chrome: &Chrome, stage: &MatchStageVm) -> MatchStageLabels {
    MatchStageLabels {
        position: chrome.import_match_position(stage.position, stage.total),
        heading: chrome.import_match_heading(kind_key(stage.kind)),
        left_caption: chrome.import_match_stored_caption(),
        right_caption: chrome.import_match_incoming_caption(),
        skip: chrome.import_match_skip(),
        cancel: chrome.import_match_cancel(),
    }
}

/// The Fluent selector for a kind the host asks about.
pub(crate) fn kind_key(kind: MatchableKind) -> &'static str {
    match kind {
        MatchableKind::Person => "person",
        MatchableKind::Place => "place",
        MatchableKind::Source => "source",
        MatchableKind::Repository => "repository",
        MatchableKind::Family
        | MatchableKind::Event
        | MatchableKind::Citation
        | MatchableKind::Media
        | MatchableKind::Note
        | MatchableKind::Tag => "other",
    }
}

/// The match stage over one pair. The host keys it by the pair, so each pair starts with an empty
/// reason and accepts one answer: a second key or click before the next pair arrives does nothing.
#[component]
pub fn MatchStage(
    /// The pair.
    stage: MatchStageVm,
    /// The stage's labels.
    labels: MatchStageLabels,
    /// The localized confidence choices for the decision foot.
    confidence_options: Vec<SelectChoice>,
    /// Fired once with the user's reply.
    onanswer: EventHandler<MatchReply>,
) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let draft = use_signal(DecisionDraft::default);
    let mut answered = use_signal(|| false);
    let answer = use_callback(move |reply: MatchReply| {
        if answered() {
            return;
        }
        answered.set(true);
        onanswer.call(reply);
    });
    let decided = stage.clone();
    let decide = use_callback(move |decision: CompareDecision| {
        let DecisionDraft { reason, confidence } = draft();
        let judgment = PairJudgment {
            rationale: Some(reason),
            confidence,
            assessment: None,
        };
        answer.call(decided.reply(decision, judgment));
    });
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
                    Button { label: labels.cancel.clone(), onclick: move |_| answer.call(MatchReply::Cancel) }
                    div { class: "spacer" }
                    Button { label: labels.skip.clone(), onclick: move |_| answer.call(MatchReply::Skip) }
                }
            }
        }
    }
}
