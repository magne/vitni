//! CSS gate for #310: the rules a record row depends on must be scoped to the *control*, not to a
//! `div.field` wrapper the one-line `.fact-row` shape no longer has — and the mockup sheet must carry
//! every one of them, because `docs/mockups/` is the design source of truth and its
//! `assets/components.css` is the superset of the app sheet (`CLAUDE.md`).
//!
//! Two failures this catches. A rule re-scoped in `src/components.css` but not mirrored into the
//! mockup sheet leaves the mockups drawing a row the app styles differently (the state the sheets were
//! in before this change: the mockup sheet had no `.field-with-revert` rules at all, so `tag.html`'s
//! edit specimen inlined the revert positioning by hand). A rule that drifts back under `.field`
//! silently unstyles every record row in the app, which no SSR markup assertion can see.
#![expect(
    clippy::expect_used,
    reason = "fixtures are the repo's own CSS on disk; a missing rule is a real test failure"
)]

mod css_sheet;

use std::fs;

use css_sheet::{rule_declarations, top_level_rules};

/// Every selector list a record row's styling now hangs off. Each must exist in **both** sheets, with
/// the same declarations, and none may be scoped under `.field` — the shape a `.fact-row` row lacks.
const ROW_SELECTORS: [&str; 22] = [
    ".fact-row > .field-label",
    ".fact-row .field.val",
    ".fact-row input.in",
    ".fact-row select.in",
    ".fact-row .number-stepper",
    "input.in.modified",
    "textarea.in.modified",
    ".number-stepper.modified",
    "select.in.modified",
    ".in.invalid",
    ".in[aria-invalid=\"true\"]",
    ".field-error",
    ".field-hint",
    ".field-with-revert",
    ".field-with-revert .in",
    ".field-with-revert .icon-btn",
    ".field-with-revert select.in",
    ".field-with-revert select.in ~ .icon-btn",
    ".number-stepper",
    ".number-stepper:focus-within",
    ".number-stepper .stepper-value",
    ".number-stepper .stepper-arrow",
];

/// The read-value rule covers both placements: the stacked `.field > .val` a settings form draws and
/// the one-line `span.field.val` a record row draws (`record-editing.html:49`).
const READ_VALUE_SELECTORS: [&str; 2] = [".field .val", ".field.val"];

/// The Media record screen's own rules, held to the same superset gate (#309). They were declared
/// deliberately app-only, which `cargo xtask css-check` cannot catch — it only scans for hex colour
/// literals — so `media.html` drew a preview and a viewer dialog the app sheet styled and the mockup
/// sheet did not. Only `.crop-capture` and `.media-save-preview` stay app-only, and the amended comment
/// above them in `src/components.css` says why.
const MEDIA_SELECTORS: [&str; 20] = [
    ".crop-rect .crop-handle",
    ".crop-rect .crop-handle.nw",
    ".crop-rect .crop-handle.ne",
    ".crop-rect .crop-handle.sw",
    ".crop-rect .crop-handle.se",
    ".modal-wide",
    ".modal-wide .mv-canvas",
    ".mv-frame",
    ".mv-frame.zoom-fit",
    ".mv-frame > .media-full",
    ".mv-frame.zoom-fit > .media-full",
    ".mv-frame.zoom-100 > .media-full",
    ".mv-frame.zoom-150 > .media-full",
    ".mv-frame.zoom-200 > .media-full",
    ".media-card",
    ".media-open",
    ".media-thumb",
    ".media-full",
    ".media-caption",
    ".media-preview",
];

/// The read value and every control that replaces it must be pinned to one height. Measured in the
/// real webview (`tests/gui-pass/tag-record-rows.toml`): unpinned they came out 37px and 38px, which is
/// invisible on one row and a whole pixel of drift by the third.
#[test]
fn a_read_value_and_the_control_it_toggles_into_are_pinned_to_one_height() {
    let (app, mockup) = sheets();
    for (name, sheet) in [
        ("src/components.css", &app),
        ("docs/mockups/assets/components.css", &mockup),
    ] {
        for selector in [
            ".fact-row .field.val",
            ".fact-row input.in",
            ".fact-row select.in",
            ".fact-row .number-stepper",
        ] {
            let declarations = rule_declarations(sheet, selector).unwrap_or_default();
            assert!(
                declarations.iter().any(|d| d == "min-height: 38px"),
                "{name}: `{selector}` must stand the same height as the box it toggles into, or every \
                 row below the pair shifts (record-editing.html §3); found {declarations:?}"
            );
        }
    }
}

#[test]
fn a_label_column_is_a_floor_the_content_can_raise() {
    let (app, mockup) = sheets();
    for (name, sheet) in [
        ("src/components.css", &app),
        ("docs/mockups/assets/components.css", &mockup),
    ] {
        let declarations = rule_declarations(sheet, ".fact-row > .field-label").unwrap_or_default();
        assert!(
            declarations.iter().any(|d| d == "min-width: max-content"),
            "{name}: a label wider than the page's column must widen its own cell, or it overflows \
             and draws on top of the value beside it (RESTRICTIONS renders 92px); found {declarations:?}"
        );
    }
}

fn sheets() -> (String, String) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let app =
        fs::read_to_string(format!("{manifest_dir}/src/components.css")).expect("crate components.css must exist");
    let mockup = fs::read_to_string(format!("{manifest_dir}/../../docs/mockups/assets/components.css"))
        .expect("mockup components.css must exist");
    (app, mockup)
}

#[test]
fn every_record_row_rule_is_in_both_sheets_with_the_same_declarations() {
    let (app, mockup) = sheets();
    for selector in ROW_SELECTORS
        .into_iter()
        .chain(READ_VALUE_SELECTORS)
        .chain(MEDIA_SELECTORS)
    {
        let in_app = rule_declarations(&app, selector);
        let in_mockup = rule_declarations(&mockup, selector);
        assert!(in_app.is_some(), "src/components.css declares no rule for `{selector}`");
        assert!(
            in_mockup.is_some(),
            "docs/mockups/assets/components.css lacks `{selector}` — the mockup sheet is the superset"
        );
        assert_eq!(
            in_app, in_mockup,
            "`{selector}` differs between the app sheet and the mockup superset"
        );
    }
}

#[test]
fn no_record_row_rule_hides_behind_a_field_wrapper() {
    let (app, mockup) = sheets();
    for (name, sheet) in [
        ("src/components.css", &app),
        ("docs/mockups/assets/components.css", &mockup),
    ] {
        for rule in top_level_rules(sheet) {
            for selector in &rule.selectors {
                let Some(inner) = selector.strip_prefix(".field ") else {
                    continue;
                };
                assert!(
                    !ROW_SELECTORS.contains(&inner),
                    "{name}: `{selector}` scopes a record-row rule under a .field wrapper a \
                     one-line .fact-row row does not have"
                );
            }
        }
    }
}

#[test]
fn a_read_value_is_padded_in_the_row_as_well_as_stacked() {
    let (app, mockup) = sheets();
    for (name, sheet) in [
        ("src/components.css", &app),
        ("docs/mockups/assets/components.css", &mockup),
    ] {
        let declarations = rule_declarations(sheet, ".field.val").unwrap_or_default();
        assert!(
            declarations.iter().any(|d| d == "padding: var(--sp-2) var(--sp-3)"),
            "{name}: a read value in a .fact-row must keep the input's padding, or read↔edit moves \
             text (record-editing.html §3); found {declarations:?}"
        );
        assert!(
            declarations.iter().any(|d| d == "margin-bottom: 0"),
            "{name}: `.field.val` also carries `.field`'s bottom margin, which would make a read row \
             taller than the input row it toggles into; found {declarations:?}"
        );
    }
}
