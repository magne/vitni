//! SSR assertions for the Explorer filter bar (ADR 0052 §2–3, #678): the *Filters* button and its count,
//! one chip per applied clause with accessible edit and remove names, and the editor's column, condition
//! and per-type value controls.

#![expect(clippy::expect_used, reason = "a clause that does not apply aborts the test")]

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_app::{FilterOp, ListedKind};
use vitni_ui::{ActiveFilter, FilterDraft, Localizer, StaleClauseVm};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::shell::list_filters::{
    FilterBar, FilterEditing, FiltersButton, commit_clause, opening_draft, remove_clause,
};
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
    let stale = use_signal(Vec::new);
    let editing = use_signal(move || editing);
    rsx! {
        FiltersButton { count: filters.read().len(), editing }
        FilterBar { kind: ListedKind::Person, filters, stale, editing }
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

#[component]
fn ReplaceMarked() -> Element {
    let filters = use_signal(applied);
    let stale = use_signal(|| {
        vec![
            StaleClauseVm {
                column: "shoe size".to_owned(),
                label: "shoe size is not a column".to_owned(),
            },
            StaleClauseVm {
                column: "shoe size".to_owned(),
                label: "other is not a column".to_owned(),
            },
        ]
    });
    use_hook(move || {
        let fixed = clause(ListedKind::Person, "sex", FilterOp::IsAnyOf, "", "", &["male"]);
        commit_clause(FilterEditing::Stale(0), filters, stale, fixed);
    });
    let marked: Vec<String> = stale.read().iter().map(|clause| clause.label.clone()).collect();
    rsx! { "FILTERS:{filters.read().len()} STALE:{marked.join(\"|\")}" }
}

#[test]
fn applying_an_edited_marked_clause_replaces_it() {
    let mut vdom = VirtualDom::new(ReplaceMarked);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("FILTERS:3"), "the corrected clause is applied:\n{html}");
    assert!(
        html.contains("STALE:other is not a column"),
        "only the edited marked clause is gone:\n{html}"
    );
}

#[component]
fn RemoveMarked() -> Element {
    let filters = use_signal(applied);
    let stale = use_signal(|| {
        vec![
            StaleClauseVm {
                column: "shoe size".to_owned(),
                label: "shoe size is not a column".to_owned(),
            },
            StaleClauseVm {
                column: "shoe size".to_owned(),
                label: "other is not a column".to_owned(),
            },
        ]
    });
    use_hook(move || remove_clause(FilterEditing::Stale(0), filters, stale));
    let marked: Vec<String> = stale.read().iter().map(|clause| clause.label.clone()).collect();
    rsx! { "FILTERS:{filters.read().len()} STALE:{marked.join(\"|\")}" }
}

#[test]
fn removing_a_marked_clause_drops_only_that_clause() {
    let mut vdom = VirtualDom::new(RemoveMarked);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("FILTERS:2"), "applied clauses are untouched:\n{html}");
    assert!(
        html.contains("STALE:other is not a column"),
        "only the removed marked clause is gone:\n{html}"
    );
}

fn marked(column: &str) -> Vec<StaleClauseVm> {
    vec![StaleClauseVm {
        column: column.to_owned(),
        label: format!("{column} cannot take that condition"),
    }]
}

#[test]
fn editing_a_marked_clause_opens_on_its_column_when_it_still_exists() {
    let kind = ListedKind::Person;
    let draft = opening_draft(kind, FilterEditing::Stale(0), &[], &marked("birth"));

    assert_eq!(draft.column, "birth");
}

#[test]
fn editing_a_marked_clause_whose_column_is_gone_opens_on_a_new_clause() {
    let kind = ListedKind::Person;
    let draft = opening_draft(kind, FilterEditing::Stale(0), &[], &marked("shoe size"));

    assert_eq!(draft, FilterDraft::new(kind));
}

#[component]
fn TaggedBar() -> Element {
    contexts();
    let filters = use_signal(|| {
        vec![clause(
            ListedKind::Person,
            "tags",
            FilterOp::IsAnyOf,
            "",
            "",
            &["0000-gone"],
        )]
    });
    let stale = use_signal(Vec::new);
    let editing = use_signal(|| Some(FilterEditing::Clause(0)));
    rsx! {
        FilterBar { kind: ListedKind::Person, filters, stale, editing }
    }
}

#[tokio::test]
async fn a_tag_clause_holding_a_deleted_tag_shows_it_ticked_by_label() {
    let mut vdom = VirtualDom::new(TaggedBar);
    vdom.rebuild_in_place();
    vdom.wait_for_work().await;
    vdom.render_immediate(&mut dioxus::core::NoOpMutations);
    let html = dioxus_ssr::render(&vdom);

    assert!(
        html.contains("<span>deleted tag</span>"),
        "the dead tag is shown by label:\n{html}"
    );
    assert_eq!(html.matches("checked").count(), 1, "and it is ticked:\n{html}");
}
