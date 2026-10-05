//! SSR assertions for the bulk-import wizard's Plan and Review stages (ADR 0040 §3, §4; `import.html`):
//! the plan's counts by kind with what it comes to and what to do with it, the compare view with the
//! bulk answers, and a Review step the step indicator shows only while the stage is up.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_app::{
    EngineVersion, KindCounts, MatchBand, MatchEvidence, MatchGroup, MatchableKind, PlanCounts, PlanSummary,
    PlannedChange, PlannedField, PlannedRecord,
};
use vitni_core::enums::FactType;
use vitni_ui::{BulkImportStage, CompareSideVm, MatchCompareVm, MatchStageVm};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::screens::{
    BulkConfirmStage, BulkImportWizardLabels, BulkPlanStage, BulkReviewStage, bulk_confirm_labels, bulk_plan_labels,
    bulk_review_labels, bulk_step_indicator,
};
use vitni_ui_dioxus::shell::ChromeCtx;

fn chrome() -> Rc<Chrome> {
    let language = "en".parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn summary(person: PlanCounts) -> PlanSummary {
    PlanSummary {
        kinds: vec![
            KindCounts {
                kind: MatchableKind::Place,
                counts: PlanCounts {
                    unchanged: 4,
                    ..PlanCounts::default()
                },
            },
            KindCounts {
                kind: MatchableKind::Person,
                counts: person,
            },
        ],
        candidates: person.candidates,
        records: Vec::new(),
    }
}

fn record(
    kind: MatchableKind,
    label: &str,
    human_id: &str,
    change: PlannedChange,
    fields: Vec<PlannedField>,
) -> PlannedRecord {
    PlannedRecord {
        kind,
        label: label.to_owned(),
        human_id: human_id.to_owned(),
        change,
        fields,
        keys: Vec::new(),
    }
}

fn plan_with_updates() -> Element {
    let mut summary = summary(PlanCounts {
        updated: 1,
        ..PlanCounts::default()
    });
    summary.records = vec![record(
        MatchableKind::Person,
        "Ole Hansen",
        "I0001",
        PlannedChange::Updates,
        vec![PlannedField::Fact(FactType::Occupation)],
    )];
    let labels = bulk_plan_labels(&chrome(), &summary);
    rsx! {
        BulkPlanStage { labels, onstep: |_| {} }
    }
}

fn reviewed_plan() -> Element {
    let mut summary = summary(PlanCounts {
        new: 1,
        ..PlanCounts::default()
    });
    summary.kinds.push(KindCounts {
        kind: MatchableKind::Source,
        counts: PlanCounts {
            linked: 1,
            ..PlanCounts::default()
        },
    });
    summary.records = vec![record(
        MatchableKind::Source,
        "1920 census",
        "S0002",
        PlannedChange::Reuses,
        vec![PlannedField::Author, PlannedField::Publication],
    )];
    let labels = bulk_confirm_labels(&chrome(), &summary);
    rsx! {
        BulkConfirmStage { labels, onconfirm: |_| {} }
    }
}

#[test]
fn the_plan_lists_each_kind_s_changed_records_folded_under_its_counts() {
    let html = render(plan_with_updates);
    let table = html.find("</table>").expect("the table");
    let list = html.find(r#"class="plan-records""#).expect("the records list:\n{html}");
    assert!(table < list, "under the counts:\n{html}");
    assert!(html.contains("Persons · 1 record changes"), "{html}");
    assert!(html.contains("Ole Hansen"), "{html}");
    assert!(html.contains("I0001"), "{html}");
    assert!(html.contains("updates occupation"), "{html}");
    assert!(!html.contains("open"), "folded at the plan:\n{html}");
}

#[test]
fn the_reviewed_plan_shows_what_a_same_adds_and_offers_import_or_cancel() {
    let html = render(reviewed_plan);
    assert!(html.contains("Ready to import"), "heading:\n{html}");
    assert!(html.contains("1920 census"), "{html}");
    assert!(html.contains("reused · adds author, publication"), "{html}");
    assert!(html.contains("open"), "unfolded once reviewed:\n{html}");
    assert!(html.contains(">Import<") && html.contains("Cancel"), "{html}");
    assert!(
        !html.contains("Review") && !html.contains("Decide all later"),
        "nothing left to review:\n{html}"
    );
}

fn plan_view(person: PlanCounts) -> Element {
    let labels = bulk_plan_labels(&chrome(), &summary(person));
    rsx! {
        BulkPlanStage { labels, onstep: |_| {} }
    }
}

fn plan_with_matches() -> Element {
    plan_view(PlanCounts {
        new: 2,
        candidates: 3,
        ..PlanCounts::default()
    })
}

fn plan_without_matches() -> Element {
    plan_view(PlanCounts {
        new: 2,
        ..PlanCounts::default()
    })
}

fn plan_writing_nothing() -> Element {
    plan_view(PlanCounts {
        unchanged: 2,
        ..PlanCounts::default()
    })
}

#[test]
fn the_plan_shows_each_kind_s_counts_in_commit_order() {
    let html = render(plan_with_matches);
    assert!(html.contains("What this import will write"), "heading:\n{html}");
    for column in [
        "New",
        "Unchanged",
        "Updated",
        "Already in the tree",
        "Possible matches",
        "Kept as recorded",
    ] {
        assert!(html.contains(column), "{column} column:\n{html}");
    }
    let places = html.find("Places").expect("the places row");
    let persons = html.find("Persons").expect("the persons row");
    assert!(places < persons, "places are written first:\n{html}");
}

#[test]
fn a_plan_with_matches_offers_to_review_them_or_decide_them_later() {
    let html = render(plan_with_matches);
    assert!(html.contains("Review 3 possible matches"), "review:\n{html}");
    assert!(html.contains("Decide all later and import"), "defer:\n{html}");
    assert!(
        html.contains("3 records may already be in your tree"),
        "the note:\n{html}"
    );
    assert!(html.contains("Cancel"), "cancel:\n{html}");
}

#[test]
fn a_plan_without_matches_only_imports() {
    let html = render(plan_without_matches);
    assert!(html.contains(">Import<"), "import:\n{html}");
    assert!(!html.contains("Decide all later"), "nothing to defer:\n{html}");
}

#[test]
fn a_plan_that_writes_nothing_says_so() {
    let html = render(plan_writing_nothing);
    assert!(
        html.contains("Every record is already on record: importing this file writes nothing."),
        "the note:\n{html}"
    );
}

fn stage(group: Option<MatchGroup>) -> MatchStageVm {
    MatchStageVm {
        kind: MatchableKind::Place,
        compare: MatchCompareVm {
            left: CompareSideVm {
                human_id: "P0004".to_owned(),
                label: "Mandal".to_owned(),
                origin: None,
                evidence: None,
            },
            right: CompareSideVm {
                human_id: "new".to_owned(),
                label: "Mandal".to_owned(),
                origin: None,
                evidence: None,
            },
            rows: Vec::new(),
            assessment: MatchEvidence {
                score_bp: 9700,
                band: MatchBand::Probable,
                engine: EngineVersion(4),
                cultures: Vec::new(),
                features: Vec::new(),
            },
            assessment_line: "Matched at 97% · probable match · engine 4".to_owned(),
            earlier_decision: None,
        },
        position: 2,
        total: 40,
        group,
    }
}

fn review_view(group: Option<MatchGroup>) -> Element {
    use_context_provider(|| ChromeCtx(chrome()));
    let stage = stage(group);
    let labels = bulk_review_labels(&chrome(), &stage);
    rsx! {
        BulkReviewStage { stage, labels, confidence_options: Vec::new(), onanswer: |_| {} }
    }
}

fn review_in_a_group() -> Element {
    review_view(Some(MatchGroup {
        band: MatchBand::Probable,
        remaining: 31,
    }))
}

fn review_alone() -> Element {
    review_view(None)
}

#[test]
fn the_review_compares_the_pair_and_offers_the_bulk_answers() {
    let html = render(review_in_a_group);
    assert!(html.contains("Possible match 2 of 40"), "position:\n{html}");
    assert!(html.contains("Is this place already in your tree?"), "heading:\n{html}");
    assert!(html.contains(r#"id="match-compare""#), "the compare view:\n{html}");
    assert!(html.contains("from this file"), "incoming caption:\n{html}");
    for label in [
        "Decide later",
        "Not the same",
        "Same (reversible)",
        "Treat all 31 probable place matches as the same",
        "Decide the rest later",
        "Cancel import",
    ] {
        assert!(html.contains(label), "{label} renders:\n{html}");
    }
    assert!(!html.contains("Skip record"), "a bulk import skips no record:\n{html}");
}

#[test]
fn a_pair_outside_a_probable_group_has_no_bulk_same() {
    let html = render(review_alone);
    assert!(!html.contains("Treat all"), "no group:\n{html}");
    assert!(html.contains("Decide the rest later"), "rest later:\n{html}");
}

fn wizard_labels() -> BulkImportWizardLabels {
    BulkImportWizardLabels {
        heading: "Bulk import stages".to_owned(),
        stages: ["Source", "Running", "Plan", "Review", "Summary"].map(str::to_owned),
    }
}

fn steps_at_review() -> Element {
    use_context_provider(|| ChromeCtx(chrome()));
    bulk_step_indicator(&wizard_labels(), &BulkImportStage::Review(Box::new(stage(None))))
}

fn steps_at_confirm() -> Element {
    bulk_step_indicator(&wizard_labels(), &BulkImportStage::Confirm(PlanSummary::default()))
}

fn steps_at_plan() -> Element {
    bulk_step_indicator(&wizard_labels(), &BulkImportStage::Plan(PlanSummary::default()))
}

#[test]
fn the_review_step_shows_only_while_the_review_is_up() {
    let at_review = render(steps_at_review);
    assert!(
        at_review.contains(r#"<span class="num">4</span> Review"#) && at_review.contains(r#"aria-current="step""#),
        "the review step is the fourth and current:\n{at_review}"
    );
    assert!(
        at_review.contains(r#"<span class="num">5</span> Summary"#),
        "{at_review}"
    );

    let at_confirm = render(steps_at_confirm);
    assert!(
        at_confirm.contains(r#"<span class="num">4</span> Review"#) && at_confirm.contains(r#"aria-current="step""#),
        "the reviewed plan keeps the review step current:\n{at_confirm}"
    );

    let at_plan = render(steps_at_plan);
    assert!(!at_plan.contains("Review"), "no review step at the plan:\n{at_plan}");
    assert!(at_plan.contains(r#"<span class="num">3</span> Plan"#), "{at_plan}");
    assert!(at_plan.contains(r#"<span class="num">4</span> Summary"#), "{at_plan}");
}
