//! SSR assertions for the provenance block shown above Save on every edit form (`record-editing.html`
//! §5b). SSR renders the initial state, so these cover the block's labels and controls, the
//! citation-chip rendering for a seeded draft, and the find-or-create citation picker that replaced the
//! blind free-text id input. The picker renders without an `AppCtx` (it falls back to a baseline
//! localizer and empty options); the pick/attach/detach interactions are event-driven and not
//! SSR-clickable, so they are covered by the picker model tests in `attach_picker.rs`.

use dioxus::prelude::*;
use vitni_ui::{Localizer, ProvenanceDraft};
use vitni_ui_dioxus::screens::{provenance_block, provenance_block_dna};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// An empty draft: the reason field, the confidence select (Normal), and the three evidence axis
/// selects each with a leading unset "—".
fn empty_block() -> Element {
    let draft = use_signal(ProvenanceDraft::default);
    provenance_block(&loc(), draft)
}

/// A draft seeded with two backing citations.
fn seeded_block() -> Element {
    let draft = use_signal(|| ProvenanceDraft {
        citations: vec!["C0001".to_owned(), "C0002".to_owned()],
        ..ProvenanceDraft::default()
    });
    provenance_block(&loc(), draft)
}

#[test]
fn the_block_labels_every_control() {
    let html = render(empty_block);
    // The rationale field: its label and the optional-shown-in-History hint.
    assert!(html.contains("Reason for this change"), "reason label:\n{html}");
    assert!(html.contains("optional · shown in History"), "reason hint:\n{html}");
    // The confidence select, leading with the unset "No judgment" option (ADR 0021 §5).
    assert!(
        html.contains(r#"aria-label="Confidence""#),
        "confidence select label:\n{html}"
    );
    assert!(html.contains("No judgment"), "the unset confidence option:\n{html}");
    assert!(html.contains("Normal"), "confidence options:\n{html}");
    // The three evidence-analysis axis selects, each accessibly named.
    assert!(html.contains(r#"aria-label="Source quality""#), "source axis:\n{html}");
    assert!(
        html.contains(r#"aria-label="Information kind""#),
        "information axis:\n{html}"
    );
    assert!(html.contains(r#"aria-label="Evidence kind""#), "evidence axis:\n{html}");
    // Each axis leads with the unset "—" option.
    assert!(html.contains("—"), "unset option:\n{html}");
}

#[test]
fn the_confidence_select_defaults_to_the_unset_option() {
    let html = render(empty_block);
    // An untouched draft records no judgment (ADR 0021 §5): the leading value="" option is selected.
    assert!(
        html.contains(r#"<option value="" selected=true>No judgment</option>"#),
        "the unset confidence option is selected by default:\n{html}"
    );
}

/// A draft whose rationale has already been typed, for the controlled-field assertion below.
fn typed_reason_block() -> Element {
    let draft = use_signal(|| ProvenanceDraft {
        rationale: "gdpr audit".to_owned(),
        ..ProvenanceDraft::default()
    });
    provenance_block(&loc(), draft)
}

#[test]
fn the_reason_field_renders_the_draft_it_is_bound_to() {
    let html = render(typed_reason_block);
    // The rationale input must carry a `value` bound to the draft. An unbound one is not merely
    // "uncontrolled": `value` is a volatile attribute, so a missing one is re-written to the live DOM
    // as a removal on every diff, blanking the field — which is how a save once carried only the last
    // character typed (`components/text_input.rs` header). SSR cannot type, but it can prove the
    // binding is in the markup at all, which is exactly what was missing.
    let field = html
        .split('<')
        .find(|tag| tag.contains(r#"name="prov-reason""#))
        .unwrap_or_default();
    assert!(
        field.contains(r#"value="gdpr audit""#),
        "the reason input is bound to the draft's rationale:\n{html}"
    );
}

/// A Person/Family relationship-assertion block: additionally offers the DNA-match evidence picker.
fn dna_block() -> Element {
    let draft = use_signal(ProvenanceDraft::default);
    provenance_block_dna(&loc(), draft)
}

/// A relationship-assertion block seeded with a cited DNA match.
fn dna_block_seeded() -> Element {
    let draft = use_signal(|| ProvenanceDraft {
        dna_matches: vec!["X0007".to_owned()],
        ..ProvenanceDraft::default()
    });
    provenance_block_dna(&loc(), draft)
}

#[test]
fn the_dna_evidence_picker_is_offered_only_on_relationship_forms() {
    // The plain block (other aggregates) has no DNA-match picker.
    let plain = render(empty_block);
    assert!(
        !plain.contains(r#"id="prov-dna-match""#),
        "the plain provenance block offers no DNA-match picker:\n{plain}"
    );
    // A person/family relationship block does: a keyboard-operable search picker with its attach label.
    let html = render(dna_block);
    assert!(
        html.contains(r#"id="prov-dna-match""#),
        "the relationship block offers a DNA-match picker:\n{html}"
    );
    assert!(
        html.contains(r#"placeholder="Find DNA match…""#),
        "the picker searches existing DNA matches:\n{html}"
    );
    assert!(
        html.contains("Cite a DNA match"),
        "the attach affordance is present:\n{html}"
    );
}

#[test]
fn cited_dna_matches_render_as_chips_with_remove() {
    let html = render(dna_block_seeded);
    assert!(html.contains("X0007"), "the cited DNA match chip:\n{html}");
    assert!(
        html.contains(r#"aria-label="Remove DNA-match evidence""#),
        "each DNA-match chip has a remove control:\n{html}"
    );
}

#[test]
fn the_citations_row_is_a_find_or_create_picker() {
    let html = render(empty_block);
    // The blind free-text id input is gone; the attach affordance is a search picker over citations.
    assert!(
        html.contains(r#"id="prov-citation""#),
        "the citation link is a picker input:\n{html}"
    );
    assert!(
        html.contains(r#"placeholder="Find citation…""#),
        "the picker searches existing citations:\n{html}"
    );
    // Its field label doubles as the attach affordance.
    assert!(
        html.contains("Attach citation"),
        "the attach affordance is present:\n{html}"
    );
}

#[test]
fn citations_render_as_chips_with_detach() {
    let html = render(seeded_block);
    assert!(html.contains("C0001"), "first citation chip:\n{html}");
    assert!(html.contains("C0002"), "second citation chip:\n{html}");
    assert!(
        html.contains(r#"aria-label="Detach citation""#),
        "each chip has a detach control:\n{html}"
    );
}

#[test]
fn an_empty_draft_shows_no_chips() {
    let html = render(empty_block);
    assert!(
        !html.contains(r#"class="chip""#),
        "an empty draft renders no citation chips:\n{html}"
    );
    // The picker is still offered even with nothing attached.
    assert!(
        html.contains(r#"placeholder="Find citation…""#),
        "the picker is still present:\n{html}"
    );
}

/// The inline new-citation card rendered in isolation so SSR exercises its body.
fn new_citation_card() -> Element {
    let draft = use_signal(ProvenanceDraft::default);
    vitni_ui_dioxus::components::provenance_new_citation_card(draft)
}

#[test]
fn the_new_citation_card_renders_a_source_picker_and_page_input() {
    let html = render(new_citation_card);
    // The draft card head: the "New citation" title and the "draft" badge.
    assert!(html.contains("New citation"), "the card title:\n{html}");
    assert!(
        html.contains(r#"class="badge draft""#),
        "the card carries a draft badge:\n{html}"
    );
    // Its body: a required source find-or-create picker plus a page input.
    assert!(
        html.contains(r#"id="prov-new-source""#) && html.contains(r#"placeholder="Find source…""#),
        "the card has a source picker:\n{html}"
    );
    assert!(
        html.contains(r#"id="prov-new-page""#),
        "the card has a page input:\n{html}"
    );
    assert!(html.contains("Add citation"), "the card has an Add button:\n{html}");
}
