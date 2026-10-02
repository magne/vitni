//! SSR assertions for the Compare/merge tool (Phase 5 PR 19): the possible-duplicates table renders
//! as an accessible `<table>` with a per-row Compare button and a confidence badge, and the compare
//! view's heading and notices render. The compare view itself is `match_compare.rs`.
//! Pure render-and-inspect over hand-built view-models — no window, no workspace — the same pattern
//! as `pedigree.rs`.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_app::{EngineVersion, MatchBand, MatchEvidence};
use vitni_ui::{CompareSideVm, DuplicateCandidateVm, MatchCompareVm, MergeBlockedVm, PedigreeNodeVm};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::screens::{DuplicatesTable, earlier_distinction_card, merge_blocked_card, merge_compare_heading};
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

fn evidence() -> MatchEvidence {
    MatchEvidence {
        score_bp: 9712,
        band: MatchBand::Probable,
        engine: EngineVersion(4),
        cultures: Vec::new(),
        features: Vec::new(),
    }
}

fn candidate(a: &str, b: &str, reason: &str, score: u8) -> DuplicateCandidateVm {
    DuplicateCandidateVm {
        a: node(a, a),
        b: node(b, b),
        reason: reason.to_owned(),
        score,
        reasons: vec!["Same given name (+3.0)".to_owned(), "Similar birth (+1.2)".to_owned()],
    }
}

/// Renders the duplicates table over two candidate pairs.
fn duplicates_table() -> Element {
    use_context_provider(NavState::new);
    use_context_provider(|| ChromeCtx(chrome("en")));
    let candidates = vec![
        candidate("I0042", "I0099", "probable match", 94),
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
    assert!(html.contains("probable match"), "the band renders:\n{html}");
    assert!(
        html.contains("Same given name (+3.0) · Similar birth (+1.2)"),
        "the reasons behind the score render beside it:\n{html}"
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

/// Renders the compare heading over a pair.
fn compare_heading() -> Element {
    let chrome = chrome("en");
    let side = |human_id: &str, label: &str| CompareSideVm {
        human_id: human_id.to_owned(),
        label: label.to_owned(),
        origin: None,
        evidence: None,
    };
    let vm = MatchCompareVm {
        left: side("I0042", "John Smith"),
        right: side("I0099", "John Smyth"),
        rows: Vec::new(),
        assessment: evidence(),
        assessment_line: "Matched at 97% · probable match · engine 4".to_owned(),
        earlier_decision: None,
    };
    merge_compare_heading(&chrome, &vm)
}

#[test]
fn compare_heading_names_the_pair() {
    let mut vdom = VirtualDom::new(compare_heading);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("John Smith ⟷ John Smyth"), "the pair is named:\n{html}");
}

/// Renders the notice that an earlier decision holds the pair distinct.
fn earlier_distinction() -> Element {
    let chrome = chrome("en");
    earlier_distinction_card(&chrome, use_callback(|()| {}))
}

#[test]
fn an_earlier_distinction_is_shown_with_an_undo_and_merge_action() {
    let mut vdom = VirtualDom::new(earlier_distinction);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(
        html.contains(r#"role="status""#),
        "the notice is a status region:\n{html}"
    );
    assert!(
        html.contains("Marked as different people"),
        "the earlier decision is named:\n{html}"
    );
    assert!(
        html.contains("Undo “not the same” and merge"),
        "the undo-and-merge action renders:\n{html}"
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
