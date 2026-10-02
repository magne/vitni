//! The Matches tool (ADR 0039 §3; `matches.html`): the possible-matches queue — every undecided pair the
//! matching engine proposes, of every kind but tags, filtered by import run, kind and band — and, once a
//! pair is picked, the shared match-compare view ([`MatchCompare`], #411) over the two records.
//! Single-view like [`super::PedigreeScreen`], not the list/detail pair.
//!
//! A merge only records a same-as link on the survivor (data-model §9 keeps both streams), so *Same*
//! is one atomic `vitni_app::decide_match` call, never a field-by-field reconciliation, and the footer
//! never claims "N relationships re-pointed": references to the merged record read as the survivor
//! (ADR 0039 §5).
//!
//! *Not the same* is the other decision (ADR 0039 §1). Either decision records the reason, confidence
//! and engine assessment the view shows, and the pair then never returns to the queue. *Decide later*
//! writes nothing and goes back to it. A pair an earlier decision holds distinct shows that decision
//! and offers *Undo "not the same" and merge* (ADR 0039 §4).

use super::match_compare::{DecisionDraft, MatchCompare, decision_foot};
use super::prelude::*;
use super::shared::confidence_choices;
use crate::i18n::Chrome;
use vitni_app::{DecidableKind, MatchBand, MatchQueueFilter};
use vitni_ui::{CompareDecision, RunOptionVm};

/// The screen's two modes: the queue, or the compare view over a chosen pair.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MatchesMode {
    Queue,
    Compare {
        kind: DecidableKind,
        left: String,
        right: String,
    },
}

/// The queue's filter when the tool opens: every pair of every kind, from every run.
const EVERY_MATCH: MatchQueueFilter = MatchQueueFilter {
    run: None,
    kind: None,
    min_band: MatchBand::Possible,
};

/// The Matches tool screen.
#[component]
pub fn MatchesScreen() -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let loading = state.chrome().loading();
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();
    let mut mode = use_signal(|| MatchesMode::Queue);
    let filter = use_signal(|| EVERY_MATCH);
    let mut draft = use_signal(DecisionDraft::default);
    let mut blocked = use_signal(|| None::<MergeBlockedVm>);
    // A decision in flight: a second one (a quick second key or click) waits for it to land.
    let mut deciding = use_signal(|| false);
    let confidence_options = confidence_choices(state.data_loc());

    let queue_services = state.services().clone();
    let queue_data = use_resource(move || {
        let services = queue_services.clone();
        let _ = nav.data_version.read();
        let filter = filter();
        async move { load_screen(services, Intent::ListMatches { filter }).await }
    });
    let compare_services = state.services().clone();
    let compare_data = use_resource(move || {
        let services = compare_services.clone();
        let mode = mode();
        async move {
            let MatchesMode::Compare { kind, left, right } = mode else {
                return None;
            };
            Some(load_screen(services, Intent::MatchCompare { kind, left, right }).await)
        }
    });

    let on_cancel = use_callback(move |()| {
        draft.set(DecisionDraft::default());
        blocked.set(None);
        mode.set(MatchesMode::Queue);
    });
    // The decision is recorded with the assessment the compare view showed (ADR 0039 §2).
    let judgment = move || {
        let assessment = match compare_data.read_unchecked().as_ref() {
            Some(Some(ScreenData::Loaded(IntentOutcome::MatchCompare(vm)))) => Some(vm.assessment.clone()),
            _ => None,
        };
        let DecisionDraft { reason, confidence } = draft();
        PairJudgment {
            rationale: Some(reason),
            confidence,
            assessment,
        }
    };
    // After either decision the pair has left the queue: reset and go back to it.
    let decided = use_callback(move |notice: String| {
        nav.notify(notice);
        draft.set(DecisionDraft::default());
        nav.mark_changed();
        mode.set(MatchesMode::Queue);
    });
    let on_failure = use_callback(move |failure: MergeFailure| match failure {
        MergeFailure::Blocked(vm) => blocked.set(Some(vm)),
        MergeFailure::Other(message) => nav.notify_error(message),
    });
    let decide_services = state.services().clone();
    let on_record = use_callback(move |decision: MatchDecision| {
        let MatchesMode::Compare { kind, left, right } = mode() else {
            return;
        };
        if deciding() {
            return;
        }
        let request = DecideMatch {
            kind,
            left,
            right,
            decision,
            judgment: judgment(),
        };
        let services = decide_services.clone();
        deciding.set(true);
        spawn(async move {
            blocked.set(None);
            let outcome = decide_match(services, request).await;
            deciding.set(false);
            match outcome {
                Ok(notice) => decided.call(notice),
                Err(failure) => on_failure.call(failure),
            }
        });
    });
    let on_decide = use_callback(move |decision: CompareDecision| match decision {
        CompareDecision::Same => on_record.call(MatchDecision::Same),
        CompareDecision::Distinct => on_record.call(MatchDecision::Distinct),
        CompareDecision::Later => on_cancel.call(()),
    });
    let on_undo_and_merge = use_callback(move |()| on_record.call(MatchDecision::UndoDistinctionAndSame));
    let right_caption = match mode() {
        MatchesMode::Compare {
            kind: DecidableKind::Person,
            ..
        } => chrome.0.merge_persona_label(),
        MatchesMode::Compare { .. } | MatchesMode::Queue => chrome.0.merge_merged_label(),
    };

    rsx! {
        div { style: "display:flex;flex-direction:column;gap:var(--sp-4)",
            h1 { class: "sr-only", "{chrome.0.rail_label(\"nav-matches\")}" }
            match mode() {
                MatchesMode::Queue => queue_body(
                    &chrome.0,
                    state.data_loc(),
                    &loading,
                    queue_data.read_unchecked().as_ref(),
                    filter,
                    mode,
                ),
                MatchesMode::Compare { .. } => compare_body(
                    &chrome.0,
                    &loading,
                    compare_data.read_unchecked().as_ref(),
                    rsx! {
                        Button { label: chrome.0.merge_back(), small: true, onclick: move |_| on_cancel.call(()) }
                    },
                    CompareCallbacks { decide: on_decide, undo_and_merge: on_undo_and_merge, right_caption },
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

/// Renders the queue body: the filters over the loaded runs, then loading/error, or [`MatchesTable`]
/// over the loaded pairs.
fn queue_body(
    chrome: &Chrome,
    loc: &Localizer,
    loading: &str,
    data: Option<&ScreenData>,
    mut filter: Signal<MatchQueueFilter>,
    mut mode: Signal<MatchesMode>,
) -> Element {
    let queue = match data {
        None => return rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => return rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::MatchQueue(queue))) => queue,
        Some(ScreenData::Loaded(_)) => return rsx! {},
    };
    rsx! {
        MatchFilters {
            choices: filter_choices(chrome, loc, &queue.runs),
            runs: queue.runs.clone(),
            filter: filter(),
            onchange: move |changed| filter.set(changed),
        }
        MatchesTable {
            pairs: queue.pairs.clone(),
            oncompare: move |(kind, left, right): (DecidableKind, String, String)| mode
                .set(MatchesMode::Compare { kind, left, right }),
        }
    }
}

/// The already-localized choices of the three filters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterChoices {
    /// Every run, then each import run by its source and day; a run's value is its id.
    pub runs: Vec<SelectChoice>,
    /// Every kind, then each decidable kind; a kind's value is its aggregate type name.
    pub kinds: Vec<SelectChoice>,
    /// The two bands a pair may be filtered to at least.
    pub bands: Vec<SelectChoice>,
}

/// The filters' choices over `runs`.
#[must_use]
pub fn filter_choices(chrome: &Chrome, loc: &Localizer, runs: &[RunOptionVm]) -> FilterChoices {
    let mut run_choices = vec![SelectChoice {
        value: String::new(),
        label: chrome.matches_filter_all_runs(),
    }];
    for run in runs {
        run_choices.push(SelectChoice {
            value: run.id.to_string(),
            label: run.label.clone(),
        });
    }
    let mut kinds = vec![SelectChoice {
        value: String::new(),
        label: chrome.matches_filter_all_kinds(),
    }];
    for kind in DecidableKind::ALL {
        kinds.push(SelectChoice {
            value: kind.as_str().to_owned(),
            label: loc.match_kind(kind),
        });
    }
    let mut bands = Vec::new();
    for band in [MatchBand::Possible, MatchBand::Probable] {
        bands.push(SelectChoice {
            value: band_value(band).to_owned(),
            label: loc.match_band(band),
        });
    }
    FilterChoices {
        runs: run_choices,
        kinds,
        bands,
    }
}

/// A band's select value.
fn band_value(band: MatchBand) -> &'static str {
    match band {
        MatchBand::Probable | MatchBand::Deterministic => "probable",
        MatchBand::Possible | MatchBand::Unlikely => "possible",
    }
}

/// The run, kind and band filters above the queue (`matches.html`). Pure over its props, so an SSR
/// test renders it directly.
#[component]
pub fn MatchFilters(
    /// The already-localized choices.
    choices: FilterChoices,
    /// The runs the run choices name, to map a chosen value back to its run.
    runs: Vec<RunOptionVm>,
    /// The current filter.
    filter: MatchQueueFilter,
    /// Fired with the changed filter.
    onchange: EventHandler<MatchQueueFilter>,
) -> Element {
    let chrome = use_context::<ChromeCtx>();
    let selected_run = filter.run.map(|run| run.to_string());
    let selected_kind = filter.kind.map(|kind| kind.as_str().to_owned());
    rsx! {
        Card {
            div { class: "grid-3",
                Select {
                    label: chrome.0.matches_filter_run(),
                    name: "matches-run",
                    value: selected_run,
                    options: choices.runs,
                    onchange: move |event: FormEvent| {
                        let value = event.value();
                        let run = runs.iter().find(|run| run.id.to_string() == value).map(|run| run.id);
                        onchange.call(MatchQueueFilter { run, ..filter });
                    },
                }
                Select {
                    label: chrome.0.matches_filter_kind(),
                    name: "matches-kind",
                    value: selected_kind,
                    options: choices.kinds,
                    onchange: move |event: FormEvent| {
                        onchange.call(MatchQueueFilter { kind: DecidableKind::parse(&event.value()), ..filter });
                    },
                }
                Select {
                    label: chrome.0.matches_filter_band(),
                    name: "matches-band",
                    value: Some(band_value(filter.min_band).to_owned()),
                    options: choices.bands,
                    onchange: move |event: FormEvent| {
                        let min_band = if event.value() == "probable" { MatchBand::Probable } else { MatchBand::Possible };
                        onchange.call(MatchQueueFilter { min_band, ..filter });
                    },
                }
            }
        }
    }
}

/// The possible-matches table (`matches.html`'s `.tbl`): Kind / Record A / Record B / Why / Match score
/// / a per-row Compare button. Pure over its props (no context but the chrome), so an SSR test renders
/// it directly over a hand-built [`QueuedMatchVm`] list.
#[component]
pub fn MatchesTable(
    /// The pairs to show, the most similar first.
    pairs: Vec<QueuedMatchVm>,
    /// Fired with `(kind, left_human_id, right_human_id)` when a row's Compare button is activated.
    oncompare: EventHandler<(DecidableKind, String, String)>,
) -> Element {
    let chrome = use_context::<ChromeCtx>();
    if pairs.is_empty() {
        return rsx! { EmptyState { message: chrome.0.merge_empty_duplicates() } };
    }
    rsx! {
        Card {
            div {
                style: "display:flex;align-items:baseline;gap:var(--sp-3);margin-bottom:var(--sp-3)",
                h3 { "{chrome.0.merge_duplicates_heading()}" }
                span { class: "muted", "{chrome.0.merge_duplicates_count(pairs.len())}" }
            }
            Table {
                caption: chrome.0.merge_duplicates_heading(),
                headers: vec![
                    chrome.0.merge_col_kind(),
                    chrome.0.merge_col_record_a(),
                    chrome.0.merge_col_record_b(),
                    chrome.0.merge_col_why(),
                    chrome.0.merge_col_score(),
                    String::new(),
                ],
                for pair in pairs.iter().cloned() {
                    {
                        let kind = pair.kind;
                        let left = pair.a.human_id.clone();
                        let right = pair.b.human_id.clone();
                        let compare_label = chrome.0.merge_compare();
                        rsx! {
                            tr {
                                td { "{pair.kind_label}" }
                                td {
                                    RecordLink {
                                        category: pair.a.category,
                                        human_id: pair.a.human_id.clone(),
                                        label: pair.a.label.clone(),
                                    }
                                }
                                td {
                                    RecordLink {
                                        category: pair.b.category,
                                        human_id: pair.b.human_id.clone(),
                                        label: pair.b.label.clone(),
                                    }
                                }
                                td { class: "muted",
                                    "{pair.band}"
                                    if !pair.reasons.is_empty() {
                                        div { class: "match-reasons", "{pair.reasons.join(\" · \")}" }
                                    }
                                }
                                td {
                                    span {
                                        class: "badge",
                                        title: chrome.0.merge_score_tooltip(),
                                        "{pair.percent}%"
                                    }
                                }
                                td {
                                    Button {
                                        label: compare_label,
                                        small: true,
                                        onclick: move |_| oncompare.call((kind, left.clone(), right.clone())),
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

/// The compare view's decision paths and the right side's caption: a decision a key or button makes,
/// and the earlier-decision notice's *Undo "not the same" and merge*.
#[derive(Clone)]
struct CompareCallbacks {
    decide: Callback<CompareDecision>,
    undo_and_merge: Callback<()>,
    right_caption: String,
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
        Some(Some(ScreenData::Loaded(IntentOutcome::MatchCompare(vm)))) => {
            let CompareCallbacks {
                decide,
                undo_and_merge,
                right_caption,
            } = callbacks;
            rsx! {
                {back}
                {merge_compare_heading(chrome, vm)}
                MatchCompare {
                    vm: (**vm).clone(),
                    left_caption: chrome.merge_survivor_label(),
                    right_caption,
                    ondecide: move |decision| decide.call(decision),
                    div { style: "display:flex;flex-direction:column;gap:var(--sp-4);margin-top:var(--sp-4)",
                        if vm.earlier_decision == Some(PairDecision::Distinct) {
                            {earlier_distinction_card(chrome, undo_and_merge)}
                        }
                        {tail}
                    }
                }
            }
        }
        Some(Some(ScreenData::Loaded(_))) => rsx! { {back} },
    }
}

/// The compare view's heading (`matches.html`): the pair. Pure over its args, so an SSR test renders it
/// directly.
pub fn merge_compare_heading(chrome: &Chrome, vm: &MatchCompareVm) -> Element {
    rsx! {
        h2 { style: "margin-bottom:var(--sp-1)", "{chrome.merge_wizard_heading(&vm.left.label, &vm.right.label)}" }
    }
}

/// The notice that an earlier decision holds the pair distinct (`matches.html`), offering *Undo "not
/// the same" and merge* (ADR 0039 §4). Pure over its args, so an SSR test renders it directly.
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

/// The blocked-decision card (`matches.html`): shown when the decision core rejects a merge with a
/// conflict, or either decision because the pair is already decided. An error-bordered `role="alert"`
/// card with the localized heading + guidance and the core's own reason detail, when it has one. Pure
/// over `vm` (already localized), so an SSR test renders it directly.
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
