//! SSR assertions for the shared match-compare view (#411; ADR 0038 §3, ADR 0039): one row per term
//! the engine compared, with both values, a non-colour outcome mark and the term's explanation; each
//! side's origin chip and evidence snippet; and the *Same* / *Not the same* / *Decide later* foot with
//! its keys. Pure render-and-inspect over hand-built view-models.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_app::{EngineVersion, MatchBand, MatchEvidence};
use vitni_ui::{CompareRowVm, CompareSideVm, EvidenceSnippetVm, MatchCompareVm, OriginChipVm, RowOutcome};
use vitni_ui_dioxus::components::SelectChoice;
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::screens::{DecisionDraft, MatchCompare, decision_foot};
use vitni_ui_dioxus::shell::ChromeCtx;

fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

fn row(feature: &str, left: Option<&str>, right: Option<&str>, outcome: RowOutcome, explanation: &str) -> CompareRowVm {
    let outcome_label = match outcome {
        RowOutcome::Agree => "same",
        RowOutcome::Partial => "similar",
        RowOutcome::Disagree => "different",
        RowOutcome::Missing => "not compared",
        RowOutcome::Conflict => "conflicting",
    };
    CompareRowVm {
        feature: feature.to_owned(),
        left: left.map(str::to_owned),
        right: right.map(str::to_owned),
        outcome,
        outcome_label: outcome_label.to_owned(),
        explanation: explanation.to_owned(),
    }
}

fn compare_vm() -> MatchCompareVm {
    MatchCompareVm {
        left: CompareSideVm {
            human_id: "I0042".to_owned(),
            label: "Ole Hansen".to_owned(),
            origin: Some(OriginChipVm {
                label: "digitalarkivet · pf01073012345".to_owned(),
                title: "Imported from record pf01073012345 of dataset digitalarkivet".to_owned(),
            }),
            evidence: Some(EvidenceSnippetVm {
                src: "/media/scan.png".to_owned(),
                crop_css: "left:5%;top:40%;width:90%;height:4%".to_owned(),
                caption: "Evidence: O0003".to_owned(),
            }),
        },
        right: CompareSideVm {
            human_id: "I0099".to_owned(),
            label: "Ole Hanssen".to_owned(),
            origin: None,
            evidence: None,
        },
        rows: vec![
            row(
                "Given name",
                Some("Ole"),
                Some("Ole"),
                RowOutcome::Agree,
                "Same given name (+3.0)",
            ),
            row(
                "Birth",
                Some("1852"),
                Some("1849 (Baptism)"),
                RowOutcome::Partial,
                "Similar birth (+2.1)",
            ),
            row(
                "Surname",
                Some("Hansen"),
                None,
                RowOutcome::Missing,
                "No surname to compare",
            ),
        ],
        assessment: MatchEvidence {
            score_bp: 8700,
            band: MatchBand::Probable,
            engine: EngineVersion(4),
            cultures: Vec::new(),
            features: Vec::new(),
        },
        assessment_line: "Matched at 87% · probable match · engine 4".to_owned(),
        earlier_decision: None,
    }
}

fn compare_view() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    rsx! {
        MatchCompare {
            vm: compare_vm(),
            left_caption: "survivor · keeps id".to_owned(),
            right_caption: "becomes a persona".to_owned(),
            ondecide: move |_| {},
            p { "the host foot" }
        }
    }
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn each_compared_term_is_a_row_with_both_values_its_mark_and_its_explanation() {
    let html = render(compare_view);

    for text in ["Given name", "Birth", "Surname", "1849 (Baptism)", "Hansen"] {
        assert!(html.contains(text), "{text} renders:\n{html}");
    }
    assert_eq!(
        html.matches(r#"class="match-mark""#).count(),
        3,
        "one outcome mark per row:\n{html}"
    );
    assert!(
        html.contains(r#"data-outcome="agree""#)
            && html.contains(r#"data-outcome="partial""#)
            && html.contains(r#"data-outcome="missing""#),
        "each mark names its outcome:\n{html}"
    );
    assert!(
        html.contains(r#"<span class="sr-only">similar</span>"#),
        "the outcome is spoken, never colour alone:\n{html}"
    );
    for explanation in [
        "Same given name (+3.0)",
        "Similar birth (+2.1)",
        "No surname to compare",
    ] {
        assert!(html.contains(explanation), "{explanation} explains its row:\n{html}");
    }
    assert!(html.contains("— not recorded"), "a side with no value says so:\n{html}");
}

#[test]
fn the_header_names_both_sides_with_origin_chip_and_evidence_snippet() {
    let html = render(compare_view);

    assert!(
        html.contains("Ole Hansen") && html.contains("Ole Hanssen"),
        "both labels:\n{html}"
    );
    assert!(
        html.contains("survivor · keeps id") && html.contains("becomes a persona"),
        "the host's captions:\n{html}"
    );
    assert!(
        html.contains("digitalarkivet · pf01073012345")
            && html.contains(r#"title="Imported from record pf01073012345 of dataset digitalarkivet""#),
        "the origin chip with its dataset tooltip:\n{html}"
    );
    assert_eq!(
        html.matches(r#"class="chip origin-chip""#).count(),
        1,
        "only the imported side:\n{html}"
    );
    assert!(
        html.contains(r#"src="/media/scan.png""#)
            && html.contains(r#"style="left:5%;top:40%;width:90%;height:4%""#)
            && html.contains("Evidence: O0003"),
        "the scan with its region outlined:\n{html}"
    );
    assert_eq!(
        html.matches("evidence-snippet").count(),
        1,
        "only the side with a scan:\n{html}"
    );
}

#[test]
fn the_view_is_a_focusable_region_wrapping_the_hosts_content() {
    let html = render(compare_view);

    assert!(
        html.contains(r#"id="match-compare""#) && html.contains(r#"tabindex="-1""#),
        "the view takes focus so its keys work:\n{html}"
    );
    assert!(
        html.contains("the host foot"),
        "the host's content renders inside:\n{html}"
    );
    assert!(
        html.contains("Matched at 87% · probable match · engine 4"),
        "the assessment the decision records:\n{html}"
    );
}

fn foot() -> Element {
    let chrome = chrome("en");
    let draft = use_signal(DecisionDraft::default);
    let options = vec![SelectChoice {
        value: "0".to_owned(),
        label: "Very low".to_owned(),
    }];
    decision_foot(&chrome, options, draft, use_callback(|_| {}))
}

#[test]
fn the_foot_offers_three_decisions_each_with_its_key() {
    let html = render(foot);

    for (label, key) in [("Decide later", "l"), ("Not the same", "n"), ("Same (reversible)", "y")] {
        assert!(html.contains(label), "{label} renders:\n{html}");
        assert!(
            html.contains(&format!(r#"aria-keyshortcuts="{key}""#)),
            "{label} announces its key:\n{html}"
        );
        assert!(
            html.contains(&format!("<kbd>{}</kbd>", key.to_uppercase())),
            "{label} shows its key:\n{html}"
        );
    }
    assert!(!html.contains(">Cancel<"), "Decide later replaces Cancel:\n{html}");
    assert!(html.contains(r#"id="merge-reason""#), "the reason field stays:\n{html}");
}

fn norwegian_foot() -> Element {
    let chrome = chrome("no");
    let draft = use_signal(DecisionDraft::default);
    decision_foot(&chrome, Vec::new(), draft, use_callback(|_| {}))
}

#[test]
fn the_decisions_are_localized() {
    let html = render(norwegian_foot);

    assert!(html.contains("Avgjør senere"), "Norwegian Decide later:\n{html}");
    assert!(html.contains("Ikke den samme"), "Norwegian Not the same:\n{html}");
}
