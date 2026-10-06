//! SSR assertions for the Matches tool (ADR 0039 §3): the possible-matches table renders as an
//! accessible `<table>` with a kind column, a per-row Compare button and a score badge, the run, kind
//! and band filters render as labelled selects, and the compare view's heading and notices render. The
//! compare view itself is `match_compare.rs`.
//! Pure render-and-inspect over hand-built view-models — no window, no workspace — the same pattern
//! as `pedigree.rs`.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_app::{DecidableKind, EngineVersion, ImportRunId, MatchBand, MatchEvidence, MatchQueueFilter};
use vitni_ui::{
    Category, CompareSideVm, Localizer, MatchCompareVm, MergeBlockedVm, QueuedMatchVm, RecordRef, RunOptionVm,
};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::screens::{
    MatchFilters, MatchesTable, earlier_distinction_card, filter_choices, merge_blocked_card, merge_compare_heading,
};
use vitni_ui_dioxus::shell::ChromeCtx;
use vitni_ui_dioxus::shell::nav_state::NavState;

fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

fn record(category: Category, human_id: &str) -> RecordRef {
    RecordRef {
        category,
        human_id: human_id.to_owned(),
        label: human_id.to_owned(),
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

fn queued(kind: DecidableKind, kind_label: &str, (a, b): (&str, &str), band: &str, percent: u8) -> QueuedMatchVm {
    let category = Category::from_matchable_kind(kind.matchable());
    QueuedMatchVm {
        kind,
        kind_label: kind_label.to_owned(),
        a: record(category, a),
        b: record(category, b),
        percent,
        band: band.to_owned(),
        reasons: vec!["Same given name (+3.0)".to_owned(), "Similar birth (+1.2)".to_owned()],
    }
}

/// Renders the matches table over a person pair and a place pair.
fn matches_table() -> Element {
    use_context_provider(NavState::new);
    use_context_provider(|| ChromeCtx(chrome("en")));
    let pairs = vec![
        queued(
            DecidableKind::Person,
            "Person",
            ("I0042", "I0099"),
            "probable match",
            94,
        ),
        queued(DecidableKind::Place, "Place", ("P0003", "P0007"), "possible match", 55),
    ];
    rsx! {
        MatchesTable { total: 2, pairs, oncompare: move |_| {} }
    }
}

/// The table listing one pair of the 250 its filter admits.
fn a_long_queue() -> Element {
    use_context_provider(NavState::new);
    use_context_provider(|| ChromeCtx(chrome("en")));
    let pairs = vec![queued(
        DecidableKind::Person,
        "Person",
        ("I0042", "I0099"),
        "probable match",
        94,
    )];
    rsx! {
        MatchesTable { total: 250, pairs, oncompare: move |_| {} }
    }
}

#[test]
fn a_long_queue_counts_every_pair_and_says_how_many_it_does_not_list() {
    let mut vdom = VirtualDom::new(a_long_queue);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(html.contains("250 undecided pairs"), "every pair is counted:\n{html}");
    assert!(
        html.contains("+249 more pairs — decide some or narrow the filter"),
        "the pairs not listed are counted below the table:\n{html}"
    );
}

#[test]
fn matches_table_renders_an_accessible_table_with_a_kind_and_a_compare_button_per_row() {
    let mut vdom = VirtualDom::new(matches_table);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("<table"), "the queue is a real table:\n{html}");
    assert!(
        html.contains("<th") && html.contains(">Kind<"),
        "the table has a Kind header:\n{html}"
    );
    assert!(
        html.contains(">Person<") && html.contains(">Place<"),
        "each row names its kind:\n{html}"
    );
    assert!(
        html.contains("I0042") && html.contains("I0099") && html.contains("P0003") && html.contains("P0007"),
        "both records of each pair render:\n{html}"
    );
    assert!(html.contains("2 undecided pairs"), "the pairs are counted:\n{html}");
    assert!(!html.contains("more pairs"), "every pair is listed:\n{html}");
    assert!(html.contains("probable match"), "the band renders:\n{html}");
    assert!(
        html.contains("Same given name (+3.0) · Similar birth (+1.2)"),
        "the reasons behind the score render beside it:\n{html}"
    );
    assert!(
        html.matches(">Compare<").count() >= 2,
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

thread_local! {
    /// The filter the filters test renders with.
    static FILTER: std::cell::Cell<MatchQueueFilter> = const {
        std::cell::Cell::new(MatchQueueFilter { run: None, kind: None, min_band: MatchBand::Possible })
    };
}

fn run_id() -> ImportRunId {
    ImportRunId::from_uuid(uuid::Uuid::from_u128(7))
}

/// Renders the three filters over one import run, with [`FILTER`] selected.
fn filters() -> Element {
    use_context_provider(NavState::new);
    let chrome = chrome("en");
    use_context_provider(|| ChromeCtx(Rc::clone(&chrome)));
    let runs = vec![RunOptionVm {
        id: run_id(),
        label: "other.ged — 2026-10-02".to_owned(),
    }];
    let choices = filter_choices(
        &chrome,
        &Localizer::with_languages(None, &["en".parse().unwrap_or_default()]),
        &runs,
    );
    rsx! {
        MatchFilters { choices, runs, filter: FILTER.with(std::cell::Cell::get), onchange: move |_| {} }
    }
}

#[test]
fn the_filters_are_three_labelled_selects_over_runs_kinds_and_bands() {
    let mut vdom = VirtualDom::new(filters);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for (name, label) in [
        ("matches-run", "Import run"),
        ("matches-kind", "Kind"),
        ("matches-band", "At least"),
    ] {
        assert!(
            html.contains(&format!(r#"for="{name}""#)) && html.contains(&format!(">{label}<")),
            "the {name} select is labelled {label}:\n{html}"
        );
    }
    assert!(
        html.contains("Every run") && html.contains("other.ged — 2026-10-02"),
        "{html}"
    );
    assert!(html.contains("Every kind") && html.contains(">Repository<"), "{html}");
    assert!(!html.contains(">Tag<"), "a tag is never a pair:\n{html}");
    assert!(
        html.contains("possible match") && html.contains("probable match"),
        "{html}"
    );
}

#[test]
fn the_filters_show_the_chosen_run_and_kind() {
    FILTER.with(|filter| {
        filter.set(MatchQueueFilter {
            run: Some(run_id()),
            kind: Some(DecidableKind::Place),
            min_band: MatchBand::Probable,
        });
    });
    let mut vdom = VirtualDom::new(filters);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    let selected = |value: &str| html.contains(&format!(r#"value="{value}" selected"#));
    assert!(selected(&run_id().to_string()), "the run is selected:\n{html}");
    assert!(selected("place"), "the kind is selected:\n{html}");
    assert!(selected("probable"), "the band is selected:\n{html}");
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
        html.contains("Marked as not the same"),
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

/// Renders the matches table's empty state, over [`LABEL_LANG`].
fn empty_matches() -> Element {
    use_context_provider(NavState::new);
    use_context_provider(|| ChromeCtx(chrome(LABEL_LANG.with(std::cell::Cell::get))));
    rsx! {
        MatchesTable { total: 0, pairs: Vec::<QueuedMatchVm>::new(), oncompare: move |_| {} }
    }
}

#[test]
fn empty_matches_state_is_localized_in_english() {
    LABEL_LANG.with(|lang| lang.set("en"));
    let mut vdom = VirtualDom::new(empty_matches);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(
        html.contains("No possible matches"),
        "expected the English empty state:\n{html}"
    );
}

#[test]
fn empty_matches_state_is_localized_in_norwegian() {
    LABEL_LANG.with(|lang| lang.set("no"));
    let mut vdom = VirtualDom::new(empty_matches);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(
        html.contains("Ingen mulige treff"),
        "expected the Norwegian empty state:\n{html}"
    );
}
