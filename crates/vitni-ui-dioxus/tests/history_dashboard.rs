//! SSR assertions for the History tab and the Dashboard (Phase 5 PR5): the audit timeline renders
//! who/when/why plus the undo control with its accessible label, and the dashboard renders the stat
//! cards, the recent-activity feed with a record link, and the computable data-quality checks. Pure
//! render-and-inspect — no window, no workspace — the same pattern as `person_detail.rs`.

use dioxus::prelude::*;
use vitni_app::{DatasetId, DecidableKind, ImportRunId, RecentItem, RunResume};
use vitni_ui::{
    ActivityVm, Category, DashboardStats, DashboardVm, DataQualityVm, HistoryEntryVm, JumpVm, Localizer, QueuedMatchVm,
    RecordRef, ResumeVm, RunDisclosureVm, RunGroupVm, RunRecordVm, RunRecordsVm,
};
use vitni_ui_dioxus::components::{HistoryEntry, HistoryTimeline, RunChanges};
use vitni_ui_dioxus::screens::{dashboard_view, run_record_changes_list, run_records_list};
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
                value: None,
                replaced: None,
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
                value: None,
                replaced: None,
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
        value: None,
        replaced: None,
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
                        child("Assertion superseded", Some("The value it replaced was recorded at or before the export of tree.ged (2026-06-21), so the file's value replaced it.")),
                    ],
                }),
                value: None,
                replaced: None,
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
                run: None,
                resume: None,
                value: Some("John Smith".to_owned()),
            },
            ActivityVm {
                when: "2026-06-22 14:30".to_owned(),
                what: "Imported from tree.ged".to_owned(),
                who: "gedcom-import (software agent)".to_owned(),
                record: None,
                count: Some("142 records".to_owned()),
                run: Some(RunDisclosureVm {
                    id: ImportRunId::from_uuid(uuid::Uuid::from_u128(7)),
                    label: "What the import from tree.ged changed".to_owned(),
                }),
                resume: None,
                value: None,
            },
            ActivityVm {
                when: "2026-06-22 14:20".to_owned(),
                what: "Imported from big.ged".to_owned(),
                who: "gedcom-import (software agent)".to_owned(),
                record: None,
                count: Some("interrupted".to_owned()),
                run: Some(RunDisclosureVm {
                    id: ImportRunId::from_uuid(uuid::Uuid::from_u128(8)),
                    label: "What the import from big.ged changed".to_owned(),
                }),
                resume: Some(ResumeVm {
                    run: RunResume {
                        plugin: "gedcom-import".to_owned(),
                        dataset: DatasetId::lineage("gedcom", uuid::Uuid::from_u128(5)),
                        source: "/home/ada/big.ged".into(),
                    },
                    label: "Resume the import from big.ged".to_owned(),
                }),
                value: None,
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
        "Name asserted: <b>John Smith</b>",                   // an activity row, naming its value
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
                value: None,
                replaced: None,
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

/// Renders a correction's History row: the value it asserted and the one it replaced.
fn corrected_timeline() -> Element {
    rsx! {
        HistoryTimeline {
            entries: vec![HistoryEntry {
                when: "2026-06-22 14:35".to_owned(),
                what: "Date asserted".to_owned(),
                who: "magne · High".to_owned(),
                why: None,
                assertion_id: "a1".to_owned(),
                can_undo: true,
                undo_text: "Undo".to_owned(),
                undo_label: "Undo: Date asserted".to_owned(),
                count: None,
                evidence: None,
                changes: None,
                value: Some("12 Apr 1850".to_owned()),
                replaced: Some("was 1850".to_owned()),
            }],
            onundo: move |_| {},
        }
    }
}

#[test]
fn a_history_row_names_its_value_and_what_it_replaced() {
    let mut vdom = VirtualDom::new(corrected_timeline);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains("Date asserted: <b>12 Apr 1850</b> (was 1850)"),
        "the value is bold after the phrase, what it replaced after it:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Undo: Date asserted""#),
        "the undo label keeps the bare phrase:\n{html}"
    );
}

#[test]
fn a_history_row_without_a_value_reads_as_its_phrase() {
    let mut vdom = VirtualDom::new(timeline);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(!html.contains("<b>"), "no value, no bold:\n{html}");
}

#[test]
fn an_import_run_row_on_the_dashboard_offers_its_records_behind_a_closed_disclosure() {
    let mut vdom = VirtualDom::new(dashboard);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert_eq!(
        html.matches(r#"aria-expanded="false""#).count(),
        2,
        "each run row carries a closed disclosure:\n{html}"
    );
    assert!(
        html.contains("▸ What it changed"),
        "named like the History tab's:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="What the import from tree.ged changed""#)
            && html.contains(r#"aria-label="What the import from big.ged changed""#),
        "each run's disclosure names its file, so the two read apart:\n{html}"
    );
    assert!(
        !html.contains("run-records"),
        "its records are read only once the disclosure opens:\n{html}"
    );
}

fn run_record(kind: &str, record: Option<RecordRef>, label: &str) -> RunRecordVm {
    RunRecordVm {
        kind: kind.to_owned(),
        aggregate_id: format!("{kind}-{label}"),
        record,
        label: label.to_owned(),
        toggle_label: format!("What it changed in {label}"),
    }
}

fn linked(category: Category, human_id: &str, label: &str) -> RecordRef {
    RecordRef {
        category,
        human_id: human_id.to_owned(),
        label: label.to_owned(),
    }
}

fn group(lead: RunRecordVm, members: Vec<RunRecordVm>, summary: Option<&str>) -> RunGroupVm {
    RunGroupVm {
        lead,
        members,
        summary: summary.map(ToOwned::to_owned),
    }
}

/// A page of a run's records under its count line: a person with the birth and citation its source
/// item wrote, and a tag with no user-facing id alone, with more to load.
fn run_records() -> Element {
    use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let person = run_record(
        "person",
        Some(linked(Category::People, "I0001", "John Smith")),
        "John Smith",
    );
    let birth = run_record("event", Some(linked(Category::Events, "E0001", "E0001")), "E0001");
    let citation = run_record("citation", Some(linked(Category::Citations, "C0001", "C0001")), "C0001");
    let page = RunRecordsVm {
        counts: "1 person · 1 birth · 1 citation · 1 tag".to_owned(),
        groups: vec![
            group(person, vec![birth, citation], Some("person · birth · citation")),
            group(run_record("tag", None, "tag"), Vec::new(), None),
        ],
        more: true,
    };
    run_records_list(
        &loc,
        ImportRunId::from_uuid(uuid::Uuid::from_u128(7)),
        &page,
        move |()| {},
    )
}

#[test]
fn an_open_run_lists_its_records_each_behind_its_own_disclosure() {
    let mut vdom = VirtualDom::new(run_records);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"class="timeline tl-children""#) && html.contains(r#"data-hook="run-records""#),
        "the records nest under the run like the History tab's changes:\n{html}"
    );
    assert!(
        html.contains("John Smith") && html.contains("👤"),
        "a person links by name:\n{html}"
    );
    assert!(html.contains(">tag<"), "a record with no id reads as its kind:\n{html}");
    for name in [
        "What it changed in John Smith",
        "What it changed in E0001",
        "What it changed in C0001",
        "What it changed in tag",
    ] {
        assert!(
            html.contains(&format!(r#"aria-expanded="false" aria-label="{name}""#))
                || html.contains(&format!(r#"aria-label="{name}" aria-expanded="false""#)),
            "each record's disclosure is closed and names its record ({name}):\n{html}"
        );
    }
    assert!(
        !html.contains("run-record-changes"),
        "a record's changes are read only once it opens:\n{html}"
    );
    assert!(html.contains(">Show more<"), "more records load on demand:\n{html}");
}

#[test]
fn an_open_run_counts_its_records_above_them() {
    let mut vdom = VirtualDom::new(run_records);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    let counts = html
        .find(r#"data-hook="run-counts""#)
        .unwrap_or_else(|| panic!("a count line:\n{html}"));
    let records = html.find(r#"data-hook="run-records""#).unwrap_or(0);
    assert!(counts < records, "the count line heads the records:\n{html}");
    assert!(html.contains("1 person · 1 birth · 1 citation · 1 tag"), "{html}");
}

#[test]
fn a_source_items_records_nest_under_its_own_entity() {
    let mut vdom = VirtualDom::new(run_records);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert_eq!(
        html.matches(r#"data-hook="run-group""#).count(),
        1,
        "only a group of several records nests; a lone record reads flat:\n{html}"
    );
    let group = html.find(r#"data-hook="run-group""#).unwrap_or(0);
    let lead = html.find("John Smith").unwrap_or(0);
    let summary = html
        .find("person · birth · citation")
        .unwrap_or_else(|| panic!("the group names its records' kinds:\n{html}"));
    let birth = html.find("E0001").unwrap_or(0);
    let tag = html.find(">tag<").unwrap_or(0);
    assert!(
        group < lead && lead < summary && summary < birth && birth < tag,
        "the group reads John Smith — its kinds, then its items' records inside it, before the next group:\n{html}"
    );
    assert_eq!(
        html.matches(r#"class="muted tl-count""#).count(),
        1,
        "only the lead names the group's kinds; each item's record reads by its link:\n{html}"
    );
}

/// A run's page with no records and nothing more.
fn no_run_records() -> Element {
    use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let page = RunRecordsVm {
        counts: String::new(),
        groups: Vec::new(),
        more: false,
    };
    run_records_list(
        &loc,
        ImportRunId::from_uuid(uuid::Uuid::from_u128(7)),
        &page,
        move |()| {},
    )
}

#[test]
fn a_run_that_changed_no_records_says_so() {
    let mut vdom = VirtualDom::new(no_run_records);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains("It changed no records."), "{html}");
    assert!(!html.contains("Show more"), "{html}");
    assert!(!html.contains("run-counts"), "no count line over nothing:\n{html}");
}

/// What a run changed in one record: a supersession and its replacement, display-only.
fn run_record_changes() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let change = |what: &str, why: Option<&str>| HistoryEntryVm {
        when: "2026-06-22 14:35".to_owned(),
        what: what.to_owned(),
        who: "gedcom-import (software agent)".to_owned(),
        why: why.map(ToOwned::to_owned),
        assertion_id: what.to_owned(),
        can_undo: false,
        count: None,
        evidence: None,
        children: Vec::new(),
        value: None,
        replaced: None,
    };
    run_record_changes_list(
        &loc,
        &[
            change("Date asserted", None),
            change("Assertion superseded", Some("The file value replaced it.")),
        ],
    )
}

#[test]
fn an_open_record_lists_what_the_run_changed_in_it_without_undo() {
    let mut vdom = VirtualDom::new(run_record_changes);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains(r#"data-hook="run-record-changes""#), "{html}");
    assert!(
        html.contains(r#"class="timeline tl-children""#),
        "nested like the run's records:\n{html}"
    );
    for needle in ["Date asserted", "Assertion superseded", "The file value replaced it."] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(!html.contains("Undo"), "the Dashboard undoes nothing:\n{html}");
}
