//! SSR assertions for the assisted-import wizard's ready-to-import stage (ADR 0046; `import.html`): the
//! host's own stage, shown only when the record being imported adds to a record already in the tree —
//! each such record with what it gains, unfolded, *Import*, *Skip record* and *Cancel import*, and a
//! *Plan* step the step indicator shows only while the stage is up.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_app::{KindCounts, MatchableKind, PlanCounts, PlanSummary, PlannedChange, PlannedField, PlannedRecord};
use vitni_ui::{ImportStage, SummaryPayload};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::screens::{ImportPlanStage, WizardLabels, import_plan_labels, step_indicator};
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

/// A census record whose source, decided *Same*, gains the author it lacks.
fn summary() -> PlanSummary {
    PlanSummary {
        kinds: vec![KindCounts {
            kind: MatchableKind::Source,
            counts: PlanCounts {
                linked: 1,
                ..PlanCounts::default()
            },
        }],
        candidates: 0,
        records: vec![PlannedRecord {
            kind: MatchableKind::Source,
            label: "1920 folketelling for Greipstad".to_owned(),
            human_id: "S0002".to_owned(),
            change: PlannedChange::Reuses,
            fields: vec![PlannedField::Author],
            keys: vec!["source.AuthorSet".to_owned()],
        }],
    }
}

fn plan_view() -> Element {
    let chrome = chrome();
    let labels = import_plan_labels(&chrome, &summary());
    use_context_provider(|| ChromeCtx(chrome));
    rsx! {
        ImportPlanStage { labels, onanswer: |_| {} }
    }
}

#[test]
fn the_plan_shows_what_the_record_adds_to_each_stored_record_unfolded() {
    let html = render(plan_view);

    assert!(html.contains("Ready to import"), "the heading:\n{html}");
    assert!(html.contains("Sources · 1 record changes"), "the kind's group:\n{html}");
    assert!(
        html.contains("1920 folketelling for Greipstad") && html.contains("S0002"),
        "the stored record:\n{html}"
    );
    assert!(html.contains("reused · adds author"), "what it gains:\n{html}");
    assert!(
        html.contains(r#"<details class="plan-records" open"#),
        "unfolded:\n{html}"
    );
}

#[test]
fn the_plan_offers_import_skip_and_cancel() {
    let html = render(plan_view);

    for label in ["Import", "Skip record", "Cancel import"] {
        assert!(html.contains(&format!(">{label}<")), "{label} renders:\n{html}");
    }
}

fn wizard_labels() -> WizardLabels {
    WizardLabels {
        heading: "Import stages".to_owned(),
        stages: ["Source", "Records", "Confirm", "Save scan", "Match", "Plan", "Summary"].map(str::to_owned),
    }
}

fn steps_at_plan() -> Element {
    step_indicator(&wizard_labels(), &ImportStage::Plan(summary()))
}

fn steps_at_summary() -> Element {
    let summary = ImportStage::Summary(SummaryPayload {
        imported: Vec::new(),
        skipped: 0,
    });
    step_indicator(&wizard_labels(), &summary)
}

#[test]
fn the_plan_step_shows_only_while_the_plan_stage_is_up() {
    let at_plan = render(steps_at_plan);
    assert!(
        at_plan.contains(r#"aria-current="step""#) && at_plan.contains(r#"<span class="num">5</span> Plan"#),
        "the plan step is the fifth and current:\n{at_plan}"
    );
    assert!(!at_plan.contains("Match"), "no match step:\n{at_plan}");
    assert!(
        at_plan.contains(r#"<span class="num">6</span> Summary"#),
        "summary follows:\n{at_plan}"
    );

    let at_summary = render(steps_at_summary);
    assert!(
        !at_summary.contains("Plan"),
        "no plan step without the stage:\n{at_summary}"
    );
    assert!(
        at_summary.contains(r#"<span class="num">5</span> Summary"#),
        "five steps:\n{at_summary}"
    );
}
