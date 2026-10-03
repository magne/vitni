//! Similar records on manual entry (ADR 0038 §8; `record-editing.html` §6c, `person.html`): the hint a
//! record being created raises, with *Use existing* and an inline *Compare*, and the *Find similar*
//! panel a stored record opens. Neither ever blocks: the hint sits beside the form, and Save stays the
//! operator's call.

use super::match_compare::MatchCompare;
use super::prelude::*;
use crate::services::load_similar;
use crate::shell::nav_state::{DraftId, NavState};
use vitni_app::DraftRecord;
use vitni_ui::SimilarHitVm;

/// The callbacks a similar record's row fires.
#[derive(Clone, Copy)]
pub struct SimilarCallbacks {
    /// Fired with a hit's record: *Use existing* on a hint, *Open* in the *Find similar* panel.
    pub onuse: Callback<RecordRef>,
    /// Fired with a hit to compare it: the hint toggles its inline compare, the panel opens the Matches
    /// tool's.
    pub oncompare: Callback<SimilarHitVm>,
}

/// The similar-record hint under a record being created: the stored records the engine judges at least
/// possibly the same as `draft`, each with *Compare* and *Use existing*. Renders nothing while `draft`
/// is `None` or like no stored record, and with no app (SSR). A failed look is logged, never shown — the
/// hint is advice, and the form works without it.
#[component]
pub fn SimilarHint(
    /// The record being created, as the engine matches it; `None` while nothing identifying is typed.
    draft: ReadSignal<Option<DraftRecord>>,
    /// Fired with the stored record the operator takes instead.
    onuse: EventHandler<RecordRef>,
) -> Element {
    let ctx = try_consume_context::<AppCtx>();
    let services = match &ctx {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        Some(AppCtx::Failed(_)) | None => None,
    };
    let mut open = use_signal(|| None::<String>);
    let hits = use_resource(move || {
        let draft = draft();
        let services = services.clone();
        async move {
            let (Some(draft), Some(services)) = (draft, services) else {
                return Vec::new();
            };
            match load_similar(services, Intent::SimilarToDraft { draft }).await {
                Ok(hits) => hits,
                Err(error) => {
                    tracing::warn!(%error, "could not look for records similar to the one being created");
                    Vec::new()
                }
            }
        }
    });
    let callbacks = SimilarCallbacks {
        onuse: use_callback(move |record: RecordRef| onuse.call(record)),
        oncompare: use_callback(move |hit: SimilarHitVm| {
            let id = hit.record.human_id;
            let next = (open.peek().as_deref() != Some(id.as_str())).then_some(id);
            open.set(next);
        }),
    };
    let Some(AppCtx::Ready(state)) = &ctx else {
        return rsx! {};
    };
    let hits = hits.read_unchecked().clone().unwrap_or_default();
    let compare = match (open(), draft()) {
        (Some(right), Some(draft)) => rsx! { DraftCompare { draft, right } },
        _ => rsx! {},
    };
    similar_hint_view(state.data_loc(), &hits, open().as_deref(), &compare, callbacks)
}

/// The similar-record hint of a create form: *Use existing* drops the draft and opens the stored record
/// in its place.
pub fn create_form_hint<D: RecordDraft>(draft: Signal<D>, draft_id: DraftId, mut nav: NavState) -> Element {
    rsx! {
        SimilarHint {
            draft: draft.read().similar_draft(),
            onuse: move |record: RecordRef| {
                nav.cancel_draft(draft_id);
                nav.open_record(record);
            },
        }
    }
}

/// The hint's markup over `hits`: one line per hit naming the record and its score, the band, and the
/// two actions; the inline `compare` under the hit whose id is `open`. Nothing when `hits` is empty. Pure,
/// so an SSR test renders it without an app.
pub fn similar_hint_view(
    loc: &Localizer,
    hits: &[SimilarHitVm],
    open: Option<&str>,
    compare: &Element,
    callbacks: SimilarCallbacks,
) -> Element {
    if hits.is_empty() {
        return rsx! {};
    }
    let compare_label = loc.action_button(ActionLabel::Compare);
    let use_label = loc.action_button(ActionLabel::UseExisting);
    rsx! {
        div { class: "similar-hint", role: "status",
            for hit in hits.iter().cloned() {
                div { key: "{hit.record.human_id}", class: "similar-hint-row",
                    span { class: "similar-hint-line", "{hit.line}" }
                    span { class: "badge", "{hit.band}" }
                    span { class: "similar-hint-actions",
                        if hit.compare.is_some() {
                            Button {
                                label: compare_label.clone(),
                                small: true,
                                onclick: {
                                    let hit = hit.clone();
                                    move |_| callbacks.oncompare.call(hit.clone())
                                },
                            }
                        }
                        Button {
                            label: use_label.clone(),
                            small: true,
                            onclick: {
                                let record = hit.record.clone();
                                move |_| callbacks.onuse.call(record.clone())
                            },
                        }
                    }
                }
                if open == Some(hit.record.human_id.as_str()) {
                    div { class: "similar-hint-compare", {compare.clone()} }
                }
            }
        }
    }
}

/// The record being created side by side with the stored record `right`, read-only: there is nothing
/// to decide about until the draft is saved.
#[component]
fn DraftCompare(draft: DraftRecord, right: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let services = state.services().clone();
    let data = use_resource(use_reactive!(|(draft, right)| {
        let services = services.clone();
        async move { load_screen(services, Intent::DraftCompare { draft, right }).await }
    }));
    let loc = state.data_loc();
    match &*data.read_unchecked() {
        None => rsx! { p { class: "loading", "{state.chrome().loading()}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::MatchCompare(vm))) => rsx! {
            MatchCompare {
                vm: (**vm).clone(),
                left_caption: loc.record_draft_badge(),
                right_caption: String::new(),
                ondecide: None,
                {rsx! {}}
            }
        },
        Some(ScreenData::Loaded(_)) => rsx! {},
    }
}

/// The *Find similar* action of the record `human_id` of `category`, titled `title`, for its header.
pub fn find_similar_action(loc: &Localizer, category: Category, human_id: &str, title: &str) -> Element {
    let record = RecordRef {
        category,
        human_id: human_id.to_owned(),
        label: title.to_owned(),
    };
    rsx! {
        FindSimilarButton { record, label: loc.action_button(ActionLabel::FindSimilar) }
    }
}

/// The record header's *Find similar* button: opens the [`FindSimilarPanel`] over `record`.
#[component]
pub fn FindSimilarButton(
    /// The record to find records similar to.
    record: RecordRef,
    /// The already-localized label.
    label: String,
) -> Element {
    let mut nav = use_context::<NavState>();
    rsx! {
        Button { label, small: true, onclick: move |_| nav.show_similar(record.clone()) }
    }
}

/// The *Find similar* side panel of the record `human_id` of `category`: the stored records the engine
/// judges at least possibly the same, each with *Open* and — every kind but a tag — *Compare*, which
/// opens the pair in the Matches tool. Shown while [`NavState::similar_panel`] names this record.
#[component]
pub fn FindSimilarPanel(category: Category, human_id: String) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let services = state.services().clone();
    let target = human_id.clone();
    let open = use_memo(move || {
        nav.similar_panel
            .read()
            .as_ref()
            .is_some_and(|record| record.category == category && record.human_id == target)
    });
    let kind = category.matchable_kind();
    let lookup = human_id.clone();
    let data = use_resource(move || {
        let open = open();
        let services = services.clone();
        let human_id = lookup.clone();
        async move {
            let kind = kind.filter(|_| open)?;
            Some(load_similar(services, Intent::FindSimilar { kind, human_id }).await)
        }
    });
    let left = human_id.clone();
    let callbacks = SimilarCallbacks {
        onuse: use_callback(move |record: RecordRef| {
            nav.close_similar();
            nav.open_record(record);
        }),
        oncompare: use_callback(move |hit: SimilarHitVm| {
            if let Some(kind) = hit.compare {
                nav.open_match_compare(kind, left.clone(), hit.record.human_id);
            }
        }),
    };
    let loc = state.data_loc();
    let body = match &*data.read_unchecked() {
        Some(Some(Ok(hits))) => similar_list_view(loc, hits, callbacks),
        Some(Some(Err(message))) => rsx! { p { class: "empty", "{message}" } },
        Some(None) | None => rsx! { p { class: "loading", "{state.chrome().loading()}" } },
    };
    rsx! {
        SidePanel {
            title: loc.similar_panel_title(),
            open: open(),
            close_label: loc.action_label(ActionLabel::Close),
            onclose: move |()| nav.close_similar(),
            footer: rsx! {},
            {body}
        }
    }
}

/// The *Find similar* list over `hits`: each record with its score, band and the engine's reasons, and
/// *Open* / *Compare*; a note when there are none. Pure, so an SSR test renders it without an app.
pub fn similar_list_view(loc: &Localizer, hits: &[SimilarHitVm], callbacks: SimilarCallbacks) -> Element {
    if hits.is_empty() {
        return rsx! { p { class: "empty", "{loc.similar_none()}" } };
    }
    let open_label = loc.action_button(ActionLabel::OpenRecord);
    let compare_label = loc.action_button(ActionLabel::Compare);
    rsx! {
        div { class: "similar-list",
            for hit in hits.iter().cloned() {
                div { key: "{hit.record.human_id}", class: "card similar-item",
                    div { class: "similar-item-head",
                        span { class: "similar-item-title", "{hit.record.label}" }
                        span { class: "row-id", "{hit.record.human_id}" }
                        span { class: "row-score", "{hit.percent}%" }
                        span { class: "badge", "{hit.band}" }
                    }
                    if !hit.reasons.is_empty() {
                        div { class: "match-reasons muted", "{hit.reasons.join(\" · \")}" }
                    }
                    div { class: "similar-hint-actions",
                        Button {
                            label: open_label.clone(),
                            small: true,
                            onclick: {
                                let record = hit.record.clone();
                                move |_| callbacks.onuse.call(record.clone())
                            },
                        }
                        if hit.compare.is_some() {
                            Button {
                                label: compare_label.clone(),
                                small: true,
                                onclick: {
                                    let hit = hit.clone();
                                    move |_| callbacks.oncompare.call(hit.clone())
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
