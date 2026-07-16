//! SSR assertions for the History tab and the Dashboard (Phase 5 PR5): the audit timeline renders
//! who/when/why plus the undo control with its accessible label, and the dashboard renders the stat
//! cards, the recent-activity feed with a record link, and the computable data-quality checks. Pure
//! render-and-inspect — no window, no workspace — the same pattern as `person_detail.rs`.

use dioxus::prelude::*;
use genealogy_app::RecentItem;
use genealogy_ui::{ActivityVm, Category, DashboardStats, DashboardVm, DataQualityVm, JumpVm, Localizer, RecordRef};
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
    // RecordLink resolves NavState from context, so the harness must provide it.
    use_context_provider(NavState::new);
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
    let data_quality = DataQualityVm {
        death_before_birth: vec![RecordRef {
            category: Category::People,
            human_id: "I0009".to_owned(),
            label: "Jane Reversed".to_owned(),
        }],
        duplicate_count: 14,
    };
    dashboard_view(&loc, &[], &vm, Some(&data_quality))
}

/// Renders the dashboard with a persisted "Jump back in" list (records only).
fn dashboard_with_recents() -> Element {
    use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let vm = DashboardVm {
        stats: DashboardStats {
            people: 0,
            families: 0,
            events: 0,
            evidence_health_pct: 100,
            facts_without_source: 0,
            facts_total: 0,
        },
        recent: vec![],
        jump_back: vec![],
    };
    let data_quality = DataQualityVm {
        death_before_birth: vec![],
        duplicate_count: 0,
    };
    let recent = vec![RecentItem::Record {
        kind: "family".to_owned(),
        human_id: "F0017".to_owned(),
        label: "Smith family".to_owned(),
    }];
    dashboard_view(&loc, &recent, &vm, Some(&data_quality))
}

#[test]
fn jump_back_renders_persisted_records() {
    let mut vdom = VirtualDom::new(dashboard_with_recents);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Smith family", // the persisted record, by its captured label
        "👪",           // the record's entity icon
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

/// Renders the dashboard while the data-quality pass is still loading (`None`).
fn dashboard_quality_loading() -> Element {
    use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let vm = DashboardVm {
        stats: DashboardStats {
            people: 3,
            families: 1,
            events: 2,
            evidence_health_pct: 100,
            facts_without_source: 0,
            facts_total: 0,
        },
        recent: vec![],
        jump_back: vec![],
    };
    dashboard_view(&loc, &[], &vm, None)
}

#[test]
fn data_quality_card_shows_a_loading_state_until_the_check_pass_resolves() {
    let mut vdom = VirtualDom::new(dashboard_quality_loading);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    // The fast dashboard is up (heading + stats) while the data-quality card shows its own loading
    // line and no check rows yet.
    assert!(
        html.contains("Workspace at a glance"),
        "fast dashboard renders:\n{html}"
    );
    assert!(
        html.contains("Checking data quality"),
        "data-quality card is loading:\n{html}"
    );
    assert!(
        !html.contains("Possible duplicates"),
        "check rows are withheld until the pass resolves:\n{html}"
    );
}

#[test]
fn dashboard_renders_stats_activity_and_data_quality() {
    let mut vdom = VirtualDom::new(dashboard);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Workspace at a glance", // the heading
        "1284",                  // the people count
        "642 families",          // the people caption
        "86%",                   // evidence health
        "31",                    // needs-attention / no-source count
        "Recent activity",       // the activity card
        r#"class="timeline""#,   // the activity feed reuses the audit timeline
        "Name asserted",         // an activity row
        "John Smith",            // the linked record + the jump-back button, by display name
        "👤",                    // the entity icon prefixes the record links
        r#"class="no-source""#,  // the computable data-quality check
        "Death before birth",    // the death-before-birth check row
        "Jane Reversed",         // its flagged person, as a navigable link
        "Possible duplicates",   // the duplicates check row
        "14",                    // the real duplicate-pair count
        "Compare",               // the Compare button routing into the merge wizard
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    // U44: the Compare row-action carries a contextual accessible name, not the bare "Compare".
    assert!(
        html.contains(r#"aria-label="Compare possible duplicates""#),
        "the Compare button carries a row-scoped accessible name:\n{html}"
    );
    // U42: the dashboard lead heading is the screen's single <h1>.
    assert_eq!(
        html.matches("<h1").count(),
        1,
        "the dashboard carries exactly one <h1>:\n{html}"
    );
}
