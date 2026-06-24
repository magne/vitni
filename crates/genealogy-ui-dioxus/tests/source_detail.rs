//! SSR assertions for the Source detail (Phase 5 PR9): render the overview (bibliographic facts +
//! the reliability synthesis with its evidence-first cues), the repositories table (call number ·
//! medium · surety), the citations table (page · backs-record · surety · evidence axes), the
//! attributes table (with the no-source flag), and the tags panel. Asserts the confidence cues, the
//! backs-record link, the no-source flag, and that a tag shows its name/colour but never its id.

use dioxus::prelude::*;
use genealogy_app::{CitingKind, TagRef};
use genealogy_ui::{
    CitationRefVm, CitingRecordVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, Localizer, RepositoryLinkVm,
    SourceAttributeVm, SourceCitationVm, SourceDetail, SourceReliabilityVm,
};
use genealogy_ui_dioxus::screens::{
    SourceEditForm, source_attributes_table, source_citations_table, source_overview, source_repositories_table,
    source_tags_panel,
};

/// A representative source detail: an 1850 census with a Normal typical surety, one repository link
/// (microfilm, High surety), a citation backing a person's Birth fact, two attributes (one
/// unsourced), and one tag.
fn sample() -> SourceDetail {
    SourceDetail {
        human_id: "S0003".to_owned(),
        id: "0190-source-id".to_owned(),
        title: "1850 U.S. Federal Census, New York".to_owned(),
        author: Some("U.S. Census Bureau".to_owned()),
        pub_info: Some("NARA microfilm M432, roll 552".to_owned()),
        abbrev: Some("1850-CEN-NY".to_owned()),
        repositories: vec![RepositoryLinkVm {
            human_id: Some("R0004".to_owned()),
            id: Some("0190-repo-id".to_owned()),
            name: "National Archives".to_owned(),
            call_number: Some("M432, roll 552".to_owned()),
            media_type_label: "Film".to_owned(),
            confidence: ConfidenceLevel::High,
            confidence_label: "High".to_owned(),
            source_count: 1,
        }],
        citations: vec![SourceCitationVm {
            citation: CitationRefVm {
                human_id: "C0001".to_owned(),
                source: Some("1850 U.S. Federal Census, New York".to_owned()),
                page: Some("p. 14, dwelling 88".to_owned()),
                confidence: Some(ConfidenceLevel::High),
                confidence_label: Some("High".to_owned()),
                evidence_axes: vec![EvidenceAxisVm {
                    axis: EvidenceAxis::Source,
                    label: "Derivative".to_owned(),
                }],
            },
            backers: vec![CitingRecordVm {
                kind: CitingKind::Person,
                human_id: "I0002".to_owned(),
                id: "0190-person-2".to_owned(),
                label: "John Smith".to_owned(),
                context_label: "Birth".to_owned(),
            }],
        }],
        attributes: vec![
            SourceAttributeVm {
                attribute_type: "microfilm series".to_owned(),
                value: "M432".to_owned(),
                source_count: 1,
            },
            SourceAttributeVm {
                attribute_type: "digitized by".to_owned(),
                value: "NARA, 2009".to_owned(),
                source_count: 0,
            },
        ],
        media: Vec::new(),
        notes: Vec::new(),
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Primary source".to_owned(),
            color: Some("#d6b32e".to_owned()),
            priority: Some(1),
        }],
        reliability: SourceReliabilityVm {
            confidence: Some(ConfidenceLevel::Normal),
            confidence_label: Some("Normal".to_owned()),
            evidence_axes: vec![EvidenceAxisVm {
                axis: EvidenceAxis::Information,
                label: "Primary".to_owned(),
            }],
            citation_count: 42,
            record_count: 31,
        },
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

/// Renders the overview, repositories, citations, attributes, and tags tabs together.
fn source_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<SourceEditForm>);
    let on_submit = use_callback(|_edit: genealogy_ui::SourceEdit| {});
    let detail = sample();
    rsx! {
        {source_overview(&loc, &detail)}
        {source_repositories_table(&loc, &detail)}
        {source_citations_table(&loc, &detail.citations)}
        {source_attributes_table(&loc, &detail)}
        {source_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn overview_shows_bibliographic_facts_and_the_reliability_synthesis() {
    let mut vdom = VirtualDom::new(source_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "1850 U.S. Federal Census, New York", // title
        "U.S. Census Bureau",                 // author
        "1850-CEN-NY",                        // abbreviation
        r#"data-level="normal""#,             // the typical-surety badge colour token
        ">Normal",                            // the surety label (colour is never the only signal)
        "Primary",                            // an evidence axis on the reliability card
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn repositories_and_citations_carry_links_and_evidence() {
    let mut vdom = VirtualDom::new(source_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "National Archives",  // the linked repository
        "M432, roll 552",     // its call number
        "Film",               // its medium
        "p. 14, dwelling 88", // the citation page
        "John Smith — Birth", // the backs-record cell (reverse index + fact sub-context)
        "Derivative",         // an evidence axis on the citation
        "microfilm series",   // an attribute key
        "no-source",          // the unsourced attribute's no-source flag
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(source_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Primary source"), "tag name shown:\n{html}");
    assert!(html.contains("#d6b32e"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
