//! Minimal top-level CSS rule reader, shared by the CSS gates under
//! `crates/vitni-ui-dioxus/tests/`. Enough to answer "which rules does this sheet declare, and with
//! what body" — no cascade, no specificity, no `@media` (none of the selectors the gates check live
//! inside one, so its nested braces are treated as opaque and skipped).

/// One CSS rule: its comma-separated, whitespace-normalized selectors and its raw declaration body.
pub struct Rule {
    /// The rule's selectors, each with runs of whitespace collapsed to one space.
    pub selectors: Vec<String>,
    /// Everything between the braces, verbatim.
    pub body: String,
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

/// The declarations of the first top-level rule whose selector list contains `selector` verbatim,
/// normalized to a comparable `property: value` list.
pub fn rule_declarations(sheet: &str, selector: &str) -> Option<Vec<String>> {
    for rule in top_level_rules(sheet) {
        if !rule.selectors.iter().any(|candidate| candidate == selector) {
            continue;
        }
        let mut declarations = Vec::new();
        for declaration in rule.body.split(';') {
            let Some((name, value)) = declaration.split_once(':') else {
                continue;
            };
            declarations.push(format!(
                "{}: {}",
                name.trim(),
                value.split_whitespace().collect::<Vec<_>>().join(" ")
            ));
        }
        return Some(declarations);
    }
    None
}

/// Splits a stylesheet into its top-level rules, skipping `@media` (see module docs) by treating its
/// nested braces as opaque.
pub fn top_level_rules(css: &str) -> Vec<Rule> {
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
