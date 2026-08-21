//! SSR assertions for the Media detail (Phase 5 PR27): the read-first Overview record (paths · MIME,
//! with the checksum locked and the date a structured editor), its edit mode swapping in inputs plus
//! the sticky-header Cancel/Save, the citations table, and the tags panel.
//!
//! Plus what #309 held the screen to against `docs/mockups/media.html`: the `mime · date` header
//! subtitle with no second MIME badge, a File card that starts at File path and orders Date above
//! Checksum, the clickable preview and the wide look-only viewer dialog it opens, and an Attributes row
//! whose type is a chip and whose value is not dimmed. What SSR cannot see here — which element the
//! click handler is on, and where focus lands in the dialog — is
//! `tests/gui-pass/media-viewer-dialog.toml`.

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
use vitni_ui_dioxus::components::TabItem;
use vitni_ui_dioxus::master_detail::DetailContainer;
use vitni_ui_dioxus::screens::{
    MediaEditForm, RecordActionLabels, RecordEditState, citations_table, media_attributes_table, media_overview,
    media_preview_dialog, media_used_by, notes_table, record_head_actions, tags_panel,
};
use vitni_ui_dioxus::shell::nav_state::NavState;

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
    // The Overview's "Used by" rows and the Notes table are `RecordLink`s (#304), which resolve
    // `NavState` from context.
    use_context_provider(NavState::new);
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
        {notes_table(&loc, &detail.notes, Some(on_retract))}
        {tags_panel(&loc, &detail.tags, use_callback(|_: (String, String)| {}))}
    }
}

/// Renders the media detail header the way `media_detail` builds it: a `DetailContainer` with the 📷
/// avatar, the filename title, the `mime · date` subtitle and the id badge — no second MIME badge
/// (`docs/mockups/media.html:62`). `media_detail` itself is private and needs an `AppState`, so the
/// header's own prop expressions are mirrored here (the `tag_detail.rs` pattern).
fn media_header(detail: &MediaDetail) -> Element {
    let active = use_signal(|| 0_usize);
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            subtitle: detail.header_subtitle(),
            id_label: Some(detail.human_id.clone()),
            avatar: "📷".to_owned(),
            extras: rsx! {},
            actions: rsx! {},
            tabs: Vec::<TabItem>::new(),
            active,
        }
    }
}

fn media_header_view() -> Element {
    media_header(&sample())
}

/// The header of a record carrying neither a MIME nor a date — the record the CLI creates.
fn media_header_bare() -> Element {
    media_header(&MediaDetail {
        mime: None,
        date: None,
        ..sample()
    })
}

#[test]
fn the_header_shows_the_mime_and_date_as_its_subtitle() {
    let html = render(media_header_view);
    assert!(
        html.contains(r#"<div class="detail-sub">image/jpeg · c. 1900</div>"#),
        "the header carries the mime · date subtitle:\n{html}"
    );
}

#[test]
fn the_header_shows_the_mime_only_in_the_subtitle_never_as_a_badge() {
    // The mockup's `<span class="badge">image/jpeg</span>` is deleted, not implemented: the subtitle
    // already carries the MIME, and a badge would say it twice.
    let html = render(media_header_view);
    assert!(
        !html.contains(r#"class="badge">image/jpeg"#),
        "no MIME badge beside the id badge:\n{html}"
    );
    assert!(
        html.contains(r#"class="badge">O0050"#),
        "the id badge is still there:\n{html}"
    );
}

#[test]
fn a_record_with_neither_mime_nor_date_renders_no_subtitle_line() {
    let html = render(media_header_bare);
    assert!(
        !html.contains("detail-sub"),
        "an empty subtitle draws no line at all:\n{html}"
    );
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
    use_context_provider(NavState::new);
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
    use_context_provider(NavState::new);
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
    use_context_provider(NavState::new);
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

/// The preview dialog with `viewing` set — the state a click on the preview frame puts the pane in.
fn media_preview_dialog_open() -> Element {
    let loc = loc();
    let viewing = use_signal(|| true);
    rsx! { {media_preview_dialog(&loc, &sample(), viewing)} }
}

#[test]
fn the_preview_frame_is_a_button_that_opens_the_image() {
    // The Media record was the one record screen that never rendered `MediaViewer`, so a media
    // object's own page was the only place its image could not be opened.
    let html = render(media_view);
    assert!(
        html.contains(r#"<button class="media-open" type="button" aria-label="Open john-smith-portrait.jpg">"#),
        "the preview frame is an accessible button:\n{html}"
    );
    let button = html.find("media-open").expect("the open button renders");
    let image = html.find("media-full").expect("the preview image renders");
    assert!(button < image, "the button wraps the image, not the reverse:\n{html}");
}

#[test]
fn the_glyph_placeholder_is_not_clickable() {
    // There is nothing to open, so an inert div is honest — a button would promise a dialog.
    let html = render(media_view_outside_the_workspace);
    assert!(
        !html.contains("media-open"),
        "an unservable location offers no open button:\n{html}"
    );
}

#[test]
fn the_preview_dialog_is_closed_until_the_frame_is_clicked() {
    let html = render(media_view);
    assert!(
        !html.contains("overlay"),
        "no dialog layer is mounted before the click:\n{html}"
    );
}

#[test]
fn the_open_preview_dialog_is_a_wide_modal_holding_a_look_only_viewer() {
    let html = render(media_preview_dialog_open);
    for needle in [
        r#"class="overlay""#,
        r#"class="modal modal-wide""#,
        r#"role="dialog""#,
        r#"aria-modal="true""#,
        "john-smith-portrait.jpg",
        "Fit",
        "200%",
        "Close",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    for needle in ["Set region", "Clear region", "mv-readout", "crop-capture"] {
        assert!(
            !html.contains(needle),
            "the record's own preview records no region, so no {needle:?}:\n{html}"
        );
    }
}

#[test]
fn a_stored_records_file_card_never_repeats_the_id_the_header_shows() {
    // `media.html:120` starts the File card at File path: the header's id badge already names the
    // record. The row survives in create mode only (`media_create.rs`), where there is no badge yet.
    for view in [media_view as fn() -> Element, media_edit as fn() -> Element] {
        let html = render(view);
        assert!(
            !html.contains("media-id"),
            "no ID row on a stored record's File card:\n{html}"
        );
    }
}

#[test]
fn the_file_card_orders_date_above_checksum() {
    // `media.html:123-124` draws Date then Checksum; the code drew them the other way round.
    let html = render(media_view);
    let date = html.find("media-date").expect("the Date row renders");
    let checksum = html.find("media-checksum").expect("the Checksum row renders");
    assert!(
        date < checksum,
        "Date precedes Checksum (date at {date}, checksum at {checksum}):\n{html}"
    );
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
fn an_attribute_row_chips_its_type_and_shows_its_value_at_full_contrast() {
    // `media.html:246` draws the type as a `.chip`, like every other typed cell on the screen, and its
    // value as plain text — the app dimmed the value with `muted`, which the mockup never asked for.
    let html = render(media_view);
    assert!(
        html.contains(r#"<td><span class="chip">dimensions</span></td>"#),
        "the attribute type is a chip:\n{html}"
    );
    assert!(
        html.contains("<td>1024x1536</td>"),
        "the attribute value carries no muted class:\n{html}"
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

/// The Overview's "Used by" card body on its own — the reverse lookup of what references this object.
fn media_used_by_only() -> Element {
    // `RecordLink` resolves `NavState` from context; the bare SSR harness must supply it.
    use_context_provider(NavState::new);
    let loc = loc();
    let detail = sample();
    rsx! { {media_used_by(&loc, &detail.used_by)} }
}

#[test]
fn each_used_by_row_opens_the_record_that_uses_the_object() {
    // Issue #304: "Used by" was a `.stack` of inert fact-rows, so a media object could not be navigated
    // back from. It is now the shared attached-records table, with the row's category resolved by
    // `Category::from_using_kind`.
    let html = render(media_used_by_only);
    assert!(
        html.contains(r#"<button class="src-link" type="button">John Smith</button>"#),
        "the row opens the person that uses this object:\n{html}"
    );
    for header in ["Object", "Type", "ID"] {
        assert!(
            html.contains(&format!("<th>{header}</th>")),
            "the used-by table names its `{header}` column:\n{html}"
        );
    }
    assert!(
        !html.contains("0190-person-42"),
        "the referencing record's aggregate id must never be rendered:\n{html}"
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
