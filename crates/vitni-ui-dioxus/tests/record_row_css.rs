//! CSS gate for #310: the rules a record row depends on must be scoped to the *control*, not to a
//! `div.field` wrapper the one-line `.fact-row` shape no longer has. A rule that drifts back under
//! `.field` silently unstyles every record row in the app, which no SSR markup assertion can see.
//! That the mockup sheet carries the same rules is `tests/mockup_superset.rs`'s job.
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
    ".field-with-revert > .in",
    ".field-with-revert > .icon-btn",
    ".field-with-revert > select.in",
    ".field-with-revert > select.in ~ .icon-btn",
    ".number-stepper",
    ".number-stepper:focus-within",
    ".number-stepper .stepper-value",
    ".number-stepper .stepper-arrow",
];

/// The read-value rule covers both placements: the stacked `.field > .val` a settings form draws and
/// the one-line `span.field.val` a record row draws (`record-editing.html:49`).
const READ_VALUE_SELECTORS: [&str; 2] = [".field .val", ".field.val"];

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

/// The mockup sheet is held to every one of these by `tests/mockup_superset.rs`; this keeps the lists
/// live, so a rule renamed in the app sheet fails here instead of leaving the gates below checking a
/// selector nothing declares.
#[test]
fn every_record_row_rule_is_in_the_app_sheet() {
    let (app, _) = sheets();
    for selector in ROW_SELECTORS.into_iter().chain(READ_VALUE_SELECTORS) {
        assert!(
            rule_declarations(&app, selector).is_some(),
            "src/components.css declares no rule for `{selector}`"
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

/// #500: a revert wrapper may hold a whole multi-control row (a date's modifier · date · quality ·
/// calendar), so its rules must reach only the wrapper's own control. A descendant rule there gave each
/// of the date's four controls `flex: 1`, and the wrapping row stacked them one per line.
#[test]
fn a_revert_wrapper_rule_reaches_only_its_own_control() {
    let (app, mockup) = sheets();
    for (name, sheet) in [
        ("src/components.css", &app),
        ("docs/mockups/assets/components.css", &mockup),
    ] {
        for rule in top_level_rules(sheet) {
            for selector in &rule.selectors {
                let Some(rest) = selector.strip_prefix(".field-with-revert ") else {
                    continue;
                };
                assert!(
                    rest.starts_with("> "),
                    "{name}: `{selector}` reaches every descendant of a revert wrapper, not just its \
                     own control — a date row's controls would each take `flex: 1` and stack"
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
