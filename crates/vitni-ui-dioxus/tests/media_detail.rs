//! SSR assertions for the Media detail (Phase 5 PR27): the read-first Overview record (id · paths ·
//! MIME, with the checksum locked and the date a structured editor), its edit mode swapping in inputs
//! plus the sticky-header Cancel/Save, the citations table, and the tags panel.

use dioxus::prelude::*;
use vitni_app::{
    Calendar, DateInput, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody, TagRef,
    UsingKind, build_genealogical_date,
};
use vitni_ui::{
    AttachedRefVm, CitationRefVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, Localizer, MediaAttributeVm,
    MediaDetail, MediaDraft, ProvenanceDraft, UsingRecordVm,
};

/// The sample media's structured date: about 1900 on the Gregorian calendar.
fn sample_date() -> GenealogicalDate {
    build_genealogical_date(DateInput {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        body: GenealogicalDateBody::Structured(DateModifier::About(DatePoint {
            year: Some(1900),
            month: None,
            day: None,
        })),
        new_year_begins: None,
        original_text: None,
        time: None,
    })
}
use vitni_ui_dioxus::screens::{
    MediaEditForm, RecordActionLabels, RecordEditState, citations_table, media_attributes_table, media_overview,
    note_cards, record_head_actions, tags_panel,
};

/// A representative media detail: a portrait JPEG with file metadata, one backing citation (Normal
/// surety, an Original evidence axis), one referencing record (a person), and one tag.
fn sample() -> MediaDetail {
    MediaDetail {
        human_id: "O0050".to_owned(),
        id: "0190-media-id".to_owned(),
        title: "john-smith-portrait.jpg".to_owned(),
        path: Some("media/portraits/john-smith-portrait.jpg".to_owned()),
        file_path: Some("media/portraits/john-smith-portrait.jpg".to_owned()),
        web_path: None,
        mime: Some("image/jpeg".to_owned()),
        checksum: Some("sha256:9f3a8c12d4e7b6a05f1e".to_owned()),
        date: Some("c. 1900".to_owned()),
        date_value: Some(sample_date()),
        attributes: vec![MediaAttributeVm {
            attribute_type: "dimensions".to_owned(),
            value: "1024x1536".to_owned(),
            assertion_id: "0190-attr-assertion-id".to_owned(),
        }],
        citations: vec![
            CitationRefVm {
                human_id: "C0007".to_owned(),
                source: Some("Family photo collection".to_owned()),
                source_id: None,
                page: Some("album 2, p.4".to_owned()),
                backs_count: 0,
                confidence: Some(ConfidenceLevel::Normal),
                confidence_label: Some("Normal".to_owned()),
                evidence_axes: vec![EvidenceAxisVm {
                    axis: EvidenceAxis::Source,
                    label: "Original".to_owned(),
                }],
                asserted_by: None,
                assertion_id: Some("0190-citation-attach-id".to_owned()),
            },
            CitationRefVm {
                human_id: "C0008".to_owned(),
                source: Some("Unsourced observation".to_owned()),
                source_id: None,
                page: None,
                backs_count: 0,
                confidence: None,
                confidence_label: None,
                evidence_axes: Vec::new(),
                asserted_by: None,
                assertion_id: None,
            },
        ],
        notes: vec![AttachedRefVm {
            human_id: "N0004".to_owned(),
            note_type: Some(vitni_app::NoteType::General),
            type_label: Some("General".to_owned()),
            text: Some("Studio portrait of John Smith, seated, c.1900. Photographer's mark on the verso.".to_owned()),
            language: Some("en".to_owned()),
            assertion_id: "0190-note-attach-id".to_owned(),
        }],
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Direct ancestor".to_owned(),
            color: Some("#e5534b".to_owned()),
            priority: Some(1),
        }],
        used_by: vec![UsingRecordVm {
            kind: UsingKind::Person,
            human_id: "I0042".to_owned(),
            id: "0190-person-42".to_owned(),
            label: "John Smith".to_owned(),
            kind_label: "Person".to_owned(),
        }],
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn state(editing: bool) -> RecordEditState<MediaDraft> {
    let seed = MediaDraft::from_detail(&sample());
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

fn media_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let on_edit_open = use_callback(|_: MediaEditForm| {});
    let on_retract = use_callback(|_: (String, String, bool)| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (MediaDraft, ProvenanceDraft)| {}))}
        {media_overview(&loc, &detail, record)}
        {media_attributes_table(&loc, &detail.attributes, on_edit_open, on_retract)}
        {citations_table::<MediaEditForm>(&loc, &detail.citations, false, on_retract)}
        {note_cards(&loc, &detail.notes, Some(on_retract))}
        {tags_panel(&loc, &detail.tags, use_callback(|_: (String, String)| {}))}
    }
}

fn media_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (MediaDraft, ProvenanceDraft)| {}))}
        {media_overview(&loc, &detail, record)}
    }
}

/// The Overview of a record with no recorded MIME — the state every record the CLI creates is in, since
/// `vitni media` has no way to set one (#301 cause 1).
fn media_view_without_mime() -> Element {
    let loc = loc();
    let record = state(false);
    let detail = MediaDetail { mime: None, ..sample() };
    rsx! {
        {media_overview(&loc, &detail, record)}
    }
}

/// The Overview of a record whose file lives outside the workspace, stored absolute — legitimate, but
/// not something the media-root asset handler will serve.
fn media_view_outside_the_workspace() -> Element {
    let loc = loc();
    let record = state(false);
    let detail = MediaDetail {
        file_path: Some("/home/ada/photos/john-smith-portrait.jpg".to_owned()),
        ..sample()
    };
    rsx! {
        {media_overview(&loc, &detail, record)}
    }
}

/// The Overview of a record named the way the app's own conventions name one: `slugify` keeps `æøå`
/// (`media_save.rs`) and an operator's own file may carry a space, so the stored path is not ASCII.
fn media_view_with_a_nordic_filename() -> Element {
    let loc = loc();
    let record = state(false);
    let detail = MediaDetail {
        file_path: Some("media/02_folketelling/1920 greipstad_bergstøl-asbjørn.jpg".to_owned()),
        ..sample()
    };
    rsx! {
        {media_overview(&loc, &detail, record)}
    }
}

#[test]
fn the_preview_of_a_nordic_or_spaced_filename_is_served_percent_encoded() {
    // The webview encodes whatever `src` it is handed, so a raw `ø` reached the asset handler as
    // `%C3%B8` and resolved to no file. Encoding here makes the request the handler can decode.
    let html = render(media_view_with_a_nordic_filename);
    assert!(
        html.contains(r#"src="/media/02_folketelling/1920%20greipstad_bergst%C3%B8l-asbj%C3%B8rn.jpg""#),
        "the preview img src is percent-encoded per segment:\n{html}"
    );
    assert!(
        !html.contains("src=\"/media/02_folketelling/1920 greipstad"),
        "no raw space or non-ASCII byte reaches the src:\n{html}"
    );
    assert!(!html.contains("📷"), "the image still previews, not the glyph:\n{html}");
}

#[test]
fn the_preview_serves_the_stored_path_with_exactly_one_media_prefix() {
    // #301 cause 2: the stored `file_path` already carries `media/`, and the URL builder prepended it
    // again, so the webview asked for `/media/media/…` and the asset handler resolved nothing.
    let html = render(media_view);
    assert!(
        html.contains(r#"src="/media/portraits/john-smith-portrait.jpg""#),
        "the preview img is served from the media root, once:\n{html}"
    );
    assert!(
        !html.contains("/media/media/"),
        "the prefix is added exactly once:\n{html}"
    );
}

#[test]
fn a_record_with_no_recorded_mime_still_previews_its_image() {
    // #301 cause 1: the image gate read only the recorded MIME, so a record created without one
    // rendered the 📷 placeholder forever.
    let html = render(media_view_without_mime);
    assert!(
        html.contains(r#"src="/media/portraits/john-smith-portrait.jpg""#),
        "the extension classifies the file when no MIME is recorded:\n{html}"
    );
    assert!(!html.contains("📷"), "no glyph placeholder for an image:\n{html}");
}

#[test]
fn a_file_outside_the_workspace_previews_the_placeholder_not_a_broken_image() {
    let html = render(media_view_outside_the_workspace);
    assert!(html.contains("📷"), "the glyph placeholder is honest:\n{html}");
    assert!(
        !html.contains("<img"),
        "an unservable location renders no img at all:\n{html}"
    );
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(media_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit:\n{html}");
    assert!(!html.contains("<input"), "no live inputs in view mode:\n{html}");
    for needle in [
        "media/portraits/john-smith-portrait.jpg",
        "image/jpeg",
        "sha256:9f3a8c12d4e7b6a05f1e",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn edit_mode_swaps_in_the_inputs_and_header_actions() {
    let html = render(media_edit);
    assert!(html.contains("<input"), "edit mode swaps in inputs:\n{html}");
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="media-file-path""#),
        "the file-path input is present:\n{html}"
    );
    assert!(
        html.contains(r#"id="media-mime""#),
        "the MIME input is present:\n{html}"
    );
}

#[test]
fn the_checksum_is_locked_and_the_date_is_a_structured_editor() {
    let html = render(media_edit);
    assert!(
        html.contains(r#"id="media-checksum""#),
        "the checksum field is present:\n{html}"
    );
    assert!(
        html.contains("disabled"),
        "the locked checksum renders a disabled input:\n{html}"
    );
    assert!(
        html.contains(r#"value="sha256:9f3a8c12d4e7b6a05f1e""#),
        "the locked checksum is seeded from the record:\n{html}"
    );
    for needle in [
        r#"for="media-date""#,
        r#"aria-label="Date modifier""#,
        r#"aria-label="Original text""#,
    ] {
        assert!(
            html.contains(needle),
            "the structured date editor renders {needle:?}:\n{html}"
        );
    }
}

#[test]
fn citations_carry_source_page_and_evidence() {
    let html = render(media_view);
    for needle in [
        "Family photo collection",
        "album 2, p.4",
        r#"data-level="normal""#,
        ">Normal",
        "Original",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(media_view);
    assert!(html.contains("Direct ancestor"), "tag name shown:\n{html}");
    assert!(html.contains("#e5534b"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}

/// A media object whose only citation is evidence-only (no attach `AssertionId`) — the Citations tab
/// shows no Detach for it.
fn media_citations_no_detach() -> Element {
    let loc = loc();
    let on_retract = use_callback(|_: (String, String, bool)| {});
    let mut citation = sample().citations[0].clone();
    citation.assertion_id = None;
    let citations = vec![citation];
    rsx! {
        {citations_table::<MediaEditForm>(&loc, &citations, false, on_retract)}
    }
}

#[test]
fn attribute_rows_carry_edit_and_retract_corrections() {
    let html = render(media_view);
    // Edit opens the attribute editor, pre-filled; Retract retracts the attribute (stays in History).
    assert!(
        html.contains(r#"aria-label="Edit dimensions""#),
        "attribute Edit is row-scoped for screen readers:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract dimensions""#),
        "attribute Retract accessible name is row-scoped:\n{html}"
    );
    assert!(
        html.contains("Retract this assertion"),
        "the Retract control carries the retract tooltip:\n{html}"
    );
}

#[test]
fn attachments_carry_detach_corrections() {
    let html = render(media_view);
    assert!(
        html.contains(r#"aria-label="Detach C0007""#),
        "an attached citation carries Detach:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Detach N0004""#),
        "an attached note carries Detach:\n{html}"
    );
}

#[test]
fn an_evidence_only_citation_has_no_detach() {
    let html = render(media_citations_no_detach);
    assert!(
        !html.contains("Detach"),
        "a citation with no attach assertion shows no Detach:\n{html}"
    );
}

#[test]
fn no_assertion_uuid_is_ever_rendered() {
    let html = render(media_view);
    for id in [
        "0190-attr-assertion-id",
        "0190-citation-attach-id",
        "0190-note-attach-id",
    ] {
        assert!(
            !html.contains(id),
            "the assertion id {id:?} must never be rendered:\n{html}"
        );
    }
}
