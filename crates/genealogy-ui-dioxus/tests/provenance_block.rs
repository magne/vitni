//! SSR assertions for the provenance block shown above Save on every edit form (`record-editing.html`
//! §5b). SSR renders the initial state, so these cover the block's labels and controls and the
//! citation-chip rendering for a seeded draft; the typing/attach/detach interactions are event-driven
//! and not SSR-clickable.

use dioxus::prelude::*;
use genealogy_ui::{Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::screens::provenance_block;

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
    // The confidence select, with Normal the default selection.
    assert!(
        html.contains(r#"aria-label="Confidence""#),
        "confidence select label:\n{html}"
    );
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
fn citations_render_as_chips_with_detach() {
    let html = render(seeded_block);
    assert!(html.contains("C0001"), "first citation chip:\n{html}");
    assert!(html.contains("C0002"), "second citation chip:\n{html}");
    assert!(
        html.contains(r#"aria-label="Detach citation""#),
        "each chip has a detach control:\n{html}"
    );
    assert!(
        html.contains("Attach citation"),
        "the attach affordance is present:\n{html}"
    );
}

#[test]
fn an_empty_draft_shows_no_chips() {
    let html = render(empty_block);
    assert!(
        !html.contains(r#"class="chip""#),
        "an empty draft renders no citation chips:\n{html}"
    );
    assert!(
        html.contains("Attach citation"),
        "the attach affordance is still present:\n{html}"
    );
}
