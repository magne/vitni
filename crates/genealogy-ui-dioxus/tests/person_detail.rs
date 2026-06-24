//! SSR assertions for the Person detail tabs (Phase 5 PR4): render the rich Facts and Names tables
//! and assert the evidence-first cues (confidence badge colour token + redundant text, the
//! no-source flag, the source-count link) and the table roles. Pure render-and-inspect — no window,
//! no workspace — the same pattern as `components.rs`. The editing side panel's `role=dialog`
//! semantics are covered by the `SidePanel` gallery assertion in `components.rs`.

use dioxus::prelude::*;
use genealogy_ui::{
    AssociationVm, CitationRefVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, FactVm, Localizer, NameVm,
};
use genealogy_ui_dioxus::screens::{associations_table, facts_table, names_table, person_citations_table};

/// Renders the Names + Facts tables over representative view-models, in English.
fn person_tables() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let names = vec![NameVm {
        type_label: "Birth name".to_owned(),
        display: "Ada Lovelace".to_owned(),
        given: Some("Ada".to_owned()),
        surname: Some("Lovelace".to_owned()),
        nickname: None,
        date: None,
        language: Some("en".to_owned()),
        confidence: ConfidenceLevel::High,
        confidence_label: "High".to_owned(),
        source_count: 1,
    }];
    let facts = vec![
        FactVm {
            type_label: "Occupation".to_owned(),
            value: Some("Mathematician".to_owned()),
            date: None,
            confidence: ConfidenceLevel::High,
            confidence_label: "High".to_owned(),
            source_count: 2,
        },
        FactVm {
            type_label: "Birth".to_owned(),
            value: None,
            date: Some("1815".to_owned()),
            confidence: ConfidenceLevel::Low,
            confidence_label: "Low".to_owned(),
            source_count: 0,
        },
    ];
    rsx! {
        {names_table(&loc, &names)}
        {facts_table(&loc, &facts)}
    }
}

#[test]
fn facts_and_names_render_tables_with_evidence_cues() {
    let mut vdom = VirtualDom::new(person_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        r#"class="tbl""#,       // both render as the design-system Table
        "Ada Lovelace",         // the name display
        "Mathematician",        // the fact value
        r#"data-level="high""#, // confidence colour token (the high-confidence fact)
        ">High",                // confidence label text — colour is never the only signal
        "2 sources",            // the sourced fact's source-count link
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn an_unsourced_fact_shows_the_no_source_flag() {
    let mut vdom = VirtualDom::new(person_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    // The birth fact has no citations: the flag carries an icon AND text (not colour alone).
    assert!(html.contains(r#"class="no-source""#), "no-source flag class:\n{html}");
    assert!(html.contains("No source"), "no-source flag text:\n{html}");
}

/// Renders the Associations table and the Citations tab over representative view-models.
fn person_evidence_tables() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let associations = vec![AssociationVm {
        other_id: "I0002".to_owned(),
        role_label: "Godparent".to_owned(),
        confidence: ConfidenceLevel::Low,
        confidence_label: "Low".to_owned(),
        source_count: 0,
    }];
    let citations = vec![CitationRefVm {
        human_id: "C0001".to_owned(),
        source: Some("S0001".to_owned()),
        confidence: Some(ConfidenceLevel::High),
        confidence_label: Some("High".to_owned()),
        evidence_axes: vec![EvidenceAxisVm {
            axis: EvidenceAxis::Source,
            label: "Original".to_owned(),
        }],
    }];
    rsx! {
        {associations_table(&loc, &associations)}
        {person_citations_table(&loc, &citations)}
    }
}

#[test]
fn associations_and_citations_carry_evidence_cues() {
    let mut vdom = VirtualDom::new(person_evidence_tables);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Godparent",            // the association role
        r#"data-level="low""#,  // the association's surety token
        r#"class="no-source""#, // the unsourced association's flag
        "S0001",                // the backing citation's source
        "ev source",            // the citation's evidence-axis chip
        "Original",             // the axis value
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}
