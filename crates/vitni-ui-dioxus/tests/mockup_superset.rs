//! CSS gate for #502: the mockup sheet is the superset of the app sheet, checked over the whole
//! sheet rather than a named list.
//!
//! `docs/mockups/` is the design source of truth, so `docs/mockups/assets/components.css` must carry
//! every rule `src/components.css` declares (`CLAUDE.md`). Rules are compared per selector *atom* and
//! per enclosing at-rule, because the mockup sheet groups selectors the app sheet declares alone
//! (`.rail .nav-item.active, .subnav .nav-item.active`). A rule both sheets declare must carry the
//! same declarations, unless [`OWN_DECLARATIONS`] names it with the reason a static page cannot draw
//! it the way the app does.
#![expect(
    clippy::expect_used,
    reason = "fixtures are the repo's own CSS on disk; an unreadable sheet is a real test failure"
)]

use std::collections::BTreeMap;
use std::fs;

/// Rules both sheets declare differently on purpose: `(at-rule context, selector, why)`.
const OWN_DECLARATIONS: [(&str, &str, &str); 0] = [];

/// Where a rule applies and to what: the enclosing at-rule preludes (`@media (max-width: 900px)`,
/// `@keyframes spin`), joined by ` / ` and empty at top level, and one selector of the rule's list.
type RuleKey = (String, String);

type Sheet = BTreeMap<RuleKey, BTreeMap<String, String>>;

/// Every rule in a sheet keyed by context and selector *atom* — one entry per comma-separated
/// selector — descending into every at-rule block. Declarations are the cascade result for that key:
/// a later declaration of a property, in the same rule or a later rule with the same selector,
/// replaces the earlier one. No sheet under test puts a brace inside a comment or a string.
fn sheet_rules(css: &str) -> Sheet {
    let mut rules = Sheet::new();
    collect_rules(&strip_comments(css), "", &mut rules);
    rules
}

fn strip_comments(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = rest[start..].find("*/").map_or("", |end| &rest[start + end + 2..]);
    }
    out.push_str(rest);
    out
}

fn collect_rules(css: &str, context: &str, rules: &mut Sheet) {
    let bytes = css.as_bytes();
    let mut prelude_start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b';' {
            // A block-less statement (`@charset`, `@import`) ends its own prelude.
            prelude_start = i + 1;
        }
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        let prelude = normalize(&css[prelude_start..i]);
        let mut depth = 1i32;
        let mut end = i + 1;
        while end < bytes.len() && depth > 0 {
            match bytes[end] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            end += 1;
        }
        let body = &css[i + 1..end.saturating_sub(1)];
        if prelude.starts_with('@') {
            let nested = if context.is_empty() {
                prelude
            } else {
                format!("{context} / {prelude}")
            };
            collect_rules(body, &nested, rules);
        } else {
            for selector in prelude.split(',') {
                let declarations = rules.entry((context.to_owned(), normalize(selector))).or_default();
                for declaration in body.split(';') {
                    if let Some((name, value)) = declaration.split_once(':') {
                        declarations.insert(name.trim().to_owned(), normalize(value));
                    }
                }
            }
        }
        prelude_start = end;
        i = end;
    }
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sheets() -> (Sheet, Sheet) {
    let root = env!("CARGO_MANIFEST_DIR");
    let app = fs::read_to_string(format!("{root}/src/components.css")).expect("the app sheet is readable");
    let mockup = fs::read_to_string(format!("{root}/../../docs/mockups/assets/components.css"))
        .expect("the mockup sheet is readable");
    (sheet_rules(&app), sheet_rules(&mockup))
}

fn describe((context, selector): &RuleKey) -> String {
    if context.is_empty() {
        selector.clone()
    } else {
        format!("{context} {{ {selector} }}")
    }
}

fn is_own((context, selector): &RuleKey) -> bool {
    OWN_DECLARATIONS
        .iter()
        .any(|(own_context, own_selector, _)| own_context == context && own_selector == selector)
}

#[test]
fn every_app_rule_has_a_mockup_rule() {
    let (app, mockup) = sheets();
    let mut missing = Vec::new();
    for key in app.keys() {
        if !mockup.contains_key(key) {
            missing.push(describe(key));
        }
    }
    assert!(
        missing.is_empty(),
        "{} app rules have no rule in docs/mockups/assets/components.css:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

#[test]
fn shared_rules_carry_the_same_declarations() {
    let (app, mockup) = sheets();
    let mut differing = Vec::new();
    for (key, app_declarations) in &app {
        let Some(mockup_declarations) = mockup.get(key) else {
            continue;
        };
        if app_declarations != mockup_declarations && !is_own(key) {
            differing.push(format!(
                "{}\n  app:    {app_declarations:?}\n  mockup: {mockup_declarations:?}",
                describe(key)
            ));
        }
    }
    assert!(
        differing.is_empty(),
        "{} rules differ between the sheets (reconcile, or list one in OWN_DECLARATIONS with why):\n{}",
        differing.len(),
        differing.join("\n")
    );
}

#[test]
fn every_own_declaration_still_differs() {
    let (app, mockup) = sheets();
    for (context, selector, _) in OWN_DECLARATIONS {
        let key = (context.to_owned(), selector.to_owned());
        let (Some(app_declarations), Some(mockup_declarations)) = (app.get(&key), mockup.get(&key)) else {
            panic!(
                "{} is listed in OWN_DECLARATIONS but is not in both sheets",
                describe(&key)
            );
        };
        assert_ne!(
            app_declarations,
            mockup_declarations,
            "{} now matches; drop it from OWN_DECLARATIONS",
            describe(&key)
        );
    }
}

#[test]
fn the_reader_keys_rules_by_context_and_selector_atom() {
    let rules = sheet_rules(
        "/* a { b } */ .a, .b { color: red; }\n.a { color: blue; margin: 0 }\n\
         @media (max-width: 900px) { .a { display: none } }\n@keyframes spin { to { opacity: 1 } }",
    );
    let declarations = |context: &str, selector: &str| {
        rules
            .get(&(context.to_owned(), selector.to_owned()))
            .map(|declarations| {
                declarations
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
            })
    };
    assert_eq!(
        declarations("", ".a"),
        Some(vec!["color: blue".to_owned(), "margin: 0".to_owned()])
    );
    assert_eq!(declarations("", ".b"), Some(vec!["color: red".to_owned()]));
    assert_eq!(
        declarations("@media (max-width: 900px)", ".a"),
        Some(vec!["display: none".to_owned()])
    );
    assert_eq!(
        declarations("@keyframes spin", "to"),
        Some(vec!["opacity: 1".to_owned()])
    );
    assert_eq!(rules.len(), 4, "a comment's braces are not a rule: {rules:?}");
}
