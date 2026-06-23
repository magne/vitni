//! SSR assertions for the Person detail tabs (Phase 5 PR4): render the rich Facts and Names tables
//! and assert the evidence-first cues (confidence badge colour token + redundant text, the
//! no-source flag, the source-count link) and the table roles. Pure render-and-inspect — no window,
//! no workspace — the same pattern as `components.rs`. The editing side panel's `role=dialog`
//! semantics are covered by the `SidePanel` gallery assertion in `components.rs`.

use dioxus::prelude::*;
use genealogy_ui::{ConfidenceLevel, FactVm, Localizer, NameVm};
use genealogy_ui_dioxus::screens::{facts_table, names_table};

/// Renders the Names + Facts tables over representative view-models, in English.
fn person_tables() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let names = vec![NameVm {
        type_label: "Birth name".to_owned(),
        display: "Ada Lovelace".to_owned(),
        given: Some("Ada".to_owned()),
        surname: Some("Lovelace".to_owned()),
        nickname: None,
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
