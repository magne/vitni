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
//! `genealogy_app::merge_persons` call, never a field-by-field reconciliation. The footer never claims
//! "N relationships re-pointed" — it reports how many other records still *reference* the merged
//! persona ([`Chrome`]/[`Localizer::merge_result_summary`](genealogy_ui::Localizer)).

use super::prelude::*;
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
    let mut toast = use_signal(|| None::<String>);
    let dismiss_label = state.data_loc().action_label("dismiss");

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

    let on_merge = use_callback(move |request: MergePersons| {
        let services = state.services().clone();
        spawn(async move {
            match merge_persons(services, request).await {
                Ok(result) => {
                    toast.set(Some(result.summary));
                    nav.mark_changed();
                    mode.set(MergeMode::Duplicates);
                }
                Err(message) => toast.set(Some(message)),
            }
        });
    });

    rsx! {
        div { style: "display:flex;flex-direction:column;gap:var(--sp-4)",
            match mode() {
                MergeMode::Duplicates => duplicates_body(&loading, duplicates_data.read_unchecked().as_ref(), mode),
                MergeMode::Compare { surviving, merged } => compare_body(
                    &chrome.0,
                    &loading,
                    compare_data.read_unchecked().as_ref(),
                    &surviving,
                    &merged,
                    mode,
                    on_merge,
                ),
            }
            Toast {
                visible: toast().is_some(),
                message: toast().unwrap_or_default(),
                action_label: Some(dismiss_label),
                onaction: move |_| toast.set(None),
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
                headers: vec![
                    chrome.0.merge_col_record_a(),
                    chrome.0.merge_col_record_b(),
                    chrome.0.merge_col_why(),
                    chrome.0.merge_col_confidence(),
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
                                td { class: "muted", "{candidate.reason}" }
                                td {
                                    ConfidenceBadge { level: candidate.confidence, label: candidate.confidence_label.clone() }
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

/// Renders the compare/merge wizard body: the loaded [`MergeCompareVm`]'s header row and field grid,
/// plus the Cancel/Merge footer. `surviving`/`merged` are the `human_id`s the wizard was opened for
/// (kept outside the resource so the Merge button can reference them without re-parsing the vm).
fn compare_body(
    chrome: &Chrome,
    loading: &str,
    data: Option<&Option<ScreenData>>,
    surviving: &str,
    merged: &str,
    mut mode: Signal<MergeMode>,
    on_merge: Callback<MergePersons>,
) -> Element {
    let back = rsx! {
        Button { label: chrome.merge_back(), small: true, onclick: move |_| mode.set(MergeMode::Duplicates) }
    };
    match data {
        None | Some(None) => rsx! { {back} p { class: "loading", "{loading}" } },
        Some(Some(ScreenData::Error(message))) => rsx! { {back} p { class: "empty", "{message}" } },
        Some(Some(ScreenData::Loaded(IntentOutcome::MergeCompare(vm)))) => {
            let request = MergePersons {
                surviving_human_id: surviving.to_owned(),
                merged_human_id: merged.to_owned(),
            };
            rsx! {
                {back}
                h2 { "{chrome.merge_wizard_heading(& vm.survivor.name, & vm.merged.name)}" }
                MergeCompareGrid { vm: (**vm).clone() }
                div {
                    class: "card",
                    style: "display:flex;align-items:center;gap:var(--sp-4)",
                    Button { label: chrome.merge_cancel(), onclick: move |_| mode.set(MergeMode::Duplicates) }
                    Button {
                        label: chrome.merge_submit(),
                        variant: ButtonVariant::Primary,
                        onclick: move |_| on_merge.call(request.clone()),
                    }
                }
            }
        }
        Some(Some(ScreenData::Loaded(_))) => rsx! { {back} },
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
                MergeFieldRow { field: field.clone(), row_index: index }
            }
        }
    }
}

/// One field row: the field's label, each side's value, and a native radio pair (grouped by the
/// field's own `name`) marking which side currently holds a value — informational only, per the
/// module doc; nothing here mutates which value the merge keeps.
#[component]
fn MergeFieldRow(field: MergeFieldRowVm, row_index: usize) -> Element {
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
                    span { "{field.merged_value.clone().unwrap_or_default()}" }
                }
            }
        }
    }
}
