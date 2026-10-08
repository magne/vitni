//! Prose gate for #501: every mockup's History tab quotes the History note the app ships.
//!
//! The note is the one piece of explanatory prose all record screens share, so a mockup that words
//! it differently advertises something the product does not say — eleven pages once claimed "no
//! competitor offers it built-in" long after the shipped `history-note` dropped the claim. The
//! expected text comes from the Fluent catalogue through the same lookup the History tab renders, so
//! rewording the catalogue fails here until the mockups follow.

use std::fs;
use std::path::PathBuf;

use vitni_ui::Localizer;

/// Pages whose History note legitimately differs from the shared one, with why.
const OWN_WORDING: [(&str, &str); 1] = [(
    "tag.html",
    "a tag has no retraction, so its note says History is display-only (the app still shows the shared note: \
     docs/issues.md, \"The Tag History tab says any entry can be undone\")",
)];

fn mockups_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/mockups")
}

/// The text of the first `.section-note` inside the page's History pane, tags stripped and
/// whitespace collapsed; `None` when the page has no History pane.
fn history_note(html: &str) -> Option<String> {
    let pane = html.find("data-pane=\"history\"")?;
    let rest = &html[pane..];
    let open = rest.find("class=\"section-note\">")? + "class=\"section-note\">".len();
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
    let shipped = loc.tab_note("history").expect("the History tab has a note");
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
        if OWN_WORDING.iter().any(|(page, _)| *page == name) {
            continue;
        }
        let html = fs::read_to_string(&path).expect("a mockup page is readable");
        let Some(note) = history_note(&html) else {
            continue;
        };
        checked += 1;
        if !note.contains(&shipped) {
            drifted.push(format!("{name}: {note}"));
        }
    }
    assert!(
        checked >= 12,
        "expected every record mockup's History pane, found {checked}"
    );
    assert!(
        drifted.is_empty(),
        "History notes that do not quote the shipped `history-note` ({shipped:?}):\n{}",
        drifted.join("\n")
    );
}

#[test]
fn every_exempt_page_still_has_its_own_history_note() {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let shipped = loc.tab_note("history").expect("the History tab has a note");
    for (page, _) in OWN_WORDING {
        let html = fs::read_to_string(mockups_dir().join(page)).expect("an exempt page exists");
        let note = history_note(&html).expect("an exempt page has a History note");
        assert!(
            !note.contains(&shipped),
            "{page} now quotes the shipped note; drop its exemption"
        );
    }
}
