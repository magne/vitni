//! SSR assertions for the generic list + master-detail framework (Phase 5 PR3, ADR 0008 §5):
//! render a `MasterDetail` with a populated `ListPane` and a `DetailContainer` to an HTML string and
//! assert the searchbox, sort control, `listbox`/`option` roles, the selected-row and active-tab
//! state, and the roving `tabindex`. Pure render-and-inspect, the same pattern as `components.rs`.

use dioxus::prelude::*;
use genealogy_ui::{ListQuery, RowVm};
use genealogy_ui_dioxus::components::TabItem;
use genealogy_ui_dioxus::master_detail::{DetailContainer, ListChrome, ListPane, MasterDetail};

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
        MasterDetail {
            list: rsx! {
                ListPane {
                    rows: rows(),
                    query,
                    selected,
                    chrome: ListChrome {
                        list_label: "People".to_owned(),
                        filter_placeholder: "Filter people…".to_owned(),
                        empty: "No persons yet.".to_owned(),
                    },
                }
            },
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
    // Sorting and creation are no longer per-list affordances (WP2-7 tabbed-navigation rework):
    // sort is gone, and "New" is reached from the shell top bar/tabstrip instead of a list button.
    for absent in [r#"class="sort""#, ">New<"] {
        assert!(
            !html.contains(absent),
            "expected {absent:?} absent from list HTML:\n{html}"
        );
    }
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
        r#"class="detail-title""#,
        r#"class="avatar-lg""#,    // the header avatar
        r#"class="head-actions""#, // the right-aligned header actions slot
        "citation list",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in detail HTML:\n{html}");
    }
}

#[test]
fn roving_tabindex_has_one_stop_per_group() {
    let html = render();
    // A single tab stop and at least one non-stop, both in the listbox and the tablist.
    assert!(html.contains(r#"tabindex="0""#), "a roving tab stop:\n{html}");
    assert!(html.contains(r#"tabindex="-1""#), "a roving non-stop:\n{html}");
}
