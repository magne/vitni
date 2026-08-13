//! SSR assertions for the Compare/merge tool (Phase 5 PR 19): the possible-duplicates table renders
//! as an accessible `<table>` with a per-row Compare button and a confidence badge, and the
//! compare/merge wizard's field grid renders native radio pairs with an accessible group label.
//! Pure render-and-inspect over hand-built view-models — no window, no workspace — the same pattern
//! as `pedigree.rs`.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_ui::{DuplicateCandidateVm, MergeBlockedVm, MergeCompareVm, MergeFieldRowVm, PedigreeNodeVm};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::screens::{DuplicatesTable, MergeCompareGrid, merge_blocked_card, merge_wizard_foot};
use vitni_ui_dioxus::shell::ChromeCtx;
use vitni_ui_dioxus::shell::nav_state::NavState;

fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

fn node(human_id: &str, name: &str) -> PedigreeNodeVm {
    PedigreeNodeVm {
        human_id: human_id.to_owned(),
        name: name.to_owned(),
        vitals: None,
        confidence: None,
        confidence_label: None,
        source_count: 0,
        restrictions: Vec::new(),
        has_more: false,
    }
}

fn candidate(a: &str, b: &str, reason: &str, score: u8) -> DuplicateCandidateVm {
    DuplicateCandidateVm {
        a: node(a, a),
        b: node(b, b),
        reason: reason.to_owned(),
        score,
    }
}

/// Renders the duplicates table over two candidate pairs.
fn duplicates_table() -> Element {
    use_context_provider(NavState::new);
    use_context_provider(|| ChromeCtx(chrome("en")));
    let candidates = vec![
        candidate("I0042", "I0099", "same birth year · name variant", 94),
        candidate("I0061", "I0140", "shared parents", 55),
    ];
    rsx! {
        DuplicatesTable { candidates, oncompare: move |_| {} }
    }
}

#[test]
fn duplicates_table_renders_an_accessible_table_with_a_compare_button_per_row() {
    let mut vdom = VirtualDom::new(duplicates_table);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("<table"), "the duplicates list is a real table:\n{html}");
    assert!(html.contains("<th"), "the table has header cells:\n{html}");
    assert!(
        html.contains("I0042") && html.contains("I0099"),
        "both people in a pair render:\n{html}"
    );
    assert!(
        html.contains("same birth year · name variant"),
        "the match reason renders:\n{html}"
    );
    assert!(
        html.matches("Compare").count() >= 2,
        "each row has its own Compare button:\n{html}"
    );
    assert!(
        html.contains(r#"class="badge""#) && html.contains("94%"),
        "the match score renders as a plain percentage badge:\n{html}"
    );
    assert!(
        !html.contains("data-level") && !html.contains(r#"class="conf"#),
        "the score is not dressed up as a 5-level confidence badge:\n{html}"
    );
}

/// Renders the compare/merge wizard's field grid over a survivor/merged pair with two differing
/// fields (name and occupation) and one field neither carries (death, which does not differ).
fn compare_grid() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let vm = MergeCompareVm {
        survivor: node("I0042", "John Smith"),
        merged: node("I0099", "John Smyth"),
        fields: vec![
            MergeFieldRowVm::new(
                "Name".to_owned(),
                Some("John Smith".to_owned()),
                Some("John Smyth".to_owned()),
            ),
            MergeFieldRowVm::new(
                "Occupation".to_owned(),
                Some("Carpenter".to_owned()),
                Some("Joiner".to_owned()),
            ),
            MergeFieldRowVm::new("Death".to_owned(), None, None),
        ],
        differs_label: "differs".to_owned(),
        differs_title: "differs from kept value".to_owned(),
    };
    rsx! {
        MergeCompareGrid { vm }
    }
}

#[test]
fn compare_grid_renders_native_radio_pairs_grouped_per_field() {
    let mut vdom = VirtualDom::new(compare_grid);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(
        html.contains("John Smith") && html.contains("John Smyth"),
        "both names render:\n{html}"
    );
    assert!(
        html.matches(r#"type="radio""#).count() == 6,
        "3 fields × 2 sides = 6 native radio inputs:\n{html}"
    );
    assert!(
        html.matches(r#"name="merge-field-0""#).count() == 2,
        "the first field's radios share one group name:\n{html}"
    );
    assert!(
        html.matches(r#"name="merge-field-1""#).count() == 2,
        "the second field's radios share their own group name:\n{html}"
    );
    assert!(
        html.contains("Carpenter"),
        "the survivor's occupation value renders:\n{html}"
    );
    assert!(
        html.matches(r#"role="group""#).count() == 3,
        "each field row is an accessible radio group:\n{html}"
    );
    // U49: a differing persona value is tinted (.diff) AND carries a non-colour "differs" badge; the
    // two differing rows (Name, Occupation) each get one, the equal row (Death) gets none.
    assert!(
        html.contains(r#"<span class="diff">John Smyth</span>"#),
        "the differing name value is tinted:\n{html}"
    );
    assert_eq!(
        html.matches(r#"aria-label="differs from kept value""#).count(),
        2,
        "each differing row carries a labelled differs badge:\n{html}"
    );
    assert!(
        html.contains(">differs</span>"),
        "the differs badge renders its visible label:\n{html}"
    );
}

/// Renders the compare/merge wizard foot (reason input + Cancel/Merge).
fn wizard_foot() -> Element {
    let chrome = chrome("en");
    let reason = use_signal(String::new);
    let oncancel = use_callback(|()| {});
    let onmerge = use_callback(|()| {});
    merge_wizard_foot(&chrome, reason, oncancel, onmerge)
}

#[test]
fn compare_foot_renders_a_labeled_reason_for_merge_input() {
    let mut vdom = VirtualDom::new(wizard_foot);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(
        html.contains("Reason for merge"),
        "the reason field is labeled:\n{html}"
    );
    assert!(
        html.contains(r#"id="merge-reason""#),
        "a reason text input renders:\n{html}"
    );
}

/// Renders the blocked-merge card over a hand-built [`MergeBlockedVm`].
fn blocked_card() -> Element {
    let vm = MergeBlockedVm {
        heading: "Merge blocked — conflicting facts".to_owned(),
        guidance: "Resolve the contradiction first (retract or supersede one claim), then merge.".to_owned(),
        detail: "death 1920 Brooklyn contradicts burial 1899 Oslo".to_owned(),
    };
    rsx! {
        {merge_blocked_card(&vm)}
    }
}

#[test]
fn blocked_card_renders_heading_guidance_detail_and_alerts() {
    let mut vdom = VirtualDom::new(blocked_card);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(
        html.contains("Merge blocked — conflicting facts"),
        "the heading renders:\n{html}"
    );
    assert!(
        html.contains("Resolve the contradiction first"),
        "the guidance renders:\n{html}"
    );
    assert!(
        html.contains("death 1920 Brooklyn contradicts burial 1899 Oslo"),
        "the core reason detail renders:\n{html}"
    );
    assert!(
        html.contains(r#"role="alert""#),
        "the blocked card is an alert region:\n{html}"
    );
}

thread_local! {
    /// The language the localized-label test renders in — mirrors `pedigree.rs`'s smuggling trick.
    static LABEL_LANG: std::cell::Cell<&'static str> = const { std::cell::Cell::new("en") };
}

/// Renders the duplicates table's empty state, over [`LABEL_LANG`].
fn empty_duplicates() -> Element {
    use_context_provider(NavState::new);
    use_context_provider(|| ChromeCtx(chrome(LABEL_LANG.with(std::cell::Cell::get))));
    rsx! {
        DuplicatesTable { candidates: Vec::<DuplicateCandidateVm>::new(), oncompare: move |_| {} }
    }
}

#[test]
fn empty_duplicates_state_is_localized_in_english() {
    LABEL_LANG.with(|lang| lang.set("en"));
    let mut vdom = VirtualDom::new(empty_duplicates);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(
        html.contains("No possible duplicates found"),
        "expected the English empty state:\n{html}"
    );
}

#[test]
fn empty_duplicates_state_is_localized_in_norwegian() {
    LABEL_LANG.with(|lang| lang.set("no"));
    let mut vdom = VirtualDom::new(empty_duplicates);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(
        html.contains("Ingen mulige dubletter funnet"),
        "expected the Norwegian empty state:\n{html}"
    );
}
