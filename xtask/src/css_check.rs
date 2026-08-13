//! `css-check` — guards the design-token convention (`docs/research/tailwind-css.md`): the shipped
//! component CSS must not hardcode colour literals; every colour comes from a `var(--token)` defined
//! in `tokens.css`. Scans `crates/vitni-ui-dioxus/src/*.css` (except `tokens.css`, the token
//! file itself) and flags any hex colour literal, reporting `file` + line. Diagnostics are collected
//! per file and the command only exits non-zero after scanning everything.
//!
//! Scope: hex colours only. Raw `px` values are deliberately not policed — border/layout px are
//! idiomatic and pervasive, whereas a hardcoded colour is the theme "magic value" the convention
//! targets. Tightening to px would be a separate decision, not a silent extension of this check.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Directory holding the shipped bundled CSS.
const CSS_DIR: &str = "crates/vitni-ui-dioxus/src";
/// The design-token file — the one allowed home for raw colour literals.
const TOKEN_FILE: &str = "tokens.css";

/// Runs the `css-check` command (see module docs).
pub fn run() -> Result<()> {
    let dir = Path::new(CSS_DIR);
    let mut error_count = 0usize;

    for file in css_sources(dir)? {
        let name = file.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if name == TOKEN_FILE {
            continue;
        }

        println!("css-check: {}", file.display());
        let text = fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        let masked = strip_comments(&text);

        let mut found = false;
        for (index, line) in masked.lines().enumerate() {
            for hex in hex_colors(line) {
                println!(
                    "  error: hex colour `{hex}` at line {} — use a var(--token) from {TOKEN_FILE}",
                    index + 1
                );
                error_count += 1;
                found = true;
            }
        }
        if !found {
            println!("  ok");
        }
    }

    if error_count > 0 {
        bail!("css-check found {error_count} hardcoded colour literal(s) (see above)");
    }
    println!("css-check: no hardcoded colour literals outside {TOKEN_FILE}.");
    Ok(())
}

/// Every `.css` file directly under `dir`, sorted, excluding none (the token file is filtered by the
/// caller). CSS lives flat in the crate's `src/`, so a non-recursive scan is sufficient.
fn css_sources(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let entry = entry.with_context(|| format!("reading an entry under {}", dir.display()))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "css") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Replaces every `/* … */` comment with spaces (newlines preserved), so a hex literal inside a
/// comment is not flagged while line numbers stay accurate for the surviving code.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_comment = false;
    while let Some(c) = chars.next() {
        if in_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                out.push_str("  ");
                in_comment = false;
            } else {
                out.push(if c == '\n' { '\n' } else { ' ' });
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            out.push_str("  ");
            in_comment = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// The hex colour literals on one (comment-stripped) line: a `#` followed by exactly 3, 4, 6, or 8
/// hex digits terminated by a non-identifier character. This is a lexical scan, not a CSS parse; an
/// all-hex id selector of one of those lengths would be a false positive, which none exist here.
fn hex_colors(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            let len = end - start;
            let terminated = end >= bytes.len() || !is_ident_byte(bytes[end]);
            if terminated && (len == 3 || len == 4 || len == 6 || len == 8) {
                out.push(line[i..end].to_owned());
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

/// Whether `byte` can continue a CSS identifier (so a longer token isn't misread as a hex colour).
fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}
