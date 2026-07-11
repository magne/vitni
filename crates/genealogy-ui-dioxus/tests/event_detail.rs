//! SSR assertions for the Event detail (Phase 5 PR27): the read-first Overview record (id · type ·
//! date · place · description), its edit mode swapping in inputs plus the sticky-header Cancel/Save,
//! the participants table, the citations table, and the tags panel (name/colour, never id).

use dioxus::prelude::*;
use genealogy_app::{
    Calendar, DateInput, DateModifier, DatePoint, DateQuality, EventType, GenealogicalDate, GenealogicalDateBody,
    ParticipantRole, TagRef, build_genealogical_date,
};
use genealogy_ui::{
    AttachedRefVm, CitationRefVm, ConfidenceLevel, EventDetail, EventDraft, EvidenceAxis, EvidenceAxisVm,
    FamilyMediaVm, Localizer, ParticipantVm, PickerSelection, PickerState, PlaceLinkVm, ProvenanceDraft,
};

/// The sample event's structured date: exact 14 Jun 1876 on the Gregorian calendar.
fn sample_date() -> GenealogicalDate {
    build_genealogical_date(DateInput {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        body: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(1876),
            month: Some(6),
            day: Some(14),
        })),
        new_year_begins: None,
        original_text: Some("14 June 1876".to_owned()),
        time: None,
    })
}
use genealogy_ui_dioxus::components::{PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use genealogy_ui_dioxus::screens::{
    EventEditCtx, EventEditForm, RecordActionLabels, RecordEditState, event_citations_table, event_overview,
    event_participants_table, event_tags_panel, family_media_gallery, id_list, record_head_actions,
};
use genealogy_ui_dioxus::shell::nav_state::NavState;

/// A representative event detail: a marriage with a High-confidence date, a linked place, two
/// participants (one sourced, one not), a citation with evidence axes, and one tag.
fn sample() -> EventDetail {
    EventDetail {
        human_id: "E0101".to_owned(),
        id: "0190-event-id".to_owned(),
        title: "Marriage".to_owned(),
        event_type: Some(EventType::Marriage),
        type_label: "Marriage".to_owned(),
        date: Some("14 Jun 1876".to_owned()),
        date_value: Some(sample_date()),
        date_confidence: Some(ConfidenceLevel::High),
        date_confidence_label: Some("High".to_owned()),
        date_source_count: 1,
        date_citations: vec![CitationRefVm {
            human_id: "C0001".to_owned(),
            source: Some("Trinity Church marriages".to_owned()),
            source_id: Some("S0003".to_owned()),
            page: Some("vol. 5, f. 18".to_owned()),
            confidence: Some(ConfidenceLevel::High),
            confidence_label: Some("High".to_owned()),
            evidence_axes: vec![EvidenceAxisVm {
                axis: EvidenceAxis::Source,
                label: "Original".to_owned(),
            }],
            asserted_by: Some("asserted by magne · 2026-06-21 16:02".to_owned()),
            assertion_id: None,
        }],
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
                role: ParticipantRole::Primary,
                role_label: "Groom".to_owned(),
                confidence: Some(ConfidenceLevel::High),
                confidence_label: "High".to_owned(),
                source_count: 1,
                assertion_id: "0190-participant-assertion-1".to_owned(),
            },
            ParticipantVm {
                human_id: "I0004".to_owned(),
                id: "0190-person-4".to_owned(),
                name: "Anna Berg".to_owned(),
                role: ParticipantRole::Witness,
                role_label: "Witness".to_owned(),
                confidence: Some(ConfidenceLevel::Low),
                confidence_label: "Low".to_owned(),
                source_count: 0,
                assertion_id: "0190-participant-assertion-2".to_owned(),
            },
        ],
        citations: vec![CitationRefVm {
            human_id: "C0001".to_owned(),
            source: Some("Trinity Church marriages".to_owned()),
            source_id: Some("S0003".to_owned()),
            page: Some("vol. 5, f. 18".to_owned()),
            confidence: Some(ConfidenceLevel::High),
            confidence_label: Some("High".to_owned()),
            evidence_axes: vec![EvidenceAxisVm {
                axis: EvidenceAxis::Source,
                label: "Original".to_owned(),
            }],
            asserted_by: Some("asserted by magne · 2026-06-21 16:02".to_owned()),
            assertion_id: Some("0190-citation-attach-assertion".to_owned()),
        }],
        media: vec![FamilyMediaVm {
            human_id: "O0007".to_owned(),
            caption: Some("Wedding portrait".to_owned()),
            assertion_id: "0190-media-attach-assertion".to_owned(),
        }],
        notes: vec![AttachedRefVm {
            human_id: "N0005".to_owned(),
            assertion_id: "0190-note-attach-assertion".to_owned(),
        }],
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

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn state(editing: bool) -> RecordEditState<EventDraft> {
    let seed = EventDraft::from_detail(&sample());
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

/// The whole-record edit context an event's overview needs: the edit state plus an existing-only
/// place picker (no rows or wiring needed under SSR — the collapsed selection derives from the draft).
fn ctx(record: RecordEditState<EventDraft>) -> EventEditCtx {
    let place = RecordPicker {
        config: PickerConfig {
            label: "Place".to_owned(),
            name: "event-place".to_owned(),
            entity_label: "place".to_owned(),
            allow_new: false,
        },
        state: use_signal(PickerState::default),
        options: PickerOptions::Ready(Vec::new()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: Callback::new(|_: PickerSelection| {}),
            onclear: Callback::new(|()| {}),
            onnew: Callback::new(|_: String| {}),
        },
    };
    EventEditCtx {
        record,
        place,
        place_reset: Callback::new(|()| {}),
    }
}

fn event_view() -> Element {
    // RecordLink resolves NavState from context, so the harness must provide it.
    use_context_provider(NavState::new);
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let editing = use_signal(|| None::<EventEditForm>);
    let on_submit = use_callback(|_edit: (genealogy_ui::EventEdit, genealogy_ui::ProvenanceDraft)| {});
    let on_edit_open = use_callback(|_: EventEditForm| {});
    let on_retract = use_callback(|_: (String, String, bool)| {});
    let on_person_retract = use_callback(|_: (String, String, bool, String)| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (EventDraft, ProvenanceDraft)| {}))}
        {event_overview(&loc, &detail, &ctx(record))}
        {event_participants_table(&loc, &detail, on_edit_open, on_person_retract)}
        {event_citations_table(&loc, &detail.citations, on_retract)}
        {family_media_gallery(&loc, &detail.media, Some(on_retract))}
        {id_list(&loc, &detail.notes, Some(on_retract))}
        {event_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

fn event_edit() -> Element {
    use_context_provider(NavState::new);
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (EventDraft, ProvenanceDraft)| {}))}
        {event_overview(&loc, &detail, &ctx(record))}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(event_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit in the header:\n{html}");
    assert!(
        !html.contains("<input"),
        "view mode shows read boxes, not inputs:\n{html}"
    );
    for needle in ["Marriage", "14 Jun 1876", "Solemnized before two witnesses."] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn edit_mode_swaps_in_the_inputs_and_header_actions() {
    let html = render(event_edit);
    assert!(html.contains("<input"), "edit mode swaps in inputs:\n{html}");
    assert!(html.contains("<select"), "edit mode swaps in the type select:\n{html}");
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="event-id""#),
        "the editable human id is present:\n{html}"
    );
    assert!(
        html.contains(r#"class="picker-value""#) && html.contains("Trinity Church, New York"),
        "the linked place shows as a collapsed picker chip:\n{html}"
    );
    for needle in [
        r#"aria-label="Date modifier""#,
        r#"aria-label="Date quality""#,
        r#"aria-label="Calendar""#,
        r#"aria-label="Original text""#,
        r#"value="14 Jun 1876""#,
    ] {
        assert!(
            html.contains(needle),
            "the structured date editor renders {needle:?}:\n{html}"
        );
    }
}

#[test]
fn participants_and_citations_carry_roles_and_evidence() {
    let html = render(event_view);
    for needle in [
        r#"class="tbl""#,
        "John Smith",
        "Groom",
        "Anna Berg",
        "no-source",
        "Trinity Church marriages",
        "vol. 5, f. 18",
        "Original",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(event_view);
    assert!(html.contains("Verified event"), "tag name shown:\n{html}");
    assert!(html.contains("#b07cf0"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}

/// An event whose only citation is evidence-only (no attach `AssertionId`) — the Citations tab shows
/// no Detach for it.
fn event_citations_no_detach() -> Element {
    use_context_provider(NavState::new);
    let loc = loc();
    let on_retract = use_callback(|_: (String, String, bool)| {});
    let mut citation = sample().citations[0].clone();
    citation.assertion_id = None;
    let citations = vec![citation];
    rsx! {
        {event_citations_table(&loc, &citations, on_retract)}
    }
}

#[test]
fn participant_rows_carry_edit_and_remove_corrections() {
    let html = render(event_view);
    // Edit opens the role editor; the tooltip's apostrophe is HTML-escaped, so match a prefix.
    assert!(
        html.contains("Change this participation"),
        "participant Edit tooltip:\n{html}"
    );
    // Participation is person-owned (ADR 0019): every row carries the Edit affordance.
    assert!(
        html.contains(r#"aria-label="Edit John Smith""#),
        "the first participant's Edit is row-scoped for screen readers:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Edit Anna Berg""#),
        "the second participant's Edit is row-scoped for screen readers:\n{html}"
    );
    // Remove retracts the participation (stays in History): row-scoped name + the remove tooltip.
    assert!(
        html.contains(r#"aria-label="Remove John Smith""#),
        "the first participant's Remove accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Remove Anna Berg""#),
        "the second participant's Remove accessible name:\n{html}"
    );
    assert!(
        html.contains("Remove this participant"),
        "participant Remove tooltip:\n{html}"
    );
}

#[test]
fn attachments_carry_detach_corrections() {
    let html = render(event_view);
    assert!(
        html.contains(r#"aria-label="Detach C0001""#),
        "an attached citation carries Detach:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Detach O0007""#),
        "an attached media object carries Detach:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Detach N0005""#),
        "an attached note carries Detach:\n{html}"
    );
}

#[test]
fn an_evidence_only_citation_has_no_detach() {
    let html = render(event_citations_no_detach);
    assert!(
        !html.contains("Detach"),
        "a citation with no attach assertion shows no Detach:\n{html}"
    );
}

#[test]
fn no_assertion_or_tag_uuid_is_ever_rendered() {
    let html = render(event_view);
    for id in [
        "0190-participant-assertion-1",
        "0190-participant-assertion-2",
        "0190-citation-attach-assertion",
        "0190-media-attach-assertion",
        "0190-note-attach-assertion",
        "0190-secret-tag-id",
    ] {
        assert!(
            !html.contains(id),
            "an assertion/tag id must never render: {id}\n{html}"
        );
    }
}
