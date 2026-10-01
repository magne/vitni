//! The Compare/merge tool (Phase 5 PR 19; `merge.html`): a possible-duplicates table, and — once a
//! pair is picked — a field-by-field compare/merge wizard. Single-view like
//! [`super::PedigreeScreen`], not the list/detail pair.
//!
//! **The re-point decision (state this plainly, it drives every choice below):** `PersonsMerged`
//! only records a same-as link on the survivor (`decide.rs`'s fold pushes the merged id onto the
//! survivor's `merged` list) — data-model §9 explicitly keeps both streams; no core event re-points a
//! Family partner/child slot or a Person association/participation. So the per-field radios here are
//! **informational** ("which record currently holds this value" — [`Chrome::merge_radio_group_label`]),
//! never a granular-apply mechanism: the "Merge" button always performs one atomic
//! `vitni_app::merge_persons` call, never a field-by-field reconciliation. The footer never claims
//! "N relationships re-pointed": references to the merged persona read as the survivor (ADR 0039 §5).
//!
//! *Not the same person* is the other decision (ADR 0039 §1): one `vitni_app::distinguish_persons`
//! call. Either decision records the reason, confidence and engine assessment the wizard shows, and the
//! pair then never returns to the duplicates table. A pair an earlier decision holds distinct shows that
//! decision and offers *Undo "not the same" and merge* (ADR 0039 §4).

use super::prelude::*;
use super::shared::confidence_choices;
use crate::components::SelectInput;
use crate::i18n::Chrome;

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
        let request = MergePersons {
            surviving_human_id: surviving,
            merged_human_id: merged,
            judgment: judgment(),
        };
        let services = merge_services.clone();
        spawn(async move {
            blocked.set(None);
            match merge_persons(services, request).await {
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
        let request = DistinguishPersons {
            person_human_id: surviving,
            other_human_id: merged,
            judgment: judgment(),
        };
        let services = distinguish_services.clone();
        spawn(async move {
            blocked.set(None);
            match distinguish_persons(services, request).await {
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
        let request = MergePersons {
            surviving_human_id: surviving,
            merged_human_id: merged,
            judgment: judgment(),
        };
        let services = undo_merge_services.clone();
        spawn(async move {
            blocked.set(None);
            match undo_distinction_and_merge(services, request).await {
                Ok(result) => decided.call(result.summary),
                Err(failure) => on_failure.call(failure),
            }
        });
    });
    let actions = DecisionActions {
        cancel: on_cancel,
        merge: on_merge,
        distinguish: on_distinguish,
        undo_and_merge: on_undo_and_merge,
    };

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
                    actions,
                    rsx! {
                        if let Some(vm) = blocked() {
                            {merge_blocked_card(&vm)}
                        }
                        {merge_wizard_foot(&chrome.0, confidence_options.clone(), draft, actions)}
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

/// Renders the compare/merge wizard body: `back`, then the loaded [`MergeCompareVm`]'s heading,
/// assessment, the earlier distinction (with `actions`' undo-and-merge) when one holds, and field
/// grid, then `tail` — the blocked-decision card and the decision foot, built by the screen.
fn compare_body(
    chrome: &Chrome,
    loading: &str,
    data: Option<&Option<ScreenData>>,
    back: Element,
    actions: DecisionActions,
    tail: Element,
) -> Element {
    match data {
        None | Some(None) => rsx! { {back} p { class: "loading", "{loading}" } },
        Some(Some(ScreenData::Error(message))) => rsx! { {back} p { class: "empty", "{message}" } },
        Some(Some(ScreenData::Loaded(IntentOutcome::MergeCompare(vm)))) => rsx! {
            {back}
            {merge_compare_heading(chrome, vm)}
            if vm.earlier_decision == Some(PairDecision::Distinct) {
                {earlier_distinction_card(chrome, actions.undo_and_merge)}
            }
            MergeCompareGrid { vm: (**vm).clone() }
            {tail}
        },
        Some(Some(ScreenData::Loaded(_))) => rsx! { {back} },
    }
}

/// The compare wizard's heading (`merge.html`): the pair, then the engine's assessment the decision
/// will record. Pure over its args, so an SSR test renders it directly.
pub fn merge_compare_heading(chrome: &Chrome, vm: &MergeCompareVm) -> Element {
    rsx! {
        h2 { style: "margin-bottom:var(--sp-1)", "{chrome.merge_wizard_heading(&vm.survivor.name, &vm.merged.name)}" }
        p { class: "muted", style: "margin-top:0", "{vm.assessment_line}" }
    }
}

/// What the operator records with either decision: their reason and confidence (ADR 0039 §1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DecisionDraft {
    /// The reason, as typed.
    pub reason: String,
    /// The confidence, or `None` when unset.
    pub confidence: Option<ConfidenceLevel>,
}

/// The compare wizard foot's actions.
#[derive(Clone, Copy, PartialEq)]
pub struct DecisionActions {
    /// Leave the wizard without deciding.
    pub cancel: Callback<()>,
    /// Merge the pair.
    pub merge: Callback<()>,
    /// Record that the pair are different people.
    pub distinguish: Callback<()>,
    /// Undo the earlier decision that the pair are different people, then merge them.
    pub undo_and_merge: Callback<()>,
}

/// The compare/merge wizard's foot (`merge.html`): the reason and confidence recorded with the
/// decision, bound to `draft`, then Cancel, *Not the same person* and *Merge (reversible)*. Pure over
/// its args (the confidence options arrive localized), so an SSR test renders it without an `AppCtx`.
/// A blank reason records no rationale ([`PairJudgment::decision`]).
pub fn merge_wizard_foot(
    chrome: &Chrome,
    confidence_options: Vec<SelectChoice>,
    draft: Signal<DecisionDraft>,
    actions: DecisionActions,
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
            Button { label: chrome.merge_cancel(), onclick: move |_| actions.cancel.call(()) }
            Button { label: chrome.merge_distinguish(), onclick: move |_| actions.distinguish.call(()) }
            Button {
                label: chrome.merge_submit(),
                variant: ButtonVariant::Primary,
                onclick: move |_| actions.merge.call(()),
            }
        }
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

/// The field-by-field compare grid (`merge.html`'s `.merge-grid`): a header row naming both people,
/// then one row per [`MergeFieldRowVm`] with each side's value and a read-only "which side holds
/// this" radio pair. Pure over `vm` (only needs [`ChromeCtx`] from context, mirroring the pedigree
/// tree items), so an SSR test can render it directly over a hand-built [`MergeCompareVm`].
#[component]
pub fn MergeCompareGrid(vm: MergeCompareVm) -> Element {
    let chrome = use_context::<ChromeCtx>();
    rsx! {
        div { class: "card", style: "padding:0",
            div { class: "grid-2", style: "gap:0",
                div { class: "muted", style: "padding:var(--sp-3)", "{vm.survivor.name}" }
                div { class: "muted", style: "padding:var(--sp-3)", "{chrome.0.merge_persona_label()}" }
            }
            for (index , field) in vm.fields.iter().enumerate() {
                MergeFieldRow {
                    field: field.clone(),
                    row_index: index,
                    differs_label: vm.differs_label.clone(),
                    differs_title: vm.differs_title.clone(),
                }
            }
        }
    }
}

/// One field row: the field's label, each side's value, and a native radio pair (grouped by the
/// field's own `name`) marking which side currently holds a value — informational only, per the
/// module doc; nothing here mutates which value the merge keeps.
#[component]
fn MergeFieldRow(field: MergeFieldRowVm, row_index: usize, differs_label: String, differs_title: String) -> Element {
    let chrome = use_context::<ChromeCtx>();
    let group = format!("merge-field-{row_index}");
    let survivor_has_value = field.survivor_value.is_some();
    let merged_has_value = field.merged_value.is_some();
    rsx! {
        div {
            class: "grid-2",
            style: "gap:0;border-top:1px solid var(--line)",
            role: "group",
            "aria-label": "{chrome.0.merge_radio_group_label()}: {field.label}",
            div { style: "padding:var(--sp-3)",
                div { class: "field-label", "{field.label}" }
                label { style: "display:flex;align-items:center;gap:var(--sp-2)",
                    input {
                        r#type: "radio",
                        name: "{group}",
                        checked: survivor_has_value,
                        disabled: !survivor_has_value,
                    }
                    span { "{field.survivor_value.clone().unwrap_or_default()}" }
                    if !survivor_has_value {
                        NoSourceFlag { label: chrome.0.merge_keep_label() }
                    }
                }
            }
            div { style: "padding:var(--sp-3)",
                div { class: "field-label", "{field.label}" }
                label { style: "display:flex;align-items:center;gap:var(--sp-2)",
                    input {
                        r#type: "radio",
                        name: "{group}",
                        checked: !survivor_has_value && merged_has_value,
                        disabled: !merged_has_value,
                    }
                    if field.differs {
                        span { class: "diff", "{field.merged_value.clone().unwrap_or_default()}" }
                        span {
                            class: "badge",
                            style: "border-color:var(--warn);color:var(--warn)",
                            aria_label: "{differs_title}",
                            title: "{differs_title}",
                            "{differs_label}"
                        }
                    } else {
                        span { "{field.merged_value.clone().unwrap_or_default()}" }
                    }
                }
            }
        }
    }
}
