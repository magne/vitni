//! SSR assertions for the Source detail (Phase 5 PR9): render the overview (bibliographic facts +
//! the reliability synthesis with its evidence-first cues), the repositories table (call number ·
//! medium · surety), the citations table (page · backs-record · surety · evidence axes), the
//! attributes table, and the tags panel. Asserts the confidence cues, the backs-record link, and
//! that a tag shows its name/colour but never its id.

use dioxus::prelude::*;
use genealogy_app::{CitingKind, SourceMediaType, TagRef};
use genealogy_ui::{
    AttachedRefVm, CitationRefVm, CitingRecordVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, FamilyMediaVm,
    Localizer, RepositoryLinkVm, SourceAttributeVm, SourceCitationVm, SourceDetail, SourceReliabilityVm,
};
use genealogy_ui::{ProvenanceDraft, SourceDraft};
use genealogy_ui_dioxus::screens::{
    RecordActionLabels, RecordEditState, SourceEditForm, family_media_gallery, id_list, record_head_actions,
    source_attributes_table, source_citations_table, source_overview, source_repositories_table, tags_panel,
};

/// A representative source detail: an 1850 census with a Normal typical surety, one repository link
/// (microfilm, High surety), a citation backing a person's Birth fact, two attributes, and one tag.
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
            media_type: SourceMediaType::Film,
            media_type_label: "Film".to_owned(),
            confidence: Some(ConfidenceLevel::High),
            confidence_label: "High".to_owned(),
            source_count: 1,
            assertion_id: "0190-repo-link-assert-1".to_owned(),
        }],
        citations: vec![SourceCitationVm {
            citation: CitationRefVm {
                human_id: "C0001".to_owned(),
                source: Some("1850 U.S. Federal Census, New York".to_owned()),
                source_id: Some("S0001".to_owned()),
                page: Some("p. 14, dwelling 88".to_owned()),
                confidence: Some(ConfidenceLevel::High),
                confidence_label: Some("High".to_owned()),
                evidence_axes: vec![EvidenceAxisVm {
                    axis: EvidenceAxis::Source,
                    label: "Derivative".to_owned(),
                }],
                asserted_by: Some("asserted by magne · 2026-06-22 14:35".to_owned()),
                assertion_id: None,
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
                assertion_id: "0190-attr-assert-1".to_owned(),
            },
            SourceAttributeVm {
                attribute_type: "digitized by".to_owned(),
                value: "NARA, 2009".to_owned(),
                assertion_id: "0190-attr-assert-2".to_owned(),
            },
        ],
        media: vec![FamilyMediaVm {
            human_id: "O0004".to_owned(),
            caption: None,
            assertion_id: "0190-media-attach-1".to_owned(),
        }],
        notes: vec![AttachedRefVm {
            human_id: "N0004".to_owned(),
            assertion_id: "0190-note-attach-1".to_owned(),
        }],
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

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn state(editing: bool) -> RecordEditState<SourceDraft> {
    let seed = SourceDraft::from_detail(&sample());
    RecordEditState {
        editing: use_signal(move || editing),
        seed: use_signal({
            let seed = seed.clone();
            move || seed
        }),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    }
}

/// Renders the overview, repositories, citations, attributes, and tags tabs together.
fn source_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let detail = sample();
    let onedit = use_callback(|_| {});
    let onretract = use_callback(|_: (String, String, bool)| {});
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (SourceDraft, ProvenanceDraft)| {}))}
        {source_overview(&loc, &detail, record)}
        {source_repositories_table(&loc, &detail, onedit, onretract)}
        {source_citations_table(&loc, &detail.citations)}
        {source_attributes_table(&loc, &detail, onedit, onretract)}
        {family_media_gallery(&loc, &detail.media, Some(onretract))}
        {id_list(&loc, &detail.notes, Some(onretract))}
        {tags_panel(&loc, &detail.tags, use_signal(|| None::<SourceEditForm>), SourceEditForm::Tag, use_callback(|_: String| {}))}
    }
}

fn source_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (SourceDraft, ProvenanceDraft)| {}))}
        {source_overview(&loc, &detail, record)}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let mut vdom = VirtualDom::new(source_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains(">Edit<"), "view mode offers Edit:\n{html}");
    assert!(!html.contains("<input"), "no live inputs in view mode:\n{html}");
}

#[test]
fn edit_mode_swaps_in_the_inputs_and_header_actions() {
    let mut vdom = VirtualDom::new(source_edit);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains("<input"), "edit mode swaps in inputs:\n{html}");
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="source-title""#),
        "the title input is present:\n{html}"
    );
    assert!(
        html.contains(r#"id="source-id""#),
        "the editable human id is present:\n{html}"
    );
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

/// Renders only the reverse-index Citations table (citations that cite this source) — it carries no
/// per-row corrections, so it must render no row-action buttons.
fn citations_only() -> Element {
    let loc = loc();
    let detail = sample();
    rsx! { {source_citations_table(&loc, &detail.citations)} }
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn repository_rows_carry_edit_and_unlink_with_row_scoped_labels() {
    let html = render(source_view);
    assert!(
        html.contains(r#"aria-label="Edit National Archives""#),
        "the repository row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Unlink National Archives""#),
        "the repository row Unlink carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains("Unlink this repository — it stays in History"),
        "the Unlink button carries the unlink-repository tooltip:\n{html}"
    );
}

#[test]
fn attribute_rows_carry_edit_and_retract_with_row_scoped_labels() {
    let html = render(source_view);
    assert!(
        html.contains(r#"aria-label="Edit microfilm series""#),
        "the attribute row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract microfilm series""#),
        "the attribute row Retract carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains("Retract this assertion — it stays in History"),
        "the Retract button carries the retract-title tooltip:\n{html}"
    );
}

#[test]
fn reverse_index_citations_table_has_no_row_actions() {
    let html = render(citations_only);
    assert!(!html.contains("row-actions"), "no row-actions cell:\n{html}");
    for needle in [">Edit<", ">Retract<", ">Detach<", ">Unlink<"] {
        assert!(
            !html.contains(needle),
            "the reverse-index citations table carries no {needle:?}:\n{html}"
        );
    }
}

#[test]
fn citations_table_header_uses_the_confidence_term_not_surety() {
    // Review finding X4 / locked decision (docs/archive/phase5/plan.md:167): the UI term for the
    // `Confidence` value object is "Confidence". The citations table's confidence column previously
    // read "Surety".
    let html = render(citations_only);
    assert!(
        html.contains(">Confidence<"),
        "the citations table's confidence column uses the Confidence term:\n{html}"
    );
    assert!(
        !html.contains("Surety"),
        "the legacy 'Surety' term is gone from the citations table:\n{html}"
    );
}

#[test]
fn notes_and_media_carry_detach() {
    let html = render(source_view);
    assert!(
        html.contains(r#"aria-label="Detach O0004""#),
        "the attached media carries a Detach:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Detach N0004""#),
        "the attached note carries a Detach:\n{html}"
    );
}

#[test]
fn no_assertion_id_is_ever_rendered() {
    let html = render(source_view);
    for assertion_id in [
        "0190-repo-link-assert-1",
        "0190-attr-assert-1",
        "0190-attr-assert-2",
        "0190-media-attach-1",
        "0190-note-attach-1",
        "0190-secret-tag-id",
    ] {
        assert!(
            !html.contains(assertion_id),
            "assertion/tag id {assertion_id:?} must never be rendered:\n{html}"
        );
    }
}
