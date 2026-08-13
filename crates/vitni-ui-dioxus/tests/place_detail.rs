//! SSR assertions for the Place detail (Phase 5 PR27): the read-first Overview record (id · type ·
//! latitude · longitude · code), its edit mode swapping in inputs plus the sticky-header Cancel/Save,
//! the names table, the hierarchy table, and the tags panel.

use dioxus::prelude::*;
use vitni_app::{PlaceType, Rect, TagRef};
use vitni_ui::{
    AttachedRefVm, Category, CitationRefVm, ConfidenceLevel, DateDraft, EvidenceAxis, EvidenceAxisVm, Localizer,
    MapPointVm, MarkerShapeVm, MediaRefVm, PickerSelection, PickerState, PlaceDetail, PlaceDraft, PlaceGeometryVm,
    PlaceHierarchyVm, PlaceNameVm, PlaceSuccessionVm, ProvenanceDraft,
};
use vitni_ui_dioxus::components::{PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{
    MediaTabState, PlaceEditForm, RecordActionLabels, RecordEditState, SuccessionFormState, citations_table, id_list,
    media_gallery, media_tab, place_hierarchy_table, place_names_table, place_overview, place_succession_card,
    place_succession_form_fields, record_head_actions, tags_panel,
};

/// A representative place detail: a city with High-confidence coordinates, two names (one sourced,
/// one not), a two-level jurisdiction chain, and one tag.
fn sample() -> PlaceDetail {
    PlaceDetail {
        human_id: "P0007".to_owned(),
        id: "0190-place-id".to_owned(),
        title: "New York".to_owned(),
        place_type: Some(PlaceType::City),
        type_label: Some("City".to_owned()),
        coordinates: Some("40.7128,-74.006".to_owned()),
        map_point: Some(MapPointVm {
            lat: 40.7128,
            lon: -74.006,
            label: "New York".to_owned(),
        }),
        resolved_geometry: None,
        geometries: Vec::new(),
        coordinates_confidence: Some(ConfidenceLevel::High),
        coordinates_confidence_label: Some("High".to_owned()),
        coordinate_citations: vec![CitationRefVm {
            human_id: "C0009".to_owned(),
            source: Some("GeoNames gazetteer".to_owned()),
            source_id: Some("S0007".to_owned()),
            page: Some("id 5128581".to_owned()),
            backs_count: 0,
            confidence: Some(ConfidenceLevel::High),
            confidence_label: Some("High".to_owned()),
            evidence_axes: vec![EvidenceAxisVm {
                axis: EvidenceAxis::Source,
                label: "Derivative".to_owned(),
            }],
            asserted_by: Some("asserted by geonames-import · 2026-06-10 11:03".to_owned()),
            assertion_id: None,
        }],
        code: Some("GeoNames 5128581".to_owned()),
        code_confidence: Some(ConfidenceLevel::High),
        code_confidence_label: Some("High".to_owned()),
        code_citations: vec![CitationRefVm {
            human_id: "C0009".to_owned(),
            source: Some("GeoNames gazetteer".to_owned()),
            source_id: Some("S0007".to_owned()),
            page: Some("id 5128581".to_owned()),
            backs_count: 0,
            confidence: Some(ConfidenceLevel::High),
            confidence_label: Some("High".to_owned()),
            evidence_axes: Vec::new(),
            asserted_by: Some("asserted by geonames-import · 2026-06-10 11:03".to_owned()),
            assertion_id: None,
        }],
        names: sample_names(),
        hierarchy: vec![
            PlaceHierarchyVm {
                human_id: "P0050".to_owned(),
                id: "0190-county".to_owned(),
                name: "New York County".to_owned(),
                type_label: Some("County".to_owned()),
                date: Some("1683 –".to_owned()),
                confidence: Some(ConfidenceLevel::High),
                confidence_label: "High".to_owned(),
                assertion_id: "0190-enclosing-assert-1".to_owned(),
            },
            PlaceHierarchyVm {
                human_id: "P0001".to_owned(),
                id: "0190-country".to_owned(),
                name: "United States".to_owned(),
                type_label: Some("Country".to_owned()),
                date: Some("1788 –".to_owned()),
                confidence: Some(ConfidenceLevel::High),
                confidence_label: "High".to_owned(),
                assertion_id: "0190-enclosing-assert-2".to_owned(),
            },
        ],
        predecessors: vec![PlaceSuccessionVm {
            human_id: "P0021".to_owned(),
            id: "0190-new-amsterdam".to_owned(),
            name: "New Amsterdam".to_owned(),
            kind_label: "absorbed".to_owned(),
            date: Some("1664".to_owned()),
            assertion_id: "0190-succession-assert-1".to_owned(),
        }],
        successors: Vec::new(),
        events: Vec::new(),
        citations: sample_citations(),
        media: sample_media(),
        notes: vec![AttachedRefVm {
            human_id: "N0004".to_owned(),
            assertion_id: "0190-note-attach-1".to_owned(),
        }],
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Home town".to_owned(),
            color: Some("#6cb6ff".to_owned()),
            priority: Some(1),
        }],
        restrictions: Vec::new(),
        research_notes: Vec::new(),
        history: Vec::new(),
    }
}

/// The place's attached media (Media tab): one captioned attachment.
fn sample_media() -> Vec<MediaRefVm> {
    vec![MediaRefVm {
        human_id: "O0004".to_owned(),
        caption: Some("City map".to_owned()),
        crop: None,
        path: None,
        mime: None,
        assertion_id: "0190-media-attach-1".to_owned(),
    }]
}

/// The place's asserted names (Names tab): one sourced/dated, one unsourced.
fn sample_names() -> Vec<PlaceNameVm> {
    vec![
        PlaceNameVm {
            text: "New York".to_owned(),
            language: Some("en".to_owned()),
            date: Some("1664".to_owned()),
            confidence: Some(ConfidenceLevel::VeryHigh),
            confidence_label: "Very high".to_owned(),
            source_count: 1,
            assertion_id: "0190-name-assert-1".to_owned(),
        },
        PlaceNameVm {
            text: "Nieuw Amsterdam".to_owned(),
            language: Some("nl".to_owned()),
            date: None,
            confidence: Some(ConfidenceLevel::Normal),
            confidence_label: "Normal".to_owned(),
            source_count: 0,
            assertion_id: "0190-name-assert-2".to_owned(),
        },
    ]
}

/// The place's backing citations (Citations tab): one attachment (a Detach target) and one shown as
/// evidence with no attach assertion (no Detach).
fn sample_citations() -> Vec<CitationRefVm> {
    vec![
        CitationRefVm {
            human_id: "C0011".to_owned(),
            source: Some("Parish register".to_owned()),
            source_id: Some("S0011".to_owned()),
            page: Some("p. 12".to_owned()),
            backs_count: 0,
            confidence: Some(ConfidenceLevel::High),
            confidence_label: Some("High".to_owned()),
            evidence_axes: Vec::new(),
            asserted_by: None,
            assertion_id: Some("0190-citation-attach-1".to_owned()),
        },
        CitationRefVm {
            human_id: "C0012".to_owned(),
            source: Some("Derived note".to_owned()),
            source_id: None,
            page: None,
            backs_count: 0,
            confidence: None,
            confidence_label: None,
            evidence_axes: Vec::new(),
            asserted_by: None,
            assertion_id: None,
        },
    ]
}

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn state(editing: bool) -> RecordEditState<PlaceDraft> {
    let seed = PlaceDraft::from_detail(&sample());
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

fn place_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let detail = sample();
    let onedit = use_callback(|_| {});
    let onretract = use_callback(|_: (String, String, bool)| {});
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (PlaceDraft, ProvenanceDraft)| {}))}
        {place_overview(&loc, &detail, record)}
        {place_names_table(&loc, &detail, onedit, onretract)}
        {place_hierarchy_table(&loc, &detail, onedit, onretract)}
        {place_succession_card(&loc, &detail, onedit, onretract)}
        {citations_table::<PlaceEditForm>(&loc, &detail.citations, false, onretract)}
        {media_gallery(&loc, &detail.media, Some(onretract), None)}
        {id_list(&loc, &detail.notes, Some(onretract))}
        {tags_panel(&loc, &detail.tags, use_signal(|| None::<PlaceEditForm>), PlaceEditForm::Tag, use_callback(|_: String| {}))}
    }
}

fn place_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (PlaceDraft, ProvenanceDraft)| {}))}
        {place_overview(&loc, &detail, record)}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(place_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit:\n{html}");
    assert!(!html.contains("<input"), "no live inputs in view mode:\n{html}");
    for needle in ["City", "40.7128", "-74.006", "GeoNames 5128581"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

/// `sample()` with a resolved-geometry Point asserted at a different location than its scalar
/// `coordinates` — the regression this guards: dropping a point on the Map tab only ever emits
/// `GeometryAsserted`, never a `SetCoordinates`, so a display that read the scalar would keep showing
/// the place's pre-drop location.
fn place_view_with_geometry_override() -> Element {
    let loc = loc();
    let detail = PlaceDetail {
        resolved_geometry: Some(PlaceGeometryVm {
            shape: MarkerShapeVm::Point { lat: 61.5, lon: 8.75 },
            kind_label: "Point".to_owned(),
            year: None,
            date: None,
            confidence: None,
            confidence_label: String::new(),
            source_count: 0,
            assertion_id: "assert-geometry-override".to_owned(),
        }),
        ..sample()
    };
    let record = state(false);
    rsx! {
        {place_overview(&loc, &detail, record)}
    }
}

#[test]
fn overview_shows_the_resolved_geometry_point_over_the_stale_scalar_coordinate() {
    let html = render(place_view_with_geometry_override);
    assert!(
        html.contains("61.500000"),
        "the resolved geometry's latitude is shown:\n{html}"
    );
    assert!(
        html.contains("8.750000"),
        "the resolved geometry's longitude is shown:\n{html}"
    );
    assert!(
        !html.contains("40.7128"),
        "the stale scalar latitude must not be shown once a geometry assertion exists:\n{html}"
    );
}

#[test]
fn edit_mode_swaps_in_the_inputs_and_header_actions() {
    let html = render(place_edit);
    assert!(html.contains("<input"), "edit mode swaps in inputs:\n{html}");
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="place-latitude""#),
        "the latitude input is present:\n{html}"
    );
    assert!(
        html.contains(r#"id="place-code""#),
        "the code input is present:\n{html}"
    );
}

#[test]
fn names_and_hierarchy_carry_language_dates_and_surety() {
    let html = render(place_view);
    for needle in [
        r#"class="tbl""#,
        "New York",
        "Nieuw Amsterdam",
        "no-source",
        r#"data-level="very-high""#,
        "New York County",
        "1683 –",
        "United States",
        // The Names table's dated column heads "Period", not "Date" (single-dated-PlaceName fix, PR39).
        ">Period<",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn coordinate_and_code_carry_the_provenance_popover() {
    let html = render(place_view);
    // Both sourced scalar claims surface the "Why we believe" source-link cue in the overview — the
    // coordinate popover (VM data existed, was never wired) and the Code field (new data path).
    let src_links = html.matches(r#"class="src-link""#).count();
    assert!(
        src_links >= 2,
        "the sourced Coordinates and Code claims each render a provenance source-link (found {src_links}):\n{html}"
    );
    // The source-count trigger text — one citation backs each claim.
    assert!(
        html.contains("❝ 1 source"),
        "the provenance trigger shows the backing source count:\n{html}"
    );
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(place_view);
    assert!(html.contains("Home town"), "tag name shown:\n{html}");
    assert!(html.contains("#6cb6ff"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}

#[test]
fn name_rows_carry_edit_and_retract_with_row_scoped_labels() {
    let html = render(place_view);
    assert!(
        html.contains(r#"aria-label="Edit New York""#),
        "the name row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract New York""#),
        "the name row Retract carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains("Retract this assertion — it stays in History"),
        "the Retract button carries the retract-title tooltip:\n{html}"
    );
}

#[test]
fn hierarchy_rows_carry_edit_and_retract_with_row_scoped_labels() {
    let html = render(place_view);
    assert!(
        html.contains(r#"aria-label="Edit New York County""#),
        "the enclosing row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract New York County""#),
        "the enclosing row Retract carries a row-scoped accessible name:\n{html}"
    );
}

#[test]
fn citations_show_detach_only_when_the_attach_assertion_is_present() {
    let html = render(place_view);
    assert!(
        html.contains(r#"aria-label="Detach C0011""#),
        "an attached citation carries a per-row Detach:\n{html}"
    );
    assert!(
        !html.contains(r#"aria-label="Detach C0012""#),
        "a citation with no attach assertion renders no Detach:\n{html}"
    );
}

#[test]
fn notes_and_media_carry_detach() {
    let html = render(place_view);
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
    let html = render(place_view);
    for assertion_id in [
        "0190-name-assert-1",
        "0190-name-assert-2",
        "0190-enclosing-assert-1",
        "0190-enclosing-assert-2",
        "0190-citation-attach-1",
        "0190-media-attach-1",
        "0190-note-attach-1",
        "0190-succession-assert-1",
    ] {
        assert!(
            !html.contains(assertion_id),
            "assertion id {assertion_id:?} must never be rendered:\n{html}"
        );
    }
}

#[test]
fn the_succession_card_shows_the_predecessors_kind_and_date() {
    let html = render(place_view);
    assert!(html.contains("Succession"), "the card title is shown:\n{html}");
    assert!(
        html.contains("New Amsterdam"),
        "the predecessor's name is shown:\n{html}"
    );
    assert!(html.contains("absorbed"), "the kind chip is shown:\n{html}");
    assert!(html.contains("1664"), "the dated effective year is shown:\n{html}");
}

fn no_succession() -> PlaceDetail {
    PlaceDetail {
        predecessors: Vec::new(),
        successors: Vec::new(),
        ..sample()
    }
}

fn no_succession_view() -> Element {
    let loc = loc();
    let onedit = use_callback(|_: PlaceEditForm| {});
    let onretract = use_callback(|_: (String, String, bool)| {});
    let detail = no_succession();
    place_succession_card(&loc, &detail, onedit, onretract)
}

#[test]
fn no_predecessors_or_successors_renders_the_empty_state() {
    let html = render(no_succession_view);
    assert!(html.contains("Succession"), "the card title still shows:\n{html}");
    assert!(
        !html.contains("New Amsterdam"),
        "no predecessor row is rendered:\n{html}"
    );
}

/// The write path's entry point (#196): the Succession card carries "+ Add succession" whether or not
/// the place already took part in one — the empty state is exactly where an operator needs it.
#[test]
fn the_succession_card_offers_the_add_action_when_populated() {
    let html = render(place_view);
    assert!(
        html.contains("Add succession"),
        "the populated card offers the add action:\n{html}"
    );
}

#[test]
fn the_succession_card_offers_the_add_action_in_the_empty_state() {
    let html = render(no_succession_view);
    assert!(
        html.contains("Add succession"),
        "the empty state offers the add action too:\n{html}"
    );
}

fn succession_picker(loc: &Localizer, name: &str) -> RecordPicker {
    RecordPicker {
        config: PickerConfig {
            label: name.to_owned(),
            name: name.to_owned(),
            entity_label: loc.picker_entity(Category::Places),
            allow_new: false,
        },
        state: use_signal(PickerState::default),
        options: PickerOptions::Ready(Vec::new()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: use_callback(|_: PickerSelection| {}),
            onclear: use_callback(|()| {}),
            onnew: use_callback(|_: String| {}),
        },
    }
}

fn succession_form_view() -> Element {
    let loc = loc();
    let to = succession_picker(&loc, "succession-to");
    let from = succession_picker(&loc, "succession-from");
    let state = SuccessionFormState {
        kind: use_signal(|| 0_usize),
        to: use_signal(Vec::<PickerSelection>::new),
        from: use_signal(Vec::<PickerSelection>::new),
        date: use_signal(DateDraft::default),
    };
    place_succession_form_fields(&loc, &to, &from, state)
}

/// The Succession panel's field set (`place.html`): the kind select, both repeatable place pickers, and
/// the effective-date cluster.
#[test]
fn the_succession_panel_renders_its_kind_pickers_and_date() {
    let html = render(succession_form_view);
    for needle in [
        r#"id="succession-kind""#,
        r#"for="succession-to""#,
        r#"for="succession-from""#,
        r#"for="succession-date""#,
        // The kind select offers every domain variant, so a merge and a split are both reachable.
        "merged",
        "split",
        "absorbed",
        "elevated",
        "renamed",
        // Both repeatable pickers carry their own "add the picked place" control.
        "Add picked place",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

/// The Place Media tab (issue #199): the gallery card opens the shared crop viewer, mirroring the
/// Person screen's `media_tab` wiring (`place.rs`'s "media" tab arm).
fn place_media_tab_view() -> Element {
    let loc = loc();
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    let detail = sample();
    let viewing = use_signal(|| detail.media.first().cloned());
    let on_view = use_callback(|_item: MediaRefVm| {});
    let on_region = use_callback(|_region: (String, Option<Rect>, Option<String>)| {});
    let media_state = MediaTabState {
        viewing,
        on_view,
        on_region,
    };
    rsx! {
        {media_tab(&loc, &detail.media, Some(on_retract), media_state)}
    }
}

#[test]
fn media_tab_opens_the_crop_viewer_on_a_card_click() {
    let html = render(place_media_tab_view);
    assert!(
        html.contains("media-open"),
        "the gallery card opens the crop viewer (ADR 0017 §GUI):\n{html}"
    );
    assert!(
        html.contains("Set region") && html.contains("Clear region"),
        "the crop viewer overlay renders with its Set/Clear region actions:\n{html}"
    );
}
