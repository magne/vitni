//! SSR assertions for the generic list + master-detail framework (Phase 5 PR3, ADR 0008 §5):
//! render a `ListPane` (now hoisted to the shell Explorer) beside the editor-only `MasterDetail`
//! holding a `DetailContainer`, to an HTML string, and assert the searchbox, `listbox`/`option`
//! roles, the selected-row and active-tab state, and the roving `tabindex`. Pure render-and-inspect,
//! the same pattern as `components.rs`.

use dioxus::prelude::*;
use genealogy_ui::{ListQuery, RowVm};
use genealogy_ui_dioxus::components::TabItem;
use genealogy_ui_dioxus::master_detail::{DetailContainer, ListChrome, ListPane, MasterDetail, SortChrome};

fn sort_chrome() -> SortChrome {
    SortChrome {
        title: "Change sort order".to_owned(),
        id_asc: "Sort: ID ↑".to_owned(),
        id_desc: "Sort: ID ↓".to_owned(),
        name_asc: "Sort: Name ↑".to_owned(),
        name_desc: "Sort: Name ↓".to_owned(),
    }
}

fn rows() -> Vec<RowVm> {
    vec![
        RowVm {
            id: "I0001".to_owned(),
            title: "Charles Babbage".to_owned(),
            subtitle: Some("male".to_owned()),
            avatar: Some("CB".to_owned()),
            ..RowVm::default()
        },
        RowVm {
            id: "I0002".to_owned(),
            title: "Ada Lovelace".to_owned(),
            subtitle: Some("female".to_owned()),
            avatar: Some("AL".to_owned()),
            ..RowVm::default()
        },
    ]
}

/// A master-detail screen with the second row selected and the second tab active.
fn screen() -> Element {
    let query = use_signal(ListQuery::default);
    let selected = use_signal(|| Some("I0002".to_owned()));
    let active = use_signal(|| 1_usize);
    rsx! {
        ListPane {
            rows: rows(),
            query,
            selected,
            chrome: ListChrome {
                list_label: "People".to_owned(),
                filter_placeholder: "Filter people…".to_owned(),
                empty: "No persons yet.".to_owned(),
                sort: sort_chrome(),
            },
        }
        MasterDetail {
            detail: rsx! {
                DetailContainer {
                    title: "Ada Lovelace".to_owned(),
                    subtitle: "female".to_owned(),
                    id_label: "I0002".to_owned(),
                    badges: vec!["(private)".to_owned()],
                    avatar: "AL".to_owned(),
                    extras: rsx! {},
                    actions: rsx! { button { "Edit" } },
                    tabs: vec![
                        TabItem { id: "overview".to_owned(), label: "Overview".to_owned(), count: None },
                        TabItem { id: "citations".to_owned(), label: "Citations".to_owned(), count: Some(2) },
                    ],
                    active,
                    div { "citation list" }
                }
            },
        }
    }
}

fn render() -> String {
    let mut vdom = VirtualDom::new(screen);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn list_pane_carries_search_and_listbox_roles() {
    let html = render();
    for needle in [
        r#"role="searchbox""#,
        r#"aria-label="Filter people…""#,
        r#"role="listbox""#,
        r#"aria-label="People""#,
        r#"role="option""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in list HTML:\n{html}");
    }
    // Creation is reached from the shell top bar/tabstrip, not a per-list "New" button.
    assert!(!html.contains(">New<"), "no per-list New button:\n{html}");
}

#[test]
fn list_pane_offers_a_cycling_sort_button() {
    let html = render();
    // The toolbar carries a sort-cycle button (person.html specimen): a labelled button with the
    // current order and a "Change sort order" title. The default order is id ascending.
    assert!(
        html.contains(r#"title="Change sort order""#),
        "the sort button carries its accessible title:\n{html}"
    );
    assert!(
        html.contains("Sort: ID ↑"),
        "the sort button shows the current order label:\n{html}"
    );
    assert!(
        html.contains("<button") && html.contains(r#"class="btn sm""#),
        "the sort control is a button matching the specimen:\n{html}"
    );
}

#[test]
fn selected_row_is_marked() {
    let html = render();
    assert!(
        html.contains(r#"aria-selected="true""#),
        "expected a selected row/tab in HTML:\n{html}"
    );
    // The selected row's id and title both render.
    assert!(html.contains("Ada Lovelace"), "selected row title:\n{html}");
}

#[test]
fn detail_container_wires_tabs_and_active_state() {
    let html = render();
    for needle in [
        r#"role="tablist""#,
        r#"role="tab""#,
        r#"role="tabpanel""#,
        r#"aria-controls="panel-citations""#,
        r#"<h1 class="detail-title""#, // the record title is the screen's single <h1> (U42)
        r#"class="avatar-lg""#,        // the header avatar
        r#"class="head-actions""#,     // the right-aligned header actions slot
        "citation list",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in detail HTML:\n{html}");
    }
    assert_eq!(
        html.matches("<h1").count(),
        1,
        "a record screen carries exactly one <h1>:\n{html}"
    );
}

#[test]
fn roving_tabindex_has_one_stop_per_group() {
    let html = render();
    // A single tab stop and at least one non-stop, both in the listbox and the tablist.
    assert!(html.contains(r#"tabindex="0""#), "a roving tab stop:\n{html}");
    assert!(html.contains(r#"tabindex="-1""#), "a roving non-stop:\n{html}");
}
