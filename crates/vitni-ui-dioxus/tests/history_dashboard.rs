//! SSR assertions for the History tab and the Dashboard (Phase 5 PR5): the audit timeline renders
//! who/when/why plus the undo control with its accessible label, and the dashboard renders the stat
//! cards, the recent-activity feed with a record link, and the computable data-quality checks. Pure
//! render-and-inspect — no window, no workspace — the same pattern as `person_detail.rs`.

use dioxus::prelude::*;
use vitni_app::{DatasetId, DecidableKind, RecentItem, RunResume};
use vitni_ui::{
    ActivityVm, Category, DashboardStats, DashboardVm, DataQualityVm, JumpVm, Localizer, QueuedMatchVm, RecordRef,
    ResumeVm,
};
use vitni_ui_dioxus::components::{HistoryEntry, HistoryTimeline, RunChanges};
use vitni_ui_dioxus::screens::dashboard_view;
use vitni_ui_dioxus::shell::nav_state::NavState;

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
                count: None,
                evidence: None,
                changes: None,
            }],
            onundo: move |_| {},
        }
    }
}

/// Renders the audit timeline with an import run's row.
fn run_timeline() -> Element {
    rsx! {
        HistoryTimeline {
            entries: vec![HistoryEntry {
                when: "2026-06-22 14:35".to_owned(),
                what: "Imported from tree.ged".to_owned(),
                who: "gedcom-import (software agent)".to_owned(),
                why: None,
                assertion_id: "a1".to_owned(),
                can_undo: true,
                undo_text: "Undo".to_owned(),
                undo_label: "Undo: Imported from tree.ged".to_owned(),
                count: Some("4 changes".to_owned()),
                evidence: None,
                changes: None,
            }],
            onundo: move |_| {},
        }
    }
}

/// An import run's row that folds a supersession and its replacement.
fn run_with_changes() -> Element {
    let child = |what: &str, why: Option<&str>| HistoryEntry {
        when: "2026-06-22 14:35".to_owned(),
        what: what.to_owned(),
        who: "gedcom-import (software agent)".to_owned(),
        why: why.map(ToOwned::to_owned),
        assertion_id: what.to_owned(),
        can_undo: false,
        undo_text: "Undo".to_owned(),
        undo_label: format!("Undo: {what}"),
        count: None,
        evidence: None,
        changes: None,
    };
    rsx! {
        HistoryTimeline {
            entries: vec![HistoryEntry {
                when: "2026-06-22 14:35".to_owned(),
                what: "Imported from tree.ged".to_owned(),
                who: "gedcom-import (software agent)".to_owned(),
                why: None,
                assertion_id: "a1".to_owned(),
                can_undo: true,
                undo_text: "Undo".to_owned(),
                undo_label: "Undo: Imported from tree.ged".to_owned(),
                count: Some("2 changes".to_owned()),
                evidence: None,
                changes: Some(RunChanges {
                    label: "What it changed".to_owned(),
                    entries: vec![
                        child("Sex asserted", None),
                        child("Assertion superseded", Some("The value it replaced was recorded before tree.ged was exported.")),
                    ],
                }),
            }],
            onundo: move |_| {},
        }
    }
}

#[test]
fn an_import_run_row_offers_its_changes_behind_a_closed_disclosure() {
    let mut vdom = VirtualDom::new(run_with_changes);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"aria-expanded="false""#) && html.contains("What it changed"),
        "the run row carries a closed disclosure naming its changes:\n{html}"
    );
    assert!(
        !html.contains("tl-children") && !html.contains("Assertion superseded"),
        "the changes stay folded until the disclosure opens:\n{html}"
    );
    assert_eq!(
        html.matches("↩ Undo").count(),
        1,
        "only the run row is undoable:\n{html}"
    );
}

#[test]
fn an_import_run_row_shows_its_count_muted_beside_it() {
    let mut vdom = VirtualDom::new(run_timeline);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(
            r#"<div class="tl-what">Imported from tree.ged<span class="muted tl-count">4 changes</span></div>"#
        ),
        "the count sits in the row's summary line, muted:\n{html}"
    );
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

fn record(category: Category, human_id: &str, label: &str) -> RecordRef {
    RecordRef {
        category,
        human_id: human_id.to_owned(),
        label: label.to_owned(),
    }
}

/// The five strongest of six possible matches, the strongest a place pair: the card lists five and
/// counts the sixth.
fn matches() -> Vec<QueuedMatchVm> {
    let mut pairs = vec![QueuedMatchVm {
        kind: DecidableKind::Place,
        kind_label: "Place".to_owned(),
        a: record(Category::Places, "P0001", "P0001"),
        b: record(Category::Places, "P0002", "P0002"),
        percent: 99,
        band: "probable match".to_owned(),
        reasons: vec!["Same place name (+5.0)".to_owned(), "Same place type (+1.0)".to_owned()],
    }];
    for n in 0..4 {
        pairs.push(QueuedMatchVm {
            kind: DecidableKind::Person,
            kind_label: "Person".to_owned(),
            a: record(Category::People, &format!("I001{n}"), &format!("Ole Olsen {n}")),
            b: record(Category::People, &format!("I002{n}"), &format!("Ola Olsen {n}")),
            percent: 80,
            band: "possible match".to_owned(),
            reasons: vec!["Same surname (+2.0)".to_owned()],
        });
    }
    pairs
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
        recent: vec![
            ActivityVm {
                when: "2026-06-22 14:35".to_owned(),
                what: "Name asserted".to_owned(),
                who: "magne · High".to_owned(),
                record: Some(RecordRef {
                    category: Category::People,
                    human_id: "I0001".to_owned(),
                    label: "John Smith".to_owned(),
                }),
                count: None,
                children: Vec::new(),
                resume: None,
            },
            ActivityVm {
                when: "2026-06-22 14:30".to_owned(),
                what: "Imported from tree.ged".to_owned(),
                who: "gedcom-import (software agent)".to_owned(),
                record: None,
                count: Some("142 records".to_owned()),
                children: Vec::new(),
                resume: None,
            },
            ActivityVm {
                when: "2026-06-22 14:20".to_owned(),
                what: "Imported from big.ged".to_owned(),
                who: "gedcom-import (software agent)".to_owned(),
                record: None,
                count: Some("interrupted".to_owned()),
                children: Vec::new(),
                resume: Some(ResumeVm {
                    run: RunResume {
                        plugin: "gedcom-import".to_owned(),
                        dataset: DatasetId::lineage("gedcom", uuid::Uuid::from_u128(5)),
                        source: "/home/ada/big.ged".into(),
                    },
                    label: "Resume the import from big.ged".to_owned(),
                }),
            },
        ],
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
        matches: matches(),
        match_total: 6,
        match_counts: vec!["Person: 5".to_owned(), "Place: 1".to_owned()],
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
        matches: vec![],
        match_total: 0,
        match_counts: vec![],
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
        !html.contains("Death before birth") && !html.contains("undecided pairs"),
        "check rows and pairs are withheld until the pass resolves:\n{html}"
    );
}

#[test]
fn only_an_interrupted_run_offers_resume() {
    let mut vdom = VirtualDom::new(dashboard);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert_eq!(
        html.matches(">Resume<").count(),
        1,
        "the finished run offers none:\n{html}"
    );
}

#[test]
fn dashboard_renders_stats_activity_and_data_quality() {
    let mut vdom = VirtualDom::new(dashboard);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Workspace at a glance",                              // the heading
        "1284",                                               // the people count
        "642 families",                                       // the people caption
        "86%",                                                // evidence health
        "31",                                                 // needs-attention / no-source count
        "Recent activity",                                    // the activity card
        r#"class="timeline""#,                                // the activity feed reuses the audit timeline
        "Name asserted",                                      // an activity row
        "Imported from tree.ged",                             // an import run's row
        r#"<span class="muted tl-count">142 records</span>"#, // its count, muted beside it
        r#"<span class="muted tl-count">interrupted</span>"#, // an abandoned run reads interrupted
        r#"aria-label="Resume the import from big.ged""#,     // and offers Resume, named by its file
        "John Smith",           // the linked record + the jump-back button, by display name
        "👤",                   // the entity icon prefixes the record links
        r#"class="no-source""#, // the computable data-quality check
        "Death before birth",   // the death-before-birth check row
        "Jane Reversed",        // its flagged person, as a navigable link
        "Possible matches",     // the possible-matches card
        "6 undecided pairs",    // the real pair count, every kind
        "Person: 5 · Place: 1", // counted per kind
        ">Review<",             // the Review button routing into the Matches tool
        "📍",                   // a place pair, linked with the place icon
        "P0002",                // …to both of its records
        r#"title="Matching-engine probability — not the 5-level assertion Confidence">99%</span>"#,
        "probable match",                                  // the engine's band
        "Same place name (+5.0) · Same place type (+1.0)", // and its reasons
        "Ole Olsen 3",                                     // the pairs are listed up to the cap
        "+1 more",                                         // and the rest are counted
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(!html.contains("Ole Olsen 4"), "the sixth pair is past the cap:\n{html}");
    // U44: the Review action carries a contextual accessible name, not the bare "Review".
    assert!(
        html.contains(r#"aria-label="Review the possible matches""#),
        "the Review button carries a card-scoped accessible name:\n{html}"
    );
    // U42: the dashboard lead heading is the screen's single <h1>.
    assert_eq!(
        html.matches("<h1").count(),
        1,
        "the dashboard carries exactly one <h1>:\n{html}"
    );
}

/// Renders the audit timeline with a merge decided on the engine's assessment.
fn decision_timeline() -> Element {
    rsx! {
        HistoryTimeline {
            entries: vec![HistoryEntry {
                when: "2026-06-22 14:35".to_owned(),
                what: "Persona merged".to_owned(),
                who: "magne · High".to_owned(),
                why: Some("same household".to_owned()),
                assertion_id: "a1".to_owned(),
                can_undo: true,
                undo_text: "Undo".to_owned(),
                undo_label: "Undo: Persona merged".to_owned(),
                count: None,
                evidence: Some("Matched at 97% · probable match · engine 4".to_owned()),
                changes: None,
            }],
            onundo: move |_| {},
        }
    }
}

#[test]
fn a_decision_entry_shows_the_assessment_it_was_made_on() {
    let mut vdom = VirtualDom::new(decision_timeline);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"<div class="tl-evidence muted">Matched at 97% · probable match · engine 4</div>"#),
        "the assessment renders as its own muted line:\n{html}"
    );
}
