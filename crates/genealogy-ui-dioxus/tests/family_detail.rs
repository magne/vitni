//! SSR assertions for the Family detail (Phase 5 PR7): render the overview (Partners + Marriage
//! cards with the evidence-first cues), the children table (a relationship column per partner), the
//! events table, and the tags panel, and assert the per-partner relationships, confidence cues, the
//! no-source flag, and that a tag shows its name/colour but never its id. Pure render-and-inspect —
//! the same pattern as `citation_detail.rs`.

use dioxus::prelude::*;
use genealogy_app::TagRef;
use genealogy_ui::{
    CitationRefVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, FamilyChildVm, FamilyDetail, FamilyEventVm,
    FamilyMediaVm, Localizer, PartnerVm,
};
use genealogy_ui_dioxus::screens::{
    FamilyEditForm, family_children_table, family_events_table, family_overview, family_tags_panel,
};

/// A representative family detail: two partners (one sourced with a lifespan, one unsourced), one
/// child with a different relationship to each partner, a High-confidence marriage, and one tag.
/// A representative marriage-register citation, used to back the partner + marriage provenance cues.
fn marriage_citation() -> CitationRefVm {
    CitationRefVm {
        human_id: "C0001".to_owned(),
        source: Some("Trinity Church marriage register".to_owned()),
        source_id: Some("S0003".to_owned()),
        page: Some("vol. 5, f. 18".to_owned()),
        confidence: Some(ConfidenceLevel::High),
        confidence_label: Some("High".to_owned()),
        evidence_axes: vec![EvidenceAxisVm {
            axis: EvidenceAxis::Source,
            label: "Original".to_owned(),
        }],
        asserted_by: Some("asserted by magne · 2026-06-21 16:05".to_owned()),
    }
}

fn sample() -> FamilyDetail {
    FamilyDetail {
        human_id: "F0017".to_owned(),
        id: "0190-family-id".to_owned(),
        title: "Mary Doe & John Smith".to_owned(),
        partners: vec![
            PartnerVm {
                human_id: "I0001".to_owned(),
                name: "Mary Doe".to_owned(),
                vitals: Some("1852 – 1921".to_owned()),
                source_count: 1,
                citations: vec![marriage_citation()],
            },
            PartnerVm {
                human_id: "I0002".to_owned(),
                name: "John Smith".to_owned(),
                vitals: None,
                source_count: 0,
                citations: Vec::new(),
            },
        ],
        marriage: Some(FamilyEventVm {
            human_id: "E0001".to_owned(),
            type_label: "Marriage".to_owned(),
            date: Some("14 Jun 1876".to_owned()),
            place: Some("Trinity Church, New York".to_owned()),
            confidence: ConfidenceLevel::High,
            confidence_label: "High".to_owned(),
            source_count: 1,
            citations: vec![marriage_citation()],
        }),
        children: vec![FamilyChildVm {
            human_id: "I0003".to_owned(),
            name: "Jonathan Smith".to_owned(),
            born: Some("1878".to_owned()),
            relationships: vec![
                ("I0001".to_owned(), "Birth".to_owned()),
                ("I0002".to_owned(), "Step".to_owned()),
            ],
            confidence: ConfidenceLevel::Normal,
            confidence_label: "Normal".to_owned(),
            source_count: 0,
        }],
        events: vec![FamilyEventVm {
            human_id: "E0001".to_owned(),
            type_label: "Marriage".to_owned(),
            date: Some("14 Jun 1876".to_owned()),
            place: Some("Trinity Church, New York".to_owned()),
            confidence: ConfidenceLevel::High,
            confidence_label: "High".to_owned(),
            source_count: 1,
            citations: Vec::new(),
        }],
        media: vec![FamilyMediaVm {
            human_id: "O0001".to_owned(),
            caption: Some("Wedding portrait, 1876".to_owned()),
        }],
        notes: Vec::new(),
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Ancestral line".to_owned(),
            color: Some("#74b449".to_owned()),
            priority: Some(1),
        }],
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

/// Renders the overview, the children table, the events table, and the tags panel together (the tag
/// panel needs a reactive scope for its editing signal + submit callback).
fn family_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<FamilyEditForm>);
    let on_submit = use_callback(|_edit: (genealogy_ui::FamilyEdit, genealogy_ui::ProvenanceDraft)| {});
    let detail = sample();
    rsx! {
        {family_overview(&loc, &detail, editing)}
        {family_children_table(&loc, &detail)}
        {family_events_table(&loc, &detail.events)}
        {family_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn overview_shows_partners_marriage_and_the_evidence_cues() {
    let mut vdom = VirtualDom::new(family_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Mary Doe",    // partner name
        "1852 – 1921", // partner lifespan
        "John Smith",  // the unsourced partner
        "no-source",   // the no-source flag class (colour is never the only signal)
        "14 Jun 1876", // marriage date
        "Trinity Church, New York",
        r#"data-level="high""#, // marriage confidence colour token
        ">High",                // marriage confidence label
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn children_table_has_a_relationship_column_per_partner() {
    let mut vdom = VirtualDom::new(family_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        r#"class="tbl""#, // the children/events tables
        "Jonathan Smith",
        "1878",                   // born
        "Birth",                  // relationship to partner 1
        "Step",                   // relationship to partner 2 (per-partner model)
        r#"data-level="normal""#, // the child surety badge
        "Marriage",               // the events table event-type label
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(family_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Ancestral line"), "tag name shown:\n{html}");
    assert!(html.contains("#74b449"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
