//! SSR assertions for the assisted-import wizard's match stage (ADR 0040 §4; `import.html`): the host's
//! own stage, shown only when a record has a possible match — the shared compare view with the stored
//! record on the left, the three decisions plus *Skip record* and *Cancel import*, and a *Match* step
//! that the step indicator shows only while the stage is up.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_app::{EngineVersion, MatchBand, MatchEvidence, MatchableKind};
use vitni_ui::{CompareSideVm, ImportStage, MatchCompareVm, MatchStageVm, OriginChipVm, SummaryPayload};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::screens::{MatchStage, MatchStageLabels, WizardLabels, step_indicator};
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

fn stage() -> MatchStageVm {
    MatchStageVm {
        kind: MatchableKind::Person,
        compare: MatchCompareVm {
            left: CompareSideVm {
                human_id: "I0001".to_owned(),
                label: "Ola Fjellstue".to_owned(),
                origin: None,
                evidence: None,
            },
            right: CompareSideVm {
                human_id: "new".to_owned(),
                label: "Ola Eksempelsen Fjellstue".to_owned(),
                origin: Some(OriginChipVm {
                    label: "digitalarkivet · pf01099901000101".to_owned(),
                    title: "Imported from record pf01099901000101 of dataset digitalarkivet".to_owned(),
                }),
                evidence: None,
            },
            rows: Vec::new(),
            assessment: MatchEvidence {
                score_bp: 8100,
                band: MatchBand::Possible,
                engine: EngineVersion(4),
                cultures: Vec::new(),
                features: Vec::new(),
            },
            assessment_line: "Matched at 81% · possible match · engine 4".to_owned(),
            earlier_decision: None,
        },
        position: 1,
        total: 2,
        group: None,
    }
}

fn labels() -> MatchStageLabels {
    MatchStageLabels {
        position: "Possible match 1 of 2".to_owned(),
        heading: "Is this person already in your tree?".to_owned(),
        left_caption: "in your tree · keeps its id".to_owned(),
        right_caption: "from this record".to_owned(),
        skip: "Skip record".to_owned(),
        cancel: "Cancel import".to_owned(),
    }
}

fn match_view() -> Element {
    use_context_provider(|| ChromeCtx(chrome()));
    rsx! {
        MatchStage { stage: stage(), labels: labels(), confidence_options: Vec::new(), onanswer: |_| {} }
    }
}

#[test]
fn the_match_stage_compares_the_stored_record_with_the_incoming_one() {
    let html = render(match_view);

    assert!(html.contains("Possible match 1 of 2"), "position:\n{html}");
    assert!(
        html.contains("Is this person already in your tree?"),
        "heading:\n{html}"
    );
    assert!(
        html.contains(r#"id="match-compare""#),
        "the shared compare view:\n{html}"
    );
    let stored = html.find("Ola Fjellstue").expect("the stored record");
    let incoming = html.find("Ola Eksempelsen Fjellstue").expect("the incoming record");
    assert!(stored < incoming, "the stored record is on the left:\n{html}");
    assert!(html.contains("in your tree · keeps its id"), "left caption:\n{html}");
    assert!(
        html.contains("digitalarkivet · pf01099901000101"),
        "the incoming origin:\n{html}"
    );
}

#[test]
fn the_match_stage_offers_three_decisions_skip_and_cancel() {
    let html = render(match_view);

    for label in [
        "Decide later",
        "Not the same",
        "Same (reversible)",
        "Skip record",
        "Cancel import",
    ] {
        assert!(html.contains(label), "{label} renders:\n{html}");
    }
}

fn wizard_labels() -> WizardLabels {
    WizardLabels {
        heading: "Import stages".to_owned(),
        stages: ["Source", "Records", "Confirm", "Save scan", "Match", "Summary"].map(str::to_owned),
    }
}

fn steps_at_match() -> Element {
    use_context_provider(|| ChromeCtx(chrome()));
    step_indicator(&wizard_labels(), &ImportStage::Match(Box::new(stage())))
}

fn steps_at_summary() -> Element {
    let summary = ImportStage::Summary(SummaryPayload {
        imported: Vec::new(),
        skipped: 0,
    });
    step_indicator(&wizard_labels(), &summary)
}

#[test]
fn the_match_step_shows_only_while_the_match_stage_is_up() {
    let at_match = render(steps_at_match);
    assert!(at_match.contains("Match"), "the match step:\n{at_match}");
    assert!(
        at_match.contains(r#"aria-current="step""#) && at_match.contains(r#"<span class="num">5</span> Match"#),
        "the match step is the fifth and current:\n{at_match}"
    );
    assert!(
        at_match.contains(r#"<span class="num">6</span> Summary"#),
        "summary follows:\n{at_match}"
    );

    let at_summary = render(steps_at_summary);
    assert!(
        !at_summary.contains("Match"),
        "no match step without the stage:\n{at_summary}"
    );
    assert!(
        at_summary.contains(r#"<span class="num">5</span> Summary"#),
        "five steps:\n{at_summary}"
    );
}
