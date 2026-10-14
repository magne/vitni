//! Prose gate for #501: every mockup's History tab quotes the History note the app ships.
//!
//! The note is the one piece of explanatory prose all record screens share, so a mockup that words
//! it differently advertises something the product does not say — eleven pages once claimed "no
//! competitor offers it built-in" long after the shipped `history-note` dropped the claim. The
//! expected text comes from the Fluent catalogue through the same lookups the History tab renders —
//! the shared `history-note`, or Tag's own display-only note — so rewording the catalogue fails here
//! until the mockups follow.

use std::fs;
use std::path::PathBuf;

use vitni_ui::Localizer;

/// The History note a page's app screen ships: Tag has no retraction, so its History tab carries its
/// own display-only note (#578); every other record shares `history-note`.
fn shipped_note(loc: &Localizer, shared: &str, page: &str) -> String {
    if page == "tag.html" {
        return loc.tag_history_note();
    }
    shared.to_owned()
}

fn mockups_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/mockups")
}

/// The text of the first `.section-note` inside the page's History pane (up to the next pane), tags
/// stripped and whitespace collapsed; `None` when the page has no History pane or the pane no note.
fn history_note(html: &str) -> Option<String> {
    let pane = html.find("data-pane=\"history\"")? + "data-pane=\"history\"".len();
    let rest = &html[pane..];
    let rest = &rest[..rest.find("data-pane=").unwrap_or(rest.len())];
    let note = rest.find("class=\"section-note")?;
    let open = note + rest[note..].find('>')? + 1;
    let close = rest[open..].find("</div>")?;
    let mut text = String::new();
    let mut in_tag = false;
    for c in rest[open..open + close].chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    Some(text.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[test]
fn every_history_pane_quotes_the_shipped_note() {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let shared = loc.tab_note("history").expect("the History tab has a note");
    let mut checked = 0;
    let mut drifted = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(mockups_dir())
        .expect("docs/mockups is readable")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "html"))
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        let html = fs::read_to_string(&path).expect("a mockup page is readable");
        let Some(note) = history_note(&html) else {
            continue;
        };
        checked += 1;
        let shipped = shipped_note(&loc, &shared, &name);
        if !note.contains(&shipped) {
            drifted.push(format!("{name}: {note}\n  ships: {shipped}"));
        }
    }
    assert!(
        checked >= 13,
        "expected every record mockup's History pane, found {checked}"
    );
    assert!(
        drifted.is_empty(),
        "History notes that do not quote the note the app ships:\n{}",
        drifted.join("\n")
    );
}
