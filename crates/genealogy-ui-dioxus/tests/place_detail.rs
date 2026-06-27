//! SSR assertions for the Place detail (Phase 5 PR8): render the overview (type/coordinates/code
//! facts with the evidence-first cues), the names table (language + date + surety + source), the
//! hierarchy table (the dated jurisdiction chain with surety), and the tags panel. Asserts the
//! confidence cues, the no-source flag, and that a tag shows its name/colour but never its id.

use dioxus::prelude::*;
use genealogy_app::TagRef;
use genealogy_ui::{ConfidenceLevel, Localizer, PlaceDetail, PlaceHierarchyVm, PlaceNameVm};
use genealogy_ui_dioxus::screens::{
    PlaceEditForm, place_hierarchy_table, place_names_table, place_overview, place_tags_panel,
};

/// A representative place detail: a city with High-confidence coordinates, two names (one sourced,
/// one not), a two-level jurisdiction chain, and one tag.
fn sample() -> PlaceDetail {
    PlaceDetail {
        human_id: "P0007".to_owned(),
        id: "0190-place-id".to_owned(),
        title: "New York".to_owned(),
        type_label: Some("City".to_owned()),
        coordinates: Some("40.7128, -74.0060".to_owned()),
        coordinates_confidence: Some(ConfidenceLevel::High),
        coordinates_confidence_label: Some("High".to_owned()),
        code: Some("GeoNames 5128581".to_owned()),
        names: vec![
            PlaceNameVm {
                text: "New York".to_owned(),
                language: Some("en".to_owned()),
                date: Some("1664".to_owned()),
                confidence: ConfidenceLevel::VeryHigh,
                confidence_label: "Very high".to_owned(),
                source_count: 1,
            },
            PlaceNameVm {
                text: "Nieuw Amsterdam".to_owned(),
                language: Some("nl".to_owned()),
                date: None,
                confidence: ConfidenceLevel::Normal,
                confidence_label: "Normal".to_owned(),
                source_count: 0,
            },
        ],
        hierarchy: vec![
            PlaceHierarchyVm {
                human_id: "P0050".to_owned(),
                id: "0190-county".to_owned(),
                name: "New York County".to_owned(),
                type_label: Some("County".to_owned()),
                date: Some("1683 –".to_owned()),
                confidence: ConfidenceLevel::High,
                confidence_label: "High".to_owned(),
            },
            PlaceHierarchyVm {
                human_id: "P0001".to_owned(),
                id: "0190-country".to_owned(),
                name: "United States".to_owned(),
                type_label: Some("Country".to_owned()),
                date: Some("1788 –".to_owned()),
                confidence: ConfidenceLevel::High,
                confidence_label: "High".to_owned(),
            },
        ],
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Home town".to_owned(),
            color: Some("#6cb6ff".to_owned()),
            priority: Some(1),
        }],
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

/// Renders the overview, the names table, the hierarchy table, and the tags panel together.
fn place_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<PlaceEditForm>);
    let on_submit = use_callback(|_edit: genealogy_ui::PlaceEdit| {});
    let detail = sample();
    rsx! {
        {place_overview(&loc, &detail, editing)}
        {place_names_table(&loc, &detail)}
        {place_hierarchy_table(&loc, &detail)}
        {place_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn overview_shows_type_coordinates_and_the_evidence_cues() {
    let mut vdom = VirtualDom::new(place_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "City",                 // type label
        "40.7128, -74.0060",    // coordinates
        "GeoNames 5128581",     // code
        r#"data-level="high""#, // confidence colour token
        ">High",                // confidence label
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn names_and_hierarchy_carry_language_dates_and_surety() {
    let mut vdom = VirtualDom::new(place_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        r#"class="tbl""#,            // the tables
        "New York",                  // primary name
        "Nieuw Amsterdam",           // the dated/historical name
        "no-source",                 // the unsourced name's flag
        r#"data-level="very-high""#, // the primary name's surety
        "New York County",           // a jurisdiction level
        "1683 –",                    // the dated link
        "United States",             // the next level up
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(place_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Home town"), "tag name shown:\n{html}");
    assert!(html.contains("#6cb6ff"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
