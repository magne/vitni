//! The record-detail router (Phase 5 tabbed-navigation rework): renders the active open record's
//! detail pane, keyed on its category, or a select-record prompt when nothing is open (or the active
//! destination — e.g. the Dashboard — has no detail pane at all). Every aggregate screen's
//! `MasterDetail` renders this as its `detail` slot, so which pane shows follows the active record
//! tab rather than which category screen is mounted (the "keep list, show detail only" navigation
//! model).

use dioxus::prelude::*;
use genealogy_ui::Category;

use crate::shell::ChromeCtx;
use crate::shell::nav_state::{NavState, OpenTab};

use super::citation::{CitationCreateRecord, CitationDetailPane};
use super::dna_match::{DnaMatchCreateRecord, DnaMatchDetailPane};
use super::dna_test::{DnaTestCreateRecord, DnaTestDetailPane};
use super::event::{EventCreateRecord, EventDetailPane};
use super::family::{FamilyCreateRecord, FamilyDetailPane};
use super::media::{MediaCreateRecord, MediaDetailPane};
use super::note::{NoteCreateRecord, NoteDetailPane};
use super::person::{PersonCreateRecord, PersonDetailPane};
use super::place::{PlaceCreateRecord, PlaceDetailPane};
use super::repository::{RepositoryCreateRecord, RepositoryDetailPane};
use super::source::{SourceCreateRecord, SourceDetailPane};
use super::tag::{TagCreateRecord, TagDetailPane};

/// The editor host: routes the active open tab to its content. A saved record shows its aggregate's
/// detail pane; an unsaved draft shows that aggregate's create form ([`draft_pane`]); nothing open
/// shows a select-a-record prompt.
#[component]
pub fn RecordDetail() -> Element {
    let nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    match nav.active_tab() {
        None => rsx! { p { class: "empty", "{chrome.0.record_select_prompt()}" } },
        Some(OpenTab::Draft(category)) => draft_pane(category),
        Some(OpenTab::Saved(record)) => detail_pane(record.category, record.human_id),
    }
}

/// Routes a draft tab to its aggregate's `*CreateRecord` create form, keyed by category so switching
/// draft categories remounts a fresh form. Each create form self-wires to [`NavState`]: Save commits
/// the draft in place ([`NavState::commit_draft`]), Cancel closes it ([`NavState::cancel_draft`]).
fn draft_pane(category: Category) -> Element {
    let key = category.id();
    rsx! {
        div { class: "detail-slot",
            {
                match category {
                    Category::People => rsx! { PersonCreateRecord { key: "{key}" } },
                    Category::Families => rsx! { FamilyCreateRecord { key: "{key}" } },
                    Category::Events => rsx! { EventCreateRecord { key: "{key}" } },
                    Category::Places => rsx! { PlaceCreateRecord { key: "{key}" } },
                    Category::Sources => rsx! { SourceCreateRecord { key: "{key}" } },
                    Category::Citations => rsx! { CitationCreateRecord { key: "{key}" } },
                    Category::Repositories => rsx! { RepositoryCreateRecord { key: "{key}" } },
                    Category::Media => rsx! { MediaCreateRecord { key: "{key}" } },
                    Category::Notes => rsx! { NoteCreateRecord { key: "{key}" } },
                    Category::Tags => rsx! { TagCreateRecord { key: "{key}" } },
                    Category::DnaTests => rsx! { DnaTestCreateRecord { key: "{key}" } },
                    Category::DnaMatches => rsx! { DnaMatchCreateRecord { key: "{key}" } },
                    Category::Dashboard => rsx! {},
                }
            }
        }
    }
}

/// The docked (second) pane: a compact header naming the docked record with an undock control,
/// followed by that record's detail pane. Renders nothing when nothing is docked (the split is only
/// mounted by [`MasterDetail`](crate::master_detail::MasterDetail) while a dock exists, but the guard
/// keeps this component safe to render on its own).
#[component]
pub fn DockedRecordDetail() -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    let Some(record) = nav.docked_record_ref() else {
        return rsx! {};
    };
    rsx! {
        div { class: "docked-head",
            span { class: "docked-title", "{record.label}" }
            button {
                class: "icon-btn",
                r#type: "button",
                aria_label: "{chrome.0.undock_label()}",
                onclick: move |_| nav.undock_record(),
                "✕"
            }
        }
        {detail_pane(record.category, record.human_id)}
    }
}

/// Routes a `(category, human_id)` to its aggregate's detail pane, keyed so a new id remounts it.
///
/// The keyed pane must be a dynamic *child* of a stable root: Dioxus ignores `key` on a component's
/// root and reuses the instance across a record change (keeping the previous record's
/// `use_resource`/effects, so the tab would show the prior record). The `div.detail-slot`
/// (`display: contents`, layout-neutral) keeps the pane a keyed dynamic child so a new `human_id`
/// remounts it. See the plan/root-cause notes.
fn detail_pane(category: Category, human_id: String) -> Element {
    let chrome = use_context::<ChromeCtx>();
    rsx! {
        div { class: "detail-slot",
            {
                match category {
                    Category::People => rsx! { PersonDetailPane { key: "{human_id}", human_id } },
                    Category::Families => rsx! { FamilyDetailPane { key: "{human_id}", human_id } },
                    Category::Events => rsx! { EventDetailPane { key: "{human_id}", human_id } },
                    Category::Places => rsx! { PlaceDetailPane { key: "{human_id}", human_id } },
                    Category::Sources => rsx! { SourceDetailPane { key: "{human_id}", human_id } },
                    Category::Citations => rsx! { CitationDetailPane { key: "{human_id}", human_id } },
                    Category::Repositories => rsx! { RepositoryDetailPane { key: "{human_id}", human_id } },
                    Category::Media => rsx! { MediaDetailPane { key: "{human_id}", human_id } },
                    Category::Notes => rsx! { NoteDetailPane { key: "{human_id}", human_id } },
                    Category::Tags => rsx! { TagDetailPane { key: "{human_id}", id: human_id } },
                    Category::DnaTests => rsx! { DnaTestDetailPane { key: "{human_id}", human_id } },
                    Category::DnaMatches => rsx! { DnaMatchDetailPane { key: "{human_id}", human_id } },
                    Category::Dashboard => rsx! { p { class: "empty", "{chrome.0.record_select_prompt()}" } },
                }
            }
        }
    }
}
