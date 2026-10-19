//! SSR assertions for the Explorer filter bar (ADR 0052 §2–3, #678): the *Filters* button and its count,
//! one chip per applied clause with accessible edit and remove names, and the editor's column, condition
//! and per-type value controls.

#![expect(clippy::expect_used, reason = "a clause that does not apply aborts the test")]

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_app::{FilterOp, ListedKind};
use vitni_ui::{ActiveFilter, FilterDraft, Localizer};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::shell::list_filters::{FilterBar, FilterEditing, FiltersButton};
use vitni_ui_dioxus::shell::{ChromeCtx, DataLocCtx};

fn contexts() {
    let language = "en".parse::<LanguageIdentifier>().unwrap_or_default();
    use_context_provider(|| ChromeCtx(Rc::new(Chrome::with_languages(None, std::slice::from_ref(&language)))));
    use_context_provider(|| DataLocCtx(Rc::new(Localizer::with_languages(None, &[language]))));
}

fn clause(kind: ListedKind, column: &str, op: FilterOp, first: &str, second: &str, values: &[&str]) -> ActiveFilter {
    let mut draft = FilterDraft::new(kind).with_column(kind, column).with_op(op);
    first.clone_into(&mut draft.first);
    second.clone_into(&mut draft.second);
    draft.values = values.iter().map(|value| (*value).to_owned()).collect();
    draft.apply(kind).expect("applies")
}

fn applied() -> Vec<ActiveFilter> {
    vec![
        clause(ListedKind::Person, "birth", FilterOp::Between, "1800", "1850", &[]),
        clause(ListedKind::Person, "sex", FilterOp::IsAnyOf, "", "", &["female"]),
    ]
}

#[component]
fn Bar(editing: Option<FilterEditing>) -> Element {
    contexts();
    let filters = use_signal(applied);
    let editing = use_signal(move || editing);
    rsx! {
        FiltersButton { count: filters.read().len(), editing }
        FilterBar { kind: ListedKind::Person, filters, editing }
    }
}

fn render(editing: Option<FilterEditing>) -> String {
    let mut vdom = VirtualDom::new_with_props(Bar, BarProps { editing });
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn the_filters_button_counts_the_applied_clauses() {
    let html = render(None);

    assert!(html.contains(">Filters<"), "the button is labelled:\n{html}");
    assert!(html.contains(r#"class="badge">2<"#), "it counts two clauses:\n{html}");
    assert!(
        html.contains(r#"aria-expanded="false""#),
        "the editor is closed:\n{html}"
    );
}

#[test]
fn each_clause_is_a_chip_that_edits_and_removes_it() {
    let html = render(None);

    for needle in [
        r#"aria-label="Edit filter: Birth between 1800 – 1850""#,
        r#"aria-label="Remove filter: Birth between 1800 – 1850""#,
        r#"aria-label="Edit filter: Sex is any of female""#,
        r#"class="chip-label""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?}:\n{html}");
    }
    assert!(
        !html.contains(r#"class="filter-editor""#),
        "no editor until asked:\n{html}"
    );
}

#[test]
fn a_new_clause_opens_on_the_title_column_with_a_text_value() {
    let html = render(Some(FilterEditing::New));

    assert!(
        html.contains(r#"aria-expanded="true""#),
        "the button shows the editor open:\n{html}"
    );
    for needle in [
        r#"class="filter-editor""#,
        r#"id="filter-column""#,
        r#"<option value="given""#,
        r#"<option value="contains" selected"#,
        r#"id="filter-first""#,
        ">Apply<",
        ">Cancel<",
    ] {
        assert!(html.contains(needle), "expected {needle:?}:\n{html}");
    }
    assert!(
        !html.contains(">Remove<"),
        "a new clause has nothing to remove:\n{html}"
    );
}

#[test]
fn a_between_clause_reopens_with_both_ends() {
    let html = render(Some(FilterEditing::Clause(0)));

    for needle in [
        r#"<option value="birth" selected"#,
        r#"<option value="between" selected"#,
        r#"value="1800""#,
        r#"id="filter-second""#,
        r#"value="1850""#,
        ">From<",
        ">To<",
        ">Remove<",
    ] {
        assert!(html.contains(needle), "expected {needle:?}:\n{html}");
    }
}

#[test]
fn a_choice_clause_offers_its_values_as_checkboxes() {
    let html = render(Some(FilterEditing::Clause(1)));

    assert!(html.contains(r#"id="filter-value-female""#), "a box per value:\n{html}");
    assert!(html.contains(r#"id="filter-value-male""#), "a box per value:\n{html}");
    assert_eq!(
        html.matches("checked").count(),
        1,
        "only the chosen value is checked:\n{html}"
    );
}
