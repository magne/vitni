//! Gate for #680: the mockups show the *Saved* control in every list toolbar and the *Saved searches*
//! group in the palette, because the mockups are the design source of truth for shipped UI.

#![expect(clippy::panic, reason = "an unreadable mockup aborts the test")]

use std::fs;
use std::path::PathBuf;

const LISTS: [&str; 13] = [
    "person.html",
    "family.html",
    "event.html",
    "place.html",
    "source.html",
    "citation.html",
    "repository.html",
    "note.html",
    "media.html",
    "tag.html",
    "dna-test.html",
    "dna-match.html",
    "research-note.html",
];

fn page(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/mockups")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn every_list_mockup_shows_the_saved_icon_button() {
    for name in LISTS {
        let html = page(name);
        assert!(
            html.contains(r#"class="icon-btn""#) && html.contains(r#"aria-label="Saved searches""#),
            "{name} lacks the Saved icon button in its toolbar"
        );
    }
}

#[test]
fn the_palette_mockup_shows_the_saved_searches_group_and_its_toolbar_control() {
    let html = page("search-palette.html");

    assert!(
        html.contains(r#"aria-label="Saved searches""#),
        "the palette mockup's toolbar lacks the Saved icon button"
    );
    assert!(
        html.contains("Saved searches</div>") || html.contains("Saved searches</"),
        "the palette mockup lacks the Saved searches group heading"
    );
}

#[test]
fn the_person_mockup_shows_the_open_menu_the_name_field_and_a_marked_chip() {
    let html = page("person.html");

    for needle in [
        r#"data-hook="saved-menu""#,
        r#"id="saved-name""#,
        r#"aria-invalid="true""#,
        r#"aria-describedby="saved-name-error""#,
        "chip stale",
        "Save as new",
    ] {
        assert!(html.contains(needle), "person.html lacks {needle:?}");
    }
}
