//! SSR assertions for the record picker + nested draft card (Phase 5 PR28): an open result list
//! (reused rows, capped, with a "+ New …" row), the "+ New" row hidden when creation is not allowed,
//! the empty state, the collapsed selection chip with a labelled clear control, and the nested draft
//! card. Every builder is a pure fn over signals, so it renders under SSR without an `AppCtx`.

use dioxus::prelude::*;
use genealogy_ui::{Localizer, PickerSelection, PickerState, RowVm};
use genealogy_ui_dioxus::components::{
    PickerCallbacks, PickerConfig, PickerOptions, RecordPicker, draft_card, record_picker,
};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn row(id: &str, title: &str) -> RowVm {
    RowVm {
        id: id.to_owned(),
        title: title.to_owned(),
        ..RowVm::default()
    }
}

fn noop_callbacks() -> PickerCallbacks {
    PickerCallbacks {
        onpick: Callback::new(|_: PickerSelection| {}),
        onclear: Callback::new(|()| {}),
        onnew: Callback::new(|_: String| {}),
    }
}

fn options() -> PickerOptions {
    PickerOptions::Ready(vec![
        row("I0001", "Anna Berg"),
        row("I0002", "Anna Lovelace"),
        row("I0003", "Anne Frank"),
        row("I0004", "Anna Karenina"),
        row("I0005", "Annette Ford"),
        row("I0006", "Ann Smith"),
        row("I0007", "Annabelle Lee"),
        row("I0008", "Anno Domini"),
    ])
}

fn picker(state: Signal<PickerState>, options: PickerOptions, allow_new: bool) -> RecordPicker {
    RecordPicker {
        config: PickerConfig {
            label: "Partner".to_owned(),
            name: "partner".to_owned(),
            entity_label: "person".to_owned(),
            allow_new,
        },
        state,
        options,
        exclude: Vec::new(),
        callbacks: noop_callbacks(),
    }
}

fn open_view() -> Element {
    let loc = loc();
    let state = use_signal(|| PickerState {
        query: "ann".to_owned(),
        open: true,
        selection: None,
    });
    record_picker(&loc, &picker(state, options(), true))
}

fn no_new_view() -> Element {
    let loc = loc();
    let state = use_signal(|| PickerState {
        query: "ann".to_owned(),
        open: true,
        selection: None,
    });
    record_picker(&loc, &picker(state, options(), false))
}

fn empty_view() -> Element {
    let loc = loc();
    let state = use_signal(|| PickerState {
        query: "zzz".to_owned(),
        open: true,
        selection: None,
    });
    record_picker(&loc, &picker(state, options(), true))
}

fn selection_view() -> Element {
    let loc = loc();
    let state = use_signal(|| PickerState {
        query: String::new(),
        open: false,
        selection: Some(PickerSelection {
            human_id: "P0007".to_owned(),
            title: "Trinity Church".to_owned(),
        }),
    });
    record_picker(&loc, &picker(state, options(), true))
}

fn nested_card_view() -> Element {
    let inner = draft_card(
        "New source",
        "draft",
        "Discard New source".to_owned(),
        Callback::new(|()| {}),
        rsx! {
            div { class: "field", "inner" }
        },
    );
    draft_card(
        "New citation",
        "draft",
        "Discard New citation".to_owned(),
        Callback::new(|()| {}),
        inner,
    )
}

#[test]
fn an_open_picker_renders_capped_rows_and_the_new_query_row() {
    let html = render(open_view);
    assert!(
        html.contains(r#"class="picker-results""#),
        "the in-flow result list renders:\n{html}"
    );
    let rows = html.matches(r#"role="option""#).count();
    assert_eq!(rows, 6, "results are capped at six:\n{html}");
    // The create row echoes the query (quotes are HTML-escaped in the SSR output).
    assert!(
        html.contains(r#"class="picker-new""#) && html.contains("New person &#34;ann&#34;…"),
        "the create row echoes the query:\n{html}"
    );
}

#[test]
fn an_existing_only_picker_hides_the_new_row() {
    let html = render(no_new_view);
    assert!(html.contains(r#"role="option""#), "results still render:\n{html}");
    assert!(
        !html.contains(r#"class="picker-new""#),
        "no create row when creation is disallowed:\n{html}"
    );
}

#[test]
fn no_matches_shows_the_empty_state() {
    let html = render(empty_view);
    assert!(
        html.contains(r#"class="picker-empty""#),
        "the empty state renders:\n{html}"
    );
    assert!(html.contains("No matches"), "the empty message shows:\n{html}");
}

#[test]
fn a_selection_collapses_to_a_labelled_value_chip() {
    let html = render(selection_view);
    assert!(
        html.contains(r#"class="picker-value""#),
        "the collapsed chip renders:\n{html}"
    );
    assert!(
        html.contains("Trinity Church") && html.contains("P0007"),
        "the title + id show:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Clear selection""#),
        "the clear control is labelled:\n{html}"
    );
    assert!(
        !html.contains(r#"class="picker-results""#),
        "no result list while collapsed:\n{html}"
    );
}

// Keyboard dispatch (↑/↓/Home/End/Enter) is runtime-only — SSR never fires DOM key events — so these
// only assert the initial (index-0) highlight `PickerSearch` seeds on open and the ARIA wiring around
// it (combobox/expanded/controls/activedescendant + the derived option ids), same constraint as the
// anchor-position tests below.

#[test]
fn an_open_picker_highlights_the_first_result_on_open() {
    let html = render(open_view);
    assert!(
        html.contains(r#"id="partner-listbox""#),
        "the result listbox carries a stable id:\n{html}"
    );
    assert!(
        html.contains(r#"id="partner-opt-0""#),
        "the first option carries a derived id:\n{html}"
    );
    assert!(
        html.contains(r#"id="partner-opt-6""#),
        "the +New row's id continues right after the six matched rows:\n{html}"
    );
    assert!(
        html.contains(r#"role="combobox""#),
        "the input exposes combobox semantics:\n{html}"
    );
    assert!(
        html.contains(r#"aria-expanded="true""#),
        "the input reports expanded while open:\n{html}"
    );
    assert!(
        html.contains(r#"aria-controls="partner-listbox""#),
        "the input points at the listbox:\n{html}"
    );
    assert!(
        html.contains(r#"aria-activedescendant="partner-opt-0""#),
        "the input names the active (first) option:\n{html}"
    );
}

#[test]
fn the_highlighted_option_carries_aria_selected_and_the_sel_class() {
    let html = render(open_view);
    assert!(
        html.contains(r#"class="row sel""#),
        "the first (highlighted) row gets the sel class:\n{html}"
    );
    let selected_true = html.matches(r#"aria-selected="true""#).count();
    assert_eq!(selected_true, 1, "exactly one option is highlighted:\n{html}");
}

// The floating list's measured top/left/width are runtime-only (WebKitGTK `getBoundingClientRect`
// via `onmounted`, no-op under SSR) and not SSR-testable — same constraint as the provenance
// popover's `.prov` positioning. These assertions cover only the markup: the anchor wrapper and the
// click-away scrim exist while open, and neither exists once collapsed.

#[test]
fn an_open_picker_renders_the_anchor_and_scrim() {
    let html = render(open_view);
    assert!(
        html.contains(r#"class="picker-anchor""#),
        "the floating list's positioned anchor renders:\n{html}"
    );
    assert!(
        html.contains(r#"class="picker-scrim""#),
        "the click-away scrim renders while open:\n{html}"
    );
}

#[test]
fn a_collapsed_picker_renders_neither_results_nor_scrim() {
    let html = render(selection_view);
    assert!(
        !html.contains(r#"class="picker-results""#),
        "no result list while collapsed:\n{html}"
    );
    assert!(
        !html.contains(r#"class="picker-scrim""#),
        "no scrim while collapsed:\n{html}"
    );
}

#[test]
fn a_nested_draft_card_renders_both_levels() {
    let html = render(nested_card_view);
    assert_eq!(
        html.matches(r#"class="draft-card""#).count(),
        2,
        "the citation card nests the source card:\n{html}"
    );
    for needle in [
        r#"class="draft-card-title""#,
        "New citation",
        "New source",
        r#"class="badge draft""#,
        r#"aria-label="Discard New citation""#,
        r#"aria-label="Discard New source""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}
