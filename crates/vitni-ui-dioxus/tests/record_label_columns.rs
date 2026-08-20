//! SSR gate on every record card's label column (issue #310 follow-up).
//!
//! A record card's `.field-label` width is not decoration: `.fact-row > .field-label` is
//! `min-width: max-content`, so a label wider than the declared column raises that one row's floor
//! rather than overlapping its value — which leaves that row's value out of line with the rest of the
//! card. So the column has to clear the longest label the card renders, **in every shipped locale**,
//! and every row in the card has to declare the *same* column or they cannot line up at all.
//!
//! This file asserts exactly those two properties, per screen: the set of `.field-label` widths a
//! record card renders is a single value, and that value is the one the screen's mockup draws. A
//! future edit that adds a row without the width, or narrows a screen's const, fails here. The widths
//! themselves were measured at the shared `.field-label` type (uppercase, `--fs-xs`, 0.4px
//! letter-spacing) — see the table in `components/fact_row.rs`.

use std::collections::BTreeSet;

use dioxus::prelude::*;
use vitni_app::MatchStatus;
use vitni_ui::{
    CitationDraft, DnaMatchDetail, DnaMatchDraft, DnaTestDraft, EventDraft, FamilyDraft, Localizer, MediaDraft,
    NoteDraft, PersonDraft, PickerSelection, PickerState, PlaceDraft, ProvenanceDraft, RecordDraft, RepositoryDraft,
    ResearchNoteDraft, SourceDraft, TagDraft,
};
use vitni_ui_dioxus::components::{PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{
    EventEditCtx, RecordEditState, citation_record_fields, dna_match_record_fields, dna_test_record_fields,
    event_record_fields, family_record_fields, media_record_fields, note_record_fields, person_record_fields,
    place_record_fields, repository_record_fields, research_note_record_fields, source_record_fields,
    tag_edit_colour_card, tag_edit_tag_card,
};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

/// An existing record's edit state over a default draft of type `$draft`. It sets `existing_human_id`,
/// because `editable_restrictions` gates on it: a create-form draft carries no restrictions, so
/// without it the Restrictions row — the one that drives the column's floor — never renders and this
/// gate would measure every card with its longest label missing.
macro_rules! existing {
    ($draft:ty, $editing:expr) => {
        state::<$draft>($editing, |draft: &mut $draft| {
            draft.existing_human_id = Some("X0001".to_owned());
        })
    };
}

/// A whole-record edit state over a default draft, run through `seed` first (see [`existing`]).
fn state<D: RecordDraft + Default>(editing: bool, seed: fn(&mut D)) -> RecordEditState<D> {
    let build = move || {
        let mut draft = D::default();
        seed(&mut draft);
        draft
    };
    RecordEditState {
        editing: use_signal(move || editing),
        seed: use_signal(build),
        draft: use_signal(build),
        prov: use_signal(ProvenanceDraft::default),
    }
}

/// Every `.field-label` column width in `html`, deduplicated. A card that lines its values up renders
/// exactly one; the stacked `multiline` rows carry no `.field-label` and so never appear here.
fn columns(html: &str) -> BTreeSet<u32> {
    let mut widths = BTreeSet::new();
    for chunk in html.split(r#"class="field-label" style="width:"#).skip(1) {
        let Some((digits, _)) = chunk.split_once("px;margin:0") else {
            continue;
        };
        if let Ok(width) = digits.parse::<u32>() {
            widths.insert(width);
        }
    }
    widths
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// Asserts that `view`'s rows all share one label column, and that it is `expected`.
fn assert_column(screen: &str, expected: u32, view: fn() -> Element) {
    let html = render(view);
    let found = columns(&html);
    assert!(
        !found.is_empty(),
        "{screen}: the card renders no one-line .fact-row at all:\n{html}"
    );
    assert_eq!(
        found,
        BTreeSet::from([expected]),
        "{screen}: every row must declare the same {expected}px label column, or the card's values \
         cannot line up (found {found:?}):\n{html}"
    );
}

// ---- The record cards ----------------------------------------------------------------------------

fn person_read() -> Element {
    person_record_fields(&loc(), existing!(PersonDraft, false))
}

fn person_edit() -> Element {
    person_record_fields(&loc(), existing!(PersonDraft, true))
}

fn place_read() -> Element {
    place_record_fields(&loc(), existing!(PlaceDraft, false), None)
}

fn place_edit() -> Element {
    place_record_fields(&loc(), existing!(PlaceDraft, true), None)
}

/// The event card's edit context: an existing-only place picker, unwired — the collapsed selection
/// derives from the draft, so SSR needs no rows behind it.
fn event_ctx(record: RecordEditState<EventDraft>) -> EventEditCtx {
    EventEditCtx {
        record,
        place: RecordPicker {
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
        },
        place_reset: Callback::new(|()| {}),
    }
}

fn event_read() -> Element {
    event_record_fields(&loc(), &event_ctx(existing!(EventDraft, false)))
}

fn event_edit() -> Element {
    event_record_fields(&loc(), &event_ctx(existing!(EventDraft, true)))
}

fn citation_read() -> Element {
    citation_record_fields(&loc(), existing!(CitationDraft, false))
}

fn citation_edit() -> Element {
    citation_record_fields(&loc(), existing!(CitationDraft, true))
}

fn family_read() -> Element {
    family_record_fields(&loc(), existing!(FamilyDraft, false))
}

fn family_edit() -> Element {
    family_record_fields(&loc(), existing!(FamilyDraft, true))
}

fn dna_test_read() -> Element {
    dna_test_record_fields(&loc(), existing!(DnaTestDraft, false))
}

fn dna_test_edit() -> Element {
    dna_test_record_fields(&loc(), existing!(DnaTestDraft, true))
}

/// A DNA match with no observations recorded: the locked rows still render (as em dashes), which is
/// what the label column has to accommodate.
fn dna_match_detail() -> DnaMatchDetail {
    DnaMatchDetail {
        human_id: "X0001".to_owned(),
        id: "0190-match-1".to_owned(),
        title: "John Smith ⟷ Mary Doe".to_owned(),
        test_a: None,
        test_b: None,
        provider: None,
        shared_cm: None,
        percent_shared: None,
        largest_segment_cm: None,
        predicted_relationship: None,
        status: "Undecided".to_owned(),
        status_kind: Some(MatchStatus::Confirmed),
        segments: Vec::new(),
        shared_ancestors: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        restrictions: Vec::new(),
        cited_by: Vec::new(),
        history: Vec::new(),
    }
}

fn dna_match_read() -> Element {
    dna_match_record_fields(&loc(), &dna_match_detail(), existing!(DnaMatchDraft, false))
}

fn dna_match_edit() -> Element {
    dna_match_record_fields(&loc(), &dna_match_detail(), existing!(DnaMatchDraft, true))
}

fn research_note_read() -> Element {
    research_note_record_fields(&loc(), existing!(ResearchNoteDraft, false))
}

fn research_note_edit() -> Element {
    research_note_record_fields(&loc(), existing!(ResearchNoteDraft, true))
}

fn repository_read() -> Element {
    repository_record_fields(&loc(), existing!(RepositoryDraft, false))
}

fn repository_edit() -> Element {
    repository_record_fields(&loc(), existing!(RepositoryDraft, true))
}

fn note_read() -> Element {
    note_record_fields(&loc(), existing!(NoteDraft, false))
}

fn note_edit() -> Element {
    note_record_fields(&loc(), existing!(NoteDraft, true))
}

fn media_read() -> Element {
    media_record_fields(&loc(), existing!(MediaDraft, false))
}

fn media_edit() -> Element {
    media_record_fields(&loc(), existing!(MediaDraft, true))
}

fn source_read() -> Element {
    source_record_fields(&loc(), existing!(SourceDraft, false))
}

fn source_edit() -> Element {
    source_record_fields(&loc(), existing!(SourceDraft, true))
}

/// Tag's edit mode is the `.grid-2` pair, and both cards take the page's one width so the two columns
/// line up beside each other.
fn tag_edit() -> Element {
    let edit = state::<TagDraft>(true, |_: &mut TagDraft| {});
    let committed = edit.seed.read().clone();
    rsx! {
        {tag_edit_tag_card(&loc(), edit, use_signal(|| false), false)}
        {tag_edit_colour_card(&loc(), edit.draft, &committed, use_signal(|| false))}
    }
}

// ---- The pins ------------------------------------------------------------------------------------

/// Every screen still on the shared record floor: their own labels are short, but all thirteen carry
/// the Restrictions row, whose Norwegian `RESTRIKSJONER` renders 102px.
#[test]
fn a_record_card_on_the_shared_floor_draws_a_110px_column() {
    for (screen, read, edit) in [
        ("place", place_read as fn() -> Element, place_edit as fn() -> Element),
        ("event", event_read, event_edit),
        ("family", family_read, family_edit),
        ("dna test", dna_test_read, dna_test_edit),
        ("research note", research_note_read, research_note_edit),
        ("repository", repository_read, repository_edit),
        ("note", note_read, note_edit),
        ("media", media_read, media_edit),
        ("source", source_read, source_edit),
    ] {
        assert_column(&format!("{screen} (read)"), 110, read);
        assert_column(&format!("{screen} (edit)"), 110, edit);
    }
}

/// `ETTERNAVNSPREFIKS` (135px) is the longest label any record card renders — the surname-prefix row.
#[test]
fn the_person_card_draws_a_140px_column() {
    assert_column("person (read)", 140, person_read);
    assert_column("person (edit)", 140, person_edit);
}

/// The three Evidence Explained axes share the card with the identity rows;
/// `INFORMASJONSTYPE` renders 133px.
#[test]
fn the_citation_card_draws_a_140px_column() {
    assert_column("citation (read)", 140, citation_read);
    assert_column("citation (edit)", 140, citation_edit);
}

/// The app draws the sharing observations in the same card as the identity rows, so the column has to
/// clear `ANTALL SEGMENTER` (130px) and `LARGEST SEGMENT` (123px), not just the short identity labels.
#[test]
fn the_dna_match_card_draws_a_140px_column() {
    assert_column("dna match (read)", 140, dna_match_read);
    assert_column("dna match (edit)", 140, dna_match_edit);
}

/// One width across the `.grid-2` pair, set by the Colour card's `FORHÅNDSVISNING` (122px).
#[test]
fn the_tag_cards_draw_a_130px_column() {
    assert_column("tag (edit)", 130, tag_edit);
}

/// A read row and the edit row it toggles into must declare the same column, or the card reflows on
/// the mode switch (`record-editing.html` §3).
#[test]
fn no_record_card_changes_its_column_between_modes() {
    for (screen, read, edit) in [
        ("person", person_read as fn() -> Element, person_edit as fn() -> Element),
        ("place", place_read, place_edit),
        ("event", event_read, event_edit),
        ("citation", citation_read, citation_edit),
        ("family", family_read, family_edit),
        ("dna test", dna_test_read, dna_test_edit),
        ("dna match", dna_match_read, dna_match_edit),
        ("research note", research_note_read, research_note_edit),
        ("repository", repository_read, repository_edit),
        ("note", note_read, note_edit),
        ("media", media_read, media_edit),
        ("source", source_read, source_edit),
    ] {
        let (before, after) = (columns(&render(read)), columns(&render(edit)));
        assert_eq!(
            before, after,
            "{screen}: read mode draws {before:?} and edit mode {after:?} — the card would reflow on \
             the mode toggle (record-editing.html §3)"
        );
    }
}
