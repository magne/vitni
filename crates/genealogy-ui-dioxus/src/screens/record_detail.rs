//! The record-detail router (Phase 5 tabbed-navigation rework): renders the active open record's
//! detail pane, keyed on its category, or a select-record prompt when nothing is open (or the active
//! destination — e.g. the Dashboard — has no detail pane at all). Every aggregate screen's
//! `MasterDetail` renders this as its `detail` slot, so which pane shows follows the active record
//! tab rather than which category screen is mounted (the "keep list, show detail only" navigation
//! model).

use dioxus::prelude::*;
use genealogy_ui::Category;

use crate::shell::ChromeCtx;
use crate::shell::nav_state::NavState;

use super::citation::CitationDetailPane;
use super::dna_match::DnaMatchDetailPane;
use super::dna_test::DnaTestDetailPane;
use super::event::EventDetailPane;
use super::family::FamilyDetailPane;
use super::media::MediaDetailPane;
use super::note::NoteDetailPane;
use super::person::PersonDetailPane;
use super::place::PlaceDetailPane;
use super::repository::RepositoryDetailPane;
use super::source::SourceDetailPane;
use super::tag::TagDetailPane;

/// Routes the active open record (if any) to its aggregate's detail pane.
#[component]
pub fn RecordDetail() -> Element {
    let nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    let Some(record) = nav.active_record_ref() else {
        return rsx! { p { class: "empty", "{chrome.0.record_select_prompt()}" } };
    };
    detail_pane(record.category, record.human_id)
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
