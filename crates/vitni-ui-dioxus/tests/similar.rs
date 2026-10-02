//! SSR assertions for the similar-record views (ADR 0038 §8): the hint under a record being created and
//! the *Find similar* list.

use dioxus::prelude::*;
use vitni_app::DecidableKind;
use vitni_ui::{Category, Localizer, RecordRef, SimilarHitVm};
use vitni_ui_dioxus::components::ListRow;
use vitni_ui_dioxus::screens::{SimilarCallbacks, similar_hint_view, similar_list_view};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn hit(human_id: &str, label: &str, compare: Option<DecidableKind>) -> SimilarHitVm {
    SimilarHitVm {
        record: RecordRef {
            category: Category::People,
            human_id: human_id.to_owned(),
            label: label.to_owned(),
        },
        percent: 87,
        band: "probable match".to_owned(),
        line: format!("Possibly the same as {label}, {human_id} (87%)"),
        reasons: vec!["Same surname (+4.0)".to_owned(), "Same birth (+2.5)".to_owned()],
        compare,
    }
}

fn callbacks() -> SimilarCallbacks {
    SimilarCallbacks {
        onuse: Callback::new(|_: RecordRef| {}),
        oncompare: Callback::new(|_: SimilarHitVm| {}),
    }
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn hint_view() -> Element {
    let hits = [hit("I0042", "Guldbrand Olsen", Some(DecidableKind::Person))];
    similar_hint_view(
        &loc(),
        &hits,
        Some("I0042"),
        &rsx! { p { id: "inline-compare" } },
        callbacks(),
    )
}

#[test]
fn the_hint_names_the_record_and_offers_compare_and_use_existing() {
    let html = render(hint_view);
    for needle in [
        r#"class="similar-hint""#,
        r#"role="status""#,
        "Possibly the same as Guldbrand Olsen, I0042 (87%)",
        "probable match",
        "Compare",
        "Use existing",
        r#"id="inline-compare""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

fn closed_hint_view() -> Element {
    let hits = [hit("I0042", "Guldbrand Olsen", Some(DecidableKind::Person))];
    similar_hint_view(&loc(), &hits, None, &rsx! { p { id: "inline-compare" } }, callbacks())
}

#[test]
fn the_compare_shows_only_once_asked_for() {
    let html = render(closed_hint_view);
    assert!(!html.contains("inline-compare"), "{html}");
}

fn empty_hint_view() -> Element {
    similar_hint_view(&loc(), &[], None, &rsx! {}, callbacks())
}

#[test]
fn no_similar_record_shows_no_hint() {
    let html = render(empty_hint_view);
    assert!(!html.contains("similar-hint"), "{html}");
}

fn list_view() -> Element {
    let hits = [
        hit("I0042", "Guldbrand Olsen", Some(DecidableKind::Person)),
        hit("Emigrant-id", "Emigrant", None),
    ];
    similar_list_view(&loc(), &hits, callbacks())
}

#[test]
fn find_similar_lists_each_record_with_its_score_and_reasons() {
    let html = render(list_view);
    for needle in [
        "Guldbrand Olsen",
        "I0042",
        "87%",
        "Same surname (+4.0) · Same birth (+2.5)",
        "Open",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert_eq!(html.matches(">Compare<").count(), 1, "a tag is never compared:\n{html}");
}

fn empty_list_view() -> Element {
    similar_list_view(&loc(), &[], callbacks())
}

#[test]
fn find_similar_says_when_it_finds_nothing() {
    let html = render(empty_list_view);
    assert!(html.contains("No similar records found."), "{html}");
}

fn scored_row() -> Element {
    rsx! {
        ListRow {
            title: "Guldbrand Olsen".to_owned(),
            id_label: Some("I0042".to_owned()),
            score: Some("87%".to_owned()),
            onclick: |_| {},
        }
    }
}

#[test]
fn a_ranked_picker_row_shows_its_score() {
    let html = render(scored_row);
    assert!(html.contains(r#"<div class="row-score">87%</div>"#), "{html}");
}
