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
    let category = record.category;
    let human_id = record.human_id;
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
