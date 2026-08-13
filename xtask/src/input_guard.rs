//! `input-guard` — guards the shared-input-primitive convention (the "global keys fire inside text
//! controls" fix): every form control in the renderer must compose the guarded behavior cores
//! ([`TextInput`]/[`SelectInput`]), so the keydown typing guard is wired in exactly one place. This
//! lint scans `crates/vitni-ui-dioxus/src/**/*.rs` and flags any RSX `input {` / `textarea {` /
//! `select {` element outside the two allowlisted primitive files, reporting `file` + line.
//! Diagnostics are collected per file; the command only exits non-zero after scanning everything.
//!
//! A raw `input {` whose `r#type` is a non-typing kind (`checkbox`/`radio`/`button`/`submit`/`file`)
//! is allowed anywhere — those controls do not capture typing, so they carry no global-shortcut risk.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Directory holding the renderer source.
const SRC_DIR: &str = "crates/vitni-ui-dioxus/src";
/// The behavior-core files allowed to render raw form elements.
const ALLOWLISTED_FILES: [&str; 2] = ["text_input.rs", "select_input.rs"];
/// Input `r#type` values that do not capture typing and so are allowed anywhere.
const NON_TYPING_TYPES: [&str; 5] = ["checkbox", "radio", "button", "submit", "file"];

/// Runs the `input-guard` command (see module docs).
pub fn run() -> Result<()> {
    let dir = Path::new(SRC_DIR);
    let mut error_count = 0usize;

    for file in rs_sources(dir)? {
        let name = file.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if ALLOWLISTED_FILES.contains(&name) {
            continue;
        }

        println!("input-guard: {}", file.display());
        let text = fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        let flagged = flagged_lines(&text);
        for (line, element) in &flagged {
            println!(
                "  error: raw <{element}> at line {line} — compose TextInput/SelectInput (or move it into a primitive)"
            );
            error_count += 1;
        }
        if flagged.is_empty() {
            println!("  ok");
        }
    }

    if error_count > 0 {
        bail!("input-guard found {error_count} raw form element(s) outside the primitives (see above)");
    }
    println!("input-guard: no raw form elements outside the behavior-core primitives.");
    Ok(())
}

/// Every `.rs` file under `dir`, recursively, sorted.
fn rs_sources(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_rs(dir, &mut files)?;
    files.sort();
    Ok(files)
}

/// Appends every `.rs` file under `dir` (recursively) to `files`.
fn collect_rs(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry.with_context(|| format!("reading an entry under {}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

/// The offending raw form elements in `source`, as `(line_number, element)` pairs (line numbers are
/// 1-based). A `<textarea>`/`<select>` is always offending; an `<input>` is offending unless its
/// `r#type` is a non-typing kind.
fn flagged_lines(source: &str) -> Vec<(usize, &'static str)> {
    let masked = strip_comments(source);
    let lines: Vec<&str> = masked.lines().collect();
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(element) = opening_element(line) else {
            continue;
        };
        let offending = match element {
            "input" => !input_type_is_non_typing(&lines, index),
            _ => true,
        };
        if offending {
            out.push((index + 1, element));
        }
    }
    out
}

/// The RSX element a (comment-stripped) line opens, if it opens `input`/`textarea`/`select`.
fn opening_element(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    for element in ["input", "textarea", "select"] {
        if let Some(rest) = trimmed.strip_prefix(element)
            && rest.trim_start().starts_with('{')
            && rest.starts_with([' ', '\t', '{'])
        {
            return Some(element);
        }
    }
    None
}

/// Whether the `input {` block opening at `lines[start]` declares a non-typing `r#type`.
fn input_type_is_non_typing(lines: &[&str], start: usize) -> bool {
    let mut depth = 0i32;
    let mut block = String::new();
    for line in &lines[start..] {
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        block.push_str(line);
        block.push('\n');
        if depth <= 0 {
            break;
        }
    }
    let Some(kind) = input_type_value(&block) else {
        return false;
    };
    NON_TYPING_TYPES.contains(&kind.as_str())
}

/// The string-literal value of the first `r#type:` attribute in `block`, if any.
fn input_type_value(block: &str) -> Option<String> {
    let after = block.split_once("r#type:")?.1;
    let start = after.find('"')? + 1;
    let end = after[start..].find('"')? + start;
    Some(after[start..end].to_owned())
}

/// Replaces Rust comments (`// …` and `/* … */`) with spaces (newlines preserved), so an element in a
/// comment is not flagged while line numbers stay accurate for the surviving code.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_block = false;
    let mut in_line = false;
    while let Some(c) = chars.next() {
        if in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                out.push_str("  ");
                in_block = false;
            } else {
                out.push(if c == '\n' { '\n' } else { ' ' });
            }
        } else if in_line {
            if c == '\n' {
                out.push('\n');
                in_line = false;
            } else {
                out.push(' ');
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            out.push_str("  ");
            in_block = true;
        } else if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            out.push_str("  ");
            in_line = true;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::flagged_lines;

    #[test]
    fn raw_text_input_is_flagged() {
        let source = r#"
            rsx! {
                input {
                    class: "in",
                    r#type: "text",
                    value: "{x}",
                }
            }
        "#;
        assert_eq!(flagged_lines(source), vec![(3, "input")]);
    }

    #[test]
    fn checkbox_and_radio_inputs_are_allowed() {
        let source = r#"
            input {
                r#type: "checkbox",
                checked,
            }
            input {
                r#type: "radio",
                name: "{group}",
            }
        "#;
        assert!(flagged_lines(source).is_empty());
    }

    #[test]
    fn textarea_and_select_are_always_flagged() {
        let source = r#"
            textarea { class: "in", value: "{x}" }
            select {
                class: "in",
                onchange: move |e| f(e),
            }
        "#;
        let flagged = flagged_lines(source);
        assert!(flagged.contains(&(2, "textarea")), "{flagged:?}");
        assert!(flagged.contains(&(3, "select")), "{flagged:?}");
    }

    #[test]
    fn elements_in_comments_are_not_flagged() {
        let source = r#"
            // input { r#type: "text" }
            /* select {
                class: "in",
            } */
        "#;
        assert!(flagged_lines(source).is_empty(), "{:?}", flagged_lines(source));
    }

    #[test]
    fn a_nested_brace_before_the_type_does_not_hide_it() {
        let source = r#"
            input {
                class: if invalid { "in invalid" } else { "in" },
                r#type: "checkbox",
            }
        "#;
        assert!(flagged_lines(source).is_empty(), "{:?}", flagged_lines(source));
    }
}
