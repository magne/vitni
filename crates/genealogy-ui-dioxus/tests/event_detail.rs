//! SSR assertions for the Event detail (Phase 5 PR8): render the overview (type/date/place facts
//! with the evidence-first cues), the participants table (role + surety + source), the citations
//! table (source · page · surety · evidence axes), and the tags panel. Asserts the confidence cues,
//! the no-source flag, and that a tag shows its name/colour but never its id. Pure render-and-inspect.

use dioxus::prelude::*;
use genealogy_app::TagRef;
use genealogy_ui::{
    CitationRefVm, ConfidenceLevel, EventDetail, EvidenceAxis, EvidenceAxisVm, Localizer, ParticipantVm, PlaceLinkVm,
};
use genealogy_ui_dioxus::screens::{
    EventEditForm, citation_table, event_overview, event_participants_table, event_tags_panel,
};

/// A representative event detail: a marriage with a High-confidence date, a linked place, two
/// participants (one sourced, one not), a citation with evidence axes, and one tag.
fn sample() -> EventDetail {
    EventDetail {
        human_id: "E0101".to_owned(),
        id: "0190-event-id".to_owned(),
        title: "Marriage".to_owned(),
        type_label: "Marriage".to_owned(),
        date: Some("14 Jun 1876".to_owned()),
        date_confidence: Some(ConfidenceLevel::High),
        date_confidence_label: Some("High".to_owned()),
        date_source_count: 1,
        place: Some(PlaceLinkVm {
            human_id: "P0021".to_owned(),
            id: "0190-place-id".to_owned(),
            name: "Trinity Church, New York".to_owned(),
        }),
        place_confidence: Some(ConfidenceLevel::High),
        place_confidence_label: Some("High".to_owned()),
        description: Some("Solemnized before two witnesses.".to_owned()),
        participants: vec![
            ParticipantVm {
                human_id: "I0002".to_owned(),
                id: "0190-person-2".to_owned(),
                name: "John Smith".to_owned(),
                role_label: "Groom".to_owned(),
                confidence: ConfidenceLevel::High,
                confidence_label: "High".to_owned(),
                source_count: 1,
            },
            ParticipantVm {
                human_id: "I0004".to_owned(),
                id: "0190-person-4".to_owned(),
                name: "Anna Berg".to_owned(),
                role_label: "Witness".to_owned(),
                confidence: ConfidenceLevel::Low,
                confidence_label: "Low".to_owned(),
                source_count: 0,
            },
        ],
        citations: vec![CitationRefVm {
            human_id: "C0001".to_owned(),
            source: Some("Trinity Church marriages".to_owned()),
            page: Some("vol. 5, f. 18".to_owned()),
            confidence: Some(ConfidenceLevel::High),
            confidence_label: Some("High".to_owned()),
            evidence_axes: vec![EvidenceAxisVm {
                axis: EvidenceAxis::Source,
                label: "Original".to_owned(),
            }],
        }],
        media: Vec::new(),
        notes: Vec::new(),
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Verified event".to_owned(),
            color: Some("#b07cf0".to_owned()),
            priority: Some(1),
        }],
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

/// Renders the overview, the participants table, the citations table, and the tags panel together.
fn event_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<EventEditForm>);
    let on_submit = use_callback(|_edit: genealogy_ui::EventEdit| {});
    let detail = sample();
    rsx! {
        {event_overview(&loc, &detail, editing)}
        {event_participants_table(&loc, &detail)}
        {citation_table(&loc, &detail.citations)}
        {event_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn overview_shows_type_date_place_and_the_evidence_cues() {
    let mut vdom = VirtualDom::new(event_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Marriage",                 // type label
        "14 Jun 1876",              // date
        "Trinity Church, New York", // linked place name
        r#"data-level="high""#,     // confidence colour token
        ">High",                    // confidence label (colour is never the only signal)
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn participants_and_citations_carry_roles_and_evidence() {
    let mut vdom = VirtualDom::new(event_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        r#"class="tbl""#,           // the tables
        "John Smith",               // a participant
        "Groom",                    // their role
        "Anna Berg",                // the unsourced witness
        "no-source",                // the no-source flag (colour is never the only signal)
        "Trinity Church marriages", // the cited source
        "vol. 5, f. 18",            // the page
        "Original",                 // an evidence axis
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(event_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Verified event"), "tag name shown:\n{html}");
    assert!(html.contains("#b07cf0"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
