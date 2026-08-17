//! The shared commit path of a detail pane: one hook that owns the retract panel's state and the five
//! callbacks every stored record's pane dispatches through.
//!
//! Before this, each of the 12 detail panes hand-copied the same `on_submit` / `on_undo` /
//! `on_tag_remove` / `on_retract` / `on_retract_confirm` closures, differing only in which `*Edit`
//! enum they named and which `save_*_edit` wrapper they called. [`DetailAggregate`] is that
//! difference, generated per aggregate from the registry (`crate::detail_aggregates`), so the
//! behaviour itself lives once — in [`use_detail_commits`].
//!
//! Tag has no impl: it has no `*Edit` enum at all (see the registry's own note).

use std::future::Future;

use dioxus::prelude::*;
use vitni_ui::{ActionLabel, ProvenanceDraft};

use crate::app::AppState;
use crate::detail_aggregates::for_each_detail_aggregate;
use crate::screens::shared::{RetractSubject, RetractTarget};
use crate::services::Services;
use crate::shell::nav_state::NavState;

/// One detail aggregate's commit wiring: the edit enum its pane dispatches, the two edits the shared
/// callbacks construct themselves, and the save that commits one.
///
/// Implemented on a per-aggregate marker type (`PersonCommits`, `SourceCommits`, …) rather than on the
/// edit enum, so [`use_detail_commits`] names the aggregate and reads its edit type off it.
pub trait DetailAggregate: 'static {
    /// The aggregate's one-command edit enum — what a side panel or a row action dispatches.
    type Edit: 'static;

    /// The edit that retracts `assertion_id` from `human_id`'s change log (`⌘Z`, and the confirm of a
    /// per-row Retract/Detach).
    fn undo(human_id: &str, assertion_id: String) -> Self::Edit;

    /// The edit that removes tag `tag_id` from `human_id` (the confirm of the Tags tab's chip `×`).
    fn untag(human_id: &str, tag_id: String) -> Self::Edit;

    /// Commits one edit, returning the record's effective `human_id` or a localized error.
    fn save(
        services: Services,
        edit: Self::Edit,
        prov: ProvenanceDraft,
    ) -> impl Future<Output = Result<String, String>>;
}

/// The state and callbacks [`use_detail_commits`] hands a detail pane. The pane threads the callbacks
/// into its tabs and side panels, passes `retract`/`retract_reason` to
/// [`retract_side_panel`](crate::screens::retract_side_panel), and subscribes its own `use_resource`
/// to `reload`.
pub struct DetailCommits<A: DetailAggregate> {
    /// The reload counter every successful commit bumps; the pane's detail resource reads it.
    pub reload: Signal<u32>,
    /// The row or tag being corrected, if the retract panel is open.
    pub retract: Signal<Option<RetractTarget>>,
    /// The rationale typed into the open retract panel.
    pub retract_reason: Signal<String>,
    /// Commits one edit, closing the pane's side panel and reloading on success.
    pub on_submit: Callback<(A::Edit, ProvenanceDraft)>,
    /// Retracts an assertion by id (the History tab, and `⌘Z` via
    /// [`use_record_undo`](crate::screens::use_record_undo)).
    pub on_undo: Callback<String>,
    /// Arms the retract panel for a tag: `(tag_id, tag name)`.
    pub on_tag_remove: Callback<(String, String)>,
    /// Arms the retract panel for a row: `(assertion_id, label, detach)`.
    pub on_retract: Callback<(String, String, bool)>,
    /// Confirms the open retract panel — dispatches the undo or the untag with the typed rationale.
    pub on_retract_confirm: Callback<()>,
}

// Every field is a `Signal`/`Callback`, both `Copy` for any payload, so the bundle is `Copy`
// regardless of `A`; deriving either impl would wrongly demand `A: Clone`/`A: Copy` of what is only
// ever a marker type (`RecordEditState`'s precedent).
impl<A: DetailAggregate> Clone for DetailCommits<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: DetailAggregate> Copy for DetailCommits<A> {}

/// The shared commit path of a stored record's detail pane, for aggregate `A` and the pane's own
/// side-panel form enum `E`.
///
/// Owns the reload counter and the retract panel's two signals, and returns the five callbacks that
/// used to be copied into every pane:
///
/// - `on_submit` — commits one edit; on success closes the side panel (`editing`), bumps `reload`, and
///   shows the shell's saved notice. A failure is a sticky error notice and changes nothing else.
/// - `on_undo` — rides `on_submit` with [`DetailAggregate::undo`] and a default provenance: `⌘Z` and
///   the History tab's undo are keystroke-and-click affordances with no panel to type a reason into.
/// - `on_retract` / `on_tag_remove` — arm the panel for a row or a tag, clearing the rationale field
///   first. Neither writes anything; the confirm does.
/// - `on_retract_confirm` — dispatches the armed subject's [`DetailAggregate::undo`] or
///   [`DetailAggregate::untag`] with the typed rationale as the provenance's `rationale` (the
///   correction stays in History — ADR 0004 §2), then closes the panel and reloads.
///
/// The pane keeps what genuinely differs: its `undo_busy` memo, its whole-record `on_record_save`, and
/// any aggregate-specific confirm (see `screens/event.rs`).
#[must_use]
pub fn use_detail_commits<A: DetailAggregate, E: 'static>(
    state: &AppState,
    human_id: &str,
    editing: Signal<Option<E>>,
) -> DetailCommits<A> {
    let nav = use_context::<NavState>();
    let mut reload = use_signal(|| 0_u32);
    let mut retract = use_signal(|| None::<RetractTarget>);
    let mut retract_reason = use_signal(String::new);
    let services = state.services().clone();
    let saved_label = state.data_loc().action_label(ActionLabel::Saved);

    let submit_services = services.clone();
    let submit_saved = saved_label.clone();
    let mut editing_for_submit = editing;
    let mut submit_nav = nav;
    let on_submit = use_callback(move |(edit, prov): (A::Edit, ProvenanceDraft)| {
        let services = submit_services.clone();
        let saved = submit_saved.clone();
        spawn(async move {
            match A::save(services, edit, prov).await {
                Ok(_) => {
                    editing_for_submit.set(None);
                    reload += 1;
                    submit_nav.notify(saved);
                }
                Err(message) => submit_nav.notify_error(message),
            }
        });
    });

    let undo_human = human_id.to_owned();
    let on_undo = use_callback(move |assertion_id: String| {
        on_submit.call((A::undo(&undo_human, assertion_id), ProvenanceDraft::default()));
    });
    // Untag arms the same panel a per-row Retract/Detach does, rather than committing on the click: it
    // is a correction, so the operator gets to say why (issue #315).
    let on_tag_remove = use_callback(move |(tag_id, name): (String, String)| {
        retract_reason.set(String::new());
        retract.set(Some(RetractTarget {
            subject: RetractSubject::Tag { tag_id },
            label: name,
        }));
    });

    let on_retract = use_callback(move |(assertion_id, label, detach): (String, String, bool)| {
        retract_reason.set(String::new());
        retract.set(Some(RetractTarget {
            subject: RetractSubject::Assertion { assertion_id, detach },
            label,
        }));
    });
    let retract_human = human_id.to_owned();
    let mut retract_nav = nav;
    let on_retract_confirm = use_callback(move |()| {
        let Some(RetractTarget { subject, .. }) = retract() else {
            return;
        };
        let services = services.clone();
        let human_id = retract_human.clone();
        let saved = saved_label.clone();
        let prov = ProvenanceDraft {
            rationale: retract_reason(),
            ..ProvenanceDraft::default()
        };
        spawn(async move {
            let edit = match subject {
                RetractSubject::Assertion { assertion_id, .. } => A::undo(&human_id, assertion_id),
                RetractSubject::Tag { tag_id } => A::untag(&human_id, tag_id),
            };
            match A::save(services, edit, prov).await {
                Ok(_) => {
                    retract.set(None);
                    reload += 1;
                    retract_nav.notify(saved);
                }
                Err(message) => retract_nav.notify_error(message),
            }
        });
    });

    DetailCommits {
        reload,
        retract,
        retract_reason,
        on_submit,
        on_undo,
        on_tag_remove,
        on_retract,
        on_retract_confirm,
    }
}

/// Generates the marker type and [`DetailAggregate`] impl of every detail aggregate from the registry.
macro_rules! detail_aggregate_impls {
    ($(($noun:literal, $Commits:ident, $Edit:ty, $save:ident, $dispatch:path)),+ $(,)?) => {
        $(
            #[doc = concat!("The ", $noun, " aggregate's detail-commit wiring (see [`DetailAggregate`]).")]
            pub struct $Commits;

            impl DetailAggregate for $Commits {
                type Edit = $Edit;

                fn undo(human_id: &str, assertion_id: String) -> Self::Edit {
                    // Aliased locally: a variant cannot be named through `Self::Edit` (a qualified
                    // path in a struct expression is unstable) nor through the `ty` metavar directly.
                    type Edit = $Edit;
                    Edit::UndoAssertion {
                        human_id: human_id.to_owned(),
                        assertion_id,
                    }
                }

                fn untag(human_id: &str, tag_id: String) -> Self::Edit {
                    type Edit = $Edit;
                    Edit::Tag {
                        human_id: human_id.to_owned(),
                        tag_id,
                        remove: true,
                    }
                }

                async fn save(
                    services: Services,
                    edit: Self::Edit,
                    prov: ProvenanceDraft,
                ) -> Result<String, String> {
                    crate::services::$save(services, edit, prov).await
                }
            }
        )+
    };
}

for_each_detail_aggregate!(detail_aggregate_impls);
