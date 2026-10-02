//! The Compare/merge tool (Phase 5 PR 19; `merge.html`): a possible-duplicates table, and — once a
//! pair is picked — the shared match-compare view ([`MatchCompare`], #411) over the two people.
//! Single-view like [`super::PedigreeScreen`], not the list/detail pair.
//!
//! `PersonsMerged` only records a same-as link on the survivor (data-model §9 keeps both streams), so
//! *Same* is one atomic `vitni_app::merge_persons` call, never a field-by-field reconciliation, and the
//! footer never claims "N relationships re-pointed": references to the merged persona read as the
//! survivor (ADR 0039 §5).
//!
//! *Not the same* is the other decision (ADR 0039 §1): one `vitni_app::distinguish_persons` call. Either
//! decision records the reason, confidence and engine assessment the view shows, and the pair then
//! never returns to the duplicates table. *Decide later* writes nothing and goes back to it. A pair an
//! earlier decision holds distinct shows that decision and offers *Undo "not the same" and merge*
//! (ADR 0039 §4).

use super::match_compare::{DecisionDraft, MatchCompare, decision_foot};
use super::prelude::*;
use super::shared::confidence_choices;
use crate::i18n::Chrome;
use vitni_ui::CompareDecision;

/// The screen's two modes: the duplicates table, or the compare/merge wizard for a chosen pair.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MergeMode {
    Duplicates,
    Compare { surviving: String, merged: String },
}

/// The Compare/merge tool screen.
#[component]
pub fn MergeScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loading = state.chrome().loading();
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();
    let mut mode = use_signal(|| MergeMode::Duplicates);
    let mut draft = use_signal(DecisionDraft::default);
    let mut blocked = use_signal(|| None::<MergeBlockedVm>);
    // A decision in flight: a second one (a quick second key or click) waits for it to land.
    let mut deciding = use_signal(|| false);
    let confidence_options = confidence_choices(state.data_loc());

    let duplicates_services = state.services().clone();
    let duplicates_data = use_resource(move || {
        let services = duplicates_services.clone();
        let _ = nav.data_version.read();
        async move { load_screen(services, Intent::ListDuplicateCandidates).await }
    });
    let compare_services = state.services().clone();
    let compare_data = use_resource(move || {
        let services = compare_services.clone();
        let (surviving, merged) = match mode() {
            MergeMode::Compare { surviving, merged } => (surviving, merged),
            MergeMode::Duplicates => (String::new(), String::new()),
        };
        async move {
            if surviving.trim().is_empty() || merged.trim().is_empty() {
                return None;
            }
            Some(
                load_screen(
                    services,
                    Intent::MergeCompare {
                        surviving_human_id: surviving,
                        merged_human_id: merged,
                    },
                )
                .await,
            )
        }
    });

    let on_cancel = use_callback(move |()| {
        draft.set(DecisionDraft::default());
        blocked.set(None);
        mode.set(MergeMode::Duplicates);
    });
    // The decision is recorded with the assessment the compare view showed (ADR 0039 §2).
    let judgment = move || {
        let assessment = match compare_data.read_unchecked().as_ref() {
            Some(Some(ScreenData::Loaded(IntentOutcome::MergeCompare(vm)))) => Some(vm.assessment.clone()),
            _ => None,
        };
        let DecisionDraft { reason, confidence } = draft();
        PairJudgment {
            rationale: Some(reason),
            confidence,
            assessment,
        }
    };
    // After either decision the pair has left the duplicates table: reset and go back to it.
    let decided = use_callback(move |notice: String| {
        nav.notify(notice);
        draft.set(DecisionDraft::default());
        nav.mark_changed();
        mode.set(MergeMode::Duplicates);
    });
    let on_failure = use_callback(move |failure: MergeFailure| match failure {
        MergeFailure::Blocked(vm) => blocked.set(Some(vm)),
        MergeFailure::Other(message) => nav.notify_error(message),
    });
    let merge_services = state.services().clone();
    let on_merge = use_callback(move |()| {
        let MergeMode::Compare { surviving, merged } = mode() else {
            return;
        };
        if deciding() {
            return;
        }
        let request = MergePersons {
            surviving_human_id: surviving,
            merged_human_id: merged,
            judgment: judgment(),
        };
        let services = merge_services.clone();
        deciding.set(true);
        spawn(async move {
            blocked.set(None);
            let outcome = merge_persons(services, request).await;
            deciding.set(false);
            match outcome {
                Ok(result) => decided.call(result.summary),
                Err(failure) => on_failure.call(failure),
            }
        });
    });
    let distinguish_services = state.services().clone();
    let on_distinguish = use_callback(move |()| {
        let MergeMode::Compare { surviving, merged } = mode() else {
            return;
        };
        if deciding() {
            return;
        }
        let request = DistinguishPersons {
            person_human_id: surviving,
            other_human_id: merged,
            judgment: judgment(),
        };
        let services = distinguish_services.clone();
        deciding.set(true);
        spawn(async move {
            blocked.set(None);
            let outcome = distinguish_persons(services, request).await;
            deciding.set(false);
            match outcome {
                Ok(notice) => decided.call(notice),
                Err(failure) => on_failure.call(failure),
            }
        });
    });
    let undo_merge_services = state.services().clone();
    let on_undo_and_merge = use_callback(move |()| {
        let MergeMode::Compare { surviving, merged } = mode() else {
            return;
        };
        if deciding() {
            return;
        }
        let request = MergePersons {
            surviving_human_id: surviving,
            merged_human_id: merged,
            judgment: judgment(),
        };
        let services = undo_merge_services.clone();
        deciding.set(true);
        spawn(async move {
            blocked.set(None);
            let outcome = undo_distinction_and_merge(services, request).await;
            deciding.set(false);
            match outcome {
                Ok(result) => decided.call(result.summary),
                Err(failure) => on_failure.call(failure),
            }
        });
    });
    let on_decide = use_callback(move |decision: CompareDecision| match decision {
        CompareDecision::Same => on_merge.call(()),
        CompareDecision::Distinct => on_distinguish.call(()),
        CompareDecision::Later => on_cancel.call(()),
    });

    rsx! {
        div { style: "display:flex;flex-direction:column;gap:var(--sp-4)",
            h1 { class: "sr-only", "{chrome.0.rail_label(\"nav-merge\")}" }
            match mode() {
                MergeMode::Duplicates => duplicates_body(&loading, duplicates_data.read_unchecked().as_ref(), mode),
                MergeMode::Compare { .. } => compare_body(
                    &chrome.0,
                    &loading,
                    compare_data.read_unchecked().as_ref(),
                    rsx! {
                        Button { label: chrome.0.merge_back(), small: true, onclick: move |_| on_cancel.call(()) }
                    },
                    CompareCallbacks { decide: on_decide, undo_and_merge: on_undo_and_merge },
                    rsx! {
                        if let Some(vm) = blocked() {
                            {merge_blocked_card(&vm)}
                        }
                        {decision_foot(&chrome.0, confidence_options.clone(), draft, on_decide)}
                    },
                ),
            }
        }
    }
}

/// Renders the duplicates table body: loading/empty/error, or [`DuplicatesTable`] over the loaded
/// candidates.
fn duplicates_body(loading: &str, data: Option<&ScreenData>, mut mode: Signal<MergeMode>) -> Element {
    match data {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::DuplicateCandidates(candidates))) => rsx! {
            DuplicatesTable {
                candidates: candidates.clone(),
                oncompare: move |(surviving, merged): (String, String)| mode
                    .set(MergeMode::Compare { surviving, merged }),
            }
        },
        Some(ScreenData::Loaded(_)) => rsx! {},
    }
}

/// The possible-duplicates table (`merge.html`'s `.tbl`): Record A / Record B / Why / Confidence /
/// a per-row Compare button. Pure over its props (no context needed), so an SSR test can render it
/// directly over a hand-built [`DuplicateCandidateVm`] list.
#[component]
pub fn DuplicatesTable(
    /// The candidate pairs to show, in scan order.
    candidates: Vec<DuplicateCandidateVm>,
    /// Fired with `(surviving_human_id, merged_human_id)` when a row's Compare button is activated.
    oncompare: EventHandler<(String, String)>,
) -> Element {
    let chrome = use_context::<ChromeCtx>();
    if candidates.is_empty() {
        return rsx! { EmptyState { message: chrome.0.merge_empty_duplicates() } };
    }
    rsx! {
        Card {
            div {
                style: "display:flex;align-items:baseline;gap:var(--sp-3);margin-bottom:var(--sp-3)",
                h3 { "{chrome.0.merge_duplicates_heading()}" }
                span { class: "muted", "{chrome.0.merge_duplicates_count(candidates.len())}" }
            }
            Table {
                caption: chrome.0.merge_duplicates_heading(),
                headers: vec![
                    chrome.0.merge_col_record_a(),
                    chrome.0.merge_col_record_b(),
                    chrome.0.merge_col_why(),
                    chrome.0.merge_col_score(),
                    String::new(),
                ],
                for candidate in candidates.iter().cloned() {
                    {
                        let surviving = candidate.a.human_id.clone();
                        let merged = candidate.b.human_id.clone();
                        let compare_label = chrome.0.merge_compare();
                        rsx! {
                            tr {
                                td {
                                    RecordLink {
                                        category: Category::People,
                                        human_id: candidate.a.human_id.clone(),
                                        label: candidate.a.name.clone(),
                                    }
                                }
                                td {
                                    RecordLink {
                                        category: Category::People,
                                        human_id: candidate.b.human_id.clone(),
                                        label: candidate.b.name.clone(),
                                    }
                                }
                                td { class: "muted",
                                    "{candidate.reason}"
                                    if !candidate.reasons.is_empty() {
                                        div { class: "match-reasons", "{candidate.reasons.join(\" · \")}" }
                                    }
                                }
                                td {
                                    span {
                                        class: "badge",
                                        title: chrome.0.merge_score_tooltip(),
                                        "{candidate.score}%"
                                    }
                                }
                                td {
                                    Button {
                                        label: compare_label,
                                        small: true,
                                        onclick: move |_| oncompare.call((surviving.clone(), merged.clone())),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The compare view's two decision paths: a decision a key or button makes, and the earlier-decision
/// notice's *Undo "not the same" and merge*.
#[derive(Clone, Copy)]
struct CompareCallbacks {
    decide: Callback<CompareDecision>,
    undo_and_merge: Callback<()>,
}

/// Renders the compare body: `back`, then the loaded [`MatchCompareVm`]'s heading and the shared
/// compare view, wrapping the earlier distinction (with its undo-and-merge) when one holds and `tail` —
/// the blocked-decision card and the decision foot, built by the screen — so the view's keys reach them.
fn compare_body(
    chrome: &Chrome,
    loading: &str,
    data: Option<&Option<ScreenData>>,
    back: Element,
    callbacks: CompareCallbacks,
    tail: Element,
) -> Element {
    match data {
        None | Some(None) => rsx! { {back} p { class: "loading", "{loading}" } },
        Some(Some(ScreenData::Error(message))) => rsx! { {back} p { class: "empty", "{message}" } },
        Some(Some(ScreenData::Loaded(IntentOutcome::MergeCompare(vm)))) => rsx! {
            {back}
            {merge_compare_heading(chrome, vm)}
            MatchCompare {
                vm: (**vm).clone(),
                left_caption: chrome.merge_survivor_label(),
                right_caption: chrome.merge_persona_label(),
                ondecide: move |decision| callbacks.decide.call(decision),
                div { style: "display:flex;flex-direction:column;gap:var(--sp-4);margin-top:var(--sp-4)",
                    if vm.earlier_decision == Some(PairDecision::Distinct) {
                        {earlier_distinction_card(chrome, callbacks.undo_and_merge)}
                    }
                    {tail}
                }
            }
        },
        Some(Some(ScreenData::Loaded(_))) => rsx! { {back} },
    }
}

/// The compare view's heading (`merge.html`): the pair. Pure over its args, so an SSR test renders it
/// directly.
pub fn merge_compare_heading(chrome: &Chrome, vm: &MatchCompareVm) -> Element {
    rsx! {
        h2 { style: "margin-bottom:var(--sp-1)", "{chrome.merge_wizard_heading(&vm.left.label, &vm.right.label)}" }
    }
}

/// The notice that an earlier decision holds the pair distinct (`merge.html`), offering *Undo "not the
/// same" and merge* (ADR 0039 §4). Pure over its args, so an SSR test renders it directly.
pub fn earlier_distinction_card(chrome: &Chrome, undo_and_merge: Callback<()>) -> Element {
    rsx! {
        div { class: "card", role: "status",
            h3 { "{chrome.merge_earlier_distinct_heading()}" }
            div { class: "muted", style: "font-size:var(--fs-sm)", "{chrome.merge_earlier_distinct_guidance()}" }
            div { style: "margin-top:var(--sp-2)",
                Button { label: chrome.merge_undo_distinction(), onclick: move |_| undo_and_merge.call(()) }
            }
        }
    }
}

/// The blocked-decision card (`merge.html:181-188`): shown when the decision core rejects the merge
/// with a [`MergeConflict`](vitni_ui::MergeBlockedVm), or either decision because the pair is already
/// decided. An error-bordered `role="alert"` card with the localized heading + guidance and the core's
/// own reason detail, when it has one. Pure over `vm` (already
/// localized), so an SSR test renders it directly.
pub fn merge_blocked_card(vm: &MergeBlockedVm) -> Element {
    rsx! {
        div { class: "card blocked", role: "alert",
            h3 { "{vm.heading}" }
            div { class: "muted", style: "font-size:var(--fs-sm)", "{vm.guidance}" }
            if !vm.detail.is_empty() {
                div { class: "mono", style: "margin-top:var(--sp-2);font-size:var(--fs-sm)", "{vm.detail}" }
            }
        }
    }
}
