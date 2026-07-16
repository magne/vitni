//! SSR assertions for the "Why we believe" provenance popover (anchored, per-claim). SSR renders the
//! initial (closed) state, so these cover the trigger wiring + no-source path, and the popover *body*
//! (`ProvenancePopover` + `provenance_claim_row`) rendered directly — the open/dismiss interaction
//! (click to open, Esc / backdrop to close) is structural and not SSR-clickable.

use dioxus::prelude::*;
use genealogy_ui::{CitationRefVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, Localizer};
use genealogy_ui_dioxus::components::ProvenancePopover;
use genealogy_ui_dioxus::screens::{provenance_claim_row, provenance_cue};
use genealogy_ui_dioxus::shell::nav_state::NavState;

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn citation() -> CitationRefVm {
    CitationRefVm {
        human_id: "C0001".to_owned(),
        source: Some("1850 U.S. Census, NY".to_owned()),
        source_id: Some("S0001".to_owned()),
        page: Some("p. 14".to_owned()),
        backs_count: 0,
        confidence: Some(ConfidenceLevel::High),
        confidence_label: Some("High".to_owned()),
        evidence_axes: vec![EvidenceAxisVm {
            axis: EvidenceAxis::Source,
            label: "Derivative".to_owned(),
        }],
        asserted_by: Some("asserted by magne · 2026-06-22 14:35".to_owned()),
        assertion_id: None,
    }
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// A sourced claim renders a clickable source-count link that opens the popover (a dialog popup),
/// not the popover itself (closed by default).
fn sourced_cue() -> Element {
    let loc = loc();
    rsx! {
        {provenance_cue(&loc, loc.provenance_title_claim("Birth"), &[citation()])}
    }
}

#[test]
fn a_sourced_claim_shows_a_popover_trigger() {
    let html = render(sourced_cue);
    assert!(
        html.contains(r#"class="src-link""#),
        "the source-count link is the trigger:\n{html}"
    );
    assert!(
        html.contains(r#"aria-haspopup="dialog""#),
        "the trigger announces a dialog popup:\n{html}"
    );
    assert!(
        html.contains("1 sources"),
        "the trigger shows the source count:\n{html}"
    );
    // The popover body is closed until activated, so its claim rows are not in the initial render.
    assert!(
        !html.contains(r#"class="prov""#),
        "the popover is closed by default:\n{html}"
    );
}

fn unsourced_cue() -> Element {
    let loc = loc();
    rsx! {
        {provenance_cue(&loc, loc.provenance_title_claim("Birth"), &[])}
    }
}

#[test]
fn an_unsourced_claim_shows_the_no_source_flag_not_a_trigger() {
    let html = render(unsourced_cue);
    assert!(
        html.contains(r#"class="no-source""#),
        "no-source flag, not a trigger:\n{html}"
    );
    assert!(
        !html.contains("aria-haspopup"),
        "an unsourced claim has no popover:\n{html}"
    );
}

/// The popover body, rendered directly (since SSR cannot simulate the open click): the heading plus
/// one claim row with surety, the backing source link, the evidence axis, and the "asserted by" line.
fn popover_body() -> Element {
    // The source link is a RecordLink, which resolves NavState from context.
    use_context_provider(NavState::new);
    let loc = loc();
    let citation = citation();
    rsx! {
        ProvenancePopover { title: loc.provenance_title_claim("Birth"),
            {provenance_claim_row(&citation)}
        }
    }
}

#[test]
fn the_popover_body_lists_the_claims_evidence() {
    let html = render(popover_body);
    for needle in [
        r#"class="prov""#,       // the popover panel
        "Why we believe: Birth", // the per-claim title
        "1850 U.S. Census, NY",  // the backing source label
        "p. 14",                 // the page locator
        ">High",                 // the surety badge label (colour is never the only signal)
        "Derivative",            // the evidence axis value
        "asserted by magne",     // the provenance "asserted by" line
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}
