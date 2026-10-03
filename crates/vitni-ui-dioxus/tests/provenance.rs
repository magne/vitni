//! SSR assertions for the "Why we believe" provenance popover (anchored, per-claim). SSR renders the
//! initial (closed) state, so these cover the trigger wiring + no-source path, and the popover *body*
//! (`ProvenancePopover` + `provenance_claim_row`) rendered directly — the open/dismiss interaction
//! (click to open, Esc / backdrop to close) is structural and not SSR-clickable.

use dioxus::prelude::*;
use vitni_ui::{CitationRefVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, Localizer};
use vitni_ui_dioxus::components::ProvenancePopover;
use vitni_ui_dioxus::screens::{provenance_claim_row, provenance_cue, provenance_origin_row};
use vitni_ui_dioxus::shell::nav_state::NavState;

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
        {provenance_cue(&loc, loc.provenance_title_claim("Birth"), &[citation()], None)}
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
        {provenance_cue(&loc, loc.provenance_title_claim("Birth"), &[], None)}
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

fn origin() -> vitni_ui::OriginVm {
    vitni_ui::OriginVm {
        label: "record pf01 in Digitalarkivet".to_owned(),
        url: Some("https://www.digitalarkivet.no/pf01".to_owned()),
        source: Some("1910 census".to_owned()),
    }
}

/// A claim read from an import record with no citation still opens *Why we believe*, beside the
/// no-source flag, since the record it came from is evidence of where it was read (ADR 0037 §2).
fn imported_unsourced_cue() -> Element {
    let loc = loc();
    let origin = origin();
    rsx! {
        {provenance_cue(&loc, loc.provenance_title_claim("Birth"), &[], Some(&origin))}
    }
}

#[test]
fn an_imported_claim_without_a_citation_still_opens_the_popover() {
    let html = render(imported_unsourced_cue);
    assert!(
        html.contains(r#"class="no-source""#),
        "still flagged unsourced:\n{html}"
    );
    assert!(html.contains(r#"aria-haspopup="dialog""#), "and a trigger:\n{html}");
}

/// The origin row names the record and links out to its page.
fn origin_row() -> Element {
    let loc = loc();
    let origin = origin();
    rsx! {
        ProvenancePopover { title: loc.provenance_title_claim("Birth"),
            {provenance_origin_row(&loc, &origin)}
        }
    }
}

#[test]
fn the_popover_names_the_origin_record_and_links_out() {
    let html = render(origin_row);
    for needle in [
        "from ",
        r#"href="https://www.digitalarkivet.no/pf01""#,
        "record pf01 in Digitalarkivet",
        "1910 census",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}
