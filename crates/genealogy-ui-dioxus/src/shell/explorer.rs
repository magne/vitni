//! The shell-level Explorer: the entity list column, shown only for an entity category.
//!
//! Hoisted out of the (now deleted) per-aggregate `*Screen`s so one component serves all 12
//! categories: it reads the active [`Category`], loads that category's list, and drives selection,
//! search, roving focus, and `[`/`]` stepping through the shared [`ListPane`] +
//! [`use_record_step`](crate::screens::use_record_step) — the only per-category variation
//! (the list intent, the chrome strings, the empty message) is a compact `match`. Selecting a row
//! opens it as a record tab in the editor host ([`NavState::open_record`]); the list stays put while
//! records open beside it (the VS Code model). Mounted by [`Shell`](crate::shell::root::Shell) only
//! when [`entity_category`] is `Some`, so a tool/Dashboard/Help destination shows no list at all.

use dioxus::prelude::*;
use genealogy_ui::{Category, Intent, IntentOutcome, ListQuery, RecordRef, RowVm};

use crate::app::AppCtx;
use crate::master_detail::{ListChrome, ListPane};
use crate::screens::use_record_step;
use crate::services::{ScreenData, load_screen};
use crate::shell::nav_state::{NavState, entity_category};

/// The Explorer column. Renders the active entity category's list, or nothing when the active
/// destination is not an entity category (a tool, the Dashboard, or Help — the guard also mirrors
/// [`Shell`](crate::shell::root::Shell)'s conditional mount, keeping this safe to render alone).
#[component]
pub fn Explorer() -> Element {
    let nav = try_consume_context::<NavState>();
    let Some(category) = nav.and_then(|nav| entity_category(*nav.active.read())) else {
        return rsx! {};
    };
    // Keyed by category so switching categories remounts a fresh list (its own resource + signals),
    // mirroring how the old per-screen mount re-fetched on every category switch.
    rsx! {
        ExplorerList { key: "{category.id()}", category }
    }
}

/// The list for one entity `category`: owns its `use_resource`, `selected`/`query` signals, wires
/// selection to [`NavState::open_record`], keeps the row highlight in sync with the active record
/// tab (only when it belongs to this category), and installs `[`/`]` stepping.
#[component]
fn ExplorerList(category: Category) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let services = state.services().clone();
    let chrome = state.chrome();
    let loc = state.data_loc();
    let entity = chrome.rail_label(category.label_id());
    let loading = chrome.loading();
    let list_chrome = ListChrome {
        list_label: entity.clone(),
        filter_placeholder: chrome.list_filter(&entity),
        empty: list_empty(loc, category),
    };
    let mut selected = use_signal(|| None::<String>);
    // Keep the row highlight in sync with the active record tab — but only when the active record is
    // in *this* category's list (a cross-category tab opened via a link leaves the list unhighlighted).
    use_effect(move || {
        selected.set(
            nav.active_record_ref()
                .filter(|record| record.category == category)
                .map(|record| record.human_id),
        );
    });
    let query = use_signal(ListQuery::default);
    let intent = list_intent(category);
    let list = use_resource(move || {
        let services = services.clone();
        let intent = intent.clone();
        async move { load_screen(services, intent).await }
    });
    use_record_step(nav, category, list, query, selected);
    match &*list.read_unchecked() {
        None => rsx! { p { class: "loading", "{loading}" } },
        Some(ScreenData::Error(message)) => rsx! { p { class: "empty", "{message}" } },
        Some(ScreenData::Loaded(IntentOutcome::List(rows))) => rsx! {
            ListPane {
                rows: rows.clone(),
                query,
                selected,
                chrome: list_chrome.clone(),
                onselect: move |row: RowVm| nav.open_record(RecordRef {
                    category,
                    human_id: row.id,
                    label: row.title,
                }),
            }
        },
        Some(ScreenData::Loaded(
            IntentOutcome::Detail(_)
            | IntentOutcome::CitationDetail(_)
            | IntentOutcome::FamilyDetail(_)
            | IntentOutcome::EventDetail(_)
            | IntentOutcome::PlaceDetail(_)
            | IntentOutcome::NotFound { .. }
            | IntentOutcome::Dashboard(_)
            | IntentOutcome::DataQuality(_)
            | IntentOutcome::SourceDetail(_)
            | IntentOutcome::RepositoryDetail(_)
            | IntentOutcome::MediaDetail(_)
            | IntentOutcome::NoteDetail(_)
            | IntentOutcome::TagDetail(_)
            | IntentOutcome::DnaTestDetail(_)
            | IntentOutcome::DnaMatchDetail(_)
            | IntentOutcome::Pedigree(_)
            | IntentOutcome::Relationship(_)
            | IntentOutcome::DuplicateCandidates(_)
            | IntentOutcome::MergeCompare(_),
        )) => rsx! {},
    }
}

/// The list-loading [`Intent`] for an entity category (`Dashboard` has no list — it is not an entity
/// destination and the Explorer is never mounted for it).
fn list_intent(category: Category) -> Intent {
    match category {
        Category::People | Category::Dashboard => Intent::ShowList,
        Category::Families => Intent::ShowFamilyList,
        Category::Events => Intent::ShowEventList,
        Category::Places => Intent::ShowPlaceList,
        Category::Sources => Intent::ShowSourceList,
        Category::Citations => Intent::ShowCitationList,
        Category::Repositories => Intent::ShowRepositoryList,
        Category::Media => Intent::ShowMediaList,
        Category::Notes => Intent::ShowNoteList,
        Category::Tags => Intent::ShowTagList,
        Category::DnaTests => Intent::ShowDnaTestList,
        Category::DnaMatches => Intent::ShowDnaMatchList,
    }
}

/// The already-localized "no records yet" message for an entity category.
fn list_empty(loc: &genealogy_ui::Localizer, category: Category) -> String {
    match category {
        Category::People | Category::Dashboard => loc.list_empty(),
        Category::Families => loc.family_list_empty(),
        Category::Events => loc.event_list_empty(),
        Category::Places => loc.place_list_empty(),
        Category::Sources => loc.source_list_empty(),
        Category::Citations => loc.citation_list_empty(),
        Category::Repositories => loc.repository_list_empty(),
        Category::Media => loc.media_list_empty(),
        Category::Notes => loc.note_list_empty(),
        Category::Tags => loc.tag_list_empty(),
        Category::DnaTests => loc.dna_test_list_empty(),
        Category::DnaMatches => loc.dna_match_list_empty(),
    }
}
