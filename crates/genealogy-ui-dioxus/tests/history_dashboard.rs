//! SSR assertions for the History tab and the Dashboard (Phase 5 PR5): the audit timeline renders
//! who/when/why plus the undo control with its accessible label, and the dashboard renders the stat
//! cards, the recent-activity feed with a record link, and the computable data-quality checks. Pure
//! render-and-inspect — no window, no workspace — the same pattern as `person_detail.rs`.

use dioxus::prelude::*;
use genealogy_ui::{ActivityVm, Category, DashboardStats, DashboardVm, JumpVm, Localizer, RecordRef};
use genealogy_ui_dioxus::components::{HistoryEntry, HistoryTimeline};
use genealogy_ui_dioxus::screens::dashboard_view;
use genealogy_ui_dioxus::shell::nav_state::NavState;

/// Renders the audit timeline with one undoable entry.
fn timeline() -> Element {
    rsx! {
        HistoryTimeline {
            entries: vec![HistoryEntry {
                when: "2026-06-22 14:35".to_owned(),
                what: "Name asserted".to_owned(),
                who: "magne · High".to_owned(),
                why: Some("Baptism register".to_owned()),
                assertion_id: "a1".to_owned(),
                can_undo: true,
                undo_text: "Undo".to_owned(),
                undo_label: "Undo: Name asserted".to_owned(),
            }],
            onundo: move |_| {},
        }
    }
}

#[test]
fn history_timeline_renders_who_when_why_and_an_undo_control() {
    let mut vdom = VirtualDom::new(timeline);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        r#"class="tl-when""#,
        "2026-06-22 14:35",
        r#"class="tl-what""#,
        "Name asserted",
        r#"class="tl-who""#,
        "magne · High",
        r#"class="tl-why""#,
        "Baptism register",
        r#"aria-label="Undo: Name asserted""#, // the undo control names the change it reverts
        "↩",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

/// Renders the dashboard over a representative view-model, in English.
fn dashboard() -> Element {
    let nav = use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let vm = DashboardVm {
        stats: DashboardStats {
            people: 1284,
            families: 642,
            events: 3910,
            evidence_health_pct: 86,
            facts_without_source: 31,
            facts_total: 220,
        },
        recent: vec![ActivityVm {
            when: "2026-06-22 14:35".to_owned(),
            what: "Name asserted".to_owned(),
            who: "magne · High".to_owned(),
            record: Some(RecordRef {
                category: Category::People,
                human_id: "I0001".to_owned(),
                label: "John Smith".to_owned(),
            }),
        }],
        jump_back: vec![JumpVm {
            record: RecordRef {
                category: Category::People,
                human_id: "I0001".to_owned(),
                label: "John Smith".to_owned(),
            },
        }],
    };
    dashboard_view(&loc, nav, &vm)
}

#[test]
fn dashboard_renders_stats_activity_and_data_quality() {
    let mut vdom = VirtualDom::new(dashboard);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Workspace at a glance",       // the heading
        "1284",                        // the people count
        "642 families",                // the people caption
        "86%",                         // evidence health
        "31",                          // needs-attention / no-source count
        "Recent activity",             // the activity card
        r#"class="timeline""#,         // the activity feed reuses the audit timeline
        "Name asserted",               // an activity row
        "John Smith",                  // the linked record + the jump-back button, by display name
        "👤",                          // the entity icon prefixes the record links
        r#"class="no-source""#,        // the computable data-quality check
        "Coming in a later milestone", // deferred checks are flagged, not faked
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}
