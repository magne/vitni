//! SSR assertions for the Compare/merge tool (Phase 5 PR 19): the possible-duplicates table renders
//! as an accessible `<table>` with a per-row Compare button and a confidence badge, and the
//! compare/merge wizard's field grid renders native radio pairs with an accessible group label.
//! Pure render-and-inspect over hand-built view-models — no window, no workspace — the same pattern
//! as `pedigree.rs`.

use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_ui::{ConfidenceLevel, DuplicateCandidateVm, MergeCompareVm, MergeFieldRowVm, PedigreeNodeVm};
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::screens::{DuplicatesTable, MergeCompareGrid, merge_wizard_foot};
use genealogy_ui_dioxus::shell::ChromeCtx;
use genealogy_ui_dioxus::shell::nav_state::NavState;
use unic_langid::LanguageIdentifier;

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

fn candidate(a: &str, b: &str, reason: &str, confidence: ConfidenceLevel) -> DuplicateCandidateVm {
    DuplicateCandidateVm {
        a: node(a, a),
        b: node(b, b),
        reason: reason.to_owned(),
        confidence,
        confidence_label: match confidence {
            ConfidenceLevel::VeryLow => "Very low",
            ConfidenceLevel::Low => "Low",
            ConfidenceLevel::Normal => "Normal",
            ConfidenceLevel::High => "High",
            ConfidenceLevel::VeryHigh => "Very high",
        }
        .to_owned(),
    }
}

/// Renders the duplicates table over two candidate pairs.
fn duplicates_table() -> Element {
    use_context_provider(NavState::new);
    use_context_provider(|| ChromeCtx(chrome("en")));
    let candidates = vec![
        candidate(
            "I0042",
            "I0099",
            "same birth year · name variant",
            ConfidenceLevel::VeryHigh,
        ),
        candidate("I0061", "I0140", "shared parents", ConfidenceLevel::Normal),
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
        html.contains(r#"data-level="very-high""#) && html.contains(">Very high"),
        "the confidence badge carries colour + text:\n{html}"
    );
}

/// Renders the compare/merge wizard's field grid over a survivor/merged pair with one differing
/// field (occupation known on the survivor only) and one field neither carries (death).
fn compare_grid() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let vm = MergeCompareVm {
        survivor: node("I0042", "John Smith"),
        merged: node("I0099", "John Smyth"),
        fields: vec![
            MergeFieldRowVm {
                label: "Name".to_owned(),
                survivor_value: Some("John Smith".to_owned()),
                merged_value: Some("John Smyth".to_owned()),
            },
            MergeFieldRowVm {
                label: "Occupation".to_owned(),
                survivor_value: Some("Carpenter".to_owned()),
                merged_value: None,
            },
            MergeFieldRowVm {
                label: "Death".to_owned(),
                survivor_value: None,
                merged_value: None,
            },
        ],
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
