//! How an open record tab is named — the one place, so the record strip and the close/quit confirm can
//! never disagree about what a tab is called.
//!
//! A saved tab is its record's own label. A draft names itself by what has been typed into it
//! ([`NavState::draft_label`], recorded from `RecordDraft::display_label`) — **the same string the
//! record is labelled with on commit**, so a tab does not rename itself the moment it is saved. A draft
//! with nothing that names it yet falls back to the localized "New <entity>", with an ordinal from the
//! second such draft of its category onwards (#260).
//!
//! The ordinal is why this is a **whole-strip** pass: a draft's number cannot be read off that draft
//! alone.

use std::collections::BTreeMap;

use dioxus::prelude::ReadableExt;
use vitni_ui::Category;

use crate::i18n::Chrome;
use crate::shell::nav_state::{NavState, OpenTab};

/// Every open tab's label, in strip order.
///
/// One pass over the whole strip, because a draft's ordinal is its position among *its category's*
/// drafts — a fact about the strip, not about the tab. Callers rendering the whole strip should hoist
/// this above their loop; [`tab_label`] is for the callers that need one index.
#[must_use]
pub fn tab_labels(nav: &NavState, chrome: &Chrome) -> Vec<String> {
    let tabs = nav.records.read().clone();
    let mut drafts_seen: BTreeMap<Category, usize> = BTreeMap::new();
    let mut labels = Vec::with_capacity(tabs.len());
    for tab in &tabs {
        match tab {
            OpenTab::Saved(record) => labels.push(record.label.clone()),
            OpenTab::Draft(category, _) => {
                let position = drafts_seen.entry(*category).or_insert(0);
                *position += 1;
                labels.push(draft_label(nav, chrome, tab, *position));
            }
        }
    }
    labels
}

/// The label of the open tab at `index`, or an empty string when nothing is open there (a tab closed
/// while a confirm still names it).
#[must_use]
pub fn tab_label(nav: &NavState, chrome: &Chrome, index: usize) -> String {
    let mut labels = tab_labels(nav, chrome);
    if index >= labels.len() {
        return String::new();
    }
    labels.swap_remove(index)
}

/// One draft tab's label: the name typed into it, else the localized "New <entity>" — numbered by
/// `position`, its 1-based place among its category's open drafts in strip order.
///
/// The **first** draft of a category is never numbered: a lone `⌘N` reads "New People", as it always
/// has. Numbering by strip position rather than by [`DraftId`](crate::shell::nav_state::DraftId) keeps
/// the numbers gap-free as drafts close, and counting *all* the category's drafts rather than only the
/// unnamed ones means typing a name into draft 1 never renumbers draft 2 while the operator watches.
fn draft_label(nav: &NavState, chrome: &Chrome, tab: &OpenTab, position: usize) -> String {
    if let Some(label) = nav.draft_label(&tab.edit_key()) {
        return label;
    }
    let entity = chrome.rail_label(tab.category().label_id());
    if position <= 1 {
        chrome.draft_tab_label(&entity)
    } else {
        chrome.draft_tab_label_nth(&entity, position)
    }
}
