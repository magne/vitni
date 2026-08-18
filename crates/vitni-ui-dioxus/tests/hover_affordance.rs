//! CSS gate for #305: a ghost row action (Detach / Remove / Edit / Retract) must still read as a
//! control on the row the pointer is over. `.btn.ghost` is borderless at rest and its hover fill
//! collided with the row's own hover tint, so the hovered action had neither a boundary nor visible
//! hover feedback. This reads both shipped sheets — the crate's `src/components.css` and the mockup
//! superset `docs/mockups/assets/components.css` — off disk in both themes, so drift between them,
//! not only a regression in one, fails the build.
#![expect(
    clippy::expect_used,
    reason = "fixtures are the repo's own CSS on disk; a missing declaration is a real test failure"
)]

mod wcag;

use std::collections::HashMap;
use std::fs;

const APP_SHEET: &str = "crates/vitni-ui-dioxus/src/components.css";
const MOCKUP_SHEET: &str = "docs/mockups/assets/components.css";
/// Each sheet is measured against **its own** sibling `tokens.css`: the two files carry the same
/// palette today, but resolving the mockup sheet against the crate's tokens would hide a retuned
/// mockup token behind the app's value.
const APP_TOKENS: &str = "src/tokens.css";
const MOCKUP_TOKENS: &str = "../../docs/mockups/assets/tokens.css";
const DARK_THEME: &str = "dark";
const LIGHT_THEME: &str = "light";
const DARK_SELECTOR: &str = "[data-theme=\"dark\"]";
const LIGHT_SELECTOR: &str = "[data-theme=\"light\"]";
const AA_NON_TEXT: f64 = 3.0;

/// One CSS rule: its comma-separated, whitespace-normalized selectors and its raw declaration body.
struct Rule {
    selectors: Vec<String>,
    body: String,
}

/// Strips `/* ... */` comments; none of the sheets under test put braces inside a comment.
fn strip_comments(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find("*/") {
            Some(end) => &rest[start + end + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// Splits a stylesheet into its top-level rules, skipping `@media` (none of the selectors this gate
/// checks live inside one) by treating its nested braces as opaque.
fn top_level_rules(css: &str) -> Vec<Rule> {
    let css = strip_comments(css);
    let bytes = css.as_bytes();
    let mut rules = Vec::new();
    let mut selector_start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        let selector_text = css[selector_start..i].trim();
        let mut depth = 1i32;
        let mut j = i + 1;
        while j < bytes.len() && depth > 0 {
            match bytes[j] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            j += 1;
        }
        if !selector_text.starts_with('@') {
            let selectors = selector_text
                .split(',')
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect();
            rules.push(Rule {
                selectors,
                body: css[i + 1..j - 1].to_string(),
            });
        }
        selector_start = j;
        i = j;
    }
    rules
}

/// The raw (unresolved) value of `property` on the first top-level rule whose selector list
/// contains `selector` verbatim, or `None` if no such rule/property exists.
fn declaration(sheet: &str, selector: &str, property: &str) -> Option<String> {
    for rule in top_level_rules(sheet) {
        if !rule.selectors.iter().any(|s| s == selector) {
            continue;
        }
        for decl in rule.body.split(';') {
            let Some((name, value)) = decl.split_once(':') else {
                continue;
            };
            if name.trim() == property {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

/// The `--token: value;` custom properties declared on the theme block selected by `theme_selector`.
fn theme_tokens(tokens_css: &str, theme_selector: &str) -> HashMap<String, String> {
    let mut tokens = HashMap::new();
    for rule in top_level_rules(tokens_css) {
        if !rule.selectors.iter().any(|s| s == theme_selector) {
            continue;
        }
        for decl in rule.body.split(';') {
            let Some((name, value)) = decl.split_once(':') else {
                continue;
            };
            let name = name.trim();
            if name.starts_with("--") {
                tokens.insert(name.to_string(), value.trim().to_string());
            }
        }
    }
    tokens
}

/// Resolves a raw declaration value (a literal hex, or a single-level `var(--token)`) to RGB.
/// Returns `None` for `transparent`, which has no boundary/fill to measure.
fn resolve(tokens: &HashMap<String, String>, raw: &str) -> Option<(f64, f64, f64)> {
    let raw = raw.trim();
    let literal = match raw.strip_prefix("var(").and_then(|rest| rest.strip_suffix(')')) {
        Some(name) => tokens
            .get(name.trim())
            .expect("every var() this gate resolves must be defined in tokens.css")
            .as_str(),
        None => raw,
    };
    if literal == "transparent" {
        return None;
    }
    Some(wcag::rgb(literal))
}

/// `.btn.ghost:hover`'s effective border-color (falling back to `.btn.ghost`'s, since the hover rule
/// need not repeat a property it doesn't override) clears the 3:1 non-text floor against every
/// surface a ghost button can sit on.
fn assert_ghost_hover_border_clears_contrast(
    sheet: &str,
    sheet_name: &str,
    theme_name: &str,
    tokens: &HashMap<String, String>,
) {
    let raw_border = declaration(sheet, ".btn.ghost:hover", "border-color")
        .or_else(|| declaration(sheet, ".btn.ghost", "border-color"))
        .expect("`.btn.ghost` must declare a border-color, even if transparent");
    let border = resolve(tokens, &raw_border);
    for surface in ["--panel", "--panel-2", "--panel-3"] {
        let surface_raw = tokens.get(surface).expect("surface token must be defined");
        let surface_rgb = resolve(tokens, surface_raw).expect("surface tokens are opaque colors, never transparent");
        let ratio = border.map_or(1.0, |b| wcag::contrast(b, surface_rgb));
        assert!(
            ratio >= AA_NON_TEXT,
            "{sheet_name} ({theme_name}): .btn.ghost:hover border-color ({raw_border}) vs {surface} \
             = {ratio:.2} < {AA_NON_TEXT}"
        );
    }
}

/// `.btn.ghost:hover`'s background must not resolve to the same color as the row-hover fills it
/// sits on top of — the exact collision that left a hovered ghost action invisible.
fn assert_ghost_hover_background_differs_from_row_hover(
    sheet: &str,
    sheet_name: &str,
    theme_name: &str,
    tokens: &HashMap<String, String>,
) {
    let ghost_raw =
        declaration(sheet, ".btn.ghost:hover", "background").expect(".btn.ghost:hover must declare a background");
    let ghost_bg = resolve(tokens, &ghost_raw);
    for row_selector in ["table.tbl tr:hover td", ".row:hover"] {
        let row_raw =
            declaration(sheet, row_selector, "background").expect("row-hover selector must declare a background");
        let row_bg = resolve(tokens, &row_raw);
        assert!(
            ghost_bg != row_bg,
            "{sheet_name} ({theme_name}): .btn.ghost:hover background ({ghost_raw}) collides with \
             {row_selector} background ({row_raw})"
        );
    }
}

/// Every ghost action in the hovered row — not only the one under the pointer — must show a
/// non-transparent boundary, via a `table.tbl tr:hover .btn.ghost` / `.row:hover .btn.ghost` rule.
fn assert_row_reveals_ghost_boundary(sheet: &str, sheet_name: &str, theme_name: &str) {
    for row_selector in ["table.tbl tr:hover .btn.ghost", ".row:hover .btn.ghost"] {
        let raw = declaration(sheet, row_selector, "border-color");
        assert!(
            raw.as_deref().is_some_and(|value| value != "transparent"),
            "{sheet_name} ({theme_name}): expected `{row_selector} {{ border-color: <non-transparent> }}`, \
             found {raw:?}"
        );
    }
}

#[test]
fn ghost_row_action_hover_is_distinguishable() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let app_css =
        fs::read_to_string(format!("{manifest_dir}/src/components.css")).expect("crate components.css must exist");
    let app_tokens_css =
        fs::read_to_string(format!("{manifest_dir}/{APP_TOKENS}")).expect("crate tokens.css must exist");
    let mockup_css = fs::read_to_string(format!("{manifest_dir}/../../docs/mockups/assets/components.css"))
        .expect("mockup components.css must exist");
    let mockup_tokens_css =
        fs::read_to_string(format!("{manifest_dir}/{MOCKUP_TOKENS}")).expect("mockup tokens.css must exist");

    let sheets = [
        (APP_SHEET, app_css.as_str(), app_tokens_css.as_str()),
        (MOCKUP_SHEET, mockup_css.as_str(), mockup_tokens_css.as_str()),
    ];
    let themes = [(DARK_THEME, DARK_SELECTOR), (LIGHT_THEME, LIGHT_SELECTOR)];

    for (sheet_name, sheet, tokens_css) in sheets {
        for (theme_name, theme_selector) in themes {
            let tokens = theme_tokens(tokens_css, theme_selector);
            assert_ghost_hover_border_clears_contrast(sheet, sheet_name, theme_name, &tokens);
            assert_ghost_hover_background_differs_from_row_hover(sheet, sheet_name, theme_name, &tokens);
            assert_row_reveals_ghost_boundary(sheet, sheet_name, theme_name);
        }
    }
}
