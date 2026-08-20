//! CSS gate for #311: a record picker's result list must place itself correctly in a *static* page,
//! not only under the renderer. `.picker-results` was `position: fixed` at `--pk-top`/`--pk-left`,
//! custom properties only `record_picker.rs` sets (from `get_client_rect`) — so every mockup page with
//! a picker specimen fell back to `0,0` and drew the dropdown over the page's top-left corner, which is
//! how a walkthrough found an unexplained "Berg, Anna / Lovelace, Anna" list floating there.
//!
//! The shared rule's default is now an absolutely-positioned dropdown anchored under
//! `.picker-anchor`, and the measured, `.detail`-clip-escaping placement is the opt-in
//! `.picker-results-viewport` variant the renderer adds (the `.sidepanel.sidepanel-viewport`
//! precedent in the same sheets). This gate holds both halves of that, in **both** shipped sheets —
//! `docs/mockups/` is the design source of truth and its `assets/components.css` is the superset of
//! the app sheet (`CLAUDE.md`) — and reads the mockup pages themselves, so a new specimen that drops
//! the wrapper fails the build instead of quietly landing in the corner again.
#![expect(
    clippy::expect_used,
    reason = "fixtures are the repo's own CSS and mockups on disk; a missing rule is a real test failure"
)]

mod css_sheet;

use std::fs;

use css_sheet::rule_declarations;

/// The default rule: an in-page dropdown, hanging off its own `.picker-anchor`.
const DEFAULT_SELECTOR: &str = ".picker-results";
/// The renderer's opt-in variant: the same list, re-placed at the box the renderer measured.
const VIEWPORT_SELECTOR: &str = ".picker-results.picker-results-viewport";

#[test]
fn the_default_result_list_hangs_off_its_own_anchor() {
    for (name, sheet) in sheets() {
        let declarations = rule_declarations(&sheet, DEFAULT_SELECTOR).unwrap_or_default();
        for expected in ["position: absolute", "top: calc(100% + var(--sp-1))", "left: 0"] {
            assert!(
                declarations.iter().any(|d| d == expected),
                "{name}: `{DEFAULT_SELECTOR}` must declare `{expected}` so a static page places the \
                 list under its own input with no renderer to measure it; found {declarations:?}"
            );
        }
    }
}

#[test]
fn the_default_result_list_reads_no_renderer_measured_property() {
    for (name, sheet) in sheets() {
        let declarations = rule_declarations(&sheet, DEFAULT_SELECTOR).unwrap_or_default();
        for property in ["--pk-top", "--pk-left"] {
            assert!(
                !declarations.iter().any(|d| d.contains(property)),
                "{name}: `{DEFAULT_SELECTOR}` reads `{property}`, which only the renderer sets — its \
                 fallback pins the list to the page's top-left corner (#311); found {declarations:?}"
            );
        }
    }
}

#[test]
fn the_viewport_variant_carries_the_measured_placement() {
    for (name, sheet) in sheets() {
        let declarations = rule_declarations(&sheet, VIEWPORT_SELECTOR).unwrap_or_default();
        for expected in [
            "position: fixed",
            "top: calc(var(--pk-top, 0px) + var(--sp-1))",
            "left: var(--pk-left, 0px)",
        ] {
            assert!(
                declarations.iter().any(|d| d == expected),
                "{name}: `{VIEWPORT_SELECTOR}` must declare `{expected}` — it is the variant the \
                 renderer opts into to escape the .detail scroll pane's overflow:hidden clip; found \
                 {declarations:?}"
            );
        }
    }
}

#[test]
fn both_picker_rules_are_in_both_sheets_with_the_same_declarations() {
    let [(_, app), (_, mockup)] = sheets();
    for selector in [DEFAULT_SELECTOR, VIEWPORT_SELECTOR] {
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
fn every_mockup_result_list_sits_inside_a_picker_anchor() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mockups = fs::read_dir(format!("{manifest_dir}/../../docs/mockups")).expect("docs/mockups/ must exist");
    let mut pages = 0usize;
    let mut offenders = Vec::new();
    for entry in mockups {
        let path = entry.expect("a readable directory entry").path();
        if path.extension().is_none_or(|extension| extension != "html") {
            continue;
        }
        let page = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let html = fs::read_to_string(&path).expect("a readable mockup page");
        pages += 1;
        for line in unanchored_result_lists(&html) {
            offenders.push(format!("docs/mockups/{page}:{line}"));
        }
    }
    assert!(
        pages > 0,
        "docs/mockups/ holds no .html pages — the gate would pass vacuously"
    );
    offenders.sort();
    assert!(
        offenders.is_empty(),
        "{offenders:?}: a `.picker-results` with no `.picker-anchor` ancestor has no containing block, \
         so the absolutely-positioned list draws over the page's top-left corner instead of under its \
         own input (#311). Wrap the input and the list in `<div class=\"picker-anchor\">`."
    );
}

/// The 1-based lines of every `.picker-results` element in `html` with no `.picker-anchor` ancestor.
///
/// A deliberately small scanner: it tracks the open `<div>` stack (every picker wrapper in the
/// mockups is a `div`) and, at each element carrying `picker-results`, asks whether any div still
/// open is the anchor. Comments are blanked to newlines first, so a commented-out specimen neither
/// unbalances the stack nor shifts the reported lines.
fn unanchored_result_lists(html: &str) -> Vec<usize> {
    let html = blank_comments(html);
    let mut open_divs: Vec<bool> = Vec::new();
    let mut unanchored = Vec::new();
    let mut index = 0usize;
    while let Some(offset) = html[index..].find('<') {
        let open = index + offset;
        let Some(close_offset) = html[open..].find('>') else {
            break;
        };
        let tag = &html[open + 1..open + close_offset];
        index = open + close_offset + 1;
        if let Some(closing) = tag.strip_prefix('/') {
            if closing.trim().eq_ignore_ascii_case("div") {
                open_divs.pop();
            }
            continue;
        }
        let Some(name) = tag.split_whitespace().next() else {
            continue;
        };
        let classes = tag_classes(tag);
        if classes.contains(&"picker-results") && !open_divs.contains(&true) {
            unanchored.push(html[..open].matches('\n').count() + 1);
        }
        if name.eq_ignore_ascii_case("div") {
            open_divs.push(classes.contains(&"picker-anchor"));
        }
    }
    unanchored
}

/// The classes on one tag's inner text (`div class="a b"` ⇒ `["a", "b"]`), double-quoted as every
/// mockup page writes them.
fn tag_classes(tag: &str) -> Vec<&str> {
    let Some((_, rest)) = tag.split_once("class=\"") else {
        return Vec::new();
    };
    let Some((value, _)) = rest.split_once('"') else {
        return Vec::new();
    };
    value.split_whitespace().collect()
}

/// Replaces every `<!-- ... -->` comment with the newlines it spanned, so offsets stay on their
/// original lines.
fn blank_comments(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find("-->") {
            Some(end) => {
                let comment = &rest[start..start + end + 3];
                for _ in 0..comment.matches('\n').count() {
                    out.push('\n');
                }
                &rest[start + end + 3..]
            }
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// Both shipped sheets, read off disk, named as the failure messages report them.
fn sheets() -> [(&'static str, String); 2] {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let app =
        fs::read_to_string(format!("{manifest_dir}/src/components.css")).expect("crate components.css must exist");
    let mockup = fs::read_to_string(format!("{manifest_dir}/../../docs/mockups/assets/components.css"))
        .expect("mockup components.css must exist");
    [
        ("crates/vitni-ui-dioxus/src/components.css", app),
        ("docs/mockups/assets/components.css", mockup),
    ]
}
