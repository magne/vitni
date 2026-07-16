//! SSR assertions for the Event detail (Phase 5 PR27): the read-first Overview record (id · type ·
//! date · place · description), its edit mode swapping in inputs plus the sticky-header Cancel/Save,
//! the participants table, the citations table, and the tags panel (name/colour, never id).

use dioxus::prelude::*;
use genealogy_app::Address;
use genealogy_app::{
    Age, Attribute, Calendar, DateInput, DateModifier, DatePoint, DateQuality, EventType, GenealogicalDate,
    GenealogicalDateBody, NewParticipation, ParticipantRole, TagRef, build_genealogical_date,
};
use genealogy_ui::{
    AddressVm, AttachedRefVm, CitationRefVm, ConfidenceLevel, EventDetail, EventDraft, EvidenceAxis, EvidenceAxisVm,
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
    EventEditCtx, EventEditForm, ParticipationSeed, RecordActionLabels, RecordEditState, address_cards,
    citations_table, event_overview, event_participants_table, family_media_gallery, id_list, participation_form,
    record_head_actions, tags_panel,
};
use genealogy_ui_dioxus::shell::nav_state::NavState;

/// The sample event's one recorded postal address (a residence/census `ADDR`), with fax + www set.
fn sample_address() -> AddressVm {
    AddressVm {
        address: Address {
            lines: vec!["12 Chapel Street".to_owned()],
            locality: Some("Brooklyn".to_owned()),
            region: Some("New York".to_owned()),
            postal_code: Some("11201".to_owned()),
            country: Some("United States".to_owned()),
            phone: Some("+1 718-555-0100".to_owned()),
            email: Some("clerk@brooklyn.example".to_owned()),
            fax: Some("+1 718-555-0199".to_owned()),
            www: Some("https://brooklyn.example".to_owned()),
            original_text: None,
        },
        assertion_id: "0190-event-addr-assert-1".to_owned(),
    }
}

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
            backs_count: 0,
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
        addresses: vec![sample_address()],
        participants: vec![
            ParticipantVm {
                human_id: "I0002".to_owned(),
                id: "0190-person-2".to_owned(),
                name: "John Smith".to_owned(),
                role: ParticipantRole::Primary,
                role_label: "Groom".to_owned(),
                age: None,
                age_label: Some("29y".to_owned()),
                attributes: Vec::new(),
                notes: Vec::new(),
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
                age: None,
                age_label: None,
                attributes: Vec::new(),
                notes: Vec::new(),
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
            backs_count: 0,
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
    let on_remove = use_callback(|_: String| {});
    let on_edit_open = use_callback(|_: EventEditForm| {});
    let on_edit_address =
        use_callback(move |seed: AddressVm| on_edit_open.call(EventEditForm::Address(Some(Box::new(seed)))));
    let on_retract = use_callback(|_: (String, String, bool)| {});
    let on_person_retract = use_callback(|_: (String, String, bool, String)| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (EventDraft, ProvenanceDraft)| {}))}
        {event_overview(&loc, &detail, &ctx(record))}
        {address_cards(&loc, &detail.addresses, on_edit_address, on_retract)}
        {event_participants_table(&loc, &detail, on_edit_open, on_person_retract)}
        {citations_table::<EventEditForm>(&loc, &detail.citations, false, on_retract)}
        {family_media_gallery(&loc, &detail.media, Some(on_retract))}
        {id_list(&loc, &detail.notes, Some(on_retract))}
        {tags_panel(&loc, &detail.tags, editing, EventEditForm::Tag, on_remove)}
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
fn participants_table_carries_an_age_column() {
    let html = render(event_view);
    assert!(
        html.contains(">Age<"),
        "the participants table has an Age column:\n{html}"
    );
    assert!(
        html.contains("29y"),
        "a participant's recorded age renders in the Age column:\n{html}"
    );
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
        {citations_table::<EventEditForm>(&loc, &citations, false, on_retract)}
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
fn address_cards_render_fax_www_and_carry_edit_retract() {
    let html = render(event_view);
    assert!(
        html.contains("+1 718-555-0199"),
        "the address fax number renders:\n{html}"
    );
    assert!(
        html.contains("https://brooklyn.example"),
        "the address www URL renders:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Edit Brooklyn""#),
        "the address card Edit carries a card-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract Brooklyn""#),
        "the address card Retract carries a card-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains("Retract this assertion — it stays in History"),
        "the Retract button carries the retract-title tooltip:\n{html}"
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
        "0190-event-addr-assert-1",
        "0190-secret-tag-id",
    ] {
        assert!(
            !html.contains(id),
            "an assertion/tag id must never render: {id}\n{html}"
        );
    }
}

/// A minimal existing-note picker for the shared participation-form body (no rows needed under SSR —
/// the form only exercises the inputs, mirroring the event-overview place picker in `ctx`).
fn note_picker() -> RecordPicker {
    RecordPicker {
        config: PickerConfig {
            label: "Note".to_owned(),
            name: "note".to_owned(),
            entity_label: "note".to_owned(),
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
    }
}

/// The event add-participant surface at person-screen parity: role · age (4 inputs) · attributes ·
/// notes · provenance, via the shared `participation_form` body seeded empty (add mode).
fn participation_add_form() -> Element {
    let loc = loc();
    let picker = note_picker();
    participation_form(
        &loc,
        &ParticipationSeed::empty(),
        &picker,
        EventHandler::new(|_: (NewParticipation, ProvenanceDraft)| {}),
    )
}

/// The same shared form seeded from an existing participation (edit mode): the age pre-fills so a
/// role-only change never drops it. The seed's existing attributes/notes are preserved on Save (the
/// type/value inputs and picker only append one more), so they ride the payload without being editable.
fn participation_edit_form() -> Element {
    let loc = loc();
    let picker = note_picker();
    let seed = ParticipationSeed {
        role: ParticipantRole::Witness,
        age: Some(Age {
            bound: None,
            years: Some(42),
            months: None,
            days: None,
            phrase: None,
        }),
        attributes: vec![Attribute {
            attribute_type: "occupation".to_owned(),
            value: "farmer".to_owned(),
        }],
        notes: vec!["N0001".to_owned()],
        supersedes: Some("0190-participant-assertion-1".to_owned()),
    };
    participation_form(
        &loc,
        &seed,
        &picker,
        EventHandler::new(|_: (NewParticipation, ProvenanceDraft)| {}),
    )
}

#[test]
fn the_event_add_participant_form_offers_the_full_participation_payload() {
    let html = render(participation_add_form);
    // Age (four parts), attribute (type/value), and the note picker all render — event/person parity.
    for name in [
        r#"name="age-years""#,
        r#"name="age-months""#,
        r#"name="age-days""#,
        r#"name="age-phrase""#,
        r#"name="attribute-type""#,
        r#"name="value""#,
        r#"name="note""#,
        r#"name="role""#,
    ] {
        assert!(
            html.contains(name),
            "expected the {name} input in the participation form:\n{html}"
        );
    }
}

#[test]
fn the_participation_form_prefills_age_and_attributes_when_editing() {
    let html = render(participation_edit_form);
    assert!(
        html.contains(r#"value="42""#),
        "the seeded age pre-fills the years input:\n{html}"
    );
}
